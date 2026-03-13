use std::collections::HashMap;
use std::path::PathBuf;

use crate::mcp::RequestId;
use crate::models::MacOsSeatbeltProfileExtensions;
use crate::models::PermissionProfile;
use crate::parse_command::ParsedCommand;
use crate::permissions::FileSystemSandboxPolicy;
use crate::permissions::NetworkSandboxPolicy;
use crate::protocol::FileChange;
use crate::protocol::ReviewDecision;
use crate::protocol::SandboxPolicy;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Permissions {
    pub sandbox_policy: SandboxPolicy,
    pub file_system_sandbox_policy: FileSystemSandboxPolicy,
    pub network_sandbox_policy: NetworkSandboxPolicy,
    pub macos_seatbelt_profile_extensions: Option<MacOsSeatbeltProfileExtensions>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EscalationPermissions {
    PermissionProfile(PermissionProfile),
    Permissions(Permissions),
}

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

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum NetworkPolicyRuleAction {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct NetworkPolicyAmendment {
    pub host: String,
    pub action: NetworkPolicyRuleAction,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub struct ExecApprovalRequestSkillMetadata {
    pub path_to_skills_md: PathBuf,
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
    /// Proposed network policy amendments (for example allow/deny this host in future).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub proposed_network_policy_amendments: Option<Vec<NetworkPolicyAmendment>>,
    /// Optional additional filesystem permissions requested for this command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub additional_permissions: Option<PermissionProfile>,
    /// Optional skill metadata when the approval was triggered by a skill script.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub skill_metadata: Option<ExecApprovalRequestSkillMetadata>,
    /// Ordered list of decisions the client may present for this prompt.
    ///
    /// When absent, clients should derive the legacy default set from the
    /// other fields on this request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub available_decisions: Option<Vec<ReviewDecision>>,
    pub parsed_cmd: Vec<ParsedCommand>,
}

impl ExecApprovalRequestEvent {
    pub fn effective_approval_id(&self) -> String {
        self.approval_id
            .clone()
            .unwrap_or_else(|| self.call_id.clone())
    }

    pub fn effective_available_decisions(&self) -> Vec<ReviewDecision> {
        // available_decisions is a new field that may not be populated by older
        // senders, so we fall back to the legacy logic if it's not present.
        match &self.available_decisions {
            Some(decisions) => decisions.clone(),
            None => Self::default_available_decisions(
                &self.command,
                self.network_approval_context.as_ref(),
                self.proposed_execpolicy_amendment.as_ref(),
                self.proposed_network_policy_amendments.as_deref(),
                self.additional_permissions.as_ref(),
            ),
        }
    }

    pub fn default_available_decisions(
        command: &[String],
        network_approval_context: Option<&NetworkApprovalContext>,
        proposed_execpolicy_amendment: Option<&ExecPolicyAmendment>,
        proposed_network_policy_amendments: Option<&[NetworkPolicyAmendment]>,
        additional_permissions: Option<&PermissionProfile>,
    ) -> Vec<ReviewDecision> {
        if network_approval_context.is_some() {
            let mut decisions = vec![ReviewDecision::Approved, ReviewDecision::ApprovedForSession];
            if let Some(amendment) = proposed_network_policy_amendments.and_then(|amendments| {
                amendments
                    .iter()
                    .find(|amendment| amendment.action == NetworkPolicyRuleAction::Allow)
            }) {
                decisions.push(ReviewDecision::NetworkPolicyAmendment {
                    network_policy_amendment: amendment.clone(),
                });
            }
            decisions.push(ReviewDecision::Abort);
            return decisions;
        }

        if additional_permissions.is_some() {
            return vec![
                ReviewDecision::Approved,
                ReviewDecision::ApprovedOverrideCommand {
                    command: command.to_vec(),
                },
                ReviewDecision::Abort,
            ];
        }

        let mut decisions = vec![
            ReviewDecision::Approved,
            ReviewDecision::ApprovedOverrideCommand {
                command: command.to_vec(),
            },
        ];
        if let Some(prefix) = proposed_execpolicy_amendment {
            decisions.push(ReviewDecision::ApprovedExecpolicyAmendment {
                proposed_execpolicy_amendment: prefix.clone(),
            });
        }
        decisions.push(ReviewDecision::Abort);
        decisions
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema, TS)]
#[serde(tag = "mode", rename_all = "snake_case")]
#[ts(tag = "mode")]
pub enum ElicitationRequest {
    Form {
        #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
        #[ts(optional, rename = "_meta")]
        meta: Option<JsonValue>,
        message: String,
        requested_schema: JsonValue,
    },
    Url {
        #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
        #[ts(optional, rename = "_meta")]
        meta: Option<JsonValue>,
        message: String,
        url: String,
        elicitation_id: String,
    },
}

impl ElicitationRequest {
    pub fn message(&self) -> &str {
        match self {
            Self::Form { message, .. } | Self::Url { message, .. } => message,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema, TS)]
pub struct ElicitationRequestEvent {
    /// Turn ID that this elicitation belongs to, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub turn_id: Option<String>,
    pub server_name: String,
    #[ts(type = "string | number")]
    pub id: RequestId,
    pub request: ElicitationRequest,
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn command_prompts_advertise_override_decision() {
        let command = vec!["echo".to_string(), "hi".to_string()];

        let decisions =
            ExecApprovalRequestEvent::default_available_decisions(&command, None, None, None, None);

        assert_eq!(
            decisions,
            vec![
                ReviewDecision::Approved,
                ReviewDecision::ApprovedOverrideCommand {
                    command: command.clone(),
                },
                ReviewDecision::Abort,
            ]
        );
    }

    #[test]
    fn network_only_prompts_do_not_advertise_override_decision() {
        let decisions = ExecApprovalRequestEvent::default_available_decisions(
            &["echo".to_string(), "hi".to_string()],
            Some(&NetworkApprovalContext {
                host: "example.com".to_string(),
                protocol: NetworkApprovalProtocol::Https,
            }),
            None,
            Some(&[
                NetworkPolicyAmendment {
                    host: "example.com".to_string(),
                    action: NetworkPolicyRuleAction::Allow,
                },
                NetworkPolicyAmendment {
                    host: "example.com".to_string(),
                    action: NetworkPolicyRuleAction::Deny,
                },
            ]),
            None,
        );

        assert_eq!(
            decisions,
            vec![
                ReviewDecision::Approved,
                ReviewDecision::ApprovedForSession,
                ReviewDecision::NetworkPolicyAmendment {
                    network_policy_amendment: NetworkPolicyAmendment {
                        host: "example.com".to_string(),
                        action: NetworkPolicyRuleAction::Allow,
                    },
                },
                ReviewDecision::Abort,
            ]
        );
    }
}
