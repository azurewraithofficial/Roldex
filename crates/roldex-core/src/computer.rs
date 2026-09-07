use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::process::Command;
use tokio::time::timeout;

use crate::tools::{AgentEvent, ToolExecution};
use crate::{PermissionMode, ToolCall, WorkspaceFs};

const MAX_DIRECTORY_ENTRIES: usize = 300;
const MAX_FIND_RESULTS: usize = 200;
const MAX_FIND_SCANNED: usize = 20_000;
const MAX_FIND_DEPTH: usize = 12;
const MAX_PROCESS_OUTPUT: usize = 96 * 1024;

#[derive(Debug, Deserialize)]
struct PathArgs {
    path: String,
    max_entries: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct FindFilesArgs {
    root: Option<String>,
    name_contains: Option<String>,
    extensions: Option<Vec<String>>,
    max_results: Option<usize>,
    max_depth: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct RunProcessArgs {
    program: String,
    #[serde(default)]
    args: Vec<String>,
    cwd: Option<String>,
    timeout_seconds: Option<u64>,
}

pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "list_directory",
                "description": "List a bounded directory. Relative paths are relative to the active project. Absolute paths are permitted only in full-access mode. Use this to inspect local tooling/config/plugin folders when needed rather than guessing paths.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "max_entries": { "type": "integer", "minimum": 1, "maximum": 300 }
                    },
                    "required": ["path"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "file_info",
                "description": "Inspect metadata for one local path, including canonical path, type, size and read-only status. Absolute paths require full-access mode unless they resolve inside the project.",
                "parameters": {
                    "type": "object",
                    "properties": { "path": { "type": "string" } },
                    "required": ["path"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "find_files",
                "description": "Recursively find files/directories by name under a bounded root. Useful for locating Roblox Studio plugin files, configs, logs, Rojo files, build tools, or project dependencies. Absolute roots require full-access mode.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string" },
                        "name_contains": { "type": "string" },
                        "extensions": { "type": "array", "items": { "type": "string" }, "maxItems": 20 },
                        "max_results": { "type": "integer", "minimum": 1, "maximum": 200 },
                        "max_depth": { "type": "integer", "minimum": 1, "maximum": 12 }
                    },
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "run_process",
                "description": "Run a local executable directly (no implicit shell), capture bounded stdout/stderr, and enforce a timeout. This requires full-access mode because arbitrary processes can affect the whole computer. Use it to run cargo, git, rojo, stylua, selene, tests, installers or diagnostic commands needed to finish the task. Prefer direct programs/args over cmd.exe or PowerShell when possible.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "program": { "type": "string" },
                        "args": { "type": "array", "items": { "type": "string" }, "maxItems": 100 },
                        "cwd": { "type": "string" },
                        "timeout_seconds": { "type": "integer", "minimum": 1, "maximum": 120 }
                    },
                    "required": ["program"],
                    "additionalProperties": false
                }
            }
        }),
    ]
}

pub async fn execute_tool(call: &ToolCall, fs: &WorkspaceFs) -> Option<ToolExecution> {
    match call.function.name.as_str() {
        "list_directory" => Some(execute_list(call, fs)),
        "file_info" => Some(execute_info(call, fs)),
        "find_files" => Some(execute_find(call, fs)),
        "run_process" => Some(execute_process(call, fs).await),
        _ => None,
    }
}

fn execute_list(call: &ToolCall, fs: &WorkspaceFs) -> ToolExecution {
    let event = AgentEvent::UsingTool("directory listing".into());
    let output = match serde_json::from_str::<PathArgs>(&call.function.arguments) {
        Ok(args) => match list_directory(fs, &args.path, args.max_entries.unwrap_or(120)) {
            Ok(entries) => json!({ "ok": true, "path": args.path, "entries": entries }).to_string(),
            Err(error) => error_output(error.to_string()),
        },
        Err(error) => error_output(format!("invalid list_directory arguments: {error}")),
    };
    ToolExecution { event, output }
}

