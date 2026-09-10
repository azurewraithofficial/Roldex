use std::env;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::Client;
use reqwest::header::RETRY_AFTER;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::time::sleep;

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
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<&'a [Value]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parallel_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<&'a Value>,
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

#[derive(Debug, Deserialize)]
struct StreamResponse {
    #[serde(default)]
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
}

#[derive(Debug, Deserialize, Default)]
struct StreamDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<StreamToolCallDelta>,
}

#[derive(Debug, Deserialize)]
struct StreamToolCallDelta {
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(default)]
    function: Option<StreamToolFunctionDelta>,
}

#[derive(Debug, Deserialize, Default)]
struct StreamToolFunctionDelta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Default)]
struct ToolCallAccumulator {
    id: Option<String>,
    kind: Option<String>,
    name: String,
    arguments: String,
}

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleProvider {
    client: Client,
    config: AiConfig,
}

impl OpenAiCompatibleProvider {
    pub fn new(config: AiConfig) -> Self {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .pool_idle_timeout(Duration::from_secs(30))
            .build()
            .expect("failed to initialize HTTP client");
        Self { client, config }
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
        self.web_research_with_system(
            "You are the market/originality research stage for a Roblox development agent. Use web search before answering. Research current Roblox experiences that have the same or a strongly similar name, mechanic, progression loop, visual premise, or core concept. Search exact candidate names when present and also semantic concept variants. Prioritize Roblox experience pages and trustworthy current sources. Do not say a name or concept is unique merely because the first search has no result. Return a compact report with: name-collision risk, closest existing experiences, concept-overlap risk, differentiators to preserve/add, and a final recommendation: keep / rename / redesign / safe-enough-to-proceed. Include source links in the report. This report is advisory; the build agent still makes the final implementation decisions.",
            request,
        )
        .await
    }

    pub async fn web_research(&self, request: &str) -> Result<String> {
        self.web_research_with_system(
            "You are the grounded web-research stage for a Roblox development agent. Use live web search before answering. Prefer official Roblox Creator Hub/Developer Forum announcements for platform facts and current Roblox experience pages or other direct sources for market/name research. Distinguish confirmed facts from inference, include useful source links, and keep the report compact enough for another agent to act on.",
            request,
        )
        .await
    }

    async fn web_research_with_system(&self, system: &str, request: &str) -> Result<String> {
        if !self.is_openrouter() {
            bail!("live web research currently requires the OpenRouter provider");
        }

        let messages = vec![ChatMessage::system(system), ChatMessage::user(request)];
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
        let api_key = self.api_key()?;
        let has_tools = !tools.is_empty();
        let provider_preferences = self.provider_preferences();
        let request = ChatRequest {
            model: &self.config.model,
            messages,
            temperature: self.config.temperature,
            stream: false,
            tools: has_tools.then_some(tools),
            tool_choice: has_tools.then_some("auto"),
            parallel_tool_calls: has_tools.then_some(false),
            provider: provider_preferences.as_ref(),
        };

        let body = self.send_json_request(api_key.as_deref(), &request).await?;
        let parsed: ChatResponse = serde_json::from_str(&body).with_context(|| {
            format!(
                "could not parse AI provider response: {}",
                bounded_text(&body, 4000)
            )
        })?;

        let message = parsed
            .choices
            .into_iter()
            .next()
            .context("AI provider returned no choices")?
            .message;

        validate_turn(&message.content, &message.tool_calls)?;
        Ok(AssistantTurn {
            content: message.content,
            tool_calls: message.tool_calls,
        })
    }

