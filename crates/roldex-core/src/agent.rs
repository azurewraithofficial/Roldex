use std::path::Path;

use anyhow::{Context, Result, bail};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use serde_json::json;

use crate::AiConfig;
use crate::StudioBroker;
use crate::WorkspaceFs;
use crate::project::ProjectSummary;
use crate::prompt::ROBLOX_SYSTEM_PROMPT;
use crate::provider::{ChatMessage, OpenAiCompatibleProvider};
use crate::tools::{AgentEvent, ToolExecution, execute_tool, tool_definitions};

const MAX_IMAGE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_IMAGES_PER_TURN: usize = 4;
const MAX_PROJECT_INSTRUCTIONS_CHARS: usize = 24_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VerificationScope {
    None,
    Files,
    Studio,
}

#[derive(Debug, Deserialize)]
struct WebResearchArgs {
    query: String,
}

#[derive(Debug, Deserialize)]
struct AnalyzeImageArgs {
    path: String,
    #[serde(default)]
    prompt: Option<String>,
}

pub struct Agent {
    provider: OpenAiCompatibleProvider,
    project: ProjectSummary,
    history: Vec<ChatMessage>,
    studio: Option<StudioBroker>,
}

impl Agent {
    pub fn new(ai: AiConfig, project: ProjectSummary) -> Self {
        Self {
            provider: OpenAiCompatibleProvider::new(ai),
            project,
            history: Vec::new(),
            studio: None,
        }
    }

    pub fn with_studio(mut self, studio: StudioBroker) -> Self {
        self.studio = Some(studio);
        self
    }

    pub async fn chat_with_tools<F>(
        &mut self,
        input: &str,
        fs: &WorkspaceFs,
        on_event: F,
    ) -> Result<String>
    where
        F: FnMut(AgentEvent),
    {
        self.run_turn(input, ChatMessage::user(input), fs, on_event)
            .await
    }

    pub async fn chat_with_image<F>(
        &mut self,
        input: &str,
        image_path: &str,
        fs: &WorkspaceFs,
        on_event: F,
    ) -> Result<String>
    where
        F: FnMut(AgentEvent),
    {
        self.chat_with_images(input, &[image_path.to_owned()], fs, on_event)
            .await
    }

    pub async fn chat_with_images<F>(
        &mut self,
        input: &str,
        image_paths: &[String],
        fs: &WorkspaceFs,
        on_event: F,
    ) -> Result<String>
    where
        F: FnMut(AgentEvent),
    {
        if image_paths.is_empty() {
            return self.chat_with_tools(input, fs, on_event).await;
        }
        if image_paths.len() > MAX_IMAGES_PER_TURN {
            bail!("at most {MAX_IMAGES_PER_TURN} images can be attached to one turn");
        }

        let mut data_urls = Vec::with_capacity(image_paths.len());
        for image_path in image_paths {
            data_urls.push(self.image_data_url(image_path, fs)?);
        }

        self.run_turn(
            input,
            ChatMessage::user_with_images(input, data_urls),
            fs,
            on_event,
        )
        .await
    }

