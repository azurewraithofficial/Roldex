use std::env;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::WorkspaceFs;
use crate::tools::{AgentEvent, ToolExecution};

const POLLINATIONS_ORIGIN: &str = "https://gen.pollinations.ai";
const MAX_MEDIA_BYTES: usize = 24 * 1024 * 1024;
const MAX_PROMPT_CHARS: usize = 4_000;

#[derive(Debug, Deserialize)]
struct GenerateImageArgs {
    prompt: String,
    output_path: Option<String>,
    model: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GenerateVoiceArgs {
    text: String,
    output_path: Option<String>,
    voice: Option<String>,
}

pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "generate_image",
                "description": "Generate a new image asset from a text prompt and save it inside the active Roblox project. Use only when the user explicitly asks to create/generate an image, icon, thumbnail, texture, decal concept, UI art, or similar visual asset. Requires POLLINATIONS_API_KEY; provider quotas or pricing can apply.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "prompt": { "type": "string", "description": "Detailed visual generation prompt" },
                        "output_path": { "type": "string", "description": "Optional workspace-relative output path, such as assets/generated/shop-icon.png" },
                        "model": { "type": "string", "description": "Optional Pollinations image model. Default: flux" }
                    },
                    "required": ["prompt"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "generate_voice_audio",
                "description": "Generate spoken voice/TTS audio and save it inside the project. This is for spoken dialogue or narration, not general sound effects or music. Requires POLLINATIONS_API_KEY; provider quotas or pricing can apply.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "text": { "type": "string" },
                        "output_path": { "type": "string", "description": "Optional workspace-relative .mp3 path" },
                        "voice": { "type": "string", "description": "Optional provider voice. Default: nova" }
                    },
                    "required": ["text"],
                    "additionalProperties": false
                }
            }
        }),
    ]
}

pub async fn execute_tool(call: &crate::ToolCall, fs: &WorkspaceFs) -> Option<ToolExecution> {
    match call.function.name.as_str() {
        "generate_image" => Some(execute_image(call, fs).await),
        "generate_voice_audio" => Some(execute_voice(call, fs).await),
        _ => None,
    }
}

async fn execute_image(call: &crate::ToolCall, fs: &WorkspaceFs) -> ToolExecution {
    let event = AgentEvent::UsingTool("image generation".into());
    let output = match serde_json::from_str::<GenerateImageArgs>(&call.function.arguments) {
        Ok(args) => match generate_image(args, fs).await {
            Ok(result) => json!({
                "ok": true,
                "provider": "Pollinations",
                "path": result.path,
                "media_type": result.media_type,
                "bytes": result.bytes
            })
            .to_string(),
            Err(error) => error_output(error),
        },
        Err(error) => json!({
            "ok": false,
            "error": format!("invalid generate_image arguments: {error}")
        })
        .to_string(),
    };
    ToolExecution { event, output }
}

async fn execute_voice(call: &crate::ToolCall, fs: &WorkspaceFs) -> ToolExecution {
    let event = AgentEvent::UsingTool("voice audio generation".into());
    let output = match serde_json::from_str::<GenerateVoiceArgs>(&call.function.arguments) {
        Ok(args) => match generate_voice(args, fs).await {
            Ok(result) => json!({
                "ok": true,
                "provider": "Pollinations",
                "path": result.path,
                "media_type": result.media_type,
                "bytes": result.bytes
            })
            .to_string(),
            Err(error) => error_output(error),
        },
        Err(error) => json!({
            "ok": false,
            "error": format!("invalid generate_voice_audio arguments: {error}")
        })
        .to_string(),
    };
    ToolExecution { event, output }
}

struct MediaResult {
    path: String,
    media_type: String,
    bytes: usize,
}

async fn generate_image(args: GenerateImageArgs, fs: &WorkspaceFs) -> Result<MediaResult> {
    let prompt = bounded_text(&args.prompt, "image prompt")?;
    let model = sanitize_simple(args.model.as_deref().unwrap_or("flux"), "image model")?;
    let key = media_key()?;
    let client = media_client()?;

    let mut url = Url::parse(&format!("{POLLINATIONS_ORIGIN}/image/"))?;
    url.path_segments_mut()
        .map_err(|_| anyhow::anyhow!("invalid media API base URL"))?
        .push(prompt);
    url.query_pairs_mut().append_pair("model", model);

    let response = client
        .get(url)
        .bearer_auth(key)
        .send()
        .await
        .context("failed to reach image generation provider")?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        bail!("image generation provider returned HTTP {status}: {body}");
    }

    let media_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("image/jpeg")
        .split(';')
        .next()
        .unwrap_or("image/jpeg")
        .trim()
        .to_owned();
    if !media_type.starts_with("image/") {
        bail!("image generation provider returned unexpected content type {media_type}");
    }

    let bytes = response
        .bytes()
        .await
        .context("failed to read generated image")?;
    if bytes.len() > MAX_MEDIA_BYTES {
        bail!("generated image exceeded {MAX_MEDIA_BYTES} bytes");
    }

    let extension = image_extension(&media_type);
    let path = output_path(args.output_path.as_deref(), "image", prompt, extension);
    fs.write_bytes(&path, &bytes)?;
    Ok(MediaResult {
        path,
        media_type,
        bytes: bytes.len(),
    })
}

