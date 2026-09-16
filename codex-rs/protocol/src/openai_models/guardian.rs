//! Model-owned Guardian coverage. Missing policy preserves legacy behavior;
//! unknown modes retain synchronous review and never enable the fast path.

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

use super::ModelInfo;
use crate::ToolName;
use crate::mcp::is_node_repl_backed_server;
use crate::mcp::is_node_repl_backed_tool;

/// How Guardian handles an action when the user selects automatic approval.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GuardianReviewMode {
    Disabled,
    Synchronous,
    /// Use a current low-risk score; otherwise run synchronous review.
    Adaptive,
    #[serde(other)]
    Unknown,
}

/// A complete model policy. Omitted scopes are disabled; unknown fields are ignored.
/// Code Mode wrappers have no approval scope; their nested tools follow this policy.
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq, TS, JsonSchema)]
pub struct GuardianModelPolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub computer_use: Option<GuardianReviewMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell: Option<GuardianReviewMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_changes: Option<GuardianReviewMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp: Option<GuardianReviewMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<GuardianReviewMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<GuardianReviewMode>,
}

impl GuardianModelPolicy {
    pub fn review_mode(&self, scope: GuardianScope) -> GuardianReviewMode {
        match scope {
            GuardianScope::ComputerUse => self.computer_use,
            GuardianScope::Shell => self.shell,
            GuardianScope::FileChanges => self.file_changes,
            GuardianScope::Mcp => self.mcp,
            GuardianScope::Network => self.network,
            GuardianScope::Permissions => self.permissions,
        }
        .unwrap_or(GuardianReviewMode::Disabled)
    }
}

/// Approval categories understood by this client. Future catalog keys are ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardianScope {
    ComputerUse,
    Shell,
    FileChanges,
    Mcp,
    Network,
    Permissions,
}

impl GuardianScope {
    pub fn for_mcp_server(server: &str) -> Self {
        if is_node_repl_backed_server(server) {
            Self::ComputerUse
        } else {
            Self::Mcp
        }
    }

    pub fn for_tool(tool: &ToolName) -> Option<Self> {
        if is_node_repl_backed_tool(&tool.name, tool.namespace.as_deref()) {
            return Some(Self::ComputerUse);
        }
        if tool
            .namespace
            .as_deref()
            .is_some_and(|namespace| namespace.starts_with("mcp__"))
            || tool.name.starts_with("mcp__")
        {
            return Some(Self::Mcp);
        }
        if !tool.is_default_namespace() {
            return None;
        }
        match tool.name.as_str() {
            "shell" | "shell_command" | "exec_command" | "write_stdin" | "execve" => {
                Some(Self::Shell)
            }
            "apply_patch" => Some(Self::FileChanges),
            "request_permissions" => Some(Self::Permissions),
            _ => None,
        }
    }
}

impl ModelInfo {
    /// An absent map uses legacy config; an omitted scope in a supplied map is disabled.
    pub fn guardian_review_mode(&self, scope: GuardianScope) -> Option<GuardianReviewMode> {
        self.guardian
            .as_ref()
            .map(|policy| policy.review_mode(scope))
    }

    /// The legacy metadata bit remains the transport understood by older CUA servers.
    pub fn computer_use_review_required(&self) -> bool {
        self.guardian_review_mode(GuardianScope::ComputerUse)
            .map_or(self.node_repl_auto_review_required, |mode| {
                mode != GuardianReviewMode::Disabled
            })
    }
}

#[cfg(test)]
#[path = "guardian_tests.rs"]
mod tests;