    pub async fn chat_stream<F>(
        &self,
        messages: &[ChatMessage],
        tools: &[Value],
        mut on_delta: F,
    ) -> Result<AssistantTurn>
    where
        F: FnMut(&str),
    {
        self.ensure_supported_provider()?;
        let api_key = self.api_key()?;
        let has_tools = !tools.is_empty();
        let provider_preferences = self.provider_preferences();
        let request = ChatRequest {
            model: &self.config.model,
            messages,
            temperature: self.config.temperature,
            stream: true,
            tools: has_tools.then_some(tools),
            tool_choice: has_tools.then_some("auto"),
            parallel_tool_calls: has_tools.then_some(false),
            provider: provider_preferences.as_ref(),
        };

        let retries = self.config.max_retries.min(4);
        for attempt in 0..=retries {
            let mut builder = self
                .client
                .post(&self.config.endpoint)
                .timeout(Duration::from_secs(
                    self.config.request_timeout_seconds.clamp(15, 600),
                ))
                .json(&request);

            if let Some(api_key) = api_key.as_deref() {
                builder = builder.bearer_auth(api_key);
            }
            if self.config.provider == "openrouter" {
                builder = builder.header("X-OpenRouter-Title", "Roldex");
            }

            let mut response = match builder.send().await {
                Ok(response) => response,
                Err(error) => {
                    if attempt < retries && (error.is_timeout() || error.is_connect()) {
                        sleep(retry_delay(attempt, None)).await;
                        continue;
                    }
                    if error.is_timeout() {
                        bail!(
                            "AI provider request timed out after {} seconds",
                            self.config.request_timeout_seconds.clamp(15, 600)
                        );
                    }
                    if self.config.is_local() {
                        return Err(error).with_context(|| {
                            format!(
                                "could not reach the local AI server at {}. Start llama-server or another OpenAI-compatible local server first",
                                self.config.endpoint
                            )
                        });
                    }
                    return Err(error).context("failed to reach AI provider");
                }
            };

            let status = response.status();
            if !status.is_success() {
                let retry_after = response
                    .headers()
                    .get(RETRY_AFTER)
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<u64>().ok());
                let body = response
                    .text()
                    .await
                    .context("failed to read AI provider response")?;
                if attempt < retries && is_transient_status(status.as_u16()) {
                    sleep(retry_delay(attempt, retry_after)).await;
                    continue;
                }
                bail!(
                    "AI provider returned HTTP {status}: {}",
                    bounded_text(&body, 4000)
                );
            }

            let mut sse_buffer = Vec::<u8>::new();
            let mut content = String::new();
            let mut tool_accumulators = Vec::<ToolCallAccumulator>::new();
            let mut stream_done = false;

            while let Some(chunk) = response
                .chunk()
                .await
                .context("failed while reading AI provider stream")?
            {
                sse_buffer.extend_from_slice(&chunk);

                while let Some(newline) = sse_buffer.iter().position(|byte| *byte == b'\n') {
                    let mut line = sse_buffer.drain(..=newline).collect::<Vec<_>>();
                    if line.last() == Some(&b'\n') {
                        line.pop();
                    }
                    if line.last() == Some(&b'\r') {
                        line.pop();
                    }
                    if line.is_empty() {
                        continue;
                    }

                    let line = std::str::from_utf8(&line)
                        .context("AI provider stream contained invalid UTF-8")?;
                    let Some(data) = line.strip_prefix("data:") else {
                        continue;
                    };
                    let data = data.trim_start();
                    if data == "[DONE]" {
                        stream_done = true;
                        break;
                    }
                    if data.is_empty() {
                        continue;
                    }

                    let value: Value = serde_json::from_str(data).with_context(|| {
                        format!(
                            "could not parse AI provider stream event: {}",
                            bounded_text(data, 1200)
                        )
                    })?;
                    if let Some(error) = value.get("error") {
                        bail!(
                            "AI provider stream returned an error: {}",
                            bounded_text(&error.to_string(), 4000)
                        );
                    }

                    let event: StreamResponse = serde_json::from_value(value)
                        .context("could not decode AI provider stream event")?;
                    for choice in event.choices {
                        if let Some(delta) = choice.delta.content {
                            if !delta.is_empty() {
                                content.push_str(&delta);
                                on_delta(&delta);
                            }
                        }

                        for tool_delta in choice.delta.tool_calls {
                            while tool_accumulators.len() <= tool_delta.index {
                                tool_accumulators.push(ToolCallAccumulator::default());
                            }
                            let accumulator = &mut tool_accumulators[tool_delta.index];
                            if let Some(id) = tool_delta.id {
                                if !id.is_empty() {
                                    accumulator.id = Some(id);
                                }
                            }
                            if let Some(kind) = tool_delta.kind {
                                if !kind.is_empty() {
                                    accumulator.kind = Some(kind);
                                }
                            }
                            if let Some(function) = tool_delta.function {
                                if let Some(name) = function.name {
                                    accumulator.name.push_str(&name);
                                }
                                if let Some(arguments) = function.arguments {
                                    accumulator.arguments.push_str(&arguments);
                                }
                            }
                        }
                    }
                }

                if stream_done {
                    break;
                }
            }

            let tool_calls = tool_accumulators
                .into_iter()
                .enumerate()
                .filter(|(_, accumulator)| !accumulator.name.is_empty())
                .map(|(index, accumulator)| ToolCall {
                    id: accumulator
                        .id
                        .unwrap_or_else(|| format!("tool-call-{}", index + 1)),
                    kind: accumulator.kind.unwrap_or_else(|| "function".into()),
                    function: ToolCallFunction {
                        name: accumulator.name,
                        arguments: accumulator.arguments,
                    },
                })
                .collect::<Vec<_>>();

            let content = (!content.trim().is_empty()).then_some(content);
            validate_turn(&content, &tool_calls)?;
            return Ok(AssistantTurn {
                content,
                tool_calls,
            });
        }

