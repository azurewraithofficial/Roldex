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

use crate::tools::{AgentEvent, ToolExecution};
use crate::{ToolCall, WorkspaceFs};

const STUDIO_COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
const STUDIO_CONNECTED_WINDOW_SECS: u64 = 5;
const MAX_QUEUE: usize = 128;
const MAX_CAPTURE_BYTES: usize = 4 * 1024 * 1024;

#[cfg(windows)]
const EMBEDDED_PLUGIN: &str =
    include_str!("../../../plugins/roldex-studio/RoldexStudio.plugin.lua");
#[cfg(windows)]
const EMBEDDED_RUNTIME_PLUGIN: &str =
    include_str!("../../../plugins/roldex-studio/RoldexStudioRuntime.plugin.lua");

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
    main_queue: Mutex<VecDeque<StudioCommand>>,
    runtime_queue: Mutex<VecDeque<StudioCommand>>,
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
                main_queue: Mutex::new(VecDeque::new()),
                runtime_queue: Mutex::new(VecDeque::new()),
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
        let last_seen = self.inner.last_seen.load(Ordering::Relaxed);
        last_seen != 0 && now_secs().saturating_sub(last_seen) <= STUDIO_CONNECTED_WINDOW_SECS
    }

    pub async fn pending_count(&self) -> usize {
        self.inner.pending.lock().await.len()
    }

    pub async fn poll(&self, max_commands: usize) -> Vec<StudioCommand> {
        self.mark_seen();
        drain_queue(&self.inner.main_queue, max_commands).await
    }

    pub async fn poll_runtime(&self, max_commands: usize) -> Vec<StudioCommand> {
        self.mark_seen();
        drain_queue(&self.inner.runtime_queue, max_commands).await
    }

    pub async fn complete(&self, result: StudioCommandResult) -> bool {
        self.mark_seen();
        self.inner
            .pending
            .lock()
            .await
            .remove(&result.id)
            .is_some_and(|sender| sender.send(result).is_ok())
    }

    pub async fn submit_action(&self, action: &str, payload: Value) -> Result<StudioCommandResult> {
        if !self.is_connected() {
            bail!(
                "Roldex Studio plugins are not connected. Try repair_studio_plugin, then re-check Studio connectivity. If Studio is already open, restart it so repaired plugin code reloads."
            );
        }

        let queue = if is_runtime_action(action) {
            &self.inner.runtime_queue
        } else {
            &self.inner.main_queue
        };
        if queue.lock().await.len() >= MAX_QUEUE {
            bail!("Studio command queue is full; wait for the plugin to catch up");
        }

        let id = format!(
            "studio-{}",
            self.inner.next_id.fetch_add(1, Ordering::Relaxed)
        );
        let (sender, receiver) = oneshot::channel();
        self.inner.pending.lock().await.insert(id.clone(), sender);
        queue.lock().await.push_back(StudioCommand {
            id: id.clone(),
            action: action.to_owned(),
            payload,
        });

        match timeout(STUDIO_COMMAND_TIMEOUT, receiver).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(_)) => {
                self.inner.pending.lock().await.remove(&id);
                bail!("Studio command result channel closed unexpectedly")
            }
            Err(_) => {
                self.inner.pending.lock().await.remove(&id);
                bail!(
                    "Studio command timed out after {} seconds. Studio may be busy, disconnected, playtesting, or waiting for a permission prompt.",
                    STUDIO_COMMAND_TIMEOUT.as_secs()
                )
            }
        }
    }
}

async fn drain_queue(
    queue: &Mutex<VecDeque<StudioCommand>>,
    max_commands: usize,
) -> Vec<StudioCommand> {
    let mut queue = queue.lock().await;
    let count = max_commands.clamp(1, 16);
    let mut commands = Vec::with_capacity(count.min(queue.len()));
    for _ in 0..count {
        let Some(command) = queue.pop_front() else {
            break;
        };
        commands.push(command);
    }
    commands
}

fn is_runtime_action(action: &str) -> bool {
    matches!(
        action,
        "capture" | "scenario_test" | "input" | "device" | "reflect"
    )
}

