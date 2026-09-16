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
#[derive(Debug, Default, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GuardianReviewMode {
    #[default]
    Disabled,
    Synchronous,
    /// Use a current low-risk score; otherwise run synchronous review.
    Adaptive,
    #[serde(other)]
    Unknown,
}

/// How actions outside adaptive coverage affect cached classification evidence.
#[derive(Debug, Default, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GuardianUnscoredAction {
    Ignore,
    AgeScore,
    #[default]
    #[serde(other)]
    InvalidateScore,
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
    /// Coverage for tools without an approval category.
    #[serde(default)]
    pub other_tools: GuardianReviewMode,
    #[serde(default)]
    pub unscored_action: GuardianUnscoredAction,
    /// Omission retains the existing allowance for adaptive computer use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_cua_call: Option<bool>,
    /// Whether adaptive shell coverage includes ordinary sandboxed commands.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandboxed_exec_commands: Option<bool>,
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

    pub fn scoring_enabled(&self) -> bool {
        [
            self.computer_use,
            self.shell,
            self.file_changes,
            self.mcp,
            self.network,
            self.permissions,
        ]
        .contains(&Some(GuardianReviewMode::Adaptive))
    }

    pub fn disable_scoring(&mut self) {
        for mode in [
            &mut self.computer_use,
            &mut self.shell,
            &mut self.file_changes,
            &mut self.mcp,
            &mut self.network,
            &mut self.permissions,
        ] {
            if *mode == Some(GuardianReviewMode::Adaptive) {
                *mode = Some(GuardianReviewMode::Synchronous);
            }
        }
        self.other_tools = GuardianReviewMode::Synchronous;
    }

    pub fn allows_initial_cua_call(&self) -> bool {
        self.initial_cua_call
            .unwrap_or(self.computer_use == Some(GuardianReviewMode::Adaptive))
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
