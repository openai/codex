//! Types used to define loaded and effective Codex configuration values.

// Note this file should generally be restricted to simple struct/enum
// definitions that do not contain business logic.

pub use crate::mcp_types::AppToolApproval;
pub use crate::mcp_types::McpServerConfig;
pub use crate::mcp_types::McpServerDisabledReason;
pub use crate::mcp_types::McpServerToolConfig;
pub use crate::mcp_types::McpServerTransportConfig;
pub use crate::mcp_types::RawMcpServerConfig;
pub use codex_protocol::config_types::AltScreenMode;
pub use codex_protocol::config_types::ApprovalsReviewer;
pub use codex_protocol::config_types::ModeKind;
pub use codex_protocol::config_types::Personality;
pub use codex_protocol::config_types::ServiceTier;
pub use codex_protocol::config_types::WebSearchMode;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fmt;
use wildmatch::WildMatchPattern;

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

pub const DEFAULT_OTEL_ENVIRONMENT: &str = "dev";
pub const DEFAULT_MEMORIES_MAX_ROLLOUTS_PER_STARTUP: usize = 16;
pub const DEFAULT_MEMORIES_MAX_ROLLOUT_AGE_DAYS: i64 = 30;
pub const DEFAULT_MEMORIES_MIN_ROLLOUT_IDLE_HOURS: i64 = 6;
pub const DEFAULT_MEMORIES_MAX_RAW_MEMORIES_FOR_CONSOLIDATION: usize = 256;
pub const DEFAULT_MEMORIES_MAX_UNUSED_DAYS: i64 = 30;

const fn default_enabled() -> bool {
    true
}

/// Determine where Codex should store CLI auth credentials.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AuthCredentialsStoreMode {
    #[default]
    /// Persist credentials in CODEX_HOME/auth.json.
    File,
    /// Persist credentials in the keyring. Fail if unavailable.
    Keyring,
    /// Use keyring when available; otherwise, fall back to a file in CODEX_HOME.
    Auto,
    /// Store credentials in memory only for the current process.
    Ephemeral,
}

/// Determine where Codex should store and read MCP credentials.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum OAuthCredentialsStoreMode {
    /// `Keyring` when available; otherwise, `File`.
    /// Credentials stored in the keyring will only be readable by Codex unless the user explicitly grants access via OS-level keyring access.
    #[default]
    Auto,
    /// CODEX_HOME/.credentials.json
    /// This file will be readable to Codex and other applications running as the same user.
    File,
    /// Keyring when available, otherwise fail.
    Keyring,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum WindowsSandboxModeToml {
    Elevated,
    Unelevated,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct WindowsToml {
    pub sandbox: Option<WindowsSandboxModeToml>,
    /// Defaults to `true`. Set to `false` to launch the final sandboxed child
    /// process on `Winsta0\\Default` instead of a private desktop.
    pub sandbox_private_desktop: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, JsonSchema)]
pub enum UriBasedFileOpener {
    #[serde(rename = "vscode")]
    VsCode,

    #[serde(rename = "vscode-insiders")]
    VsCodeInsiders,

    #[serde(rename = "windsurf")]
    Windsurf,

    #[serde(rename = "cursor")]
    Cursor,

    /// Option to disable the URI-based file opener.
    #[serde(rename = "none")]
    None,
}

impl UriBasedFileOpener {
    pub fn get_scheme(&self) -> Option<&str> {
        match self {
            UriBasedFileOpener::VsCode => Some("vscode"),
            UriBasedFileOpener::VsCodeInsiders => Some("vscode-insiders"),
            UriBasedFileOpener::Windsurf => Some("windsurf"),
            UriBasedFileOpener::Cursor => Some("cursor"),
            UriBasedFileOpener::None => None,
        }
    }
}

/// Settings that govern if and what will be written to `~/.codex/history.jsonl`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct History {
    /// If true, history entries will not be written to disk.
    pub persistence: HistoryPersistence,

    /// If set, the maximum size of the history file in bytes. The oldest entries
    /// are dropped once the file exceeds this limit.
    pub max_bytes: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Default, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum HistoryPersistence {
    /// Save all history entries to disk.
    #[default]
    SaveAll,
    /// Do not write history to disk.
    None,
}

