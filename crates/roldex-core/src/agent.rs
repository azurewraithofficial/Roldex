use std::path::Path;

use anyhow::{Context, Result, bail};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use crate::AiConfig;
use crate::StudioBroker;
use crate::WorkspaceFs;
use crate::project::ProjectSummary;
use crate::prompt::ROBLOX_SYSTEM_PROMPT;
use crate::provider::{ChatMessage, OpenAiCompatibleProvider};
use crate::tools::{AgentEvent, execute_tool, tool_definitions};

const MAX_IMAGE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_IMAGES_PER_TURN: usize = 4;
const MAX_PROJECT_INSTRUCTIONS_CHARS: usize = 24_000;

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
            let mime = image_mime(image_path)?;
            let bytes = fs.read_bytes(image_path, MAX_IMAGE_BYTES)?;
            if bytes.is_empty() {
                bail!("image file is empty: {image_path}");
            }
            data_urls.push(format!("data:{mime};base64,{}", STANDARD.encode(bytes)));
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

        let mut messages = Vec::with_capacity(self.history.len() + 20);
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
        messages.extend(self.history.iter().cloned());
        messages.push(user_message);

        const MAX_TOOL_STEPS: usize = 48;
        for _ in 0..MAX_TOOL_STEPS {
            let turn = self.provider.chat(&messages, &tools).await?;

            if turn.tool_calls.is_empty() {
                let answer = turn
                    .content
                    .filter(|content| !content.trim().is_empty())
                    .context("AI provider returned an empty final response")?;
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
                let execution = if let Some(execution) =
                    crate::studio::execute_tool(&call, self.studio.as_ref()).await
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
                had_tool_error |= execution.output.contains("\"ok\":false");
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
}