fn execute_info(call: &ToolCall, fs: &WorkspaceFs) -> ToolExecution {
    let event = AgentEvent::UsingTool("file metadata".into());
    let output = match serde_json::from_str::<PathArgs>(&call.function.arguments) {
        Ok(args) => match fs.resolve_user_read_path(&args.path) {
            Ok(path) => match fs::metadata(&path) {
                Ok(metadata) => json!({
                    "ok": true,
                    "requested_path": args.path,
                    "canonical_path": path,
                    "is_file": metadata.is_file(),
                    "is_dir": metadata.is_dir(),
                    "bytes": metadata.len(),
                    "readonly": metadata.permissions().readonly()
                })
                .to_string(),
                Err(error) => error_output(error.to_string()),
            },
            Err(error) => error_output(error.to_string()),
        },
        Err(error) => error_output(format!("invalid file_info arguments: {error}")),
    };
    ToolExecution { event, output }
}

fn execute_find(call: &ToolCall, fs: &WorkspaceFs) -> ToolExecution {
    let event = AgentEvent::UsingTool("file discovery".into());
    let output = match serde_json::from_str::<FindFilesArgs>(&call.function.arguments) {
        Ok(args) => match find_files(fs, args) {
            Ok((results, scanned, truncated)) => json!({
                "ok": true,
                "results": results,
                "scanned": scanned,
                "truncated": truncated
            })
            .to_string(),
            Err(error) => error_output(error.to_string()),
        },
        Err(error) => error_output(format!("invalid find_files arguments: {error}")),
    };
    ToolExecution { event, output }
}

async fn execute_process(call: &ToolCall, fs: &WorkspaceFs) -> ToolExecution {
    let event = AgentEvent::UsingTool("local process".into());
    let output = match serde_json::from_str::<RunProcessArgs>(&call.function.arguments) {
        Ok(args) => match run_process(fs, args).await {
            Ok(result) => result.to_string(),
            Err(error) => error_output(error.to_string()),
        },
        Err(error) => error_output(format!("invalid run_process arguments: {error}")),
    };
    ToolExecution { event, output }
}

fn list_directory(fs_scope: &WorkspaceFs, requested: &str, max_entries: usize) -> anyhow::Result<Vec<Value>> {
    let path = fs_scope.resolve_user_read_path(requested)?;
    let metadata = fs::metadata(&path)?;
    anyhow::ensure!(metadata.is_dir(), "{} is not a directory", path.display());
    let mut entries = fs::read_dir(&path)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    let mut output = Vec::new();
    for entry in entries.into_iter().take(max_entries.clamp(1, MAX_DIRECTORY_ENTRIES)) {
        let metadata = entry.metadata().ok();
        output.push(json!({
            "name": entry.file_name().to_string_lossy(),
            "path": entry.path(),
            "is_file": metadata.as_ref().is_some_and(|m| m.is_file()),
            "is_dir": metadata.as_ref().is_some_and(|m| m.is_dir()),
            "bytes": metadata.as_ref().map(|m| m.len())
        }));
    }
    Ok(output)
}