// ===== Analytics configuration =====

/// Analytics settings loaded from config.toml. Fields are optional so we can apply defaults.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct AnalyticsConfigToml {
    /// When `false`, disables analytics across Codex product surfaces in this profile.
    pub enabled: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct FeedbackConfigToml {
    /// When `false`, disables the feedback flow across Codex product surfaces.
    pub enabled: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ToolSuggestDiscoverableType {
    Connector,
    Plugin,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ToolSuggestDiscoverable {
    #[serde(rename = "type")]
    pub kind: ToolSuggestDiscoverableType,
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ToolSuggestConfig {
    #[serde(default)]
    pub discoverables: Vec<ToolSuggestDiscoverable>,
}

/// Memories settings loaded from config.toml.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct MemoriesToml {
    /// When `true`, web searches and MCP tool calls mark the thread `memory_mode` as `"polluted"`.
    pub no_memories_if_mcp_or_web_search: Option<bool>,
    /// When `false`, newly created threads are stored with `memory_mode = "disabled"` in the state DB.
    pub generate_memories: Option<bool>,
    /// When `false`, skip injecting memory usage instructions into developer prompts.
    pub use_memories: Option<bool>,
    /// Maximum number of recent raw memories retained for global consolidation.
    pub max_raw_memories_for_consolidation: Option<usize>,
    /// Maximum number of days since a memory was last used before it becomes ineligible for phase 2 selection.
    pub max_unused_days: Option<i64>,
    /// Maximum age of the threads used for memories.
    pub max_rollout_age_days: Option<i64>,
    /// Maximum number of rollout candidates processed per pass.
    pub max_rollouts_per_startup: Option<usize>,
    /// Minimum idle time between last thread activity and memory creation (hours). > 12h recommended.
    pub min_rollout_idle_hours: Option<i64>,
    /// Model used for thread summarisation.
    pub extract_model: Option<String>,
    /// Model used for memory consolidation.
    pub consolidation_model: Option<String>,
}

/// Session title settings loaded from config.toml.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct SessionTitlesToml {
    /// When `false`, skip generating a default `/rename` title for new interactive sessions.
    pub enabled: Option<bool>,
    /// Model used for default session title generation.
    pub model: Option<String>,
}

/// Effective session title settings after defaults are applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionTitlesConfig {
    /// Whether interactive sessions should ask an auxiliary model for a default title.
    pub enabled: bool,
    /// Optional model override for the title-only background request.
    pub model: Option<String>,
}

impl Default for SessionTitlesConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            model: None,
        }
    }
}

impl From<SessionTitlesToml> for SessionTitlesConfig {
    fn from(toml: SessionTitlesToml) -> Self {
        let defaults = Self::default();
        Self {
            enabled: toml.enabled.unwrap_or(defaults.enabled),
            model: toml.model,
        }
    }
}

/// Effective memories settings after defaults are applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoriesConfig {
    pub no_memories_if_mcp_or_web_search: bool,
    pub generate_memories: bool,
    pub use_memories: bool,
    pub max_raw_memories_for_consolidation: usize,
    pub max_unused_days: i64,
    pub max_rollout_age_days: i64,
    pub max_rollouts_per_startup: usize,
    pub min_rollout_idle_hours: i64,
    pub extract_model: Option<String>,
    pub consolidation_model: Option<String>,
}

impl Default for MemoriesConfig {
    fn default() -> Self {
        Self {
            no_memories_if_mcp_or_web_search: false,
            generate_memories: true,
            use_memories: true,
            max_raw_memories_for_consolidation: DEFAULT_MEMORIES_MAX_RAW_MEMORIES_FOR_CONSOLIDATION,
            max_unused_days: DEFAULT_MEMORIES_MAX_UNUSED_DAYS,
            max_rollout_age_days: DEFAULT_MEMORIES_MAX_ROLLOUT_AGE_DAYS,
            max_rollouts_per_startup: DEFAULT_MEMORIES_MAX_ROLLOUTS_PER_STARTUP,
            min_rollout_idle_hours: DEFAULT_MEMORIES_MIN_ROLLOUT_IDLE_HOURS,
            extract_model: None,
            consolidation_model: None,
        }
    }
}

