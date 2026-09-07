use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(windows)]
use std::{env, fs, path::PathBuf};

#[cfg(windows)]
use anyhow::Context;
use anyhow::{Result, bail};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tokio::sync::{Mutex, oneshot};
use tokio::time::timeout;

use crate::ToolCall;
use crate::WorkspaceFs;
use crate::tools::{AgentEvent, ToolExecution};

const STUDIO_COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
const STUDIO_CONNECTED_WINDOW_SECS: u64 = 5;
const MAX_QUEUE: usize = 128;
const MAX_CAPTURE_BYTES: usize = 4 * 1024 * 1024;
#[cfg(windows)]
const EMBEDDED_PLUGIN: &str =
    include_str!("../../../plugins/roldex-studio/RoldexStudio.plugin.lua");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudioCommand {
    pub id: String,
    pub action: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudioCommandResult {
    pub id: String,
    pub ok: bool,
    #[serde(default)]
    pub output: Option<Value>,
    #[serde(default)]
    pub error: Option<String>,
}

struct BrokerInner {
    queue: Mutex<VecDeque<StudioCommand>>,
    pending: Mutex<HashMap<String, oneshot::Sender<StudioCommandResult>>>,
    next_id: AtomicU64,
    last_seen: AtomicU64,
}

#[derive(Clone)]
pub struct StudioBroker {
    inner: Arc<BrokerInner>,
}

impl Default for StudioBroker {
    fn default() -> Self {
        Self::new()
    }
}

impl StudioBroker {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(BrokerInner {
                queue: Mutex::new(VecDeque::new()),
                pending: Mutex::new(HashMap::new()),
                next_id: AtomicU64::new(1),
                last_seen: AtomicU64::new(0),
            }),
        }
    }

    pub fn mark_seen(&self) {
        self.inner.last_seen.store(now_secs(), Ordering::Relaxed);
    }

    pub fn is_connected(&self) -> bool {
        let last = self.inner.last_seen.load(Ordering::Relaxed);
        last != 0 && now_secs().saturating_sub(last) <= STUDIO_CONNECTED_WINDOW_SECS
    }

    pub async fn pending_count(&self) -> usize {
        self.inner.pending.lock().await.len()
    }

    pub async fn poll(&self, max_commands: usize) -> Vec<StudioCommand> {
        self.mark_seen();
        let mut queue = self.inner.queue.lock().await;
        let max_commands = max_commands.clamp(1, 16);
        let mut commands = Vec::with_capacity(max_commands.min(queue.len()));
        for _ in 0..max_commands {
            let Some(command) = queue.pop_front() else {
                break;
            };
            commands.push(command);
        }
        commands
    }

    pub async fn complete(&self, result: StudioCommandResult) -> bool {
        self.mark_seen();
        let sender = self.inner.pending.lock().await.remove(&result.id);
        sender.is_some_and(|sender| sender.send(result).is_ok())
    }

    async fn submit(&self, action: &str, payload: Value) -> Result<StudioCommandResult> {
        if !self.is_connected() {
            bail!(
                "Roldex Studio plugin is not connected. Try repair_studio_plugin, then re-check Studio connectivity. If Studio already has the plugin open, it may need a restart to reload repaired plugin code."
            );
        }

        if self.inner.queue.lock().await.len() >= MAX_QUEUE {
            bail!("Studio command queue is full; wait for the plugin to catch up");
        }

        let id = format!(
            "studio-{}",
            self.inner.next_id.fetch_add(1, Ordering::Relaxed)
        );
        let (tx, rx) = oneshot::channel();
        self.inner.pending.lock().await.insert(id.clone(), tx);
        self.inner.queue.lock().await.push_back(StudioCommand {
            id: id.clone(),
            action: action.to_owned(),
            payload,
        });

        match timeout(STUDIO_COMMAND_TIMEOUT, rx).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(_)) => {
                self.inner.pending.lock().await.remove(&id);
                bail!("Studio command result channel closed unexpectedly")
            }
            Err(_) => {
                self.inner.pending.lock().await.remove(&id);
                bail!(
                    "Studio command timed out after {} seconds. The plugin may be busy, disconnected, playtesting, or blocked by a Studio permission/prompt.",
                    STUDIO_COMMAND_TIMEOUT.as_secs()
                )
            }
        }
    }
}

pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "studio_health",
                "description": "Check whether the Roldex Studio plugin is actively connected and polling for commands.",
                "parameters": { "type": "object", "properties": {}, "additionalProperties": false }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "studio_query",
                "description": "Read live Roblox Studio state through the connected plugin. Operations: selection, inspect, children, find. For inspect/children provide path. For find provide optional root_path, name_contains, class_name, and max_results. Paths use Roblox full names such as Workspace.Map.Door or ReplicatedStorage.Remotes.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "operation": { "type": "string", "enum": ["selection", "inspect", "children", "find"] },
                        "path": { "type": "string" },
                        "root_path": { "type": "string" },
                        "name_contains": { "type": "string" },
                        "class_name": { "type": "string" },
                        "properties": { "type": "array", "items": { "type": "string" }, "maxItems": 80 },
                        "max_results": { "type": "integer", "minimum": 1, "maximum": 100 }
                    },
                    "required": ["operation"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "studio_batch",
                "description": "Apply an atomic, undoable batch of Roblox Studio changes. Changes execute one action at a time in Studio so the user can watch Roldex build in real time. Prefer this tool and real Instances over generating one-off builder scripts. Supported action op values: create, set_properties, set_attributes, move, clone, delete, update_script, select, add_tag, remove_tag, pivot_to, terrain_fill_block, terrain_fill_ball, terrain_clear. A create/clone action may include ref so later actions can target '$ref'. Property values may be primitives or typed objects using $type: Vector3{x,y,z}, Vector2{x,y}, Color3{r,g,b}, UDim{scale,offset}, UDim2{x_scale,x_offset,y_scale,y_offset}, CFrame{components:[12 numbers]}, Enum{enum_type,item}, BrickColor{name}, NumberRange{min,max}, Rect{min_x,min_y,max_x,max_y}, Instance{path}, NumberSequence, ColorSequence. For scripts, create can include source or update_script can replace source through ScriptEditorService. Keep live_delay_ms around 30-80 for visible builds and lower it only for very large repetitive batches. Verify important changes afterward with studio_query and visually when appropriate.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "label": { "type": "string", "description": "Short undo-history label" },
                        "live_delay_ms": { "type": "integer", "minimum": 0, "maximum": 500 },
                        "highlight_created": { "type": "boolean" },
                        "actions": {
                            "type": "array",
                            "minItems": 1,
                            "maxItems": 100,
                            "items": { "type": "object", "additionalProperties": true }
                        }
                    },
                    "required": ["actions"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "studio_capture_view",
                "description": "Capture the current Roblox Studio viewport as a downscaled PNG and save it under .roldex/captures for visual AI analysis. Use after map/UI/lighting/visual changes and during visual debugging. Studio may ask for screenshot permission the first time.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "width": { "type": "integer", "minimum": 160, "maximum": 1280 },
                        "height": { "type": "integer", "minimum": 90, "maximum": 720 },
                        "include_ui": { "type": "boolean" }
                    },
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "studio_device",
                "description": "Inspect or control Studio device simulation for responsive UI testing. Operations: status, list, set_device, set_resolution, set_orientation, stop. Use this to validate phone/tablet/desktop layouts instead of assuming one viewport size.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "operation": { "type": "string", "enum": ["status", "list", "set_device", "set_resolution", "set_orientation", "stop"] },
                        "device_id": { "type": "string" },
                        "width": { "type": "integer", "minimum": 240, "maximum": 7680 },
                        "height": { "type": "integer", "minimum": 240, "maximum": 4320 },
                        "orientation": { "type": "string", "enum": ["landscape", "portrait", "sensor"] }
                    },
                    "required": ["operation"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "studio_test",
                "description": "Run an automated Roblox Studio smoke/play/multiplayer test and collect Studio output. Modes: run, play, multiplayer. Tests are bounded by timeout_seconds; multiplayer supports 1-8 players. Optional input_steps can simulate player-facing keyboard, mouse, pointer, or text interactions when Roblox exposes VirtualInput to this plugin context. Optional capture_at_seconds takes a visual snapshot during the test. Use this for end-to-end UI, movement, interaction, animation, progression, spawn and regression checks. Treat output errors/warnings or unavailable automation capabilities as signals to inspect/repair rather than pretending the test passed.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "mode": { "type": "string", "enum": ["run", "play", "multiplayer"] },
                        "players": { "type": "integer", "minimum": 1, "maximum": 8 },
                        "timeout_seconds": { "type": "integer", "minimum": 2, "maximum": 90 },
                        "capture_at_seconds": { "type": "number", "minimum": 0.2, "maximum": 80 },
                        "input_steps": {
                            "type": "array",
                            "maxItems": 100,
                            "items": {
                                "type": "object",
                                "properties": {
                                    "at_seconds": { "type": "number", "minimum": 0 },
                                    "type": { "type": "string", "enum": ["key", "mouse_button", "mouse_move", "mouse_position", "text", "pointer"] },
                                    "key_code": { "type": "string" },
                                    "hold_seconds": { "type": "number", "minimum": 0, "maximum": 10 },
                                    "x": { "type": "number" },
                                    "y": { "type": "number" },
                                    "button": { "type": "string" },
                                    "text": { "type": "string" },
                                    "pointer_action": { "type": "object", "additionalProperties": true }
                                },
                                "required": ["type"],
                                "additionalProperties": false
                            }
                        },
                        "args": {}
                    },
                    "required": ["mode"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "studio_undo",
                "description": "Undo the most recent Studio change using ChangeHistoryService. Use only when the task requires undoing a Studio mutation.",
                "parameters": { "type": "object", "properties": {}, "additionalProperties": false }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "studio_redo",
                "description": "Redo the next Studio history action using ChangeHistoryService.",
                "parameters": { "type": "object", "properties": {}, "additionalProperties": false }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "repair_studio_plugin",
                "description": "Repair/reinstall the local Roldex Studio plugin from the exact plugin source embedded in this Roldex build. Use automatically when Studio tools fail because the plugin is missing/corrupt on Windows. Studio may need to be restarted after repair so it reloads the plugin.",
                "parameters": { "type": "object", "properties": {}, "additionalProperties": false }
            }
        }),
    ]
}

