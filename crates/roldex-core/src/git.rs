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
        &["--literal-pathspecs", "restore", "--staged", "--", path],
    )?;
    git_path_status(root, path)
}

pub fn git_restore_worktree_file(root: &Path, path: &str) -> Result<String> {
    run_git(
        root,
        &["--literal-pathspecs", "restore", "--worktree", "--", path],
    )?;
    git_path_status(root, path)
}

fn git_path_status(root: &Path, path: &str) -> Result<String> {
    let status = run_git(
        root,
        &["--literal-pathspecs", "status", "--short", "--", path],
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn test_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "roldex-git-{label}-{}-{unique}",
            std::process::id()
        ))
    }

    fn test_git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn init_repo(label: &str) -> PathBuf {
        let root = test_dir(label);
        fs::create_dir_all(&root).expect("create repo");
        test_git(&root, &["init", "--quiet"]);
        test_git(
            &root,
            &["config", "user.email", "roldex-tests@example.invalid"],
        );
        test_git(&root, &["config", "user.name", "Roldex Tests"]);
        fs::write(root.join("Main.luau"), "original\n").expect("seed file");
        test_git(&root, &["add", "--", "Main.luau"]);
        test_git(&root, &["commit", "--quiet", "-m", "initial"]);
        root
    }

    #[test]
    fn restore_worktree_discards_only_unstaged_changes() {
        let root = init_repo("restore");
        fs::write(root.join("Main.luau"), "staged\n").expect("write staged change");
        test_git(&root, &["add", "--", "Main.luau"]);
        fs::write(root.join("Main.luau"), "unstaged\n").expect("write unstaged change");

        git_restore_worktree_file(&root, "Main.luau").expect("restore worktree");

        assert_eq!(
            fs::read_to_string(root.join("Main.luau")).expect("read file"),
            "staged\n"
        );
        assert_eq!(git_diff(&root, Some("Main.luau"), false).expect("diff"), "");
        assert!(
            !git_diff(&root, Some("Main.luau"), true)
                .expect("staged diff")
                .is_empty()
        );

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn unstage_preserves_worktree_contents() {
        let root = init_repo("unstage");
        fs::write(root.join("Main.luau"), "changed\n").expect("write change");
        test_git(&root, &["add", "--", "Main.luau"]);

        git_unstage_file(&root, "Main.luau").expect("unstage");

        assert_eq!(
            fs::read_to_string(root.join("Main.luau")).expect("read file"),
            "changed\n"
        );
        assert!(
            !git_diff(&root, Some("Main.luau"), false)
                .expect("diff")
                .is_empty()
        );
        assert_eq!(
            git_diff(&root, Some("Main.luau"), true).expect("staged diff"),
            ""
        );

        fs::remove_dir_all(root).expect("cleanup");
    }
}
