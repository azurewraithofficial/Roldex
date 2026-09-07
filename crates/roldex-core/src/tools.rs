use std::fmt;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::git::{git_diff, git_status};
use crate::search::search_text;
use crate::{ToolCall, WorkspaceFs, project_tree};

#[derive(Debug, Clone)]
pub enum AgentEvent {
    Reading(String),
    Writing(String),
    Patching(String),
    Deleting(String),
    Searching(String),
    InspectingProject,
    InspectingGit(String),
    UsingTool(String),
}

impl fmt::Display for AgentEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reading(path) => write!(f, "Reading {path}"),
            Self::Writing(path) => write!(f, "Editing {path}"),
            Self::Patching(path) => write!(f, "Patching {path}"),
            Self::Deleting(path) => write!(f, "Deleting {path}"),
            Self::Searching(query) => write!(f, "Searching project for {query:?}"),
            Self::InspectingProject => write!(f, "Inspecting project tree"),
            Self::InspectingGit(action) => write!(f, "Checking Git {action}"),
            Self::UsingTool(name) => write!(f, "Using tool {name}"),
        }
    }
}

pub struct ToolExecution {
    pub event: AgentEvent,
    pub output: String,
}

#[derive(Debug, Deserialize)]
struct PathArgs {
    path: String,
}

#[derive(Debug, Deserialize)]
struct WriteFileArgs {
    path: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ReplaceInFileArgs {
    path: String,
    old_text: String,
    new_text: String,
}

#[derive(Debug, Deserialize)]
struct SearchTextArgs {
    query: String,
    case_sensitive: Option<bool>,
    max_results: Option<usize>,
}

#[derive(Debug, Deserialize, Default)]
struct ProjectTreeArgs {
    max_depth: Option<usize>,
    max_entries: Option<usize>,
}

#[derive(Debug, Deserialize, Default)]
struct GitDiffArgs {
    path: Option<String>,
    staged: Option<bool>,
}

pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "project_tree",
                "description": "Inspect a bounded tree of files and folders in the active Roblox project.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "max_depth": { "type": "integer", "minimum": 1, "maximum": 6 },
                        "max_entries": { "type": "integer", "minimum": 1, "maximum": 300 }
                    },
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "search_text",
                "description": "Search text files in the active project for a literal string. Use this to find scripts, symbols and Roblox API usage before reading specific files.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" },
                        "case_sensitive": { "type": "boolean" },
                        "max_results": { "type": "integer", "minimum": 1, "maximum": 100 }
                    },
                    "required": ["query"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read a bounded UTF-8 text file inside the active Roblox project workspace. Inspect relevant files before changing them.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Workspace-relative file path" }
                    },
                    "required": ["path"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "replace_in_file",
                "description": "Make a small exact edit in an existing UTF-8 file. old_text must occur exactly once. Prefer this over rewriting a whole file for localized changes.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "old_text": { "type": "string" },
                        "new_text": { "type": "string" }
                    },
                    "required": ["path", "old_text", "new_text"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "write_file",
                "description": "Create or completely replace a UTF-8 text file inside the active project workspace. Supply the complete desired file contents.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Workspace-relative file path" },
                        "content": { "type": "string", "description": "Complete UTF-8 file contents" }
                    },
                    "required": ["path", "content"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "delete_file",
                "description": "Delete one file inside the active project workspace. Use only when deletion is necessary for the requested change.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Workspace-relative file path" }
                    },
                    "required": ["path"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "git_status",
                "description": "Show concise Git status for the active project. This is read-only.",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "git_diff",
                "description": "Inspect Git changes. Without path, returns a compact diff summary. With path, returns the detailed diff for that workspace-relative file.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "staged": { "type": "boolean" }
                    },
                    "additionalProperties": false
                }
            }
        }),
    ]
}

pub fn execute_tool(call: &ToolCall, fs: &WorkspaceFs) -> ToolExecution {
    match call.function.name.as_str() {
        "project_tree" => execute_tree(call, fs),
        "search_text" => execute_search(call, fs),
        "read_file" => execute_read(call, fs),
        "replace_in_file" => execute_replace(call, fs),
        "write_file" => execute_write(call, fs),
        "delete_file" => execute_delete(call, fs),
        "git_status" => execute_git_status(fs),
        "git_diff" => execute_git_diff(call, fs),
        _ => ToolExecution {
            event: AgentEvent::UsingTool(call.function.name.clone()),
            output: error_output(format!("unknown tool: {}", call.function.name)),
        },
    }
}

fn execute_tree(call: &ToolCall, fs: &WorkspaceFs) -> ToolExecution {
    let parsed = parse_optional_args::<ProjectTreeArgs>(&call.function.arguments);
    match parsed {
        Ok(args) => {
            let max_depth = args.max_depth.unwrap_or(3).clamp(1, 6);
            let max_entries = args.max_entries.unwrap_or(120).clamp(1, 300);
            let output = match project_tree(fs.root(), max_depth, max_entries) {
                Ok(tree) => json!({ "ok": true, "tree": tree }).to_string(),
                Err(error) => error_output(error.to_string()),
            };
            ToolExecution {
                event: AgentEvent::InspectingProject,
                output,
            }
        }
        Err(error) => invalid_arguments("project_tree", error),
    }
}