pub fn tool_definitions() -> Vec<Value> {
    vec![
        function_tool(
            "studio_health",
            "Check whether Roldex Studio is actively connected.",
            json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        ),
        function_tool(
            "studio_query",
            "Read live Roblox Studio state. Operations: selection, inspect, children, find. Use before editing and after important changes.",
            json!({
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
            }),
        ),
        function_tool(
            "studio_batch",
            "Apply an undoable batch of Studio changes one action at a time so the user can watch Roldex build. Prefer real Instances over one-off builder scripts. Supported ops include create, set_properties, set_attributes, move, clone, delete, update_script, select, tags, pivot, and terrain operations.",
            json!({
                "type": "object",
                "properties": {
                    "label": { "type": "string" },
                    "live_delay_ms": { "type": "integer", "minimum": 0, "maximum": 500 },
                    "highlight_created": { "type": "boolean" },
                    "actions": { "type": "array", "minItems": 1, "maxItems": 100, "items": { "type": "object", "additionalProperties": true } }
                },
                "required": ["actions"],
                "additionalProperties": false
            }),
        ),
        function_tool(
            "studio_capture_view",
            "Capture the current Studio viewport as PNG for visual AI QA. The capture is saved under .roldex/captures for vision review.",
            json!({
                "type": "object",
                "properties": {
                    "width": { "type": "integer", "minimum": 160, "maximum": 960 },
                    "height": { "type": "integer", "minimum": 90, "maximum": 540 },
                    "include_ui": { "type": "boolean" },
                    "review_goal": { "type": "string" }
                },
                "additionalProperties": false
            }),
        ),
        function_tool(
            "studio_device",
            "Inspect or control Studio device simulation for responsive UI testing.",
            json!({
                "type": "object",
                "properties": {
                    "operation": { "type": "string", "enum": ["status", "list", "set_device", "set_resolution", "set_orientation", "set_dpi", "set_scaling", "stop"] },
                    "device_id": { "type": "string" },
                    "width": { "type": "integer", "minimum": 1, "maximum": 7680 },
                    "height": { "type": "integer", "minimum": 1, "maximum": 4320 },
                    "orientation": { "type": "string", "enum": ["portrait", "landscape", "landscape_right"] },
                    "dpi": { "type": "number", "minimum": 72, "maximum": 10000 },
                    "scaling_mode": { "type": "string", "enum": ["fit", "actual", "physical"] }
                },
                "required": ["operation"],
                "additionalProperties": false
            }),
        ),
        function_tool(
            "studio_input",
            "Simulate bounded keyboard, mouse, pointer, and text input in an already-running Studio test for testing the experience's own controls.",
            json!({
                "type": "object",
                "properties": {
                    "steps": { "type": "array", "maxItems": 80, "items": { "type": "object", "additionalProperties": true } }
                },
                "required": ["steps"],
                "additionalProperties": false
            }),
        ),
        function_tool(
            "studio_test",
            "Run a bounded Studio run, play, or multiplayer smoke test and collect Output.",
            test_schema(false),
        ),
        function_tool(
            "studio_scenario_test",
            "Run a bounded end-to-end Studio playtest with simulated inputs and viewport capture checkpoints. Use for UI flows, interactions, movement, animations, Tools, spawns, and regression testing.",
            test_schema(true),
        ),
        function_tool(
            "studio_reflect",
            "Inspect a Roblox class through Studio ReflectionService when dynamic engine API inspection helps.",
            json!({
                "type": "object",
                "properties": {
                    "class_name": { "type": "string" },
                    "include_properties": { "type": "boolean" },
                    "include_methods": { "type": "boolean" },
                    "include_events": { "type": "boolean" },
                    "filter": { "type": "object", "additionalProperties": true }
                },
                "required": ["class_name"],
                "additionalProperties": false
            }),
        ),
        empty_tool("studio_undo", "Undo the most recent Studio change."),
        empty_tool("studio_redo", "Redo the next Studio history action."),
        empty_tool(
            "repair_studio_plugin",
            "Repair or reinstall both local Roldex Studio plugin files on Windows.",
        ),
    ]
}

fn function_tool(name: &str, description: &str, parameters: Value) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": name,
            "description": description,
            "parameters": parameters
        }
    })
}

fn empty_tool(name: &str, description: &str) -> Value {
    function_tool(
        name,
        description,
        json!({ "type": "object", "properties": {}, "additionalProperties": false }),
    )
}

