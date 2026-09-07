use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

const PROJECT_SCAN_MAX_DEPTH: usize = 5;
const PROJECT_SCAN_MAX_ENTRIES: usize = 4_000;
const UNKNOWN_ROOT_SCAN_MAX_DEPTH: usize = 1;
const UNKNOWN_ROOT_SCAN_MAX_ENTRIES: usize = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectKind {
    Rojo,
    StudioExport,
    Luau,
    Unknown,
}

impl fmt::Display for ProjectKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rojo => write!(f, "Rojo project"),
            Self::StudioExport => write!(f, "Studio-style filesystem project"),
            Self::Luau => write!(f, "Luau project"),
            Self::Unknown => write!(f, "unknown project"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectSummary {
    pub root: PathBuf,
    pub kind: ProjectKind,
    pub luau_files: usize,
    pub lua_files: usize,
    pub rojo_project_files: usize,
}

impl ProjectSummary {
    pub fn detect(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        let studio_markers = [
            "ServerScriptService",
            "ReplicatedStorage",
            "StarterPlayer",
            "StarterGui",
            "Workspace",
        ];
        let has_studio_marker = studio_markers.iter().any(|name| root.join(name).is_dir());
        let has_root_rojo_marker = has_rojo_marker(&root)?;
        let looks_like_project =
            has_studio_marker || has_root_rojo_marker || has_source_marker(&root)?;

        let (max_depth, max_entries) = if looks_like_project {
            (PROJECT_SCAN_MAX_DEPTH, PROJECT_SCAN_MAX_ENTRIES)
        } else {
            (UNKNOWN_ROOT_SCAN_MAX_DEPTH, UNKNOWN_ROOT_SCAN_MAX_ENTRIES)
        };

        let mut stats = ScanStats::default();
        let mut scanned = 0usize;
        scan(&root, 0, max_depth, max_entries, &mut scanned, &mut stats)?;

        let kind = if has_root_rojo_marker || stats.rojo_project_files > 0 {
            ProjectKind::Rojo
        } else if has_studio_marker {
            ProjectKind::StudioExport
        } else if looks_like_project && stats.luau_files + stats.lua_files > 0 {
            ProjectKind::Luau
        } else {
            ProjectKind::Unknown
        };

        Ok(Self {
            root,
            kind,
            luau_files: stats.luau_files,
            lua_files: stats.lua_files,
            rojo_project_files: stats.rojo_project_files,
        })
    }

    pub fn describe(&self) -> String {
        format!(
            "Root: {}\nProject type: {}\nLuau files: {}\nLua files: {}\nRojo project files: {}",
            self.root.display(),
            self.kind,
            self.luau_files,
            self.lua_files,
            self.rojo_project_files
        )
    }
}

#[derive(Default)]
struct ScanStats {
    luau_files: usize,
    lua_files: usize,
    rojo_project_files: usize,
}

fn has_rojo_marker(root: &Path) -> Result<bool> {
    for entry in fs::read_dir(root).with_context(|| format!("failed to scan {}", root.display()))? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            let name = entry.file_name();
            if name.to_string_lossy().ends_with(".project.json") {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn has_source_marker(root: &Path) -> Result<bool> {
    for name in ["src", "game", "scripts", "packages"] {
        if root.join(name).is_dir() {
            return Ok(true);
        }
    }

    for entry in fs::read_dir(root).with_context(|| format!("failed to scan {}", root.display()))? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        match entry.path().extension().and_then(|value| value.to_str()) {
            Some("luau" | "lua") => return Ok(true),
            _ => {}
        }
    }
    Ok(false)
}

fn should_skip_dir(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | "target"
            | "node_modules"
            | ".roldex"
            | ".cargo"
            | ".rustup"
            | "AppData"
            | "Application Data"
            | "Local Settings"
            | "Temp"
            | "tmp"
            | "$Recycle.Bin"
            | "System Volume Information"
    )
}

fn scan(
    path: &Path,
    depth: usize,
    max_depth: usize,
    max_entries: usize,
    scanned: &mut usize,
    stats: &mut ScanStats,
) -> Result<()> {
    if depth > max_depth || *scanned >= max_entries {
        return Ok(());
    }

    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) if depth > 0 && error.kind() == std::io::ErrorKind::PermissionDenied => {
            return Ok(());
        }
        Err(error) => {
            return Err(error).with_context(|| format!("failed to scan {}", path.display()));
        }
    };

    for entry in entries {
        if *scanned >= max_entries {
            break;
        }

        let entry = entry?;
        *scanned += 1;
        let child = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();

        if entry.file_type()?.is_dir() {
            if should_skip_dir(&name) {
                continue;
            }
            scan(&child, depth + 1, max_depth, max_entries, scanned, stats)?;
            continue;
        }

        match child.extension().and_then(|value| value.to_str()) {
            Some("luau") => stats.luau_files += 1,
            Some("lua") => stats.lua_files += 1,
            Some("json") if name.ends_with(".project.json") => stats.rojo_project_files += 1,
            _ => {}
        }
    }

    Ok(())
}

pub fn project_tree(root: &Path, max_depth: usize, max_entries: usize) -> Result<String> {
    let mut out = String::new();
    let mut count = 0usize;
    tree_inner(root, root, 0, max_depth, max_entries, &mut count, &mut out)?;
    if count >= max_entries {
        out.push_str("… tree truncated\n");
    }
    Ok(out)
}

fn tree_inner(
    root: &Path,
    current: &Path,
    depth: usize,
    max_depth: usize,
    max_entries: usize,
    count: &mut usize,
    out: &mut String,
) -> Result<()> {
    if depth > max_depth || *count >= max_entries {
        return Ok(());
    }

    let mut entries = fs::read_dir(current)
        .with_context(|| format!("failed to read {}", current.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        if *count >= max_entries {
            break;
        }

        let name = entry.file_name();
        let name = name.to_string_lossy();
        if should_skip_dir(&name) {
            continue;
        }

        let entry_path = entry.path();
        let relative = entry_path
            .strip_prefix(root)
            .unwrap_or(&entry_path)
            .to_path_buf();
        out.push_str(&format!("{}{}\n", "  ".repeat(depth), relative.display()));
        *count += 1;

        if entry.file_type()?.is_dir() {
            tree_inner(
                root,
                &entry_path,
                depth + 1,
                max_depth,
                max_entries,
                count,
                out,
            )?;
        }
    }

    Ok(())
}
