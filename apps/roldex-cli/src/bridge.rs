use std::sync::Arc;

use anyhow::{Context, Result, bail};
use roldex_core::{Agent, WorkspaceFs};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 256 * 1024;
const MAX_MESSAGE_CHARS: usize = 16_000;
const MAX_SCRIPT_CHARS: usize = 80_000;
const MAX_SELECTION_ITEMS: usize = 24;

#[derive(Debug, Deserialize)]
struct StudioChatRequest {
    message: String,
    #[serde(default)]
    selection: Vec<StudioSelection>,
    active_script: Option<StudioScriptContext>,
}

#[derive(Debug, Deserialize)]
struct StudioSelection {
    name: String,
    class_name: String,
    full_name: String,
}

#[derive(Debug, Deserialize)]
struct StudioScriptContext {
    class_name: String,
    full_name: String,
    source: String,
}

struct HttpRequest {
    method: String,
    path: String,
    bridge_header: bool,
    body: Vec<u8>,
}

pub async fn start(
    port: u16,
    agent: Arc<Mutex<Agent>>,
    fs: Arc<WorkspaceFs>,
) -> Result<JoinHandle<()>> {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .await
        .with_context(|| format!("could not bind Studio bridge to 127.0.0.1:{port}"))?;

    Ok(tokio::spawn(async move {
        loop {
            let Ok((stream, _peer)) = listener.accept().await else {
                break;
            };
            let agent = Arc::clone(&agent);
            let fs = Arc::clone(&fs);
            tokio::spawn(async move {
                if let Err(error) = handle_connection(stream, agent, fs).await {
                    eprintln!("Studio bridge request failed: {error:#}");
                }
            });
        }
    }))
}

async fn handle_connection(
    mut stream: TcpStream,
    agent: Arc<Mutex<Agent>>,
    fs: Arc<WorkspaceFs>,
) -> Result<()> {
    let request = read_request(&mut stream).await?;

    if !request.bridge_header {
        return write_json(
            &mut stream,
            403,
            "Forbidden",
            &json!({ "ok": false, "error": "missing Roldex Studio bridge header" }),
        )
        .await;
    }

    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/health") => {
            write_json(
                &mut stream,
                200,
                "OK",
                &json!({ "ok": true, "service": "roldex-studio-bridge" }),
            )
            .await
        }
        ("POST", "/v1/chat") => {
            let payload: StudioChatRequest = serde_json::from_slice(&request.body)
                .context("Studio bridge received invalid JSON")?;
            let enriched = build_studio_prompt(payload)?;
            let answer = {
                let mut agent = agent.lock().await;
                agent.chat_with_tools(&enriched, fs.as_ref(), |_| {}).await?
            };

            write_json(
                &mut stream,
                200,
                "OK",
                &json!({ "ok": true, "answer": answer }),
            )
            .await
        }
        _ => {
            write_json(
                &mut stream,
                404,
                "Not Found",
                &json!({ "ok": false, "error": "unknown Studio bridge endpoint" }),
            )
            .await
        }
    }
}

fn build_studio_prompt(payload: StudioChatRequest) -> Result<String> {
    let message = payload.message.trim();
    if message.is_empty() {
        bail!("Studio prompt cannot be empty");
    }

    let mut prompt = truncate_chars(message, MAX_MESSAGE_CHARS);
    prompt.push_str("\n\n[Live Roblox Studio context]\n");

    if payload.selection.is_empty() {
        prompt.push_str("Selection: none\n");
    } else {
        prompt.push_str("Selection:\n");
        for item in payload.selection.iter().take(MAX_SELECTION_ITEMS) {
            prompt.push_str(&format!(
                "- {} ({}) at {}\n",
                truncate_chars(&item.name, 160),
                truncate_chars(&item.class_name, 120),
                truncate_chars(&item.full_name, 600)
            ));
        }
        if payload.selection.len() > MAX_SELECTION_ITEMS {
            prompt.push_str("- … additional selected instances omitted\n");
        }
    }

    if let Some(script) = payload.active_script {
        prompt.push_str(&format!(
            "Active script: {} at {}\n",
            truncate_chars(&script.class_name, 120),
            truncate_chars(&script.full_name, 600)
        ));
        prompt.push_str("Active editor source follows. Treat it as live Studio state and prefer it over stale filesystem copies when they conflict:\n```luau\n");
        prompt.push_str(&truncate_chars(&script.source, MAX_SCRIPT_CHARS));
        prompt.push_str("\n```\n");
    } else {
        prompt.push_str("Active script: none\n");
    }

    Ok(prompt)
}

async fn read_request(stream: &mut TcpStream) -> Result<HttpRequest> {
    let mut buffer = Vec::with_capacity(4096);
    let header_end = loop {
        let mut chunk = [0u8; 4096];
        let read = stream.read(&mut chunk).await.context("failed to read HTTP request")?;
        if read == 0 {
            bail!("Studio bridge connection closed before a complete request arrived");
        }
        buffer.extend_from_slice(&chunk[..read]);

        if let Some(position) = find_header_end(&buffer) {
            break position;
        }
        if buffer.len() > MAX_HEADER_BYTES {
            bail!("Studio bridge HTTP headers are too large");
        }
    };

    if header_end > MAX_HEADER_BYTES {
        bail!("Studio bridge HTTP headers are too large");
    }

    let headers = std::str::from_utf8(&buffer[..header_end])
        .context("Studio bridge HTTP headers were not UTF-8")?;
    let mut lines = headers.split("\r\n");
    let request_line = lines.next().context("missing HTTP request line")?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().context("missing HTTP method")?.to_owned();
    let path = request_parts.next().context("missing HTTP path")?.to_owned();

    let mut content_length = 0usize;
    let mut bridge_header = false;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim();
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") {
            content_length = value
                .parse::<usize>()
                .context("invalid Studio bridge Content-Length")?;
        } else if name.eq_ignore_ascii_case("x-roldex-bridge")
            && value.eq_ignore_ascii_case("studio")
        {
            bridge_header = true;
        }
    }

    if content_length > MAX_BODY_BYTES {
        bail!("Studio bridge request body exceeds {MAX_BODY_BYTES} bytes");
    }

    let body_start = header_end + 4;
    while buffer.len() < body_start + content_length {
        let remaining = body_start + content_length - buffer.len();
        let mut chunk = vec![0u8; remaining.min(8192)];
        let read = stream.read(&mut chunk).await.context("failed to read HTTP body")?;
        if read == 0 {
            bail!("Studio bridge connection closed during request body");
        }
        buffer.extend_from_slice(&chunk[..read]);
    }

    let body = buffer[body_start..body_start + content_length].to_vec();
    Ok(HttpRequest {
        method,
        path,
        bridge_header,
        body,
    })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

async fn write_json(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    value: &Value,
) -> Result<()> {
    let body = serde_json::to_vec(value).context("failed to encode Studio bridge response")?;
    let headers = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n",
        body.len()
    );
    stream
        .write_all(headers.as_bytes())
        .await
        .context("failed to write Studio bridge headers")?;
    stream
        .write_all(&body)
        .await
        .context("failed to write Studio bridge body")?;
    stream.shutdown().await.context("failed to close Studio bridge response")
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let text: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{text}…")
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_http_header_boundary() {
        assert_eq!(find_header_end(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n"), Some(23));
    }

    #[test]
    fn truncation_is_unicode_safe() {
        assert_eq!(truncate_chars("a🧱b", 2), "a🧱…");
    }
}
