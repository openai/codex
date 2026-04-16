//! Shared approval routing for user and guardian review prompts.

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::guardian::new_guardian_review_id;
use crate::guardian::review_approval_request;
use crate::guardian::routes_approval_to_guardian;
use crate::sandboxing::SandboxPermissions;
use crate::tools::sandboxing::with_cached_approval;
use codex_protocol::approvals::ExecPolicyAmendment;
use codex_protocol::approvals::GuardianCommandSource;
use codex_protocol::approvals::NetworkApprovalContext;
use codex_protocol::approvals::NetworkApprovalProtocol;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::FileChange;
use codex_protocol::protocol::ReviewDecision;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize)]
pub(crate) struct ApprovalCacheKey {
    namespace: &'static str,
    value: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ApprovalCacheKeys {
    pub tool_name: &'static str,
    pub keys: Vec<ApprovalCacheKey>,
}

#[derive(Debug)]
pub(crate) struct ApprovalOutcome {
    pub decision: ReviewDecision,
    pub guardian_review_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ApprovalRequest {
    pub prompt_reason: Option<String>,
    pub review_reason: Option<String>,
    pub kind: ApprovalRequestKind,
    cache: Option<ApprovalCacheKeys>,
}

impl ApprovalRequest {
    pub(crate) fn new(
        prompt_reason: Option<String>,
        review_reason: Option<String>,
        kind: ApprovalRequestKind,
    ) -> Self {
        Self {
            prompt_reason,
            review_reason,
            kind,
            cache: None,
        }
    }

    pub(crate) fn with_session_cache<T>(mut self, tool_name: &'static str, keys: Vec<T>) -> Self
    where
        T: Serialize,
    {
        let keys = keys
            .iter()
            .map(|key| {
                serde_json::to_string(key)
                    .ok()
                    .map(|value| ApprovalCacheKey {
                        namespace: tool_name,
                        value,
                    })
            })
            .collect::<Option<Vec<_>>>();
        self.cache = keys
            .filter(|keys| !keys.is_empty())
            .map(|keys| ApprovalCacheKeys { tool_name, keys });
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ApprovalRequestKind {
    Command(CommandApprovalRequest),
    #[cfg(unix)]
    Execve(ExecveApprovalRequest),
    Patch(PatchApprovalRequest),
    NetworkAccess(NetworkAccessApprovalRequest),
    McpToolCall(McpToolApprovalRequest),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CommandApprovalRequest {
    pub id: String,
    pub approval_id: Option<String>,
    pub source: GuardianCommandSource,
    pub command: Vec<String>,
    pub cwd: AbsolutePathBuf,
    pub sandbox_permissions: SandboxPermissions,
    pub additional_permissions: Option<PermissionProfile>,
    pub justification: Option<String>,
    pub network_approval_context: Option<NetworkApprovalContext>,
    pub proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
    pub available_decisions: Option<Vec<ReviewDecision>>,
    pub tty: bool,
}

#[cfg(unix)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ExecveApprovalRequest {
    pub id: String,
    pub approval_id: String,
    pub source: GuardianCommandSource,
    pub program: String,
    pub argv: Vec<String>,
    pub command: Vec<String>,
    pub cwd: AbsolutePathBuf,
    pub additional_permissions: Option<PermissionProfile>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PatchApprovalRequest {
    pub id: String,
    pub cwd: AbsolutePathBuf,
    pub files: Vec<AbsolutePathBuf>,
    pub patch: String,
    pub changes: HashMap<PathBuf, FileChange>,
    pub grant_root: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NetworkAccessApprovalRequest {
    pub id: String,
    pub turn_id: String,
    pub target: String,
    pub host: String,
    pub protocol: NetworkApprovalProtocol,
    pub port: u16,
    pub cwd: AbsolutePathBuf,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct McpToolApprovalRequest {
    pub id: String,
    pub server: String,
    pub tool_name: String,
    pub arguments: Option<Value>,
    pub connector_id: Option<String>,
    pub connector_name: Option<String>,
    pub connector_description: Option<String>,
    pub tool_title: Option<String>,
    pub tool_description: Option<String>,
    pub annotations: Option<McpToolApprovalAnnotations>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct McpToolApprovalAnnotations {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) destructive_hint: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) open_world_hint: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) read_only_hint: Option<bool>,
}

async fn dispatch_user_approval(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    request: ApprovalRequest,
) -> ReviewDecision {
    let ApprovalRequest {
        prompt_reason,
        kind,
        ..
    } = request;
    match kind {
        ApprovalRequestKind::Command(request) => {
            session
                .request_command_approval(
                    turn.as_ref(),
                    request.id,
                    request.approval_id,
                    request.command,
                    request.cwd,
                    prompt_reason,
                    request.network_approval_context,
                    request.proposed_execpolicy_amendment,
                    request.additional_permissions,
                    request.available_decisions,
                )
                .await
        }
        #[cfg(unix)]
        ApprovalRequestKind::Execve(request) => {
            session
                .request_command_approval(
                    turn.as_ref(),
                    request.id,
                    Some(request.approval_id),
                    request.command,
                    request.cwd,
                    prompt_reason,
                    /*network_approval_context*/ None,
                    /*proposed_execpolicy_amendment*/ None,
                    request.additional_permissions,
                    Some(vec![ReviewDecision::Approved, ReviewDecision::Abort]),
                )
                .await
        }
        ApprovalRequestKind::Patch(request) => {
            let rx_approve = session
                .request_patch_approval(
                    turn.as_ref(),
                    request.id,
                    request.changes,
                    prompt_reason,
                    request.grant_root,
                )
                .await;
            rx_approve.await.unwrap_or_default()
        }
        ApprovalRequestKind::NetworkAccess(request) => {
            session
                .request_command_approval(
                    turn.as_ref(),
                    request.id,
                    /*approval_id*/ None,
                    vec!["network-access".to_string(), request.target],
                    request.cwd,
                    prompt_reason,
                    Some(NetworkApprovalContext {
                        host: request.host,
                        protocol: request.protocol,
                    }),
                    /*proposed_execpolicy_amendment*/ None,
                    /*additional_permissions*/ None,
                    /*available_decisions*/ None,
                )
                .await
        }
        ApprovalRequestKind::McpToolCall(_) => ReviewDecision::Abort,
    }
}

async fn request_user_approval(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    request: ApprovalRequest,
) -> ReviewDecision {
    if let Some(cache) = request.cache.clone() {
        with_cached_approval(&session.services, cache.tool_name, cache.keys, || {
            dispatch_user_approval(session, turn, request)
        })
        .await
    } else {
        dispatch_user_approval(session, turn, request).await
    }
}

pub(crate) async fn request_approval(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    request: ApprovalRequest,
) -> ApprovalOutcome {
    if routes_approval_to_guardian(turn) {
        let review_id = new_guardian_review_id();
        return ApprovalOutcome {
            decision: review_approval_request(session, turn, review_id.clone(), request).await,
            guardian_review_id: Some(review_id),
        };
    }

    ApprovalOutcome {
        decision: request_user_approval(session, turn, request).await,
        guardian_review_id: None,
    }
}