pub async fn execute_tool(
    call: &ToolCall,
    broker: Option<&StudioBroker>,
    fs_scope: &WorkspaceFs,
) -> Option<ToolExecution> {
    match call.function.name.as_str() {
        "studio_health" => Some(execute_health(broker).await),
        "studio_query" => Some(execute_remote(call, broker, "query", "Studio query").await),
        "studio_batch" => Some(execute_remote(call, broker, "batch", "Studio live build").await),
        "studio_capture_view" => Some(execute_capture(call, broker, fs_scope).await),
        "studio_device" => Some(execute_remote(call, broker, "device", "Studio device simulation").await),
        "studio_test" => Some(execute_test(call, broker, fs_scope).await),
        "studio_undo" => Some(execute_remote(call, broker, "undo", "Studio undo").await),
        "studio_redo" => Some(execute_remote(call, broker, "redo", "Studio redo").await),
        "repair_studio_plugin" => Some(execute_repair_plugin()),
        _ => None,
    }
}

async fn execute_health(broker: Option<&StudioBroker>) -> ToolExecution {
    let (connected, pending) = if let Some(broker) = broker {
        (broker.is_connected(), broker.pending_count().await)
    } else {
        (false, 0)
    };
    ToolExecution {
        event: AgentEvent::UsingTool("Studio health".into()),
        output: json!({ "ok": true, "connected": connected, "pending_commands": pending })
            .to_string(),
    }
}

async fn execute_remote(
    call: &ToolCall,
    broker: Option<&StudioBroker>,
    action: &str,
    event_name: &str,
) -> ToolExecution {
    let event = AgentEvent::UsingTool(event_name.into());
    let Some(broker) = broker else {
        return ToolExecution {
            event,
            output: error_output("Studio broker is not available in this Roldex session"),
        };
    };

    let payload = match parse_payload(call) {
        Ok(payload) => payload,
        Err(error) => {
            return ToolExecution {
                event,
                output: error_output(error),
            };
        }
    };

    let output = match broker.submit(action, payload).await {
        Ok(result) => serde_json::to_string(&result).unwrap_or_else(|error| {
            error_output(format!("failed to serialize Studio result: {error}"))
        }),
        Err(error) => error_output(error.to_string()),
    };

    ToolExecution { event, output }
}

async fn execute_capture(
    call: &ToolCall,
    broker: Option<&StudioBroker>,
    fs_scope: &WorkspaceFs,
) -> ToolExecution {
    let event = AgentEvent::UsingTool("Capturing Studio viewport".into());
    let Some(broker) = broker else {
        return ToolExecution {
            event,
            output: error_output("Studio broker is not available in this Roldex session"),
        };
    };
    let payload = match parse_payload(call) {
        Ok(payload) => payload,
        Err(error) => {
            return ToolExecution {
                event,
                output: error_output(error),
            };
        }
    };

    let output = match broker.submit("capture", payload).await {
        Ok(mut result) => match persist_capture_in_result(&mut result, fs_scope) {
            Ok(_) => serde_json::to_string(&result).unwrap_or_else(|error| {
                error_output(format!("failed to serialize Studio capture result: {error}"))
            }),
            Err(error) => error_output(error.to_string()),
        },
        Err(error) => error_output(error.to_string()),
    };
    ToolExecution { event, output }
}

