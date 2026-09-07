mod bridge;

use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use roldex_core::{Agent, Config, ProjectSummary, WorkspaceFs, project_tree};
use tokio::sync::Mutex;

const DEFAULT_STUDIO_PORT: u16 = 38247;

#[derive(Debug, Parser)]
#[command(name = "roldex", version, about = "Roblox Studio development agent")]
struct Cli {
    /// Project root Roldex is allowed to inspect.
    #[arg(long, default_value = ".")]
    root: PathBuf,

    /// Optional configuration file. Defaults to ./roldex.toml when present.
    #[arg(long)]
    config: Option<PathBuf>,

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

    let config = Config::load(cli.config.as_deref())?;
    let project = ProjectSummary::detect(&root)?;
    let fs = Arc::new(WorkspaceFs::new(&root, config.permissions.mode)?);
    let agent = Arc::new(Mutex::new(Agent::new(config.ai.clone(), project.clone())));

    print_banner(&project, &config);

    let _bridge_handle = if cli.no_studio_bridge {
        println!("Studio bridge: disabled");
        None
    } else {
        match bridge::start(cli.studio_port, Arc::clone(&agent), Arc::clone(&fs)).await {
            Ok(handle) => {
                println!(
                    "Studio bridge: http://127.0.0.1:{} (Roldex Studio plugin ready)",
                    cli.studio_port
                );
                Some(handle)
            }
            Err(error) => {
                eprintln!("Studio bridge unavailable: {error:#}");
                eprintln!(
                    "The CLI will continue. Use --studio-port <port> or --no-studio-bridge if needed."
                );
                None
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

        if config.ui.show_progress {
            println!("• Working in Roblox/Luau context...");
        }

        let show_progress = config.ui.show_progress;
        let result = {
            let mut agent = agent.lock().await;
            agent
                .chat_with_tools(input, fs.as_ref(), |event| {
                    if show_progress {
                        println!("• {event}");
                    }
                })
                .await
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
    println!("Type /help for commands.");
}

fn print_help() {
    println!(
        "Commands:\n  /help         Show this help\n  /status       Show project detection details\n  /tree         Show project tree\n  /read <path>  Read a UTF-8 workspace file\n  /quit         Exit Roldex\n\nNormal chat can inspect, search, analyze, patch, create and delete workspace files, inspect Git, and accept live context from the Roldex Studio plugin."
    );
}
