use std::env;
use std::fmt;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const DEFAULT_LOCAL_AI_ENDPOINT: &str = "http://127.0.0.1:8080/v1/chat/completions";
pub const DEFAULT_LOCAL_AI_MODEL: &str = "local-model";

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

        let mut config = if let Some(path) = path {
            let raw = fs::read_to_string(&path)
                .with_context(|| format!("failed to read configuration {}", path.display()))?;
            toml::from_str(&raw)
                .with_context(|| format!("invalid configuration {}", path.display()))?
        } else {
            Self::default()
        };

        if env::var("ROLDEX_AI_MODE")
            .is_ok_and(|mode| mode.trim().eq_ignore_ascii_case("local"))
        {
            config.ai.activate_local(None, None);
        }

        if let Ok(provider) = env::var("ROLDEX_AI_PROVIDER") {
            if !provider.trim().is_empty() {
                config.ai.provider = provider.trim().to_string();
            }
        }
        if let Ok(endpoint) = env::var("ROLDEX_AI_ENDPOINT") {
            if !endpoint.trim().is_empty() {
                config.ai.endpoint = endpoint.trim().to_string();
            }
        }
        if let Ok(model) = env::var("ROLDEX_MODEL") {
            if !model.trim().is_empty() {
                config.ai.model = model.trim().to_string();
            }
        }
        if let Ok(api_key_env) = env::var("ROLDEX_API_KEY_ENV") {
            config.ai.api_key_env = api_key_env.trim().to_string();
        }
        if let Ok(raw) = env::var("ROLDEX_AI_TIMEOUT_SECONDS") {
            if let Ok(seconds) = raw.parse::<u64>() {
                config.ai.request_timeout_seconds = seconds.clamp(15, 600);
            }
        }
        if let Ok(sort) = env::var("ROLDEX_OPENROUTER_SORT") {
            let sort = sort.trim().to_ascii_lowercase();
            if matches!(sort.as_str(), "latency" | "throughput" | "price") {
                config.ai.openrouter_sort = sort;
            }
        }

        Ok(config)
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
    pub request_timeout_seconds: u64,
    pub max_retries: u32,
    pub openrouter_sort: String,
}

impl AiConfig {
    pub fn is_local(&self) -> bool {
        self.provider.eq_ignore_ascii_case("local")
    }

    pub fn requires_api_key(&self) -> bool {
        !self.is_local() && !self.api_key_env.trim().is_empty()
    }

    pub fn activate_local(&mut self, endpoint: Option<&str>, model: Option<&str>) {
        self.provider = "local".into();
        self.endpoint = endpoint
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(DEFAULT_LOCAL_AI_ENDPOINT)
            .to_string();
        self.model = model
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(DEFAULT_LOCAL_AI_MODEL)
            .to_string();
        self.api_key_env.clear();
        self.request_timeout_seconds = self.request_timeout_seconds.max(180);
        self.max_retries = self.max_retries.min(1);
    }
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: "openrouter".into(),
            endpoint: "https://openrouter.ai/api/v1/chat/completions".into(),
            model: "openrouter/free".into(),
            api_key_env: "OPENROUTER_API_KEY".into(),
            temperature: 0.2,
            request_timeout_seconds: 75,
            max_retries: 2,
            openrouter_sort: "latency".into(),
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
            compact: true,
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
        assert_eq!(config.ai.request_timeout_seconds, 75);
        assert_eq!(config.ai.max_retries, 2);
        assert_eq!(config.ai.openrouter_sort, "latency");
        assert!(config.ai.requires_api_key());
    }

    #[test]
    fn local_mode_needs_no_api_key() {
        let mut ai = AiConfig::default();
        ai.activate_local(None, None);
        assert!(ai.is_local());
        assert!(!ai.requires_api_key());
        assert_eq!(ai.endpoint, DEFAULT_LOCAL_AI_ENDPOINT);
        assert_eq!(ai.model, DEFAULT_LOCAL_AI_MODEL);
        assert!(ai.api_key_env.is_empty());
    }

    #[test]
    fn local_mode_accepts_custom_endpoint_and_model() {
        let mut ai = AiConfig::default();
        ai.activate_local(
            Some("http://127.0.0.1:9000/v1/chat/completions"),
            Some("qwen3-coder"),
        );
        assert_eq!(
            ai.endpoint,
            "http://127.0.0.1:9000/v1/chat/completions"
        );
        assert_eq!(ai.model, "qwen3-coder");
    }
}
