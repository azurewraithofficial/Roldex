mod bridge;
mod intent;
mod plugin_sync;
#[cfg_attr(not(windows), allow(unused_mut))]
#[allow(clippy::collapsible_str_replace)]
mod ui;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use roldex_core::{
    Agent, Config, DEFAULT_LOCAL_AI_ENDPOINT, DEFAULT_LOCAL_AI_MODEL, PermissionMode,
    ProjectSummary, StudioBroker, WorkspaceFs,
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

    /// Use a keyless local OpenAI-compatible model server instead of the cloud provider.
    #[arg(long)]
    local_ai: bool,

    /// Local OpenAI-compatible chat-completions endpoint. Implies --local-ai.
    #[arg(long, value_name = "URL")]
    local_endpoint: Option<String>,

    /// Model name sent to the local server. Implies --local-ai.
    #[arg(long, value_name = "MODEL")]
    local_model: Option<String>,

    /// Localhost port used by the Roldex Studio plugin.
    #[arg(long, default_value_t = DEFAULT_STUDIO_PORT)]
    studio_port: u16,

    /// Disable the localhost Roblox Studio bridge.
    #[arg(long)]
    no_studio_bridge: bool,

    /// Reinstall both bundled Roldex Studio plugins, then exit.
    #[arg(long)]
    repair_studio_plugin: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.repair_studio_plugin {
        let report = plugin_sync::repair_plugins()?;
        if report.paths.is_empty() {
            println!("Automatic Studio plugin repair is currently available on Windows.");
        } else {
            for path in report.paths {
                println!("Repaired {}", path.display());
            }
            println!("Restart Roblox Studio so the repaired plugins reload.");
        }
        return Ok(());
    }

    let plugin_sync = plugin_sync::sync_plugins()?;
    if plugin_sync.changed {
        eprintln!("Roldex Studio plugins were refreshed. Restart Studio if it is already open.");
    }

    let root = cli
        .root
        .canonicalize()
        .with_context(|| format!("could not open project root {}", cli.root.display()))?;

    let mut config = Config::load(cli.config.as_deref())?;
    if cli.full_access {
        config.permissions.mode = PermissionMode::FullAccess;
    }

    if cli.local_ai || cli.local_endpoint.is_some() || cli.local_model.is_some() {
        config
            .ai
            .activate_local(cli.local_endpoint.as_deref(), cli.local_model.as_deref());
        if cli.local_endpoint.is_none() {
            config.ai.endpoint = DEFAULT_LOCAL_AI_ENDPOINT.into();
        }
        if cli.local_model.is_none() {
            config.ai.model = DEFAULT_LOCAL_AI_MODEL.into();
        }
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
