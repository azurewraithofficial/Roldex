use anyhow::Result;

use crate::project::ProjectSummary;
use crate::prompt::ROBLOX_SYSTEM_PROMPT;
use crate::provider::{ChatMessage, OpenAiCompatibleProvider};
use crate::AiConfig;

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

    pub async fn chat(&mut self, input: &str) -> Result<String> {
        let mut messages = Vec::with_capacity(self.history.len() + 3);
        messages.push(ChatMessage::system(ROBLOX_SYSTEM_PROMPT));
        messages.push(ChatMessage::system(format!(
            "Current project context:\n{}",
            self.project.describe()
        )));
        messages.extend(self.history.iter().cloned());
        messages.push(ChatMessage::user(input));

        let answer = self.provider.chat(&messages).await?;
        self.history.push(ChatMessage::user(input));
        self.history.push(ChatMessage::assistant(answer.clone()));

        // Keep v0.1 sessions bounded so long chats do not grow RAM forever.
        const MAX_HISTORY_MESSAGES: usize = 24;
        if self.history.len() > MAX_HISTORY_MESSAGES {
            let remove = self.history.len() - MAX_HISTORY_MESSAGES;
            self.history.drain(0..remove);
        }

        Ok(answer)
    }
}
