use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Serialize;

const MAX_SEARCH_FILE_BYTES: u64 = 256 * 1024;
const MAX_SCANNED_FILES: usize = 5_000;

#[derive(Debug, Clone, Serialize)]
pub struct TextMatch {
    pub path: String,
    pub line: usize,
    pub text: String,
}

pub fn search_text(
    root: &Path,
    query: &str,
    case_sensitive: bool,
    max_results: usize,
) -> Result<Vec<TextMatch>> {
    if query.trim().is_empty() {
        bail!("search query cannot be empty");
    }

    let mut matches = Vec::new();
    let mut scanned_files = 0usize;
    let max_results = max_results.clamp(1, 100);
    search_dir(
        root,
        root,
        query,
        case_sensitive,
        max_results,
        0,
        &mut scanned_files,
        &mut matches,
    )?;
    Ok(matches)
}

#[allow(clippy::too_many_arguments)]
fn search_dir(
    root: &Path,
    current: &Path,
    query: &str,
    case_sensitive: bool,
    max_results: usize,
    depth: usize,
    scanned_files: &mut usize,
    matches: &mut Vec<TextMatch>,
) -> Result<()> {
    if depth > 16 || *scanned_files >= MAX_SCANNED_FILES || matches.len() >= max_results {
        return Ok(());
    }

    let mut entries = fs::read_dir(current)
        .with_context(|| format!("failed to search {}", current.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        if *scanned_files >= MAX_SCANNED_FILES || matches.len() >= max_results {
            break;
        }

        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }

        let name = entry.file_name();
        let name = name.to_string_lossy();
        if file_type.is_dir() {
            if matches!(
                name.as_ref(),
                ".git" | "target" | "node_modules" | ".roldex"
            ) {
                continue;
            }
            search_dir(
                root,
                &entry.path(),
                query,
                case_sensitive,
                max_results,
                depth + 1,
                scanned_files,
                matches,
            )?;
            continue;
        }

        if !file_type.is_file() || !is_text_candidate(&entry.path()) {
            continue;
        }

        *scanned_files += 1;
        let path = entry.path();
        let metadata = entry.metadata()?;
        if metadata.len() > MAX_SEARCH_FILE_BYTES {
            continue;
        }

        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let needle = (!case_sensitive).then(|| query.to_lowercase());

        for (index, line) in content.lines().enumerate() {
            let is_match = if case_sensitive {
                line.contains(query)
            } else {
                line.to_lowercase()
                    .contains(needle.as_deref().expect("needle exists"))
            };

            if is_match {
                let relative = path.strip_prefix(root).unwrap_or(&path);
                matches.push(TextMatch {
                    path: relative.to_string_lossy().into_owned(),
                    line: index + 1,
                    text: truncate_line(line, 240),
                });
                if matches.len() >= max_results {
                    break;
                }
            }
        }
    }

    Ok(())
}

fn is_text_candidate(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
        return matches!(
            path.file_name().and_then(|value| value.to_str()),
            Some("README" | "LICENSE" | "Makefile")
        );
    };

    matches!(
        extension.to_ascii_lowercase().as_str(),
        "lua"
            | "luau"
            | "json"
            | "toml"
            | "yaml"
            | "yml"
            | "md"
            | "txt"
            | "rs"
            | "js"
            | "jsx"
            | "ts"
            | "tsx"
            | "css"
            | "html"
            | "xml"
            | "csv"
            | "ron"
    )
}

fn truncate_line(line: &str, max_chars: usize) -> String {
    let mut chars = line.chars();
    let text: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{text}…")
    } else {
        text
    }
}
