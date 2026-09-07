use std::env;

use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::AiConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    temperature: f32,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleProvider {
    client: Client,
    config: AiConfig,
}

impl OpenAiCompatibleProvider {
    pub fn new(config: AiConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    pub async fn chat(&self, messages: &[ChatMessage]) -> Result<String> {
        if self.config.provider != "openrouter" && self.config.provider != "openai-compatible" {
            bail!(
                "unsupported provider '{}'; v0.1 supports openrouter/openai-compatible endpoints",
                self.config.provider
            );
        }

        let api_key = env::var(&self.config.api_key_env).with_context(|| {
            format!(
                "missing API key environment variable {}",
                self.config.api_key_env
            )
        })?;

        let response = self
            .client
            .post(&self.config.endpoint)
            .bearer_auth(api_key)
            .header("X-OpenRouter-Title", "Roldex")
            .json(&ChatRequest {
                model: &self.config.model,
                messages,
                temperature: self.config.temperature,
            })
            .send()
            .await
            .context("failed to reach AI provider")?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("failed to read AI provider response")?;

        if !status.is_success() {
            bail!("AI provider returned HTTP {status}: {body}");
        }

        let parsed: ChatResponse = serde_json::from_str(&body)
            .with_context(|| format!("could not parse AI provider response: {body}"))?;

        parsed
            .choices
            .into_iter()
            .find_map(|choice| choice.message.content)
            .filter(|content| !content.trim().is_empty())
            .context("AI provider returned no text response")
    }
}
