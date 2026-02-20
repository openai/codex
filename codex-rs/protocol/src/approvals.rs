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

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum NetworkApprovalProtocol {
    // TODO(viyatb): Add websocket protocol variants when managed proxy policy
    // decisions expose websocket traffic as a distinct approval context.
    Http,
    #[serde(alias = "https_connect", alias = "http-connect")]
    Https,
    Socks5Tcp,
    Socks5Udp,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct NetworkApprovalContext {
    pub host: String,
    pub protocol: NetworkApprovalProtocol,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct ExecApprovalRequestEvent {
    /// Identifier for the associated command execution item.
    pub call_id: String,
    /// Identifier for this specific approval callback.
    ///
    /// When absent, the approval is for the command item itself (`call_id`).
    /// This is present for subcommand approvals (via execve intercept).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub approval_id: Option<String>,
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
    /// Optional network context for a blocked request that can be approved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub network_approval_context: Option<NetworkApprovalContext>,
    /// Proposed execpolicy amendment that can be applied to allow future runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
    pub parsed_cmd: Vec<ParsedCommand>,
}

impl ExecApprovalRequestEvent {
    pub fn effective_approval_id(&self) -> String {
        self.approval_id
            .clone()
            .unwrap_or_else(|| self.call_id.clone())
    }
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