async fn execute_test(
    call: &ToolCall,
    broker: Option<&StudioBroker>,
    fs_scope: &WorkspaceFs,
) -> ToolExecution {
    let event = AgentEvent::UsingTool("Studio automated playtest".into());
    let Some(broker) = broker else {
        return ToolExecution {
            event,
            output: error_output("Studio broker is not available in this Roldex session"),
        };
    };
    let payload = match parse_payload(call) {
        Ok(payload) => payload,
        Err(error) => {
            return ToolExecution {
                event,
                output: error_output(error),
            };
        }
    };

    let output = match broker.submit("test", payload).await {
        Ok(mut result) => match persist_capture_in_result(&mut result, fs_scope) {
            Ok(_) => serde_json::to_string(&result).unwrap_or_else(|error| {
                error_output(format!("failed to serialize Studio test result: {error}"))
            }),
            Err(error) => error_output(error.to_string()),
        },
        Err(error) => error_output(error.to_string()),
    };
    ToolExecution { event, output }
}

fn parse_payload(call: &ToolCall) -> std::result::Result<Value, String> {
    if call.function.arguments.trim().is_empty() {
        Ok(json!({}))
    } else {
        serde_json::from_str::<Value>(&call.function.arguments)
            .map_err(|error| format!("invalid {} arguments: {error}", call.function.name))
    }
}

fn persist_capture_in_result(
    result: &mut StudioCommandResult,
    fs_scope: &WorkspaceFs,
) -> Result<Option<String>> {
    let Some(Value::Object(output)) = result.output.as_mut() else {
        return Ok(None);
    };

    if let Some(path) = persist_capture_object(output, fs_scope)? {
        return Ok(Some(path));
    }

    let Some(Value::Object(capture)) = output.get_mut("capture") else {
        return Ok(None);
    };
    persist_capture_object(capture, fs_scope)
}

fn persist_capture_object(
    object: &mut Map<String, Value>,
    fs_scope: &WorkspaceFs,
) -> Result<Option<String>> {
    let Some(Value::String(encoded)) = object.remove("data_base64") else {
        return Ok(None);
    };
    let bytes = STANDARD
        .decode(encoded.as_bytes())
        .map_err(|error| anyhow::anyhow!("Studio capture base64 was invalid: {error}"))?;
    if bytes.is_empty() {
        bail!("Studio capture returned an empty image");
    }
    if bytes.len() > MAX_CAPTURE_BYTES {
        bail!(
            "Studio capture exceeded the {} byte local limit after decoding",
            MAX_CAPTURE_BYTES
        );
    }

    let relative = format!(".roldex/captures/studio-{}.png", now_millis());
    fs_scope.write_bytes(&relative, &bytes)?;
    object.insert("path".into(), Value::String(relative.clone()));
    object.insert("bytes".into(), json!(bytes.len()));
    Ok(Some(relative))
}

fn execute_repair_plugin() -> ToolExecution {
    let event = AgentEvent::UsingTool("Studio plugin repair".into());
    let output = match install_embedded_plugin() {
        Ok(path) => json!({
            "ok": true,
            "path": path,
            "message": "Roldex Studio plugin source was repaired. If Studio is already open, restart Studio so it reloads the plugin."
        })
        .to_string(),
        Err(error) => error_output(error.to_string()),
    };
    ToolExecution { event, output }
}

fn install_embedded_plugin() -> Result<String> {
    #[cfg(windows)]
    {
        let local_app_data = env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .context("LOCALAPPDATA is not available")?;
        let plugin_dir = local_app_data.join("Roblox").join("Plugins");
        fs::create_dir_all(&plugin_dir)
            .with_context(|| format!("failed to create {}", plugin_dir.display()))?;
        let path = plugin_dir.join("RoldexStudio.plugin.lua");
        fs::write(&path, EMBEDDED_PLUGIN)
            .with_context(|| format!("failed to write {}", path.display()))?;
        return Ok(path.display().to_string());
    }

    #[cfg(not(windows))]
    {
        bail!(
            "automatic Studio plugin repair is currently implemented for Windows; reinstall the plugin from the Roldex repository on this operating system"
        )
    }
}

fn error_output(error: impl ToString) -> String {
    json!({ "ok": false, "error": error.to_string() }).to_string()
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn broker_round_trip() {
        let broker = StudioBroker::new();
        broker.mark_seen();
        let cloned = broker.clone();
        let task = tokio::spawn(async move {
            cloned
                .submit("query", json!({"operation":"selection"}))
                .await
        });
        let commands = broker.poll(4).await;
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].action, "query");
        assert!(
            broker
                .complete(StudioCommandResult {
                    id: commands[0].id.clone(),
                    ok: true,
                    output: Some(json!({"items": []})),
                    error: None,
                })
                .await
        );
        assert!(task.await.expect("join").expect("result").ok);
    }
}
