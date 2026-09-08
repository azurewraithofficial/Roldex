use std::path::PathBuf;

use anyhow::{Context, Result};

#[derive(Debug, Default)]
pub struct PluginSyncReport {
    pub changed: bool,
    pub paths: Vec<PathBuf>,
}

#[cfg(windows)]
const MAIN_PLUGIN: &str =
    include_str!("../../../plugins/roldex-studio/RoldexStudio.plugin.lua");
#[cfg(windows)]
const RUNTIME_PLUGIN: &str =
    include_str!("../../../plugins/roldex-studio/RoldexStudioRuntime.plugin.lua");

pub fn sync_plugins() -> Result<PluginSyncReport> {
    sync_plugins_inner(false)
}

pub fn repair_plugins() -> Result<PluginSyncReport> {
    sync_plugins_inner(true)
}

#[cfg(windows)]
fn sync_plugins_inner(force: bool) -> Result<PluginSyncReport> {
    use std::fs;

    let local_app_data = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .context("LOCALAPPDATA is not available")?;
    let plugin_dir = local_app_data.join("Roblox").join("Plugins");
    fs::create_dir_all(&plugin_dir)
        .with_context(|| format!("failed to create {}", plugin_dir.display()))?;

    let files = [
        (plugin_dir.join("RoldexStudio.plugin.lua"), MAIN_PLUGIN),
        (
            plugin_dir.join("RoldexStudioRuntime.plugin.lua"),
            RUNTIME_PLUGIN,
        ),
    ];

    let mut report = PluginSyncReport::default();
    for (path, expected) in files {
        let matches = !force
            && fs::read_to_string(&path)
                .map(|current| current == expected)
                .unwrap_or(false);
        if !matches {
            fs::write(&path, expected)
                .with_context(|| format!("failed to write {}", path.display()))?;
            report.changed = true;
        }
        report.paths.push(path);
    }

    Ok(report)
}

#[cfg(not(windows))]
fn sync_plugins_inner(_force: bool) -> Result<PluginSyncReport> {
    Ok(PluginSyncReport::default())
}