impl From<MemoriesToml> for MemoriesConfig {
    fn from(toml: MemoriesToml) -> Self {
        let defaults = Self::default();
        Self {
            no_memories_if_mcp_or_web_search: toml
                .no_memories_if_mcp_or_web_search
                .unwrap_or(defaults.no_memories_if_mcp_or_web_search),
            generate_memories: toml.generate_memories.unwrap_or(defaults.generate_memories),
            use_memories: toml.use_memories.unwrap_or(defaults.use_memories),
            max_raw_memories_for_consolidation: toml
                .max_raw_memories_for_consolidation
                .unwrap_or(defaults.max_raw_memories_for_consolidation)
                .min(4096),
            max_unused_days: toml
                .max_unused_days
                .unwrap_or(defaults.max_unused_days)
                .clamp(0, 365),
            max_rollout_age_days: toml
                .max_rollout_age_days
                .unwrap_or(defaults.max_rollout_age_days)
                .clamp(0, 90),
            max_rollouts_per_startup: toml
                .max_rollouts_per_startup
                .unwrap_or(defaults.max_rollouts_per_startup)
                .min(128),
            min_rollout_idle_hours: toml
                .min_rollout_idle_hours
                .unwrap_or(defaults.min_rollout_idle_hours)
                .clamp(1, 48),
            extract_model: toml.extract_model,
            consolidation_model: toml.consolidation_model,
        }
    }
}

/// Default settings that apply to all apps.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct AppsDefaultConfig {
    /// When `false`, apps are disabled unless overridden by per-app settings.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Whether tools with `destructive_hint = true` are allowed by default.
    #[serde(
        default = "default_enabled",
        skip_serializing_if = "std::clone::Clone::clone"
    )]
    pub destructive_enabled: bool,

    /// Whether tools with `open_world_hint = true` are allowed by default.
    #[serde(
        default = "default_enabled",
        skip_serializing_if = "std::clone::Clone::clone"
    )]
    pub open_world_enabled: bool,
}

/// Per-tool settings for a single app tool.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct AppToolConfig {
    /// Whether this tool is enabled. `Some(true)` explicitly allows this tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    /// Approval mode for this tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_mode: Option<AppToolApproval>,
}

/// Tool settings for a single app.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct AppToolsConfig {
    /// Per-tool overrides keyed by tool name (for example `repos/list`).
    #[serde(default, flatten)]
    pub tools: HashMap<String, AppToolConfig>,
}

/// Config values for a single app/connector.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct AppConfig {
    /// When `false`, Codex does not surface this app.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Whether tools with `destructive_hint = true` are allowed for this app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destructive_enabled: Option<bool>,

    /// Whether tools with `open_world_hint = true` are allowed for this app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_world_enabled: Option<bool>,

    /// Approval mode for tools in this app unless a tool override exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_tools_approval_mode: Option<AppToolApproval>,

    /// Whether tools are enabled by default for this app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_tools_enabled: Option<bool>,

    /// Per-tool settings for this app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<AppToolsConfig>,
}

/// App/connector settings loaded from `config.toml`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct AppsConfigToml {
    /// Default settings for all apps.
    #[serde(default, rename = "_default", skip_serializing_if = "Option::is_none")]
    pub default: Option<AppsDefaultConfig>,

    /// Per-app settings keyed by app ID (for example `[apps.google_drive]`).
    #[serde(default, flatten)]
    pub apps: HashMap<String, AppConfig>,
}

