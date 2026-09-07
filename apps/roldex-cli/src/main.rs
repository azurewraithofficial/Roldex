use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use roldex_core::{Agent, Config, ProjectSummary, WorkspaceFs, project_tree};

#[derive(Debug, Parser)]
#[command(name = "roldex", version, about = "Roblox Studio development agent")]
struct Cli {
    /// Project root Roldex is allowed to inspect.
    #[arg(long, default_value = ".")]
    root: PathBuf,

    /// Optional configuration file. Defaults to ./roldex.toml when present.
    #[arg(long)]
    config: Option<PathBuf>,
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
    let fs = WorkspaceFs::new(&root, config.permissions.mode)?;
    let mut agent = Agent::new(config.ai.clone(), project.clone());

    print_banner(&project, &config);

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
            println!("• Thinking in Roblox/Luau context...");
        }

        match agent.chat(input).await {
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
        "Commands:\n  /help         Show this help\n  /status       Show project detection details\n  /tree         Show project tree\n  /read <path>  Read a UTF-8 workspace file\n  /quit         Exit Roldex\n\nAnything else is sent to the Roblox-specialized AI."
    );
}
