use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::PermissionMode;

const MAX_TEXT_FILE_BYTES: u64 = 256 * 1024;

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

    pub fn read_text(&self, relative: impl AsRef<Path>) -> Result<String> {
        let path = self.resolve(relative.as_ref())?;
        let metadata = fs::metadata(&path)
            .with_context(|| format!("failed to inspect {}", path.display()))?;
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
        let path = self.resolve(relative.as_ref())?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&path, content).with_context(|| format!("failed to write {}", path.display()))
    }

    pub fn delete_file(&self, relative: impl AsRef<Path>) -> Result<()> {
        self.require_write()?;
        let path = self.resolve(relative.as_ref())?;
        fs::remove_file(&path).with_context(|| format!("failed to delete {}", path.display()))
    }

    fn require_write(&self) -> Result<()> {
        if self.mode == PermissionMode::ReadOnly {
            bail!("Roldex is in read-only mode");
        }
        Ok(())
    }

    fn resolve(&self, relative: &Path) -> Result<PathBuf> {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_parent_traversal() {
        let fs = WorkspaceFs {
            root: PathBuf::from("."),
            mode: PermissionMode::Workspace,
        };
        assert!(fs.resolve(Path::new("../secret.txt")).is_err());
    }
}