fn find_files(fs_scope: &WorkspaceFs, args: FindFilesArgs) -> anyhow::Result<(Vec<String>, usize, bool)> {
    let root_requested = args.root.as_deref().unwrap_or(".");
    let root = fs_scope.resolve_user_read_path(root_requested)?;
    anyhow::ensure!(fs::metadata(&root)?.is_dir(), "{} is not a directory", root.display());
    let needle = args.name_contains.unwrap_or_default().to_ascii_lowercase();
    let extensions: Vec<String> = args
        .extensions
        .unwrap_or_default()
        .into_iter()
        .map(|extension| extension.trim_start_matches('.').to_ascii_lowercase())
        .collect();
    let max_results = args.max_results.unwrap_or(60).clamp(1, MAX_FIND_RESULTS);
    let max_depth = args.max_depth.unwrap_or(8).clamp(1, MAX_FIND_DEPTH);

    let mut stack = vec![(root, 0usize)];
    let mut results = Vec::new();
    let mut scanned = 0usize;
    let mut truncated = false;

    while let Some((directory, depth)) = stack.pop() {
        if scanned >= MAX_FIND_SCANNED || results.len() >= max_results {
            truncated = true;
            break;
        }
        let Ok(read_dir) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in read_dir.flatten() {
            if scanned >= MAX_FIND_SCANNED || results.len() >= max_results {
                truncated = true;
                break;
            }
            scanned += 1;
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            let name_matches = needle.is_empty() || name.contains(&needle);
            let extension_matches = extensions.is_empty()
                || path
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|extension| extensions.iter().any(|candidate| candidate == &extension.to_ascii_lowercase()));
            if name_matches && extension_matches {
                results.push(path.display().to_string());
            }
            if file_type.is_dir() && depth < max_depth && !skip_directory(&path) {
                stack.push((path, depth + 1));
            }
        }
    }

    Ok((results, scanned, truncated))
}

async fn run_process(fs_scope: &WorkspaceFs, args: RunProcessArgs) -> anyhow::Result<Value> {
    anyhow::ensure!(
        fs_scope.mode() == PermissionMode::FullAccess,
        "run_process requires full-access mode; restart Roldex with --full-access or set permissions.mode = \"full_access\""
    );
    let program = args.program.trim();
    anyhow::ensure!(!program.is_empty(), "program cannot be empty");

    let cwd: PathBuf = if let Some(cwd) = args.cwd.as_deref() {
        fs_scope.resolve_user_read_path(cwd)?
    } else {
        fs_scope.root().to_path_buf()
    };
    anyhow::ensure!(fs::metadata(&cwd)?.is_dir(), "process cwd is not a directory");

    let mut command = Command::new(program);
    command.args(&args.args).current_dir(&cwd).kill_on_drop(true);
    let duration = Duration::from_secs(args.timeout_seconds.unwrap_or(60).clamp(1, 120));
    let output = timeout(duration, command.output())
        .await
        .map_err(|_| anyhow::anyhow!("process timed out after {} seconds", duration.as_secs()))??;

    let stdout = bounded_utf8(&output.stdout, MAX_PROCESS_OUTPUT);
    let stderr = bounded_utf8(&output.stderr, MAX_PROCESS_OUTPUT);
    Ok(json!({
        "ok": output.status.success(),
        "program": program,
        "args": args.args,
        "cwd": cwd,
        "exit_code": output.status.code(),
        "stdout": stdout,
        "stderr": stderr,
        "stdout_truncated": output.stdout.len() > MAX_PROCESS_OUTPUT,
        "stderr_truncated": output.stderr.len() > MAX_PROCESS_OUTPUT
    }))
}

fn bounded_utf8(bytes: &[u8], max_bytes: usize) -> String {
    String::from_utf8_lossy(&bytes[..bytes.len().min(max_bytes)]).into_owned()
}

fn skip_directory(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(name.as_str(), ".git" | "target" | "node_modules" | ".cache")
}

fn error_output(error: impl ToString) -> String {
    json!({ "ok": false, "error": error.to_string() }).to_string()
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn test_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("roldex-computer-{label}-{}-{unique}", std::process::id()))
    }

    #[test]
    fn finds_files_in_bounded_tree() {
        let root = test_dir("find");
        fs::create_dir_all(root.join("nested")).expect("mkdir");
        fs::write(root.join("nested/RoldexStudio.plugin.lua"), "plugin").expect("write");
        let scope = WorkspaceFs::new(&root, PermissionMode::Workspace).expect("scope");
        let (results, _, _) = find_files(
            &scope,
            FindFilesArgs {
                root: None,
                name_contains: Some("roldexstudio".into()),
                extensions: Some(vec!["lua".into()]),
                max_results: None,
                max_depth: None,
            },
        )
        .expect("find");
        assert_eq!(results.len(), 1);
        fs::remove_dir_all(root).expect("cleanup");
    }
}