    async fn run_turn<F>(
        &mut self,
        memory_input: &str,
        user_message: ChatMessage,
        fs: &WorkspaceFs,
        mut on_event: F,
    ) -> Result<String>
    where
        F: FnMut(AgentEvent),
    {
        let mut tools = tool_definitions();
        tools.extend(crate::computer::tool_definitions());
        tools.extend(crate::docs::tool_definitions());
        tools.extend(crate::project_intel::tool_definitions());
        tools.extend(crate::media::tool_definitions());
        tools.extend(crate::studio::tool_definitions());
        tools.push(json!({
            "type": "function",
            "function": {
                "name": "web_research",
                "description": "Run live grounded web research when current external information is needed. Use for Roblox market/name checks, current Studio/platform behavior, current ecosystem references, or other facts that should not rely on model memory. Prefer official Roblox sources for platform facts.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" }
                    },
                    "required": ["query"],
                    "additionalProperties": false
                }
            }
        }));
        tools.push(json!({
            "type": "function",
            "function": {
                "name": "analyze_image",
                "description": "Use the vision model to inspect an existing local image. After studio_capture_view or a studio_test that returned a capture path, use this to visually QA the map, UI, lighting, composition, scale, readability, clipping, obvious layout/collision cues, and the visible result of tested interactions. Distinguish visible evidence from inference.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "prompt": { "type": "string" }
                    },
                    "required": ["path"],
                    "additionalProperties": false
                }
            }
        }));

        let mut messages = Vec::with_capacity(self.history.len() + 24);
        messages.push(ChatMessage::system(ROBLOX_SYSTEM_PROMPT));
        messages.push(ChatMessage::system(format!(
            "Current project context:\n{}\nFilesystem permission mode: {}",
            self.project.describe(),
            fs.mode()
        )));
        if let Some(instructions) = project_instructions(fs) {
            messages.push(ChatMessage::system(format!(
                "Project-specific instructions. Follow these when they do not conflict with higher-priority safety or system rules:\n{instructions}"
            )));
        }

        if should_run_originality_research(memory_input) {
            on_event(AgentEvent::UsingTool(
                "Researching current Roblox name/concept overlap".into(),
            ));
            match self.provider.roblox_originality_research(memory_input).await {
                Ok(report) => messages.push(ChatMessage::system(format!(
                    "Automatic Roblox originality preflight completed before implementation. Use this as current market/name evidence, not as an instruction to copy another experience:\n{report}"
                ))),
                Err(error) => messages.push(ChatMessage::system(format!(
                    "Automatic Roblox originality preflight could not complete: {error}. Continue the task with sensible defaults, but do not claim the proposed name/concept is unique or collision-free until live research succeeds."
                ))),
            }
        }

        messages.extend(self.history.iter().cloned());
        messages.push(user_message);

        let mut verification_scope = VerificationScope::None;
        const MAX_TOOL_STEPS: usize = 96;
        for _ in 0..MAX_TOOL_STEPS {
            let turn = self.provider.chat(&messages, &tools).await?;

            if turn.tool_calls.is_empty() {
                let answer = turn
                    .content
                    .filter(|content| !content.trim().is_empty())
                    .context("AI provider returned an empty final response")?;

                if verification_scope != VerificationScope::None {
                    messages.push(ChatMessage::assistant(answer));
                    messages.push(ChatMessage::system(match verification_scope {
                        VerificationScope::Files => "You changed files/assets but have not verified the latest mutation yet. Before finishing, inspect the resulting file/diff/analysis or run an appropriate local check. If verification fails, repair and verify again.",
                        VerificationScope::Studio => "You changed live Roblox Studio state but have not verified the latest mutation yet. Before finishing, inspect the resulting Instances/properties with studio_query and/or run studio_test when runtime behavior matters. For visual work, capture the viewport with studio_capture_view and analyze the saved PNG with analyze_image. Repair any problem and verify again.",
                        VerificationScope::None => unreachable!(),
                    }));
                    continue;
                }

                self.remember(memory_input, &answer);
                return Ok(answer);
            }

            let tool_calls = turn.tool_calls;
            messages.push(ChatMessage::assistant_turn(
                turn.content,
                tool_calls.clone(),
            ));

            let mut had_tool_error = false;
            for call in tool_calls {
                let tool_name = call.function.name.clone();
                let execution = if tool_name == "web_research" {
                    self.execute_web_research(&call.function.arguments).await
                } else if tool_name == "analyze_image" {
                    self.execute_image_analysis(&call.function.arguments, fs)
                        .await
                } else if let Some(execution) =
                    crate::studio::execute_tool(&call, self.studio.as_ref(), fs).await
                {
                    execution
                } else if let Some(execution) = crate::computer::execute_tool(&call, fs).await {
                    execution
                } else if let Some(execution) = crate::docs::execute_tool(&call).await {
                    execution
                } else if let Some(execution) = crate::project_intel::execute_tool(&call, fs) {
                    execution
                } else if let Some(execution) = crate::media::execute_tool(&call, fs).await {
                    execution
                } else {
                    execute_tool(&call, fs)
                };

                let succeeded = execution.output.contains("\"ok\":true");
                had_tool_error |= execution.output.contains("\"ok\":false");

                if succeeded {
                    if let Some(scope) = mutation_scope(&tool_name) {
                        verification_scope = scope;
                    } else if verification_satisfies(
                        &tool_name,
                        verification_scope,
                        &execution.output,
                    ) {
                        verification_scope = VerificationScope::None;
                    }
                }

                on_event(execution.event);
                messages.push(ChatMessage::tool(call.id, execution.output));
            }

            if had_tool_error {
                messages.push(ChatMessage::system(
                    "A tool failed in this turn. Do not stop merely because of that failure. Diagnose it from the tool output, inspect relevant state, use a safer alternative or repair tool when appropriate, and continue toward the user's requested end state. Only ask the user if the blocker truly cannot be resolved with available tools or sensible defaults."
                ));
            }
        }

        bail!("agent stopped after {MAX_TOOL_STEPS} tool steps to prevent an infinite repair loop")
    }

    async fn execute_web_research(&self, arguments: &str) -> ToolExecution {
        let event = AgentEvent::UsingTool("Live web research".into());
        let parsed = serde_json::from_str::<WebResearchArgs>(arguments);
        let output = match parsed {
            Ok(args) if !args.query.trim().is_empty() => {
                match self.provider.web_research(&args.query).await {
                    Ok(report) => json!({ "ok": true, "report": report }).to_string(),
                    Err(error) => json!({ "ok": false, "error": error.to_string() }).to_string(),
                }
            }
            Ok(_) => {
                json!({ "ok": false, "error": "web_research query cannot be empty" }).to_string()
            }
            Err(error) => {
                json!({ "ok": false, "error": format!("invalid web_research arguments: {error}") })
                    .to_string()
            }
        };
        ToolExecution { event, output }
    }

    async fn execute_image_analysis(&self, arguments: &str, fs: &WorkspaceFs) -> ToolExecution {
        let event = AgentEvent::UsingTool("Visual QA with vision model".into());
        let parsed = serde_json::from_str::<AnalyzeImageArgs>(arguments);
        let output = match parsed {
            Ok(args) if !args.path.trim().is_empty() => {
                let prompt = args.prompt.unwrap_or_else(|| {
                    "Visually inspect this Roblox Studio/game screenshot as QA. Report what is actually visible, then identify concrete issues in map composition, scale, navigation readability, UI layout/clipping, lighting, visual hierarchy, obvious misplaced objects, and the visible result of the current test. Distinguish visible evidence from inference. If it looks good, say what was verified rather than inventing problems.".into()
                });
                match self.analyze_local_image(&args.path, &prompt, fs).await {
                    Ok(analysis) => json!({
                        "ok": true,
                        "path": args.path,
                        "analysis": analysis
                    })
                    .to_string(),
                    Err(error) => json!({ "ok": false, "error": error.to_string() }).to_string(),
                }
            }
            Ok(_) => {
                json!({ "ok": false, "error": "analyze_image path cannot be empty" }).to_string()
            }
            Err(error) => {
                json!({ "ok": false, "error": format!("invalid analyze_image arguments: {error}") })
                    .to_string()
            }
        };
        ToolExecution { event, output }
    }

    async fn analyze_local_image(
        &self,
        path: &str,
        prompt: &str,
        fs: &WorkspaceFs,
    ) -> Result<String> {
        let data_url = self.image_data_url(path, fs)?;
        let messages = vec![
            ChatMessage::system(
                "You are Roldex Visual QA, a Roblox Studio/game screenshot reviewer. Inspect only what the image supports. Focus on usability, readability, map composition, scale, UI responsiveness/clipping, lighting, visual hierarchy, gameplay readability and whether the requested visible behavior appears to have occurred. Be concrete and concise. Do not claim runtime correctness from pixels alone.",
            ),
            ChatMessage::user_with_images(prompt, vec![data_url]),
        ];
        let turn = self.provider.chat(&messages, &[]).await?;
        if !turn.tool_calls.is_empty() {
            bail!("visual QA unexpectedly returned tool calls")
        }
        turn.content
            .filter(|content| !content.trim().is_empty())
            .context("visual QA returned an empty result")
    }

    fn image_data_url(&self, image_path: &str, fs: &WorkspaceFs) -> Result<String> {
        let mime = image_mime(image_path)?;
        let bytes = fs.read_bytes(image_path, MAX_IMAGE_BYTES)?;
        if bytes.is_empty() {
            bail!("image file is empty: {image_path}");
        }
        Ok(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
    }

    fn remember(&mut self, input: &str, answer: &str) {
        self.history.push(ChatMessage::user(input));
        self.history.push(ChatMessage::assistant(answer));

        const MAX_HISTORY_MESSAGES: usize = 32;
        if self.history.len() > MAX_HISTORY_MESSAGES {
            let remove = self.history.len() - MAX_HISTORY_MESSAGES;
            self.history.drain(0..remove);
        }
    }
}