async fn generate_voice(args: GenerateVoiceArgs, fs: &WorkspaceFs) -> Result<MediaResult> {
    let text = bounded_text(&args.text, "voice text")?;
    let voice = sanitize_simple(args.voice.as_deref().unwrap_or("nova"), "voice")?;
    let key = media_key()?;
    let client = media_client()?;

    let mut url = Url::parse(&format!("{POLLINATIONS_ORIGIN}/audio/"))?;
    url.path_segments_mut()
        .map_err(|_| anyhow::anyhow!("invalid media API base URL"))?
        .push(text);
    url.query_pairs_mut().append_pair("voice", voice);

    let response = client
        .get(url)
        .bearer_auth(key)
        .send()
        .await
        .context("failed to reach voice generation provider")?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        bail!("voice generation provider returned HTTP {status}: {body}");
    }

    let media_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("audio/mpeg")
        .split(';')
        .next()
        .unwrap_or("audio/mpeg")
        .trim()
        .to_owned();
    if !media_type.starts_with("audio/") {
        bail!("voice generation provider returned unexpected content type {media_type}");
    }

    let bytes = response
        .bytes()
        .await
        .context("failed to read generated audio")?;
    if bytes.len() > MAX_MEDIA_BYTES {
        bail!("generated audio exceeded {MAX_MEDIA_BYTES} bytes");
    }

    let path = output_path(args.output_path.as_deref(), "voice", text, "mp3");
    fs.write_bytes(&path, &bytes)?;
    Ok(MediaResult {
        path,
        media_type,
        bytes: bytes.len(),
    })
}

fn media_key() -> Result<String> {
    env::var("POLLINATIONS_API_KEY").context(
        "media generation needs POLLINATIONS_API_KEY; Roldex never stores this key in the repository",
    )
}

fn media_client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent("Roldex/0.1 Roblox asset generation")
        .build()
        .context("failed to create media HTTP client")
}

fn bounded_text<'a>(value: &'a str, label: &str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{label} cannot be empty");
    }
    if value.chars().count() > MAX_PROMPT_CHARS {
        bail!("{label} is too long; limit is {MAX_PROMPT_CHARS} characters");
    }
    Ok(value)
}

fn sanitize_simple<'a>(value: &'a str, label: &str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 100
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '/' | '.'))
    {
        bail!("invalid {label}");
    }
    Ok(value)
}

fn output_path(requested: Option<&str>, kind: &str, prompt: &str, extension: &str) -> String {
    if let Some(requested) = requested.map(str::trim).filter(|value| !value.is_empty()) {
        return requested.to_owned();
    }

    let slug = slugify(prompt);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("assets/generated/{kind}-{slug}-{stamp}.{extension}")
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut separator = false;
    for ch in value.chars().take(120) {
        if ch.is_ascii_alphanumeric() {
            if separator && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(ch.to_ascii_lowercase());
            separator = false;
        } else {
            separator = true;
        }
        if slug.len() >= 48 {
            break;
        }
    }
    if slug.is_empty() {
        "asset".into()
    } else {
        slug.trim_end_matches('-').to_owned()
    }
}

fn image_extension(media_type: &str) -> &'static str {
    match media_type {
        "image/png" => "png",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        _ => "jpg",
    }
}

fn error_output(error: anyhow::Error) -> String {
    json!({ "ok": false, "error": error.to_string() }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn makes_safe_asset_slugs() {
        assert_eq!(slugify("Hand-drawn Shop Icon!"), "hand-drawn-shop-icon");
        assert_eq!(slugify("***"), "asset");
    }

    #[test]
    fn rejects_provider_parameter_injection() {
        assert!(sanitize_simple("flux&evil=true", "model").is_err());
        assert!(sanitize_simple("seedream5", "model").is_ok());
    }
}
