//! TUI-owned approval request models used while rendering and queueing prompts.
//!
//! These structs normalize app-server request params into the shape the TUI
//! needs while an approval may be deferred behind streaming output. Exec
//! approvals keep app-server decision and permission types; patch approvals add
//! the file-change display model collected from nearby thread items.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::diff_model::FileChange;
use codex_app_server_protocol::AdditionalPermissionProfile;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_app_server_protocol::ExecPolicyAmendment;
use codex_app_server_protocol::NetworkApprovalContext;
use codex_app_server_protocol::NetworkPolicyAmendment;
use codex_app_server_protocol::NetworkPolicyRuleAction;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ExecApprovalRequestEvent {
    pub(crate) call_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) approval_id: Option<String>,
    #[serde(default)]
    pub(crate) turn_id: String,
    pub(crate) command: Vec<String>,
    pub(crate) cwd: AbsolutePathBuf,
    pub(crate) reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) proposed_network_policy_amendments: Option<Vec<NetworkPolicyAmendment>>,
    #[serde(default)]
    pub(crate) available_decisions: Option<Vec<CommandExecutionApprovalDecision>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) network_approval_context: Option<NetworkApprovalContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) additional_permissions: Option<AdditionalPermissionProfile>,
}

impl ExecApprovalRequestEvent {
    pub(crate) fn effective_approval_id(&self) -> String {
        self.approval_id
            .clone()
            .unwrap_or_else(|| self.call_id.clone())
    }

    pub(crate) fn effective_available_decisions(&self) -> Vec<CommandExecutionApprovalDecision> {
        match &self.available_decisions {
            Some(decisions) => decisions.clone(),
            None => Self::default_available_decisions(
                self.network_approval_context.as_ref(),
                self.proposed_execpolicy_amendment.as_ref(),
                self.proposed_network_policy_amendments.as_deref(),
                self.additional_permissions.as_ref(),
            ),
        }
    }

    pub(crate) fn default_available_decisions(
        network_approval_context: Option<&NetworkApprovalContext>,
        proposed_execpolicy_amendment: Option<&ExecPolicyAmendment>,
        proposed_network_policy_amendments: Option<&[NetworkPolicyAmendment]>,
        additional_permissions: Option<&AdditionalPermissionProfile>,
    ) -> Vec<CommandExecutionApprovalDecision> {
        if network_approval_context.is_some() {
            let mut decisions = vec![
                CommandExecutionApprovalDecision::Accept,
                CommandExecutionApprovalDecision::AcceptForSession,
            ];
            if let Some(amendment) = proposed_network_policy_amendments.and_then(|amendments| {
                amendments
                    .iter()
                    .find(|amendment| amendment.action == NetworkPolicyRuleAction::Allow)
            }) {
                decisions.push(
                    CommandExecutionApprovalDecision::ApplyNetworkPolicyAmendment {
                        network_policy_amendment: amendment.clone(),
                    },
                );
            }
            decisions.push(CommandExecutionApprovalDecision::Cancel);
            return decisions;
        }

        if additional_permissions.is_some() {
            return vec![
                CommandExecutionApprovalDecision::Accept,
                CommandExecutionApprovalDecision::Cancel,
            ];
        }

        let mut decisions = vec![CommandExecutionApprovalDecision::Accept];
        if let Some(prefix) = proposed_execpolicy_amendment {
            decisions.push(
                CommandExecutionApprovalDecision::AcceptWithExecpolicyAmendment {
                    execpolicy_amendment: prefix.clone(),
                },
            );
        }
        decisions.push(CommandExecutionApprovalDecision::Cancel);
        decisions
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ApplyPatchApprovalRequestEvent {
    pub(crate) call_id: String,
    #[serde(default)]
    pub(crate) turn_id: String,
    pub(crate) changes: HashMap<PathBuf, FileChange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) grant_root: Option<PathBuf>,
}