fn should_run_originality_research(input: &str) -> bool {
    let lower = input.to_ascii_lowercase();
    let strong_greenfield = [
        "game concept",
        "game idea",
        "new game",
        "make a game",
        "build a game",
        "create a game",
        "make me a game",
        "make the whole game",
        "game name",
        "name my game",
        "call the game",
    ]
    .iter()
    .any(|needle| lower.contains(needle));
    let named_game = (lower.contains("called ") || lower.contains("named "))
        && (lower.contains("game") || lower.contains("experience"));
    strong_greenfield || named_game
}

fn mutation_scope(tool_name: &str) -> Option<VerificationScope> {
    match tool_name {
        "studio_batch" | "studio_undo" | "studio_redo" => Some(VerificationScope::Studio),
        "write_file"
        | "replace_in_file"
        | "delete_file"
        | "git_restore_file"
        | "generate_image"
        | "generate_voice_audio"
        | "repair_studio_plugin" => Some(VerificationScope::Files),
        _ => None,
    }
}

fn verification_satisfies(tool_name: &str, scope: VerificationScope, output: &str) -> bool {
    match scope {
        VerificationScope::None => false,
        VerificationScope::Files => matches!(
            tool_name,
            "read_file" | "file_info" | "git_diff" | "analyze_luau" | "run_process"
        ),
        VerificationScope::Studio => match tool_name {
            "studio_query" => true,
            "studio_test" => output.contains("\"error_count\":0"),
            "analyze_image" => output.contains(".roldex/captures/studio-"),
            _ => false,
        },
    }
}