// ===== OTEL configuration =====

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum OtelHttpProtocol {
    /// Binary payload
    Binary,
    /// JSON payload
    Json,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct OtelTlsConfig {
    pub ca_certificate: Option<AbsolutePathBuf>,
    pub client_certificate: Option<AbsolutePathBuf>,
    pub client_private_key: Option<AbsolutePathBuf>,
}

/// Which OTEL exporter to use.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[schemars(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum OtelExporterKind {
    None,
    Statsig,
    OtlpHttp {
        endpoint: String,
        #[serde(default)]
        headers: HashMap<String, String>,
        protocol: OtelHttpProtocol,
        #[serde(default)]
        tls: Option<OtelTlsConfig>,
    },
    OtlpGrpc {
        endpoint: String,
        #[serde(default)]
        headers: HashMap<String, String>,
        #[serde(default)]
        tls: Option<OtelTlsConfig>,
    },
}

/// OTEL settings loaded from config.toml. Fields are optional so we can apply defaults.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct OtelConfigToml {
    /// Log user prompt in traces
    pub log_user_prompt: Option<bool>,

    /// Mark traces with environment (dev, staging, prod, test). Defaults to dev.
    pub environment: Option<String>,

    /// Optional log exporter
    pub exporter: Option<OtelExporterKind>,

    /// Optional trace exporter
    pub trace_exporter: Option<OtelExporterKind>,

    /// Optional metrics exporter
    pub metrics_exporter: Option<OtelExporterKind>,
}

/// Effective OTEL settings after defaults are applied.
#[derive(Debug, Clone, PartialEq)]
pub struct OtelConfig {
    pub log_user_prompt: bool,
    pub environment: String,
    pub exporter: OtelExporterKind,
    pub trace_exporter: OtelExporterKind,
    pub metrics_exporter: OtelExporterKind,
}

