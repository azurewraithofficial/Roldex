use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Value, json};

use crate::WorkspaceFs;
use crate::tools::{AgentEvent, ToolExecution};

const MAX_FILES: usize = 2_000;
const MAX_FILE_BYTES: u64 = 128 * 1024;
const MAX_DEPTH: usize = 12;

#[derive(Debug, Default, Serialize)]
struct ProjectInventory {
    files_scanned: usize,
    luau_files: usize,
    client_scripts: usize,
    server_scripts: usize,
    shared_scripts: usize,
    module_scripts: usize,
    remotes_mentions: usize,
    datastore_mentions: usize,
    services: BTreeMap<String, usize>,
    notable_files: Vec<String>,
    truncated: bool,
}

pub fn tool_definitions() -> Vec<Value> {
    vec![json!({
        "type": "function",
        "function": {
            "name": "roblox_project_inventory",
            "description": "Build a compact deterministic inventory of the Roblox/Rojo project: Luau file counts, client/server/shared split, commonly used Roblox services, RemoteEvent/RemoteFunction mentions, DataStore usage, and notable project files. Use this before broad architecture work instead of reading the whole repository.",
            "parameters": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }
        }
    })]
}

pub fn execute_tool(call: &crate::ToolCall, fs: &WorkspaceFs) -> Option<ToolExecution> {
    if call.function.name != "roblox_project_inventory" {
        return None;
    }

    let output = match inventory(fs.root()) {
        Ok(report) => json!({ "ok": true, "inventory": report }).to_string(),
        Err(error) => json!({ "ok": false, "error": error.to_string() }).to_string(),
    };

    Some(ToolExecution {
        event: AgentEvent::UsingTool("Roblox project inventory".into()),
        output,
    })
}

fn inventory(root: &Path) -> anyhow::Result<ProjectInventory> {
    let mut report = ProjectInventory::default();
    let mut stack = vec![(root.to_path_buf(), 0usize)];

    while let Some((directory, depth)) = stack.pop() {
        if depth > MAX_DEPTH || report.files_scanned >= MAX_FILES {
            report.truncated = true;
            continue;
        }

        let mut entries: Vec<_> = fs::read_dir(&directory)?.filter_map(Result::ok).collect();
        entries.sort_by_key(|entry| entry.file_name());

        for entry in entries {
            if report.files_scanned >= MAX_FILES {
                report.truncated = true;
                break;
            }

            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(value) => value,
                Err(_) => continue,
            };

            if file_type.is_dir() {
                if should_skip_dir(&path) {
                    continue;
                }
                stack.push((path, depth + 1));
                continue;
            }
            if !file_type.is_file() {
                continue;
            }

            report.files_scanned += 1;
            inspect_file(root, &path, &mut report);
        }
    }

    Ok(report)
}

fn inspect_file(root: &Path, path: &Path, report: &mut ProjectInventory) {
    let relative = path.strip_prefix(root).unwrap_or(path);
    let relative_text = relative.to_string_lossy().replace('\\', "/");
    let lower = relative_text.to_ascii_lowercase();

    if is_notable(&lower) && report.notable_files.len() < 40 {
        report.notable_files.push(relative_text.clone());
    }

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension != "lua" && extension != "luau" {
        return;
    }

    report.luau_files += 1;
    if lower.ends_with(".client.lua")
        || lower.ends_with(".client.luau")
        || lower.contains("/client/")
        || lower.contains("starterplayerscripts")
        || lower.contains("startergui")
    {
        report.client_scripts += 1;
    } else if lower.ends_with(".server.lua")
        || lower.ends_with(".server.luau")
        || lower.contains("/server/")
        || lower.contains("serverscriptservice")
        || lower.contains("serverstorage")
    {
        report.server_scripts += 1;
    } else {
        report.shared_scripts += 1;
    }

    if lower.contains("module") || lower.ends_with(".module.lua") || lower.ends_with(".module.luau") {
        report.module_scripts += 1;
    }

    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.len() > MAX_FILE_BYTES {
        return;
    }
    let Ok(source) = fs::read_to_string(path) else {
        return;
    };

    report.remotes_mentions += count_any(
        &source,
        &["RemoteEvent", "RemoteFunction", "OnServerEvent", "OnClientEvent"],
    );
    report.datastore_mentions += count_any(
        &source,
        &["DataStoreService", "GetDataStore", "UpdateAsync", "SetAsync"],
    );
    collect_services(&source, &mut report.services);
}

fn collect_services(source: &str, services: &mut BTreeMap<String, usize>) {
    let marker = "GetService(";
    let mut remaining = source;
    while let Some(position) = remaining.find(marker) {
        remaining = &remaining[position + marker.len()..];
        let trimmed = remaining.trim_start();
        let Some(quote) = trimmed.chars().next().filter(|value| *value == '\'' || *value == '"') else {
            continue;
        };
        let rest = &trimmed[quote.len_utf8()..];
        let Some(end) = rest.find(quote) else {
            continue;
        };
        let name = &rest[..end];
        if !name.is_empty() && name.len() <= 80 && name.chars().all(|ch| ch.is_ascii_alphanumeric()) {
            *services.entry(name.to_owned()).or_insert(0) += 1;
        }
        remaining = &rest[end + quote.len_utf8()..];
    }
}

fn count_any(source: &str, needles: &[&str]) -> usize {
    needles
        .iter()
        .map(|needle| source.matches(needle).count())
        .sum()
}

fn is_notable(path: &str) -> bool {
    let file = PathBuf::from(path);
    let name = file
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    matches!(
        name,
        "default.project.json"
            | "game.project.json"
            | "rojo.json"
            | "wally.toml"
            | "aftman.toml"
            | "selene.toml"
            | "stylua.toml"
            | "roldex.toml"
            | "ROLDEX.md"
            | "AGENTS.md"
    )
}

fn should_skip_dir(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        name.as_str(),
        ".git" | "target" | "node_modules" | ".cache" | ".idea" | ".vscode"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_services() {
        let mut services = BTreeMap::new();
        collect_services(
            "local Players = game:GetService(\"Players\")\nlocal DSS=game:GetService('DataStoreService')",
            &mut services,
        );
        assert_eq!(services.get("Players"), Some(&1));
        assert_eq!(services.get("DataStoreService"), Some(&1));
    }
}
