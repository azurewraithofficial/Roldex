use std::fmt;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{ToolCall, WorkspaceFs, project_tree};

#[derive(Debug, Clone)]
pub enum AgentEvent {
    Reading(String),
    Writing(String),
    Deleting(String),
    InspectingProject,
    UsingTool(String),
}

impl fmt::Display for AgentEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reading(path) => write!(f, "Reading {path}"),
            Self::Writing(path) => write!(f, "Editing {path}"),
            Self::Deleting(path) => write!(f, "Deleting {path}"),
            Self::InspectingProject => write!(f, "Inspecting project tree"),
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

#[derive(Debug, Deserialize, Default)]
struct ProjectTreeArgs {
    max_depth: Option<usize>,
    max_entries: Option<usize>,
}

pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read a UTF-8 text file inside the active Roblox project workspace. Inspect relevant files before changing them.",
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
    ]
}

pub fn execute_tool(call: &ToolCall, fs: &WorkspaceFs) -> ToolExecution {
    let name = call.function.name.as_str();

    match name {
        "read_file" => execute_read(call, fs),
        "write_file" => execute_write(call, fs),
        "delete_file" => execute_delete(call, fs),
        "project_tree" => execute_tree(call, fs),
        _ => ToolExecution {
            event: AgentEvent::UsingTool(call.function.name.clone()),
            output: error_output(format!("unknown tool: {}", call.function.name)),
        },
    }
}

fn execute_read(call: &ToolCall, fs: &WorkspaceFs) -> ToolExecution {
    match serde_json::from_str::<PathArgs>(&call.function.arguments) {
        Ok(args) => {
            let event = AgentEvent::Reading(args.path.clone());
            let output = match fs.read_text(&args.path) {
                Ok(content) => json!({ "ok": true, "path": args.path, "content": content }).to_string(),
                Err(error) => error_output(error.to_string()),
            };
            ToolExecution { event, output }
        }
        Err(error) => invalid_arguments("read_file", error),
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

fn execute_tree(call: &ToolCall, fs: &WorkspaceFs) -> ToolExecution {
    let parsed = if call.function.arguments.trim().is_empty() {
        Ok(ProjectTreeArgs::default())
    } else {
        serde_json::from_str::<ProjectTreeArgs>(&call.function.arguments)
    };

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

fn invalid_arguments(name: &str, error: serde_json::Error) -> ToolExecution {
    ToolExecution {
        event: AgentEvent::UsingTool(name.to_owned()),
        output: error_output(format!("invalid tool arguments: {error}")),
    }
}

fn error_output(message: impl Into<String>) -> String {
    json!({ "ok": false, "error": message.into() }).to_string()
}
