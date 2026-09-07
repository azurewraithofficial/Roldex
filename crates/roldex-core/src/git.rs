use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

const MAX_GIT_OUTPUT_BYTES: usize = 192 * 1024;

pub fn git_status(root: &Path) -> Result<String> {
    run_git(root, &["status", "--short", "--branch"])
}

pub fn git_diff(root: &Path, path: Option<&str>, staged: bool) -> Result<String> {
    let mut args = vec![
        "--no-pager",
        "diff",
        "--no-ext-diff",
        "--no-textconv",
        "--color=never",
    ];
    if staged {
        args.push("--cached");
    }

    match path {
        Some(path) => {
            args.insert(0, "--literal-pathspecs");
            args.push("--");
            args.push(path);
            run_git(root, &args)
        }
        None => {
            args.push("--stat");
            let summary = run_git(root, &args)?;
            if summary.trim().is_empty() {
                Ok("(no diff)".into())
            } else {
                Ok(summary)
            }
        }
    }
}

pub fn git_unstage_file(root: &Path, path: &str) -> Result<String> {
    run_git(
        root,
        &[
            "--literal-pathspecs",
            "restore",
            "--staged",
            "--",
            path,
        ],
    )?;
    git_path_status(root, path)
}

pub fn git_restore_worktree_file(root: &Path, path: &str) -> Result<String> {
    run_git(
        root,
        &[
            "--literal-pathspecs",
            "restore",
            "--worktree",
            "--",
            path,
        ],
    )?;
    git_path_status(root, path)
}

fn git_path_status(root: &Path, path: &str) -> Result<String> {
    let status = run_git(
        root,
        &[
            "--literal-pathspecs",
            "status",
            "--short",
            "--",
            path,
        ],
    )?;
    if status.trim().is_empty() {
        Ok(format!("{path}: clean"))
    } else {
        Ok(status)
    }
}

fn run_git(root: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .context("failed to run git; make sure Git is installed")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git command failed: {}", truncate_output(stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(truncate_output(stdout.trim()))
}

fn truncate_output(value: &str) -> String {
    if value.len() <= MAX_GIT_OUTPUT_BYTES {
        return value.to_owned();
    }

    let mut end = MAX_GIT_OUTPUT_BYTES;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n… git output truncated", &value[..end])
}
