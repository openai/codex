use std::collections::HashMap;
use std::path::PathBuf;

use crate::mcp::RequestId;
use crate::parse_command::ParsedCommand;
use crate::protocol::FileChange;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

/// Proposed execpolicy change to allow commands starting with this prefix.
///
/// The `command` tokens form the prefix that would be added as an execpolicy
/// `prefix_rule(..., decision="allow")`, letting the agent bypass approval for
/// commands that start with this token sequence.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(transparent)]
#[ts(type = "Array<string>")]
pub struct ExecPolicyAmendment {
    pub command: Vec<String>,
}

impl ExecPolicyAmendment {
    pub fn new(command: Vec<String>) -> Self {
        Self { command }
    }

    pub fn command(&self) -> &[String] {
        &self.command
    }
}

impl From<Vec<String>> for ExecPolicyAmendment {
    fn from(command: Vec<String>) -> Self {
        Self { command }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct ExecApprovalRequestEvent {
    /// Identifier for the associated exec call, if available.
    pub call_id: String,
    /// Turn ID that this command belongs to.
    /// Uses `#[serde(default)]` for backwards compatibility.
    #[serde(default)]
    pub turn_id: String,
    /// The command to be executed.
    pub command: Vec<String>,
    /// The command's working directory.
    pub cwd: PathBuf,
    /// Optional human-readable reason for the approval (e.g. retry without sandbox).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Proposed execpolicy amendment that can be applied to allow future runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
    pub parsed_cmd: Vec<ParsedCommand>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct ElicitationRequestEvent {
    pub server_name: String,
    #[ts(type = "string | number")]
    pub id: RequestId,
    pub message: String,
    // TODO: MCP servers can request we fill out a schema for the elicitation. We don't support
    // this yet.
    // pub requested_schema: ElicitRequestParamsRequestedSchema,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
pub enum ElicitationAction {
    Accept,
    Decline,
    Cancel,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct ApplyPatchApprovalRequestEvent {
    /// Responses API call id for the associated patch apply call, if available.
    pub call_id: String,
    /// Turn ID that this patch belongs to.
    /// Uses `#[serde(default)]` for backwards compatibility with older senders.
    #[serde(default)]
    pub turn_id: String,
    pub changes: HashMap<PathBuf, FileChange>,
    /// Optional explanatory reason (e.g. request for extra write access).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// When set, the agent is asking the user to allow writes under this root for the remainder of the session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grant_root: Option<PathBuf>,
}

// ============================================================================
// CRAFT AGENTS: PreToolUse Hook Event Types
// ============================================================================

/// Type of tool being executed, for PreToolUse hook.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub enum ToolCallType {
    Bash,
    FileWrite,
    FileEdit,
    Mcp,
    Custom,
    Function,
    LocalShell,
}

/// CRAFT AGENTS: Event requesting PreToolUse hook decision.
/// Sent to client BEFORE any tool execution to allow interception.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallPreExecuteRequestEvent {
    /// Identifier for the tool call.
    pub call_id: String,
    /// Turn ID that this tool call belongs to.
    #[serde(default)]
    pub turn_id: String,
    /// The type of tool being executed.
    pub tool_type: ToolCallType,
    /// The name of the tool (e.g., "bash", "mcp__github__create_issue").
    pub tool_name: String,
    /// The tool input as JSON string.
    pub input: String,
    /// For MCP tools: the server name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_server: Option<String>,
    /// For MCP tools: the actual tool name within the server.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_tool: Option<String>,
}

/// CRAFT AGENTS: Decision from PreToolUse hook.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub enum ToolCallPreExecuteDecision {
    Allow,
    Block,
    Modify,
    /// Ask user for permission before proceeding.
    /// Client should display a permission prompt and respond with user's decision.
    AskUser,
}

/// CRAFT AGENTS: Type of permission prompt to display.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum PermissionPromptType {
    /// Shell command execution
    Bash,
    /// File write operation (Write, Edit, MultiEdit, NotebookEdit)
    FileWrite,
    /// MCP tool that may mutate data
    McpMutation,
    /// API endpoint call that may mutate data
    ApiMutation,
}

/// CRAFT AGENTS: Metadata for permission prompt display.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct PermissionPromptMetadata {
    /// Type of permission being requested
    pub prompt_type: PermissionPromptType,
    /// Human-readable description of the operation
    pub description: String,
    /// For bash: the command being executed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// For file operations: the target file path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    /// For MCP/API: the tool or endpoint name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

/// CRAFT AGENTS: User's decision on a permission prompt.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub enum UserPermissionDecision {
    /// User approved the operation
    Approved,
    /// User denied the operation
    Denied,
    /// Permission request timed out
    TimedOut,
}

/// CRAFT AGENTS: User's response to a permission prompt.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct UserPermissionResponse {
    /// The user's decision
    pub decision: UserPermissionDecision,
    /// If true, auto-approve similar operations for the rest of the session
    #[serde(default)]
    pub accept_for_session: bool,
}

/// CRAFT AGENTS: Response to PreToolUse hook request.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallPreExecuteResponse {
    pub decision: ToolCallPreExecuteDecision,
    /// If decision is Block, the reason to return to the model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// If decision is Modify, the modified input as JSON string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_input: Option<String>,
    /// If decision is AskUser, metadata for displaying the permission prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_metadata: Option<PermissionPromptMetadata>,
    /// If decision is AskUser, the user's response after prompting.
    /// Client fills this in when responding to an AskUser request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_response: Option<UserPermissionResponse>,
}
