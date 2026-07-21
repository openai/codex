use crate::sessions::ExternalAgentSessionMigration;
use std::path::PathBuf;
use std::time::Duration;

const DEFAULT_SESSION_IMPORT_MAX_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const DEFAULT_SESSION_IMPORT_MAX_COUNT: usize = 50;

/// Bounds session discovery for an external-agent import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalAgentSessionImportLimits {
    /// Oldest source-session modification age that remains eligible.
    pub max_age: Duration,
    /// Maximum number of eligible sessions returned by detection.
    pub max_sessions: usize,
}

impl Default for ExternalAgentSessionImportLimits {
    fn default() -> Self {
        Self {
            max_age: DEFAULT_SESSION_IMPORT_MAX_AGE,
            max_sessions: DEFAULT_SESSION_IMPORT_MAX_COUNT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalAgentConfigDetectOptions {
    pub include_home: bool,
    pub include_memory: bool,
    pub cwds: Option<Vec<PathBuf>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalAgentConfigMigrationItemType {
    Config,
    Skills,
    AgentsMd,
    Plugins,
    McpServerConfig,
    Subagents,
    Hooks,
    Commands,
    Memory,
    Sessions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginsMigration {
    pub marketplace_name: String,
    pub plugin_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedMigration {
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrationDetails {
    pub plugins: Vec<PluginsMigration>,
    pub skills: Vec<NamedMigration>,
    pub sessions: Vec<ExternalAgentSessionMigration>,
    pub mcp_servers: Vec<NamedMigration>,
    pub hooks: Vec<NamedMigration>,
    pub subagents: Vec<NamedMigration>,
    pub commands: Vec<NamedMigration>,
    pub memory: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingPluginImport {
    pub cwd: Option<PathBuf>,
    pub description: String,
    pub details: MigrationDetails,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginImportOutcome {
    pub succeeded_marketplaces: Vec<String>,
    pub succeeded_plugin_ids: Vec<String>,
    pub failed_marketplaces: Vec<String>,
    pub failed_plugin_ids: Vec<String>,
    pub raw_errors: Vec<ExternalAgentConfigImportRawError>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExternalAgentConfigImportOutcome {
    pub pending_plugin_imports: Vec<PendingPluginImport>,
    pub item_results: Vec<ExternalAgentConfigImportItemResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalAgentConfigImportItemResult {
    pub item_type: ExternalAgentConfigMigrationItemType,
    pub description: String,
    pub cwd: Option<PathBuf>,
    pub success_count: u32,
    pub error_count: u32,
    pub successes: Vec<ExternalAgentConfigImportSuccess>,
    pub raw_errors: Vec<ExternalAgentConfigImportRawError>,
}

impl ExternalAgentConfigImportItemResult {
    pub fn new(
        item_type: ExternalAgentConfigMigrationItemType,
        description: String,
        cwd: Option<PathBuf>,
    ) -> Self {
        Self {
            item_type,
            description,
            cwd,
            success_count: 0,
            error_count: 0,
            successes: Vec::new(),
            raw_errors: Vec::new(),
        }
    }

    pub fn record_error(&mut self, raw_error: ExternalAgentConfigImportRawError) {
        self.error_count = self.error_count.saturating_add(1);
        self.raw_errors.push(raw_error);
    }

    pub fn record_success(&mut self, source: Option<String>, target: Option<String>) {
        self.success_count = self.success_count.saturating_add(1);
        self.successes.push(ExternalAgentConfigImportSuccess {
            item_type: self.item_type,
            cwd: self.cwd.clone(),
            source,
            target,
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalAgentConfigImportSuccess {
    pub item_type: ExternalAgentConfigMigrationItemType,
    pub cwd: Option<PathBuf>,
    pub source: Option<String>,
    pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalAgentConfigImportRawError {
    pub item_type: ExternalAgentConfigMigrationItemType,
    pub error_type: Option<String>,
    pub sub_error_type: Option<String>,
    pub failure_stage: String,
    pub message: String,
    pub cwd: Option<PathBuf>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalAgentConfigMigrationItem {
    pub item_type: ExternalAgentConfigMigrationItemType,
    pub description: String,
    pub cwd: Option<PathBuf>,
    pub details: Option<MigrationDetails>,
}