impl Default for OtelConfig {
    fn default() -> Self {
        OtelConfig {
            log_user_prompt: false,
            environment: DEFAULT_OTEL_ENVIRONMENT.to_owned(),
            exporter: OtelExporterKind::None,
            trace_exporter: OtelExporterKind::None,
            metrics_exporter: OtelExporterKind::Statsig,
        }
    }
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Notifications {
    Enabled(bool),
    Custom(Vec<String>),
}

impl Default for Notifications {
    fn default() -> Self {
        Self::Enabled(true)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum NotificationMethod {
    #[default]
    Auto,
    Osc9,
    Bel,
}

impl fmt::Display for NotificationMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotificationMethod::Auto => write!(f, "auto"),
            NotificationMethod::Osc9 => write!(f, "osc9"),
            NotificationMethod::Bel => write!(f, "bel"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum NotificationCondition {
    /// Emit TUI notifications only while the terminal is unfocused.
    #[default]
    Unfocused,
    /// Emit TUI notifications regardless of terminal focus.
    Always,
}

impl fmt::Display for NotificationCondition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotificationCondition::Unfocused => write!(f, "unfocused"),
            NotificationCondition::Always => write!(f, "always"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct TuiNotificationSettings {
    /// Enable desktop notifications from the TUI.
    /// Defaults to `true`.
    #[serde(default, rename = "notifications")]
    pub notifications: Notifications,

    /// Notification method to use for terminal notifications.
    /// Defaults to `auto`.
    #[serde(default, rename = "notification_method")]
    pub method: NotificationMethod,

    /// Controls whether TUI notifications are delivered only when the terminal is unfocused or
    /// regardless of focus. Defaults to `unfocused`.
    #[serde(default, rename = "notification_condition")]
    pub condition: NotificationCondition,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ModelAvailabilityNuxConfig {
    /// Number of times a startup availability NUX has been shown per model slug.
    #[serde(default, flatten)]
    pub shown_count: HashMap<String, u32>,
}

/// Collection of settings that are specific to the TUI.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct Tui {
    #[serde(default, flatten)]
    pub notification_settings: TuiNotificationSettings,

    /// Enable animations (welcome screen, shimmer effects, spinners).
    /// Defaults to `true`.
    #[serde(default = "default_true")]
    pub animations: bool,

    /// Show startup tooltips in the TUI welcome screen.
    /// Defaults to `true`.
    #[serde(default = "default_true")]
    pub show_tooltips: bool,

    /// Controls whether the TUI uses the terminal's alternate screen buffer.
    ///
    /// - `auto` (default): Disable alternate screen in Zellij, enable elsewhere.
    /// - `always`: Always use alternate screen (original behavior).
    /// - `never`: Never use alternate screen (inline mode only, preserves scrollback).
    ///
    /// Using alternate screen provides a cleaner fullscreen experience but prevents
    /// scrollback in terminal multiplexers like Zellij that follow the xterm spec.
    #[serde(default)]
    pub alternate_screen: AltScreenMode,

    /// Ordered list of status line item identifiers.
    ///
    /// When set, the TUI renders the selected items as the status line.
    /// When unset, the TUI defaults to: `model-with-reasoning` and `current-dir`.
    #[serde(default)]
    pub status_line: Option<Vec<String>>,

    /// Ordered list of terminal title item identifiers.
    ///
    /// When set, the TUI renders the selected items into the terminal window/tab title.
    /// When unset, the TUI defaults to: `spinner` and `project`.
    #[serde(default)]
    pub terminal_title: Option<Vec<String>>,

    /// Syntax highlighting theme name (kebab-case).
    ///
    /// When set, overrides automatic light/dark theme detection.
    /// Use `/theme` in the TUI or see `$CODEX_HOME/themes` for custom themes.
    #[serde(default)]
    pub theme: Option<String>,

    /// Startup tooltip availability NUX state persisted by the TUI.
    #[serde(default)]
    pub model_availability_nux: ModelAvailabilityNuxConfig,
}

const fn default_true() -> bool {
    true
}

/// Settings for notices we display to users via the tui and app-server clients
/// (primarily the Codex IDE extension). NOTE: these are different from
/// notifications - notices are warnings, NUX screens, acknowledgements, etc.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
pub struct Notice {
    /// Tracks whether the user has acknowledged the full access warning prompt.
    pub hide_full_access_warning: Option<bool>,
    /// Tracks whether the user has acknowledged the Windows world-writable directories warning.
    pub hide_world_writable_warning: Option<bool>,
    /// Tracks whether the user opted out of the rate limit model switch reminder.
    pub hide_rate_limit_model_nudge: Option<bool>,
    /// Tracks whether the user has seen the model migration prompt
    pub hide_gpt5_1_migration_prompt: Option<bool>,
    /// Tracks whether the user has seen the gpt-5.1-codex-max migration prompt
    #[serde(rename = "hide_gpt-5.1-codex-max_migration_prompt")]
    pub hide_gpt_5_1_codex_max_migration_prompt: Option<bool>,
    /// Tracks acknowledged model migrations as old->new model slug mappings.
    #[serde(default)]
    pub model_migrations: BTreeMap<String, String>,
}

pub use crate::skills_config::BundledSkillsConfig;
pub use crate::skills_config::SkillConfig;
pub use crate::skills_config::SkillsConfig;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct PluginConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct MarketplaceConfig {
    /// Last time Codex successfully added or refreshed this marketplace.
    #[serde(default)]
    pub last_updated: Option<String>,
    /// Source kind used to install this marketplace.
    #[serde(default)]
    pub source_type: Option<MarketplaceSourceType>,
    /// Source location used when the marketplace was added.
    #[serde(default)]
    pub source: Option<String>,
    /// Git ref to check out when `source_type` is `git`.
    #[serde(default, rename = "ref")]
    pub ref_name: Option<String>,
    /// Sparse checkout paths used when `source_type` is `git`.
    #[serde(default)]
    pub sparse_paths: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceSourceType {
    Git,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct SandboxWorkspaceWrite {
    #[serde(default)]
    pub writable_roots: Vec<AbsolutePathBuf>,
    #[serde(default)]
    pub network_access: bool,
    #[serde(default)]
    pub exclude_tmpdir_env_var: bool,
    #[serde(default)]
    pub exclude_slash_tmp: bool,
}

impl From<SandboxWorkspaceWrite> for codex_app_server_protocol::SandboxSettings {
    fn from(sandbox_workspace_write: SandboxWorkspaceWrite) -> Self {
        Self {
            writable_roots: sandbox_workspace_write.writable_roots,
            network_access: Some(sandbox_workspace_write.network_access),
            exclude_tmpdir_env_var: Some(sandbox_workspace_write.exclude_tmpdir_env_var),
            exclude_slash_tmp: Some(sandbox_workspace_write.exclude_slash_tmp),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ShellEnvironmentPolicyInherit {
    /// "Core" environment variables for the platform. On UNIX, this would
    /// include HOME, LOGNAME, PATH, SHELL, and USER, among others.
    Core,

    /// Inherits the full environment from the parent process.
    #[default]
    All,

    /// Do not inherit any environment variables from the parent process.
    None,
}

/// Policy for building the `env` when spawning a process via either the
/// `shell` or `local_shell` tool.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ShellEnvironmentPolicyToml {
    pub inherit: Option<ShellEnvironmentPolicyInherit>,

    pub ignore_default_excludes: Option<bool>,

    /// List of regular expressions.
    pub exclude: Option<Vec<String>>,

    pub r#set: Option<HashMap<String, String>>,

    /// List of regular expressions.
    pub include_only: Option<Vec<String>>,

    pub experimental_use_profile: Option<bool>,
}

pub type EnvironmentVariablePattern = WildMatchPattern<'*', '?'>;

/// Deriving the `env` based on this policy works as follows:
/// 1. Create an initial map based on the `inherit` policy.
/// 2. If `ignore_default_excludes` is false, filter the map using the default
///    exclude pattern(s), which are: `"*KEY*"`, `"*SECRET*"`, and `"*TOKEN*"`.
/// 3. If `exclude` is not empty, filter the map using the provided patterns.
/// 4. Insert any entries from `r#set` into the map.
/// 5. If non-empty, filter the map using the `include_only` patterns.
#[derive(Debug, Clone, PartialEq)]
pub struct ShellEnvironmentPolicy {
    /// Starting point when building the environment.
    pub inherit: ShellEnvironmentPolicyInherit,

    /// True to skip the check to exclude default environment variables that
    /// contain "KEY", "SECRET", or "TOKEN" in their name. Defaults to true.
    pub ignore_default_excludes: bool,

    /// Environment variable names to exclude from the environment.
    pub exclude: Vec<EnvironmentVariablePattern>,

    /// (key, value) pairs to insert in the environment.
    pub r#set: HashMap<String, String>,

    /// Environment variable names to retain in the environment.
    pub include_only: Vec<EnvironmentVariablePattern>,

    /// If true, the shell profile will be used to run the command.
    pub use_profile: bool,
}

impl From<ShellEnvironmentPolicyToml> for ShellEnvironmentPolicy {
    fn from(toml: ShellEnvironmentPolicyToml) -> Self {
        // Default to inheriting the full environment when not specified.
        let inherit = toml.inherit.unwrap_or(ShellEnvironmentPolicyInherit::All);
        let ignore_default_excludes = toml.ignore_default_excludes.unwrap_or(true);
        let exclude = toml
            .exclude
            .unwrap_or_default()
            .into_iter()
            .map(|s| EnvironmentVariablePattern::new_case_insensitive(&s))
            .collect();
        let r#set = toml.r#set.unwrap_or_default();
        let include_only = toml
            .include_only
            .unwrap_or_default()
            .into_iter()
            .map(|s| EnvironmentVariablePattern::new_case_insensitive(&s))
            .collect();
        let use_profile = toml.experimental_use_profile.unwrap_or(false);

        Self {
            inherit,
            ignore_default_excludes,
            exclude,
            r#set,
            include_only,
            use_profile,
        }
    }
}

impl Default for ShellEnvironmentPolicy {
    fn default() -> Self {
        Self {
            inherit: ShellEnvironmentPolicyInherit::All,
            ignore_default_excludes: true,
            exclude: Vec::new(),
            r#set: HashMap::new(),
            include_only: Vec::new(),
            use_profile: false,
        }
    }
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