        unreachable!("provider retry loop always returns or errors")
    }

    fn ensure_supported_provider(&self) -> Result<()> {
        if self.config.provider != "openrouter"
            && self.config.provider != "openai-compatible"
            && self.config.provider != "local"
        {
            bail!(
                "unsupported provider '{}'; v0.1 supports openrouter, openai-compatible, and local endpoints",
                self.config.provider
            );
        }
        Ok(())
    }

    fn api_key(&self) -> Result<Option<String>> {
        self.ensure_supported_provider()?;
        if !self.config.requires_api_key() {
            return Ok(None);
        }
        env::var(&self.config.api_key_env)
            .map(Some)
            .with_context(|| {
                format!(
                    "missing API key environment variable {}",
                    self.config.api_key_env
                )
            })
    }

    fn provider_preferences(&self) -> Option<Value> {
        if !self.is_openrouter() {
            return None;
        }
        let sort = match self.config.openrouter_sort.as_str() {
            "price" | "throughput" | "latency" => self.config.openrouter_sort.as_str(),
            _ => "latency",
        };
        Some(json!({
            "sort": sort,
            "allow_fallbacks": true
        }))
    }

    async fn send_json_request(
        &self,
        api_key: Option<&str>,
        request: &ChatRequest<'_>,
    ) -> Result<String> {
        self.ensure_supported_provider()?;
        let retries = self.config.max_retries.min(4);
        for attempt in 0..=retries {
            let mut builder = self
                .client
                .post(&self.config.endpoint)
                .timeout(Duration::from_secs(
                    self.config.request_timeout_seconds.clamp(15, 600),
                ))
                .json(request);

            if let Some(api_key) = api_key {
                builder = builder.bearer_auth(api_key);
            }
            if self.config.provider == "openrouter" {
                builder = builder.header("X-OpenRouter-Title", "Roldex");
            }

            let response = match builder.send().await {
                Ok(response) => response,
                Err(error) => {
                    if attempt < retries && (error.is_timeout() || error.is_connect()) {
                        sleep(retry_delay(attempt, None)).await;
                        continue;
                    }
                    if error.is_timeout() {
                        bail!(
                            "AI provider request timed out after {} seconds",
                            self.config.request_timeout_seconds.clamp(15, 600)
                        );
                    }
                    if self.config.is_local() {
                        return Err(error).with_context(|| {
                            format!(
                                "could not reach the local AI server at {}. Start llama-server or another OpenAI-compatible local server first",
                                self.config.endpoint
                            )
                        });
                    }
                    return Err(error).context("failed to reach AI provider");
                }
            };

            let status = response.status();
            let retry_after = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok());
            let body = response
                .text()
                .await
                .context("failed to read AI provider response")?;

            if !status.is_success() {
                if attempt < retries && is_transient_status(status.as_u16()) {
                    sleep(retry_delay(attempt, retry_after)).await;
                    continue;
                }
                bail!(
                    "AI provider returned HTTP {status}: {}",
                    bounded_text(&body, 4000)
                );
            }

            return Ok(body);
        }

        unreachable!("provider retry loop always returns or errors")
    }
}

fn validate_turn(content: &Option<String>, tool_calls: &[ToolCall]) -> Result<()> {
    if tool_calls.is_empty()
        && content
            .as_deref()
            .is_none_or(|content| content.trim().is_empty())
    {
        bail!("AI provider returned neither text nor tool calls");
    }
    Ok(())
}

fn is_transient_status(status: u16) -> bool {
    matches!(status, 408 | 425 | 429 | 500 | 502 | 503 | 504)
}

fn retry_delay(attempt: u32, retry_after_seconds: Option<u64>) -> Duration {
    if let Some(seconds) = retry_after_seconds {
        return Duration::from_secs(seconds.clamp(1, 10));
    }
    let factor = 1u64 << attempt.min(4);
    Duration::from_millis(450 * factor)
}

fn bounded_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut output = text.chars().take(max_chars).collect::<String>();
    output.push('…');
    output
}
