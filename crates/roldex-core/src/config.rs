use std::fmt;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub ai: AiConfig,
    pub permissions: PermissionConfig,
    pub ui: UiConfig,
}

impl Config {
    pub fn load(explicit_path: Option<&Path>) -> Result<Self> {
        let path = explicit_path.map(ToOwned::to_owned).or_else(|| {
            Path::new("roldex.toml")
                .exists()
                .then(|| Path::new("roldex.toml").to_path_buf())
        });

        let Some(path) = path else {
            return Ok(Self::default());
        };

        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read configuration {}", path.display()))?;
        toml::from_str(&raw).with_context(|| format!("invalid configuration {}", path.display()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    pub provider: String,
    pub endpoint: String,
    pub model: String,
    pub api_key_env: String,
    pub temperature: f32,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: "openrouter".into(),
            endpoint: "https://openrouter.ai/api/v1/chat/completions".into(),
            model: "openrouter/free".into(),
            api_key_env: "OPENROUTER_API_KEY".into(),
            temperature: 0.2,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    ReadOnly,
    #[default]
    Workspace,
    FullAccess,
}

impl fmt::Display for PermissionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadOnly => write!(f, "read-only"),
            Self::Workspace => write!(f, "workspace"),
            Self::FullAccess => write!(f, "full-access"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PermissionConfig {
    pub mode: PermissionMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub show_progress: bool,
    pub compact: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            show_progress: true,
            compact: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_workspace_scoped_and_free_router_first() {
        let config = Config::default();
        assert_eq!(config.permissions.mode, PermissionMode::Workspace);
        assert_eq!(config.ai.model, "openrouter/free");
    }
}
