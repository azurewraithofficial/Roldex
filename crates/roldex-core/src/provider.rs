use std::env;

use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::AiConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self::text("system", content)
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::text("user", content)
    }

    pub fn user_with_image(text: impl Into<String>, data_url: impl Into<String>) -> Self {
        let text = text.into();
        let data_url = data_url.into();
        Self {
            role: "user".into(),
            content: Some(json!([
                {
                    "type": "text",
                    "text": text
                },
                {
                    "type": "image_url",
                    "image_url": {
                        "url": data_url
                    }
                }
            ])),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::text("assistant", content)
    }

    pub fn assistant_turn(content: Option<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.map(Value::String),
            tool_calls: Some(tool_calls),
            tool_call_id: None,
        }
    }

    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: "tool".into(),
            content: Some(Value::String(content.into())),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }

    fn text(role: &str, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: Some(Value::String(content.into())),
            tool_calls: None,
            tool_call_id: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AssistantTurn {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<&'a [Value]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parallel_tool_calls: Option<bool>,
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
    #[serde(default)]
    tool_calls: Vec<ToolCall>,
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

    pub async fn chat(&self, messages: &[ChatMessage], tools: &[Value]) -> Result<AssistantTurn> {
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

        let has_tools = !tools.is_empty();
        let request = ChatRequest {
            model: &self.config.model,
            messages,
            temperature: self.config.temperature,
            tools: has_tools.then_some(tools),
            tool_choice: has_tools.then_some("auto"),
            parallel_tool_calls: has_tools.then_some(false),
        };

        let mut builder = self
            .client
            .post(&self.config.endpoint)
            .bearer_auth(api_key)
            .json(&request);

        if self.config.provider == "openrouter" {
            builder = builder.header("X-OpenRouter-Title", "Roldex");
        }

        let response = builder
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

        let message = parsed
            .choices
            .into_iter()
            .next()
            .context("AI provider returned no choices")?
            .message;

        if message.tool_calls.is_empty()
            && message
                .content
                .as_deref()
                .is_none_or(|content| content.trim().is_empty())
        {
            bail!("AI provider returned neither text nor tool calls");
        }

        Ok(AssistantTurn {
            content: message.content,
            tool_calls: message.tool_calls,
        })
    }
}
