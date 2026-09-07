mod bridge;
mod intent;

use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use roldex_core::{
    Agent, Config, PermissionMode, ProjectSummary, StudioBroker, WorkspaceFs, project_tree,
};
use tokio::sync::Mutex;

const DEFAULT_STUDIO_PORT: u16 = 38247;

#[derive(Debug, Parser)]
#[command(name = "roldex", version, about = "Roblox Studio development agent")]
struct Cli {
    /// Project root Roldex should treat as the active Roblox workspace.
    #[arg(long, default_value = ".")]
    root: PathBuf,

    /// Optional configuration file. Defaults to ./roldex.toml when present.
    #[arg(long)]
    config: Option<PathBuf>,

    /// Allow Roldex file/process tools to access paths outside the active project when required by the task.
    #[arg(long)]
    full_access: bool,

    /// Localhost port used by the Roldex Studio plugin.
    #[arg(long, default_value_t = DEFAULT_STUDIO_PORT)]
    studio_port: u16,

    /// Disable the localhost Roblox Studio bridge.
    #[arg(long)]
    no_studio_bridge: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = cli
        .root
        .canonicalize()
        .with_context(|| format!("could not open project root {}", cli.root.display()))?;

    let mut config = Config::load(cli.config.as_deref())?;
    if cli.full_access {
        config.permissions.mode = PermissionMode::FullAccess;
    }

    let project = ProjectSummary::detect(&root)?;
    let fs = Arc::new(WorkspaceFs::new(&root, config.permissions.mode)?);
    let studio_broker = StudioBroker::new();
    let agent = Arc::new(Mutex::new(
        Agent::new(config.ai.clone(), project.clone()).with_studio(studio_broker.clone()),
    ));

    print_banner(&project, &config);

    let (_bridge_handle, bridge_available) = if cli.no_studio_bridge {
        println!("Studio bridge: disabled");
        (None, false)
    } else {
        match bridge::start(
            cli.studio_port,
            Arc::clone(&agent),
            Arc::clone(&fs),
            studio_broker.clone(),
        )
        .await
        {
            Ok(handle) => {
                println!(
                    "Studio bridge: http://127.0.0.1:{} (Roldex Studio plugin ready)",
                    cli.studio_port
                );
                (Some(handle), true)
            }
            Err(error) => {
                eprintln!("Studio bridge unavailable: {error:#}");
                eprintln!(
                    "The CLI will continue. Use --studio-port <port> or --no-studio-bridge if needed."
                );
                (None, false)
            }
        }
    };

    let stdin = io::stdin();
    loop {
        print!("\nroldex> ");
        io::stdout().flush()?;

        let mut input = String::new();
        if stdin.read_line(&mut input)? == 0 {
            break;
        }

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        if matches!(input, "/quit" | "/exit") {
            break;
        }

        if input == "/help" {
            print_help();
            continue;
        }

        if input == "/status" {
            println!("{}", project.describe());
            println!("Studio connected: {}", studio_broker.is_connected());
            continue;
        }

        if input == "/doctor" {
            print_doctor(
                &project,
                &config,
                bridge_available,
                studio_broker.is_connected(),
                cli.studio_port,
            );
            continue;
        }

        if input == "/tree" {
            println!("{}", project_tree(&root, 3, 120)?);
            continue;
        }

        if let Some(path) = input.strip_prefix("/read ") {
            match fs.read_text(path.trim()) {
                Ok(text) => println!("{text}"),
                Err(error) => eprintln!("error: {error:#}"),
            }
            continue;
        }

        if let Some(arguments) = input.strip_prefix("/image ") {
            let (path, prompt) = match arguments.split_once("::") {
                Some((path, prompt)) => (path.trim(), prompt.trim()),
                None => (
                    arguments.trim(),
                    "Analyze this Roblox Studio screenshot or image. Identify the relevant UI, code, scene, error, or design details and help me fix or improve the project.",
                ),
            };

            if path.is_empty() {
                eprintln!("usage: /image <path> :: <optional prompt>");
                continue;
            }

            if config.ui.show_progress {
                println!("• Reading image and sending vision context...");
            }

            let show_progress = config.ui.show_progress;
            let result = {
                let mut agent = agent.lock().await;
                agent
                    .chat_with_image(prompt, path, fs.as_ref(), |event| {
                        if show_progress {
                            println!("• {event}");
                        }
                    })
                    .await
            };

            match result {
                Ok(answer) => println!("\n{answer}\n\n✓ Finished"),
                Err(error) => eprintln!("\nVision request failed: {error:#}"),
            }
            continue;
        }

        let image_paths = intent::detect_image_paths(input, fs.as_ref());
        if config.ui.show_progress {
            if image_paths.is_empty() {
                println!("• Working autonomously in Roblox/Luau context...");
            } else {
                println!(
                    "• Attached {} image{} from your message...",
                    image_paths.len(),
                    if image_paths.len() == 1 { "" } else { "s" }
                );
            }
        }

        let show_progress = config.ui.show_progress;
        let result = {
            let mut agent = agent.lock().await;
            if image_paths.is_empty() {
                agent
                    .chat_with_tools(input, fs.as_ref(), |event| {
                        if show_progress {
                            println!("• {event}");
                        }
                    })
                    .await
            } else {
                agent
                    .chat_with_images(input, &image_paths, fs.as_ref(), |event| {
                        if show_progress {
                            println!("• {event}");
                        }
                    })
                    .await
            }
        };

        match result {
            Ok(answer) => println!("\n{answer}\n\n✓ Finished"),
            Err(error) => eprintln!("\nAI request failed: {error:#}"),
        }
    }

