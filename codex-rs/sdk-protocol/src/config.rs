//! Configuration types for SDK clients.
//!
//! These types define the options that SDK clients can pass when creating
//! sessions or making queries.

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::PathBuf;
use ts_rs::TS;

/// Configuration options for the Codex Agent SDK.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default, TS, JsonSchema)]
#[serde(default)]
pub struct CodexAgentOptions {
    /// List of tools to enable or a preset.
    pub tools: Option<ToolsConfig>,
    /// List of allowed tool patterns.
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    /// Custom system prompt or a preset.
    pub system_prompt: Option<SystemPromptConfig>,
    /// MCP server configurations.
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,
    /// Permission mode for tool usage.
    pub permission_mode: Option<PermissionMode>,
    /// Model to use.
    pub model: Option<String>,
    /// Working directory for the session.
    pub working_directory: Option<PathBuf>,
    /// Maximum number of turns.
    pub max_turns: Option<i32>,
    /// Sandbox mode.
    pub sandbox_mode: Option<SandboxMode>,
    /// Whether to enable internet access.
    pub internet_access: Option<bool>,
    /// Maximum thinking budget tokens.
    pub max_thinking_budget: Option<i32>,
    /// Whether to enable reasoning output.
    pub reasoning_output: Option<bool>,
    /// Custom configuration file path.
    pub config_file: Option<PathBuf>,
    /// Whether to disable conversation history.
    pub disable_conversation_history: Option<bool>,
    /// Custom environment variables.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Hook configurations.
    #[serde(default)]
    pub hooks: HashMap<String, Vec<HookConfig>>,
}

/// Tools configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
#[serde(untagged)]
pub enum ToolsConfig {
    /// List of specific tool names.
    List(Vec<String>),
    /// A preset name.
    Preset(ToolsPreset),
}

/// Predefined tool presets.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ToolsPreset {
    /// All available tools.
    All,
    /// Default set of tools.
    Default,
    /// Read-only tools.
    ReadOnly,
    /// No tools.
    None,
}

/// System prompt configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
#[serde(untagged)]
pub enum SystemPromptConfig {
    /// Custom system prompt text.
    Custom(String),
    /// A preset name.
    Preset(SystemPromptPreset),
}

/// Predefined system prompt presets.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SystemPromptPreset {
    /// Default system prompt.
    Default,
    /// Minimal system prompt.
    Minimal,
    /// No system prompt.
    None,
}

/// MCP server configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct McpServerConfig {
    /// Command to run the server.
    pub command: String,
    /// Arguments to pass to the command.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables for the server.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Working directory for the server.
    pub cwd: Option<PathBuf>,
}

/// Permission mode for tool usage.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    /// Default permission mode - prompt for dangerous operations.
    Default,
    /// Accept all edits automatically.
    AcceptEdits,
    /// Plan mode - don't execute, just plan.
    Plan,
    /// Bypass all permission checks (dangerous).
    BypassPermissions,
}

impl Default for PermissionMode {
    fn default() -> Self {
        Self::Default
    }
}

/// Sandbox mode for execution.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SandboxMode {
    /// Read-only access to the filesystem.
    ReadOnly,
    /// Write access to the workspace only.
    WorkspaceWrite,
    /// Full access (dangerous).
    DangerFullAccess,
}

impl Default for SandboxMode {
    fn default() -> Self {
        Self::WorkspaceWrite
    }
}

/// Hook configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
#[serde(untagged)]
pub enum HookConfig {
    /// Command to execute.
    Command(HookCommandConfig),
    /// SDK callback.
    Callback(HookCallbackConfig),
}

/// Hook command configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct HookCommandConfig {
    /// Command to execute.
    pub command: String,
    /// Timeout in milliseconds.
    pub timeout: Option<i64>,
}

/// Hook callback configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct HookCallbackConfig {
    /// Callback identifier.
    pub callback_id: String,
    /// Timeout in milliseconds.
    pub timeout: Option<i64>,
}

/// Session state for resumption.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct SessionState {
    /// Session ID.
    pub session_id: String,
    /// Thread ID for continuation.
    pub thread_id: Option<String>,
    /// Serialized conversation state.
    pub state: Option<JsonValue>,
}
