use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::PermissionMode;

const MAX_TEXT_FILE_BYTES: u64 = 256 * 1024;
const MAX_WRITE_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone)]
pub struct WorkspaceFs {
    root: PathBuf,
    mode: PermissionMode,
}

impl WorkspaceFs {
    pub fn new(root: impl AsRef<Path>, mode: PermissionMode) -> Result<Self> {
        let root = root
            .as_ref()
            .canonicalize()
            .with_context(|| format!("failed to open workspace {}", root.as_ref().display()))?;
        Ok(Self { root, mode })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn validate_relative_path(&self, relative: impl AsRef<Path>) -> Result<PathBuf> {
        self.resolve_lexical(relative.as_ref())
    }

    pub fn ensure_writable(&self) -> Result<()> {
        self.require_write()
    }

    pub fn read_text(&self, relative: impl AsRef<Path>) -> Result<String> {
        let path = self.resolve_existing(relative.as_ref())?;
        let metadata =
            fs::metadata(&path).with_context(|| format!("failed to inspect {}", path.display()))?;
        if metadata.len() > MAX_TEXT_FILE_BYTES {
            bail!(
                "{} is too large for one read ({} bytes; limit is {} bytes)",
                path.display(),
                metadata.len(),
                MAX_TEXT_FILE_BYTES
            );
        }
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))
    }

    pub fn write_text(&self, relative: impl AsRef<Path>, content: &str) -> Result<()> {
        self.require_write()?;
        if content.len() > MAX_WRITE_BYTES {
            bail!(
                "refusing one write larger than {} bytes; split the change into smaller files",
                MAX_WRITE_BYTES
            );
        }

        let path = self.resolve_for_write(relative.as_ref())?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&path, content).with_context(|| format!("failed to write {}", path.display()))
    }

    pub fn replace_text(
        &self,
        relative: impl AsRef<Path>,
        old_text: &str,
        new_text: &str,
    ) -> Result<()> {
        self.require_write()?;
        if old_text.is_empty() {
            bail!("old_text cannot be empty");
        }

        let relative = relative.as_ref();
        let content = self.read_text(relative)?;
        let occurrences = content.matches(old_text).count();
        if occurrences != 1 {
            bail!(
                "expected old_text to occur exactly once, but found {occurrences}; read the file again and use a more specific match"
            );
        }

        let updated = content.replacen(old_text, new_text, 1);
        self.write_text(relative, &updated)
    }

    pub fn delete_file(&self, relative: impl AsRef<Path>) -> Result<()> {
        self.require_write()?;
        let path = self.resolve_existing(relative.as_ref())?;
        fs::remove_file(&path).with_context(|| format!("failed to delete {}", path.display()))
    }

    fn require_write(&self) -> Result<()> {
        if self.mode == PermissionMode::ReadOnly {
            bail!("Roldex is in read-only mode");
        }
        Ok(())
    }

    fn resolve_lexical(&self, relative: &Path) -> Result<PathBuf> {
        if relative.as_os_str().is_empty() {
            bail!("path cannot be empty");
        }
        if relative.is_absolute() {
            bail!("absolute paths are not allowed in workspace tools");
        }
        if relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            bail!("path traversal outside the workspace is not allowed");
        }
        Ok(self.root.join(relative))
    }

    fn resolve_existing(&self, relative: &Path) -> Result<PathBuf> {
        let path = self.resolve_lexical(relative)?;
        let canonical = path
            .canonicalize()
            .with_context(|| format!("failed to resolve {}", path.display()))?;
        self.require_inside_root(&canonical)?;
        Ok(canonical)
    }

    fn resolve_for_write(&self, relative: &Path) -> Result<PathBuf> {
        let path = self.resolve_lexical(relative)?;

        if path.exists() {
            let canonical = path
                .canonicalize()
                .with_context(|| format!("failed to resolve {}", path.display()))?;
            self.require_inside_root(&canonical)?;
            return Ok(canonical);
        }

        let mut ancestor = path.parent().context("file path has no parent")?;
        while !ancestor.exists() {
            ancestor = ancestor
                .parent()
                .context("could not find an existing workspace ancestor")?;
        }

        let canonical_ancestor = ancestor
            .canonicalize()
            .with_context(|| format!("failed to resolve {}", ancestor.display()))?;
        self.require_inside_root(&canonical_ancestor)?;
        Ok(path)
    }

    fn require_inside_root(&self, path: &Path) -> Result<()> {
        if !path.starts_with(&self.root) {
            bail!(
                "resolved path {} escapes workspace {}",
                path.display(),
                self.root.display()
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn test_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("roldex-{label}-{}-{unique}", std::process::id()))
    }

    #[test]
    fn rejects_parent_traversal() {
        let root = test_dir("traversal");
        fs::create_dir_all(&root).expect("create root");
        let workspace = WorkspaceFs::new(&root, PermissionMode::Workspace).expect("workspace");
        assert!(workspace.read_text("../secret.txt").is_err());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn exact_replace_requires_one_match() {
        let root = test_dir("replace");
        fs::create_dir_all(&root).expect("create root");
        fs::write(root.join("Main.luau"), "local value = 1\n").expect("seed file");
        let workspace = WorkspaceFs::new(&root, PermissionMode::Workspace).expect("workspace");

        workspace
            .replace_text("Main.luau", "value = 1", "value = 2")
            .expect("replace");
        assert_eq!(
            workspace.read_text("Main.luau").expect("read"),
            "local value = 2\n"
        );
        assert!(
            workspace
                .replace_text("Main.luau", "missing", "replacement")
                .is_err()
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escape() {
        use std::os::unix::fs::symlink;

        let root = test_dir("symlink-root");
        let outside = test_dir("symlink-outside");
        fs::create_dir_all(&root).expect("create root");
        fs::create_dir_all(&outside).expect("create outside");
        fs::write(outside.join("secret.txt"), "outside").expect("seed outside");
        symlink(&outside, root.join("escape")).expect("create symlink");

        let workspace = WorkspaceFs::new(&root, PermissionMode::Workspace).expect("workspace");
        assert!(workspace.read_text("escape/secret.txt").is_err());
        assert!(workspace.write_text("escape/new.txt", "blocked").is_err());

        fs::remove_dir_all(root).expect("cleanup root");
        fs::remove_dir_all(outside).expect("cleanup outside");
    }
}