fn test_schema(with_scenario: bool) -> Value {
    let mut properties = serde_json::Map::from_iter([
        (
            "mode".into(),
            json!({ "type": "string", "enum": ["run", "play", "multiplayer"] }),
        ),
        (
            "players".into(),
            json!({ "type": "integer", "minimum": 1, "maximum": 8 }),
        ),
        (
            "timeout_seconds".into(),
            json!({ "type": "integer", "minimum": 2, "maximum": 60 }),
        ),
        ("args".into(), json!({})),
    ]);
    if with_scenario {
        properties.extend([
            (
                "start_delay_seconds".into(),
                json!({ "type": "number", "minimum": 0, "maximum": 10 }),
            ),
            ("capture_at_end".into(), json!({ "type": "boolean" })),
            (
                "capture_width".into(),
                json!({ "type": "integer", "minimum": 160, "maximum": 960 }),
            ),
            (
                "capture_height".into(),
                json!({ "type": "integer", "minimum": 90, "maximum": 540 }),
            ),
            ("include_ui".into(), json!({ "type": "boolean" })),
            ("review_goal".into(), json!({ "type": "string" })),
            (
                "steps".into(),
                json!({ "type": "array", "maxItems": 80, "items": { "type": "object", "additionalProperties": true } }),
            ),
        ]);
    }
    json!({
        "type": "object",
        "properties": properties,
        "required": ["mode"],
        "additionalProperties": false
    })
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
        "studio_device" => {
            Some(execute_remote(call, broker, "device", "Studio device simulation").await)
        }
        "studio_input" => {
            Some(execute_remote(call, broker, "input", "Studio virtual input").await)
        }
        "studio_test" => Some(execute_remote(call, broker, "test", "Studio playtest").await),
        "studio_scenario_test" => Some(execute_scenario_test(call, broker, fs_scope).await),
        "studio_reflect" => {
            Some(execute_remote(call, broker, "reflect", "Studio API reflection").await)
        }
        "studio_undo" => Some(execute_remote(call, broker, "undo", "Studio undo").await),
        "studio_redo" => Some(execute_remote(call, broker, "redo", "Studio redo").await),
        "repair_studio_plugin" => Some(execute_repair_plugin()),
        _ => None,
    }
}

