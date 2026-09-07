mod bridge;
mod intent;
#[cfg_attr(not(windows), allow(unused_mut))]
#[allow(clippy::collapsible_str_replace)]
mod ui;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use roldex_core::{Agent, Config, PermissionMode, ProjectSummary, StudioBroker, WorkspaceFs};
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

    // Project detection is deliberately bounded so Roldex stays responsive even when
    // launched from a broad folder such as the Windows user profile.
    let detection_root = root.clone();
    let project = tokio::task::spawn_blocking(move || ProjectSummary::detect(&detection_root))
        .await
        .context("project detection task failed")??;

    let fs = Arc::new(WorkspaceFs::new(&root, config.permissions.mode)?);
    let studio_broker = StudioBroker::new();
    let agent = Arc::new(Mutex::new(
        Agent::new(config.ai.clone(), project.clone()).with_studio(studio_broker.clone()),
    ));

    let (_bridge_handle, bridge_available) = if cli.no_studio_bridge {
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
            Ok(handle) => (Some(handle), true),
            Err(error) => {
                eprintln!("Studio bridge unavailable: {error:#}");
                eprintln!(
                    "The CLI will continue. Use --studio-port <port> or --no-studio-bridge if needed."
                );
                (None, false)
            }
        }
    };

    ui::run(ui::UiContext {
        root,
        project,
        config,
        agent,
        fs,
        studio_broker,
        bridge_available,
        studio_port: cli.studio_port,
    })
    .await
}
