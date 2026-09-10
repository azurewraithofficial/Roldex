#[allow(clippy::type_complexity)]
mod agent;
mod analysis;
mod computer;
mod config;
mod docs;
mod git;
mod media;
mod project;
mod project_intel;
mod prompt;
mod provider;
mod search;
mod studio;
mod tools;
mod workspace_fs;

pub use agent::Agent;
pub use analysis::{FindingSeverity, LuauAnalysisReport, LuauFinding, analyze_luau};
pub use config::{
    AiConfig, Config, DEFAULT_LOCAL_AI_ENDPOINT, DEFAULT_LOCAL_AI_MODEL, PermissionConfig,
    PermissionMode, UiConfig,
};
pub use project::{ProjectKind, ProjectSummary, project_tree};
pub use provider::ToolCall;
pub use studio::{StudioBroker, StudioCommand, StudioCommandResult};
pub use tools::AgentEvent;
pub use workspace_fs::WorkspaceFs;
