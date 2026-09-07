use anyhow::{Context, Result, bail};

use crate::AiConfig;
use crate::WorkspaceFs;
use crate::project::ProjectSummary;
use crate::prompt::ROBLOX_SYSTEM_PROMPT;
use crate::provider::{ChatMessage, OpenAiCompatibleProvider};
use crate::tools::{AgentEvent, execute_tool, tool_definitions};

pub struct Agent {
    provider: OpenAiCompatibleProvider,
    project: ProjectSummary,
    history: Vec<ChatMessage>,
}

impl Agent {
    pub fn new(ai: AiConfig, project: ProjectSummary) -> Self {
        Self {
            provider: OpenAiCompatibleProvider::new(ai),
            project,
            history: Vec::new(),
        }
    }

    pub async fn chat_with_tools<F>(
        &mut self,
        input: &str,
        fs: &WorkspaceFs,
        mut on_event: F,
    ) -> Result<String>
    where
        F: FnMut(AgentEvent),
    {
        let tools = tool_definitions();
        let mut messages = Vec::with_capacity(self.history.len() + 10);
        messages.push(ChatMessage::system(ROBLOX_SYSTEM_PROMPT));
        messages.push(ChatMessage::system(format!(
            "Current project context:\n{}",
            self.project.describe()
        )));
        messages.extend(self.history.iter().cloned());
        messages.push(ChatMessage::user(input));

        const MAX_TOOL_STEPS: usize = 8;
        for _ in 0..MAX_TOOL_STEPS {
            let turn = self.provider.chat(&messages, &tools).await?;

            if turn.tool_calls.is_empty() {
                let answer = turn
                    .content
                    .filter(|content| !content.trim().is_empty())
                    .context("AI provider returned an empty final response")?;
                self.remember(input, &answer);
                return Ok(answer);
            }

            let tool_calls = turn.tool_calls;
            messages.push(ChatMessage::assistant_turn(
                turn.content,
                tool_calls.clone(),
            ));

            for call in tool_calls {
                let execution = execute_tool(&call, fs);
                on_event(execution.event);
                messages.push(ChatMessage::tool(call.id, execution.output));
            }
        }

        bail!("agent stopped after {MAX_TOOL_STEPS} tool steps to prevent an infinite loop")
    }

    fn remember(&mut self, input: &str, answer: &str) {
        self.history.push(ChatMessage::user(input));
        self.history.push(ChatMessage::assistant(answer));

        const MAX_HISTORY_MESSAGES: usize = 24;
        if self.history.len() > MAX_HISTORY_MESSAGES {
            let remove = self.history.len() - MAX_HISTORY_MESSAGES;
            self.history.drain(0..remove);
        }
    }
}
