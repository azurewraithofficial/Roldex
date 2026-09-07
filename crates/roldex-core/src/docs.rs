use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::ToolCall;
use crate::tools::{AgentEvent, ToolExecution};

const DOCS_ORIGIN: &str = "https://create.roblox.com";
const DOCS_INDEX: &str = "https://create.roblox.com/docs/llms.txt";
const MAX_INDEX_BYTES: u64 = 2 * 1024 * 1024;
const DEFAULT_PAGE_CHARS: usize = 18_000;

#[derive(Debug, Deserialize)]
struct DocsSearchArgs {
    query: String,
    max_results: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct DocsPageArgs {
    path: String,
    max_chars: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
struct DocsHit {
    title: String,
    path: String,
    url: String,
    summary: String,
    score: usize,
}

pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "roblox_docs_search",
                "description": "Search the current official Roblox Creator Hub documentation index. Use this when Roblox API behavior, current Studio features, security guidance, services, scripting, UI, animation, physics, data stores, Open Cloud, or other platform-specific facts matter.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Focused Roblox documentation search query" },
                        "max_results": { "type": "integer", "minimum": 1, "maximum": 10 }
                    },
                    "required": ["query"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "roblox_docs_page",
                "description": "Fetch a bounded current Markdown page from the official Roblox Creator Hub. Pass a /docs/... path returned by roblox_docs_search.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Official /docs/... Markdown path from roblox_docs_search" },
                        "max_chars": { "type": "integer", "minimum": 2000, "maximum": 30000 }
                    },
                    "required": ["path"],
                    "additionalProperties": false
                }
            }
        }),
    ]
}

pub async fn execute_tool(call: &ToolCall) -> Option<ToolExecution> {
    match call.function.name.as_str() {
        "roblox_docs_search" => Some(execute_search(call).await),
        "roblox_docs_page" => Some(execute_page(call).await),
        _ => None,
    }
}

async fn execute_search(call: &ToolCall) -> ToolExecution {
    let parsed = serde_json::from_str::<DocsSearchArgs>(&call.function.arguments);
    match parsed {
        Ok(args) => {
            let query = args.query.trim().to_owned();
            let event = AgentEvent::UsingTool(format!("Roblox docs search: {query}"));
            let output = match search_docs(&query, args.max_results.unwrap_or(6)).await {
                Ok(results) => json!({
                    "ok": true,
                    "source": "Roblox Creator Hub",
                    "index": DOCS_INDEX,
                    "results": results
                })
                .to_string(),
                Err(error) => error_output(error),
            };
            ToolExecution { event, output }
        }
        Err(error) => ToolExecution {
            event: AgentEvent::UsingTool("roblox_docs_search".into()),
            output: json!({ "ok": false, "error": format!("invalid tool arguments: {error}") })
                .to_string(),
        },
    }
}

async fn execute_page(call: &ToolCall) -> ToolExecution {
    let parsed = serde_json::from_str::<DocsPageArgs>(&call.function.arguments);
    match parsed {
        Ok(args) => {
            let event = AgentEvent::UsingTool(format!("Roblox docs page: {}", args.path));
            let output =
                match fetch_page(&args.path, args.max_chars.unwrap_or(DEFAULT_PAGE_CHARS)).await {
                    Ok((url, content, truncated)) => json!({
                        "ok": true,
                        "source": "Roblox Creator Hub",
                        "path": args.path,
                        "url": url,
                        "truncated": truncated,
                        "content": content
                    })
                    .to_string(),
                    Err(error) => error_output(error),
                };
            ToolExecution { event, output }
        }
        Err(error) => ToolExecution {
            event: AgentEvent::UsingTool("roblox_docs_page".into()),
            output: json!({ "ok": false, "error": format!("invalid tool arguments: {error}") })
                .to_string(),
        },
    }
}

async fn search_docs(query: &str, max_results: usize) -> Result<Vec<DocsHit>> {
    if query.trim().is_empty() {
        bail!("Roblox docs search query cannot be empty");
    }

    let client = docs_client()?;
    let response = client
        .get(DOCS_INDEX)
        .send()
        .await
        .context("failed to reach Roblox Creator Hub docs index")?;
    if !response.status().is_success() {
        bail!(
            "Roblox Creator Hub docs index returned HTTP {}",
            response.status()
        );
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_INDEX_BYTES)
    {
        bail!("Roblox docs index exceeded the configured size limit");
    }

    let index = response
        .text()
        .await
        .context("failed to read Roblox Creator Hub docs index")?;
    if index.len() as u64 > MAX_INDEX_BYTES {
        bail!("Roblox docs index exceeded the configured size limit");
    }

    let mut hits: Vec<DocsHit> = index
        .lines()
        .filter_map(parse_index_line)
        .map(|mut hit| {
            hit.score = score_hit(&hit, query);
            hit
        })
        .filter(|hit| hit.score > 0)
        .collect();

    hits.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.title.cmp(&right.title))
    });
    hits.truncate(max_results.clamp(1, 10));
    Ok(hits)
}