    Ok(())
}

fn print_banner(project: &ProjectSummary, config: &Config) {
    println!("Roldex v{}", env!("CARGO_PKG_VERSION"));
    println!("Roblox Studio development agent");
    println!("Project: {}", project.root.display());
    println!("Detected: {}", project.kind);
    println!("Model: {}", config.ai.model);
    println!("Permissions: {}", config.permissions.mode);
    println!(
        "Just type the finished result you want. Roldex should inspect, build, test, repair and verify it without unnecessary pauses."
    );
}

fn print_doctor(
    project: &ProjectSummary,
    config: &Config,
    bridge_available: bool,
    studio_connected: bool,
    port: u16,
) {
    let ai_key = env::var_os(&config.ai.api_key_env).is_some();
    let media_key = env::var_os("POLLINATIONS_API_KEY").is_some();
    let git_available = Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success());

    println!("Roldex doctor");
    println!("OS/arch: {}/{}", env::consts::OS, env::consts::ARCH);
    println!("Project: {} ({})", project.root.display(), project.kind);
    println!("Permissions: {}", config.permissions.mode);
    println!(
        "AI key {}: {}",
        config.ai.api_key_env,
        if ai_key { "configured" } else { "missing" }
    );
    println!(
        "Media key POLLINATIONS_API_KEY: {}",
        if media_key {
            "configured"
        } else {
            "optional / missing"
        }
    );
    println!(
        "Git: {}",
        if git_available {
            "available"
        } else {
            "not found"
        }
    );
    println!(
        "Studio bridge listener: {} on 127.0.0.1:{port}",
        if bridge_available {
            "running"
        } else {
            "not running"
        }
    );
    println!(
        "Studio plugin connection: {}",
        if studio_connected {
            "connected"
        } else {
            "not connected"
        }
    );

    #[cfg(windows)]
    if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
        let plugin = PathBuf::from(local_app_data)
            .join("Roblox")
            .join("Plugins")
            .join("RoldexStudio.plugin.lua");
        println!(
            "Studio plugin: {} ({})",
            if plugin.exists() {
                "installed"
            } else {
                "not found"
            },
            plugin.display()
        );
    }
}

fn print_help() {
    println!(
        "Roldex understands normal language by default. Examples:\n  build my whole lobby in the open Studio place and test it\n  create a shop GUI using real Instances, then wire the scripts\n  fix my datastore system and keep going until it passes checks\n  look at \"screenshots/studio error.png\" and fix it\n  generate a square hand-drawn shop icon and save it under assets/ui\n  search current Roblox docs for MemoryStore sorted maps\n\nRoldex prefers real Studio Instances for maps/UI/building and can execute visible Studio changes when the plugin is connected.\n\nOptional shortcuts:\n  /help                         Show this help\n  /doctor                       Check local Roldex setup\n  /status                       Show project and Studio connection details\n  /tree                         Show project tree\n  /read <path>                  Read a UTF-8 file allowed by the current permission mode\n  /image <path> :: <prompt>     Legacy explicit image-analysis shortcut\n  /quit                         Exit Roldex\n\nCLI option:\n  --full-access                 Permit needed file/process access outside the active project"
    );
}
