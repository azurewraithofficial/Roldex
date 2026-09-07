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

    pub fn user_with_images(text: impl Into<String>, data_urls: Vec<String>) -> Self {
        let mut parts = Vec::with_capacity(data_urls.len() + 1);
        parts.push(json!({
            "type": "text",
            "text": text.into()
        }));
        for data_url in data_urls {
            parts.push(json!({
                "type": "image_url",
                "image_url": {
                    "url": data_url
                }
            }));
        }

        Self {
            role: "user".into(),
            content: Some(Value::Array(parts)),
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

    pub fn is_openrouter(&self) -> bool {
        self.config.provider == "openrouter"
    }

    pub fn openrouter_web_tools(&self) -> Vec<Value> {
        if !self.is_openrouter() {
            return Vec::new();
        }
        vec![
            json!({
                "type": "openrouter:web_search",
                "parameters": {
                    "max_results": 8
                }
            }),
            json!({
                "type": "openrouter:web_fetch",
                "parameters": {
                    "max_content_tokens": 12000
                }
            }),
        ]
    }

    pub async fn roblox_originality_research(&self, request: &str) -> Result<String> {
        if !self.is_openrouter() {
            bail!("automatic live originality research currently requires the OpenRouter provider");
        }

        let messages = vec![
            ChatMessage::system(
                "You are the market/originality research stage for a Roblox development agent. Use web search before answering. Research current Roblox experiences that have the same or a strongly similar name, mechanic, progression loop, visual premise, or core concept. Search exact candidate names when present and also semantic concept variants. Prioritize Roblox experience pages and trustworthy current sources. Do not say a name or concept is unique merely because the first search has no result. Return a compact report with: name-collision risk, closest existing experiences, concept-overlap risk, differentiators to preserve/add, and a final recommendation: keep / rename / redesign / safe-enough-to-proceed. Include source links in the report. This report is advisory; the build agent still makes the final implementation decisions."
            ),
            ChatMessage::user(request),
        ];
        let tools = self.openrouter_web_tools();
        let turn = self.chat(&messages, &tools).await?;
        if !turn.tool_calls.is_empty() {
            bail!("web research unexpectedly returned unresolved client tool calls");
        }
        turn.content
            .filter(|content| !content.trim().is_empty())
            .context("web research returned an empty report")
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