async fn fetch_page(path: &str, max_chars: usize) -> Result<(String, String, bool)> {
    let normalized = normalize_docs_path(path)?;
    let url = format!("{DOCS_ORIGIN}{normalized}");
    let client = docs_client()?;
    let response = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("failed to reach Roblox Creator Hub page {normalized}"))?;
    if !response.status().is_success() {
        bail!(
            "Roblox Creator Hub page returned HTTP {}",
            response.status()
        );
    }

    let text = response
        .text()
        .await
        .with_context(|| format!("failed to read Roblox Creator Hub page {normalized}"))?;
    let max_chars = max_chars.clamp(2_000, 30_000);
    let (content, truncated) = truncate_chars(&text, max_chars);
    Ok((url, content, truncated))
}

fn docs_client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("Roldex/0.1 Roblox documentation retrieval")
        .build()
        .context("failed to create Roblox docs HTTP client")
}

fn parse_index_line(line: &str) -> Option<DocsHit> {
    let line = line.trim();
    if !line.starts_with("- [") {
        return None;
    }

    let title_end = line.find("](")?;
    let title = line.get(3..title_end)?.trim();
    let path_start = title_end + 2;
    let path_end = line.get(path_start..)?.find(')')? + path_start;
    let path = line.get(path_start..path_end)?.trim();
    if !path.starts_with("/docs/") || path.contains("..") {
        return None;
    }

    let summary = line
        .get(path_end + 1..)
        .unwrap_or_default()
        .trim()
        .trim_start_matches(':')
        .trim();

    Some(DocsHit {
        title: title.to_owned(),
        path: path.to_owned(),
        url: format!("{DOCS_ORIGIN}{path}"),
        summary: summary.to_owned(),
        score: 0,
    })
}

fn score_hit(hit: &DocsHit, query: &str) -> usize {
    let query = query.to_ascii_lowercase();
    let title = hit.title.to_ascii_lowercase();
    let path = hit.path.to_ascii_lowercase();
    let summary = hit.summary.to_ascii_lowercase();
    let mut score = 0usize;

    if title.contains(&query) {
        score += 30;
    }
    if path.contains(&query.replace(' ', "-")) {
        score += 18;
    }
    if summary.contains(&query) {
        score += 12;
    }

    for token in query.split_whitespace().filter(|token| token.len() >= 2) {
        if title.contains(token) {
            score += 8;
        }
        if path.contains(token) {
            score += 5;
        }
        if summary.contains(token) {
            score += 2;
        }
    }

    score
}

fn normalize_docs_path(path: &str) -> Result<String> {
    let path = path.trim();
    if path.is_empty() || path.contains("..") || path.contains('?') || path.contains('#') {
        bail!("invalid Roblox docs path");
    }
    if path.starts_with("http://") || path.starts_with("https://") {
        bail!("pass the /docs/... path returned by roblox_docs_search, not an arbitrary URL");
    }

    let normalized = if path.starts_with("/docs/") {
        path.to_owned()
    } else if path.starts_with("docs/") {
        format!("/{path}")
    } else {
        bail!("Roblox docs paths must begin with /docs/");
    };

    Ok(normalized)
}

fn truncate_chars(value: &str, max_chars: usize) -> (String, bool) {
    let mut chars = value.chars();
    let text: String = chars.by_ref().take(max_chars).collect();
    let truncated = chars.next().is_some();
    (text, truncated)
}

fn error_output(error: anyhow::Error) -> String {
    json!({ "ok": false, "error": error.to_string() }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_creator_hub_index_entries() {
        let hit = parse_index_line(
            "- [Remote events and callbacks](/docs/en-us/scripting/events/remote.md): Remote network events and callbacks.",
        )
        .expect("hit");
        assert_eq!(hit.title, "Remote events and callbacks");
        assert_eq!(hit.path, "/docs/en-us/scripting/events/remote.md");
        assert!(score_hit(&hit, "remote event") > 0);
    }

    #[test]
    fn rejects_arbitrary_docs_urls() {
        assert!(normalize_docs_path("https://example.com/evil").is_err());
        assert!(normalize_docs_path("/docs/../secret").is_err());
        assert_eq!(
            normalize_docs_path("docs/en-us/scripting.md").expect("path"),
            "/docs/en-us/scripting.md"
        );
    }
}
