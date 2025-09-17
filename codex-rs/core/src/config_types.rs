//! Types used to define the fields of [`crate::config::Config`].

// Note this file should generally be restricted to simple struct/enum
// definitions that do not contain business logic.

use std::collections::HashMap;
use std::path::PathBuf;
use wildmatch::WildMatchPattern;

use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct McpServerConfig {
    #[serde(default)]
    pub display_name: Option<String>,

    #[serde(default)]
    pub category: Option<String>,

    #[serde(default)]
    pub template_id: Option<String>,

    #[serde(default)]
    pub description: Option<String>,

    pub command: String,

    #[serde(default)]
    pub args: Vec<String>,

    #[serde(default)]
    pub env: Option<HashMap<String, String>>,

    /// Startup timeout in milliseconds for initializing MCP server & initially listing tools.
    #[serde(default)]
    pub startup_timeout_ms: Option<u64>,

    #[serde(default)]
    pub auth: Option<McpAuthConfig>,

    #[serde(default)]
    pub healthcheck: Option<McpHealthcheckConfig>,

    #[serde(default)]
    pub tags: Vec<String>,

    #[serde(default)]
    pub created_at: Option<String>,

    #[serde(default)]
    pub last_verified_at: Option<String>,

    #[serde(default)]
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct McpAuthConfig {
    #[serde(rename = "type")]
    pub kind: Option<String>,

    #[serde(default)]
    pub secret_ref: Option<String>,

    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct McpHealthcheckConfig {
    #[serde(rename = "type")]
    pub kind: Option<String>,

    #[serde(default)]
    pub command: Option<String>,

    #[serde(default)]
    pub args: Vec<String>,

    #[serde(default)]
    pub timeout_ms: Option<u64>,

    #[serde(default)]
    pub interval_seconds: Option<u64>,

    #[serde(default)]
    pub endpoint: Option<String>,

    #[serde(default)]
    pub protocol: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct McpTemplate {
    #[serde(default)]
    pub version: Option<String>,

    #[serde(default)]
    pub summary: Option<String>,

    #[serde(default)]
    pub category: Option<String>,

    #[serde(default)]
    pub defaults: Option<McpTemplateDefaults>,

    #[serde(default)]
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct McpTemplateDefaults {
    #[serde(default)]
    pub command: Option<String>,

    #[serde(default)]
    pub args: Vec<String>,

    #[serde(default)]
    pub env: Option<HashMap<String, String>>,

    #[serde(default)]
    pub auth: Option<McpAuthConfig>,

    #[serde(default)]
    pub healthcheck: Option<McpHealthcheckConfig>,

    #[serde(default)]
    pub tags: Vec<String>,

    #[serde(default)]
    pub startup_timeout_ms: Option<u64>,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Debug, Copy, Clone, PartialEq)]
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
#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
pub struct History {
    /// If true, history entries will not be written to disk.
    pub persistence: HistoryPersistence,

    /// If set, the maximum size of the history file in bytes.
    /// TODO(mbolin): Not currently honored.
    pub max_bytes: Option<usize>,
}

#[derive(Deserialize, Debug, Copy, Clone, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum HistoryPersistence {
    /// Save all history entries to disk.
    #[default]
    SaveAll,
    /// Do not write history to disk.
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum Notifications {
    Enabled(bool),
    Custom(Vec<String>),
}

impl Default for Notifications {
    fn default() -> Self {
        Self::Enabled(false)
    }
}

/// Collection of settings that are specific to the TUI.
#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Tui {
    /// Enable desktop notifications from the TUI when the terminal is unfocused.
    /// Defaults to `false`.
    pub notifications: Notifications,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
pub struct SandboxWorkspaceWrite {
    #[serde(default)]
    pub writable_roots: Vec<PathBuf>,
    #[serde(default)]
    pub network_access: bool,
    #[serde(default)]
    pub exclude_tmpdir_env_var: bool,
    #[serde(default)]
    pub exclude_slash_tmp: bool,
}

impl From<SandboxWorkspaceWrite> for codex_protocol::mcp_protocol::SandboxSettings {
    fn from(sandbox_workspace_write: SandboxWorkspaceWrite) -> Self {
        Self {
            writable_roots: sandbox_workspace_write.writable_roots,
            network_access: Some(sandbox_workspace_write.network_access),
            exclude_tmpdir_env_var: Some(sandbox_workspace_write.exclude_tmpdir_env_var),
            exclude_slash_tmp: Some(sandbox_workspace_write.exclude_slash_tmp),
        }
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
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
#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
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
///    exclude pattern(s), which are: `"*KEY*"` and `"*TOKEN*"`.
/// 3. If `exclude` is not empty, filter the map using the provided patterns.
/// 4. Insert any entries from `r#set` into the map.
/// 5. If non-empty, filter the map using the `include_only` patterns.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ShellEnvironmentPolicy {
    /// Starting point when building the environment.
    pub inherit: ShellEnvironmentPolicyInherit,

    /// True to skip the check to exclude default environment variables that
    /// contain "KEY" or "TOKEN" in their name.
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
        let ignore_default_excludes = toml.ignore_default_excludes.unwrap_or(false);
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

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Default, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum ReasoningSummaryFormat {
    #[default]
    None,
    Experimental,
}