fn execute_search(call: &ToolCall, fs: &WorkspaceFs) -> ToolExecution {
    match serde_json::from_str::<SearchTextArgs>(&call.function.arguments) {
        Ok(args) => {
            let event = AgentEvent::Searching(args.query.clone());
            let output = match search_text(
                fs.root(),
                &args.query,
                args.case_sensitive.unwrap_or(false),
                args.max_results.unwrap_or(40),
            ) {
                Ok(matches) => json!({ "ok": true, "matches": matches }).to_string(),
                Err(error) => error_output(error.to_string()),
            };
            ToolExecution { event, output }
        }
        Err(error) => invalid_arguments("search_text", error),
    }
}

fn execute_read(call: &ToolCall, fs: &WorkspaceFs) -> ToolExecution {
    match serde_json::from_str::<PathArgs>(&call.function.arguments) {
        Ok(args) => {
            let event = AgentEvent::Reading(args.path.clone());
            let output = match fs.read_text(&args.path) {
                Ok(content) => {
                    json!({ "ok": true, "path": args.path, "content": content }).to_string()
                }
                Err(error) => error_output(error.to_string()),
            };
            ToolExecution { event, output }
        }
        Err(error) => invalid_arguments("read_file", error),
    }
}

fn execute_replace(call: &ToolCall, fs: &WorkspaceFs) -> ToolExecution {
    match serde_json::from_str::<ReplaceInFileArgs>(&call.function.arguments) {
        Ok(args) => {
            let event = AgentEvent::Patching(args.path.clone());
            let output = match fs.replace_text(&args.path, &args.old_text, &args.new_text) {
                Ok(()) => json!({ "ok": true, "path": args.path }).to_string(),
                Err(error) => error_output(error.to_string()),
            };
            ToolExecution { event, output }
        }
        Err(error) => invalid_arguments("replace_in_file", error),
    }
}

fn execute_write(call: &ToolCall, fs: &WorkspaceFs) -> ToolExecution {
    match serde_json::from_str::<WriteFileArgs>(&call.function.arguments) {
        Ok(args) => {
            let event = AgentEvent::Writing(args.path.clone());
            let output = match fs.write_text(&args.path, &args.content) {
                Ok(()) => json!({ "ok": true, "path": args.path }).to_string(),
                Err(error) => error_output(error.to_string()),
            };
            ToolExecution { event, output }
        }
        Err(error) => invalid_arguments("write_file", error),
    }
}

fn execute_delete(call: &ToolCall, fs: &WorkspaceFs) -> ToolExecution {
    match serde_json::from_str::<PathArgs>(&call.function.arguments) {
        Ok(args) => {
            let event = AgentEvent::Deleting(args.path.clone());
            let output = match fs.delete_file(&args.path) {
                Ok(()) => json!({ "ok": true, "path": args.path }).to_string(),
                Err(error) => error_output(error.to_string()),
            };
            ToolExecution { event, output }
        }
        Err(error) => invalid_arguments("delete_file", error),
    }
}

fn execute_git_status(fs: &WorkspaceFs) -> ToolExecution {
    let output = match git_status(fs.root()) {
        Ok(status) => json!({ "ok": true, "status": status }).to_string(),
        Err(error) => error_output(error.to_string()),
    };
    ToolExecution {
        event: AgentEvent::InspectingGit("status".into()),
        output,
    }
}

fn execute_git_diff(call: &ToolCall, fs: &WorkspaceFs) -> ToolExecution {
    let parsed = parse_optional_args::<GitDiffArgs>(&call.function.arguments);
    match parsed {
        Ok(args) => {
            if let Some(path) = args.path.as_deref()
                && let Err(error) = fs.validate_relative_path(path)
            {
                return ToolExecution {
                    event: AgentEvent::InspectingGit("diff".into()),
                    output: error_output(error.to_string()),
                };
            }

            let output = match git_diff(
                fs.root(),
                args.path.as_deref(),
                args.staged.unwrap_or(false),
            ) {
                Ok(diff) => json!({ "ok": true, "diff": diff }).to_string(),
                Err(error) => error_output(error.to_string()),
            };
            ToolExecution {
                event: AgentEvent::InspectingGit("diff".into()),
                output,
            }
        }
        Err(error) => invalid_arguments("git_diff", error),
    }
}

fn parse_optional_args<T>(arguments: &str) -> Result<T, serde_json::Error>
where
    T: for<'de> Deserialize<'de> + Default,
{
    if arguments.trim().is_empty() {
        Ok(T::default())
    } else {
        serde_json::from_str(arguments)
    }
}

fn invalid_arguments(name: &str, error: serde_json::Error) -> ToolExecution {
    ToolExecution {
        event: AgentEvent::UsingTool(name.to_owned()),
        output: error_output(format!("invalid tool arguments: {error}")),
    }
}

fn error_output(message: impl Into<String>) -> String {
    json!({ "ok": false, "error": message.into() }).to_string()
}