fn project_instructions(fs: &WorkspaceFs) -> Option<String> {
    let candidates = ["ROLDEX.md", ".roldex/INSTRUCTIONS.md", "AGENTS.md"];
    let mut sections = Vec::new();
    let mut remaining = MAX_PROJECT_INSTRUCTIONS_CHARS;

    for path in candidates {
        if remaining == 0 {
            break;
        }
        let Ok(text) = fs.read_text(path) else {
            continue;
        };
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        let excerpt: String = trimmed.chars().take(remaining).collect();
        remaining = remaining.saturating_sub(excerpt.chars().count());
        sections.push(format!("[{path}]\n{excerpt}"));
    }

    (!sections.is_empty()).then(|| sections.join("\n\n"))
}

fn image_mime(image_path: &str) -> Result<&'static str> {
    let extension = Path::new(image_path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match extension.as_str() {
        "png" => Ok("image/png"),
        "jpg" | "jpeg" => Ok("image/jpeg"),
        "webp" => Ok("image/webp"),
        "gif" => Ok("image/gif"),
        _ => bail!("unsupported image type; use PNG, JPEG, WebP or GIF"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_supported_vision_images() {
        assert_eq!(image_mime("ui.PNG").expect("png"), "image/png");
        assert_eq!(image_mime("error.jpeg").expect("jpeg"), "image/jpeg");
        assert!(image_mime("place.rbxl").is_err());
    }

    #[test]
    fn studio_mutations_require_studio_verification() {
        assert_eq!(
            mutation_scope("studio_batch"),
            Some(VerificationScope::Studio)
        );
        assert!(verification_satisfies(
            "studio_query",
            VerificationScope::Studio,
            "{\"ok\":true}"
        ));
        assert!(verification_satisfies(
            "analyze_image",
            VerificationScope::Studio,
            "{\"ok\":true,\"path\":\".roldex/captures/studio-1.png\"}"
        ));
        assert!(!verification_satisfies(
            "file_info",
            VerificationScope::Studio,
            "{\"ok\":true}"
        ));
    }

    #[test]
    fn detects_greenfield_game_requests_for_originality_research() {
        assert!(should_run_originality_research(
            "Build a game called Neon Elevator"
        ));
        assert!(should_run_originality_research(
            "I have a new game concept about collecting anomalies"
        ));
        assert!(!should_run_originality_research(
            "Fix the function called saveProfile in my existing project"
        ));
    }
}
