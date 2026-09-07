use std::collections::HashSet;
use std::path::Path;

use roldex_core::WorkspaceFs;

const MAX_IMAGES: usize = 4;
const MAX_TOKENS: usize = 64;
const MAX_SPAN_TOKENS: usize = 8;

pub fn detect_image_paths(input: &str, fs: &WorkspaceFs) -> Vec<String> {
    let mut candidates = quoted_candidates(input);
    let tokens: Vec<&str> = input.split_whitespace().take(MAX_TOKENS).collect();

    for start in 0..tokens.len() {
        let max_end = (start + MAX_SPAN_TOKENS).min(tokens.len());
        for end in start + 1..=max_end {
            let joined = tokens[start..end].join(" ");
            let cleaned = clean_candidate(&joined);
            if looks_like_image(&cleaned) {
                candidates.push(cleaned);
            }
        }
    }

    let mut seen = HashSet::new();
    let mut paths = Vec::new();
    for candidate in candidates {
        if !looks_like_image(&candidate) {
            continue;
        }
        let Ok(canonical) = fs.resolve_user_read_path(&candidate) else {
            continue;
        };
        let key = canonical.to_string_lossy().to_string();
        if seen.insert(key) {
            paths.push(candidate);
        }
        if paths.len() >= MAX_IMAGES {
            break;
        }
    }
    paths
}

fn quoted_candidates(input: &str) -> Vec<String> {
    let mut output = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut index = 0usize;

    while index < chars.len() {
        let quote = chars[index];
        if quote != '"' && quote != '\'' {
            index += 1;
            continue;
        }
        let start = index + 1;
        index = start;
        while index < chars.len() && chars[index] != quote {
            index += 1;
        }
        if index > start {
            output.push(chars[start..index].iter().collect());
        }
        index += 1;
    }

    output
}

fn clean_candidate(value: &str) -> String {
    value
        .trim_matches(|ch: char| {
            matches!(
                ch,
                '"' | '\'' | ',' | ';' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>'
            )
        })
        .to_owned()
}

fn looks_like_image(value: &str) -> bool {
    let extension = Path::new(value)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use roldex_core::PermissionMode;

    use super::*;

    fn test_dir() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("roldex-intent-{}-{unique}", std::process::id()))
    }

    #[test]
    fn finds_quoted_image_path_with_spaces() {
        let root = test_dir();
        fs::create_dir_all(&root).expect("root");
        fs::write(root.join("studio error.png"), b"fake").expect("image");
        let workspace = WorkspaceFs::new(&root, PermissionMode::Workspace).expect("workspace");

        let paths = detect_image_paths(
            "look at \"studio error.png\" and explain this Roblox error",
            &workspace,
        );
        assert_eq!(paths, vec!["studio error.png"]);
        fs::remove_dir_all(root).expect("cleanup");
    }
}
