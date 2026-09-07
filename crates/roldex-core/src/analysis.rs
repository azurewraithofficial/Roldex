use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

const MAX_ANALYSIS_FILE_BYTES: u64 = 256 * 1024;
const MAX_SCANNED_FILES: usize = 5_000;
const MAX_DEPTH: usize = 16;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    Info,
    Warning,
    High,
}

#[derive(Debug, Clone, Serialize)]
pub struct LuauFinding {
    pub rule_id: &'static str,
    pub severity: FindingSeverity,
    pub path: String,
    pub line: usize,
    pub message: String,
    pub remediation: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LuauAnalysisReport {
    pub scanned_files: usize,
    pub findings: Vec<LuauFinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScriptSide {
    Client,
    Server,
    Shared,
    Unknown,
}

pub fn analyze_luau(root: &Path, max_findings: usize) -> Result<LuauAnalysisReport> {
    let mut report = LuauAnalysisReport {
        scanned_files: 0,
        findings: Vec::new(),
    };
    let max_findings = max_findings.clamp(1, 200);
    analyze_dir(root, root, 0, max_findings, &mut report)?;
    Ok(report)
}

fn analyze_dir(
    root: &Path,
    current: &Path,
    depth: usize,
    max_findings: usize,
    report: &mut LuauAnalysisReport,
) -> Result<()> {
    if depth > MAX_DEPTH
        || report.scanned_files >= MAX_SCANNED_FILES
        || report.findings.len() >= max_findings
    {
        return Ok(());
    }

    let mut entries = fs::read_dir(current)
        .with_context(|| format!("failed to analyze {}", current.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        if report.scanned_files >= MAX_SCANNED_FILES || report.findings.len() >= max_findings {
            break;
        }

        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }

        let name = entry.file_name();
        let name = name.to_string_lossy();
        let path = entry.path();

        if file_type.is_dir() {
            if matches!(
                name.as_ref(),
                ".git" | "target" | "node_modules" | ".roldex"
            ) {
                continue;
            }
            analyze_dir(root, &path, depth + 1, max_findings, report)?;
            continue;
        }

        if !file_type.is_file() || !is_luau_file(&path) {
            continue;
        }

        report.scanned_files += 1;
        let metadata = entry.metadata()?;
        if metadata.len() > MAX_ANALYSIS_FILE_BYTES {
            continue;
        }

        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let relative = path.strip_prefix(root).unwrap_or(&path);
        analyze_file(relative, &content, max_findings, &mut report.findings);
    }

    Ok(())
}

fn analyze_file(
    relative: &Path,
    content: &str,
    max_findings: usize,
    findings: &mut Vec<LuauFinding>,
) {
    let display_path = relative.to_string_lossy().into_owned();
    let side = detect_side(relative);
    let lines: Vec<&str> = content.lines().collect();

    if side == ScriptSide::Server && is_replicated_storage_path(relative) {
        push_finding(
            findings,
            max_findings,
            LuauFinding {
                rule_id: "RLX005",
                severity: FindingSeverity::Warning,
                path: display_path.clone(),
                line: 1,
                message: "Server script appears to live in ReplicatedStorage, where its source is replicated to clients.".into(),
                remediation: "Prefer ServerScriptService for server-only scripts and ServerStorage for server-only modules/assets.".into(),
            },
        );
    }

    for (index, raw_line) in lines.iter().enumerate() {
        if findings.len() >= max_findings {
            break;
        }

        let line_number = index + 1;
        let code = strip_line_comment(raw_line).trim();
        if code.is_empty() {
            continue;
        }

        for (legacy, replacement) in [
            ("wait(", "task.wait"),
            ("spawn(", "task.defer/task.spawn"),
            ("delay(", "task.delay"),
        ] {
            if contains_legacy_scheduler_call(code, legacy) {
                push_finding(
                    findings,
                    max_findings,
                    LuauFinding {
                        rule_id: "RLX001",
                        severity: FindingSeverity::Warning,
                        path: display_path.clone(),
                        line: line_number,
                        message: format!("Legacy scheduler call `{legacy}` detected."),
                        remediation: format!(
                            "Use {replacement} instead of the legacy global scheduler API."
                        ),
                    },
                );
            }
        }

        if side == ScriptSide::Client && code.contains("DataStoreService") {
            push_finding(
                findings,
                max_findings,
                LuauFinding {
                    rule_id: "RLX002",
                    severity: FindingSeverity::High,
                    path: display_path.clone(),
                    line: line_number,
                    message: "Client-side DataStoreService usage detected. Roblox data stores are server-only.".into(),
                    remediation: "Move persistent data access into a server Script and expose only validated, minimal RemoteEvent/RemoteFunction operations to clients.".into(),
                },
            );
        }

        for (legacy, replacement) in [
            ("BodyVelocity", "LinearVelocity"),
            ("BodyPosition", "AlignPosition"),
            ("BodyGyro", "AlignOrientation"),
        ] {
            if code.contains(legacy) {
                push_finding(
                    findings,
                    max_findings,
                    LuauFinding {
                        rule_id: "RLX003",
                        severity: FindingSeverity::Warning,
                        path: display_path.clone(),
                        line: line_number,
                        message: format!("Deprecated Roblox physics mover `{legacy}` detected."),
                        remediation: format!(
                            "Prefer the modern constraint-based `{replacement}` API for new work."
                        ),
                    },
                );
            }
        }

        if side == ScriptSide::Server && code.contains(":InvokeClient(") {
            push_finding(
                findings,
                max_findings,
                LuauFinding {
                    rule_id: "RLX004",
                    severity: FindingSeverity::Warning,
                    path: display_path.clone(),
                    line: line_number,
                    message: "RemoteFunction:InvokeClient() can make the server depend on an untrusted client response and can yield indefinitely.".into(),
                    remediation: "Prefer a RemoteEvent when a response is unnecessary. If a response is required, design the server so client failure cannot block or corrupt authoritative state.".into(),
                },
            );
        }

        if code.contains("while true do") && !nearby_yield(&lines, index) {
            push_finding(
                findings,
                max_findings,
                LuauFinding {
                    rule_id: "RLX006",
                    severity: FindingSeverity::Warning,
                    path: display_path.clone(),
                    line: line_number,
                    message: "Potential unbounded `while true do` loop without an obvious yield nearby.".into(),
                    remediation: "Ensure every iteration yields or exits. Prefer an event-driven design when possible.".into(),
                },
            );
        }

        if side == ScriptSide::Server
            && (code.contains("OnServerEvent") || code.contains("OnServerInvoke"))
            && remote_window_has_sensitive_operation(&lines, index)
            && !remote_window_has_validation(&lines, index)
        {
            push_finding(
                findings,
                max_findings,
                LuauFinding {
                    rule_id: "RLX007",
                    severity: FindingSeverity::Warning,
                    path: display_path.clone(),
                    line: line_number,
                    message: "Remote handler is near a state-changing or cross-client operation without obvious validation markers. This is a heuristic finding.".into(),
                    remediation: "Validate client-supplied type/structure/value plus permission/context on the server, and rate-limit abuse-prone actions before changing authoritative state.".into(),
                },
            );
        }
    }
}

fn contains_legacy_scheduler_call(code: &str, pattern: &str) -> bool {
    let Some(position) = code.find(pattern) else {
        return false;
    };

    if position > 0 {
        let prefix = &code[..position];
        if prefix.ends_with('.') || prefix.chars().last().is_some_and(is_identifier_char) {
            return false;
        }
    }
    true
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn nearby_yield(lines: &[&str], start: usize) -> bool {
    lines
        .iter()
        .skip(start)
        .take(25)
        .map(|line| strip_line_comment(line).trim())
        .any(|line| {
            line.contains("task.wait(")
                || line.contains(":Wait(")
                || line.contains("Heartbeat:Wait(")
                || line.contains("Stepped:Wait(")
                || line.contains("RenderStepped:Wait(")
                || line == "break"
                || line.starts_with("break ")
        })
}

fn remote_window_has_sensitive_operation(lines: &[&str], start: usize) -> bool {
    lines
        .iter()
        .skip(start)
        .take(30)
        .map(|line| strip_line_comment(line))
        .any(|line| {
            [
                "SetAsync(",
                "UpdateAsync(",
                "IncrementAsync(",
                "RemoveAsync(",
                "leaderstats",
                "TakeDamage(",
                "FireAllClients(",
                ":Destroy(",
                ":PivotTo(",
                ".Value =",
            ]
            .iter()
            .any(|needle| line.contains(needle))
        })
}

fn remote_window_has_validation(lines: &[&str], start: usize) -> bool {
    lines
        .iter()
        .skip(start)
        .take(30)
        .map(|line| strip_line_comment(line))
        .any(|line| {
            [
                "typeof(",
                "type(",
                ":IsA(",
                ":IsDescendantOf(",
                "math.clamp(",
                "MaxActivationDistance",
                "cooldown",
                "rateLimit",
                "rate_limit",
                "permission",
                "authorized",
            ]
            .iter()
            .any(|needle| line.contains(needle))
        })
}

fn strip_line_comment(line: &str) -> &str {
    line.split_once("--").map_or(line, |(code, _)| code)
}

fn detect_side(path: &Path) -> ScriptSide {
    let normalized = path.to_string_lossy().replace('\\', "/").to_lowercase();
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase();

    if file_name.ends_with(".client.lua") || file_name.ends_with(".client.luau") {
        return ScriptSide::Client;
    }
    if file_name.ends_with(".server.lua") || file_name.ends_with(".server.luau") {
        return ScriptSide::Server;
    }

    if normalized.contains("/serverscriptservice/")
        || normalized.starts_with("serverscriptservice/")
        || normalized.contains("/serverstorage/")
        || normalized.starts_with("serverstorage/")
    {
        return ScriptSide::Server;
    }

    if [
        "starterplayer/",
        "startergui/",
        "starterpack/",
        "replicatedfirst/",
    ]
    .iter()
    .any(|marker| normalized.contains(marker) || normalized.starts_with(marker))
    {
        return ScriptSide::Client;
    }

    if normalized.contains("replicatedstorage/") {
        return ScriptSide::Shared;
    }

    ScriptSide::Unknown
}

fn is_replicated_storage_path(path: &Path) -> bool {
    let normalized = path.to_string_lossy().replace('\\', "/").to_lowercase();
    normalized.contains("replicatedstorage/")
}

fn is_luau_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("lua" | "luau")
    )
}

fn push_finding(findings: &mut Vec<LuauFinding>, max_findings: usize, finding: LuauFinding) {
    if findings.len() < max_findings {
        findings.push(finding);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn test_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "roldex-analysis-{label}-{}-{unique}",
            std::process::id()
        ))
    }

    #[test]
    fn flags_client_datastore_and_legacy_wait() {
        let root = test_dir("client");
        let scripts = root.join("StarterPlayer/StarterPlayerScripts");
        fs::create_dir_all(&scripts).expect("create scripts");
        fs::write(
            scripts.join("Data.client.luau"),
            "local DataStoreService = game:GetService(\"DataStoreService\")\nwait(1)\n",
        )
        .expect("write script");

        let report = analyze_luau(&root, 50).expect("analyze");
        assert_eq!(report.scanned_files, 1);
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.rule_id == "RLX001")
        );
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.rule_id == "RLX002")
        );

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn does_not_flag_task_wait_as_legacy_wait() {
        let root = test_dir("task-wait");
        fs::create_dir_all(&root).expect("create root");
        fs::write(root.join("Loop.luau"), "task.wait(1)\n").expect("write script");

        let report = analyze_luau(&root, 50).expect("analyze");
        assert!(
            !report
                .findings
                .iter()
                .any(|finding| finding.rule_id == "RLX001")
        );

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn does_not_flag_method_named_wait() {
        let root = test_dir("method-wait");
        fs::create_dir_all(&root).expect("create root");
        fs::write(root.join("Module.luau"), "object.wait(1)\n").expect("write script");

        let report = analyze_luau(&root, 50).expect("analyze");
        assert!(
            !report
                .findings
                .iter()
                .any(|finding| finding.rule_id == "RLX001")
        );

        fs::remove_dir_all(root).expect("cleanup");
    }
}