async fn execute_health(broker: Option<&StudioBroker>) -> ToolExecution {
    let (connected, pending_commands) = if let Some(broker) = broker {
        (broker.is_connected(), broker.pending_count().await)
    } else {
        (false, 0)
    };
    ToolExecution {
        event: AgentEvent::UsingTool("Studio health".into()),
        output: json!({
            "ok": true,
            "connected": connected,
            "pending_commands": pending_commands
        })
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
    let output = result_json(broker.submit_action(action, payload).await);
    ToolExecution { event, output }
}

async fn execute_capture(
    call: &ToolCall,
    broker: Option<&StudioBroker>,
    fs_scope: &WorkspaceFs,
) -> ToolExecution {
    execute_capture_action(
        call,
        broker,
        fs_scope,
        "capture",
        "Capturing Studio viewport",
    )
    .await
}

async fn execute_scenario_test(
    call: &ToolCall,
    broker: Option<&StudioBroker>,
    fs_scope: &WorkspaceFs,
) -> ToolExecution {
    execute_capture_action(
        call,
        broker,
        fs_scope,
        "scenario_test",
        "Studio visual scenario test",
    )
    .await
}

async fn execute_capture_action(
    call: &ToolCall,
    broker: Option<&StudioBroker>,
    fs_scope: &WorkspaceFs,
    action: &str,
    event_name: &str,
) -> ToolExecution {
    let event = AgentEvent::UsingTool(event_name.into());
    let Some(broker) = broker else {
        return ToolExecution {
            event,
            output: error_output("Studio broker is not available"),
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
    let output = match broker.submit_action(action, payload).await {
        Ok(mut result) => match persist_captures_in_result(&mut result, fs_scope) {
            Ok(_) => serde_json::to_string(&result)
                .unwrap_or_else(|error| error_output(error.to_string())),
            Err(error) => error_output(error.to_string()),
        },
        Err(error) => error_output(error.to_string()),
    };
    ToolExecution { event, output }
}

fn result_json(result: Result<StudioCommandResult>) -> String {
    match result {
        Ok(result) => serde_json::to_string(&result)
            .unwrap_or_else(|error| error_output(format!("failed to serialize Studio result: {error}"))),
        Err(error) => error_output(error.to_string()),
    }
}

fn parse_payload(call: &ToolCall) -> std::result::Result<Value, String> {
    if call.function.arguments.trim().is_empty() {
        Ok(json!({}))
    } else {
        serde_json::from_str::<Value>(&call.function.arguments)
            .map_err(|error| format!("invalid {} arguments: {error}", call.function.name))
    }
}

fn persist_captures_in_result(
    result: &mut StudioCommandResult,
    fs_scope: &WorkspaceFs,
) -> Result<Vec<String>> {
    let Some(Value::Object(output)) = result.output.as_mut() else {
        return Ok(Vec::new());
    };
    let mut paths = Vec::new();
    if let Some(path) = persist_capture_object(output, fs_scope)? {
        paths.push(path);
    }
    if let Some(Value::Object(capture)) = output.get_mut("capture") {
        if let Some(path) = persist_capture_object(capture, fs_scope)? {
            paths.push(path);
        }
    }
    if let Some(Value::Array(captures)) = output.get_mut("captures") {
        for capture in captures.iter_mut().take(2) {
            if let Value::Object(capture) = capture {
                if let Some(path) = persist_capture_object(capture, fs_scope)? {
                    paths.push(path);
                }
            }
        }
    }
    Ok(paths)
}

fn persist_capture_object(
    object: &mut Map<String, Value>,
    fs_scope: &WorkspaceFs,
) -> Result<Option<String>> {
    let encoded = object
        .remove("data_base64")
        .or_else(|| object.remove("png_base64"));
    let Some(Value::String(encoded)) = encoded else {
        return Ok(None);
    };
    let bytes = STANDARD
        .decode(encoded.as_bytes())
        .map_err(|error| anyhow::anyhow!("Studio capture base64 was invalid: {error}"))?;
    if bytes.is_empty() {
        bail!("Studio capture returned an empty image");
    }
    if bytes.len() > MAX_CAPTURE_BYTES {
        bail!("Studio capture exceeded the {MAX_CAPTURE_BYTES} byte local limit");
    }

    let relative_path = format!(".roldex/captures/studio-{}.png", now_millis());
    fs_scope.write_bytes(&relative_path, &bytes)?;
    object.insert("path".into(), Value::String(relative_path.clone()));
    object.insert("bytes".into(), json!(bytes.len()));
    Ok(Some(relative_path))
}

fn execute_repair_plugin() -> ToolExecution {
    let event = AgentEvent::UsingTool("Studio plugin repair".into());
    let output = match install_embedded_plugins() {
        Ok(paths) => json!({
            "ok": true,
            "paths": paths,
            "message": "Roldex Studio plugins were repaired. Restart Studio if it is already open."
        })
        .to_string(),
        Err(error) => error_output(error.to_string()),
    };
    ToolExecution { event, output }
}

fn install_embedded_plugins() -> Result<Vec<String>> {
    #[cfg(windows)]
    {
        let local_app_data = env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .context("LOCALAPPDATA is not available")?;
        let plugin_dir = local_app_data.join("Roblox").join("Plugins");
        fs::create_dir_all(&plugin_dir)
            .with_context(|| format!("failed to create {}", plugin_dir.display()))?;
        let main_path = plugin_dir.join("RoldexStudio.plugin.lua");
        let runtime_path = plugin_dir.join("RoldexStudioRuntime.plugin.lua");
        fs::write(&main_path, EMBEDDED_PLUGIN)
            .with_context(|| format!("failed to write {}", main_path.display()))?;
        fs::write(&runtime_path, EMBEDDED_RUNTIME_PLUGIN)
            .with_context(|| format!("failed to write {}", runtime_path.display()))?;
        return Ok(vec![
            main_path.display().to_string(),
            runtime_path.display().to_string(),
        ]);
    }

    #[cfg(not(windows))]
    {
        bail!(
            "automatic Studio plugin repair is currently implemented for Windows; reinstall the plugins from the Roldex repository on this operating system"
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

    async fn wait_for_main_command(broker: &StudioBroker) -> StudioCommand {
        for _ in 0..20 {
            if let Some(command) = broker.poll(1).await.into_iter().next() {
                return command;
            }
            tokio::task::yield_now().await;
        }
        panic!("main Studio command was not enqueued")
    }

    async fn wait_for_runtime_command(broker: &StudioBroker) -> StudioCommand {
        for _ in 0..20 {
            if let Some(command) = broker.poll_runtime(1).await.into_iter().next() {
                return command;
            }
            tokio::task::yield_now().await;
        }
        panic!("runtime Studio command was not enqueued")
    }

    #[tokio::test]
    async fn broker_routes_main_and_runtime_commands() {
        let broker = StudioBroker::new();
        broker.mark_seen();

        let main_broker = broker.clone();
        let main_task = tokio::spawn(async move {
            main_broker
                .submit_action("query", json!({ "operation": "selection" }))
                .await
        });
        let main_command = wait_for_main_command(&broker).await;
        assert_eq!(main_command.action, "query");
        assert!(
            broker
                .complete(StudioCommandResult {
                    id: main_command.id,
                    ok: true,
                    output: Some(json!({ "items": [] })),
                    error: None,
                })
                .await
        );
        assert!(main_task.await.expect("join").expect("result").ok);

        let runtime_broker = broker.clone();
        let runtime_task = tokio::spawn(async move {
            runtime_broker.submit_action("capture", json!({})).await
        });
        let runtime_command = wait_for_runtime_command(&broker).await;
        assert_eq!(runtime_command.action, "capture");
        assert!(
            broker
                .complete(StudioCommandResult {
                    id: runtime_command.id,
                    ok: true,
                    output: Some(json!({ "data_base64": "cG5n" })),
                    error: None,
                })
                .await
        );
        assert!(runtime_task.await.expect("join").expect("result").ok);
    }

    #[test]
    fn runtime_actions_route_to_runtime_plugin_only() {
        assert!(is_runtime_action("capture"));
        assert!(is_runtime_action("scenario_test"));
        assert!(is_runtime_action("device"));
        assert!(!is_runtime_action("batch"));
        assert!(!is_runtime_action("query"));
    }
}
