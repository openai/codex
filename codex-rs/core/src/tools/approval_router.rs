use crate::codex::Session;
use crate::codex::TurnContext;
use crate::guardian::GuardianApprovalRequest;
use crate::guardian::new_guardian_review_id;
use crate::guardian::review_approval_request;
use crate::guardian::routes_approval_to_guardian;
use codex_protocol::approvals::ExecPolicyAmendment;
use codex_protocol::approvals::GuardianCommandSource;
use codex_protocol::approvals::NetworkApprovalContext;
use codex_protocol::approvals::NetworkApprovalProtocol;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::FileChange;
use codex_protocol::protocol::ReviewDecision;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ApprovalOutcome {
    pub decision: ReviewDecision,
    pub guardian_review_id: Option<String>,
}

impl ApprovalOutcome {
    pub(crate) fn new(decision: ReviewDecision) -> Self {
        Self {
            decision,
            guardian_review_id: None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ApprovalCache {
    tool_name: &'static str,
    serialized_keys: Vec<String>,
    key_count: usize,
}

impl ApprovalCache {
    pub(crate) fn new<K>(tool_name: &'static str, keys: Vec<K>) -> Self
    where
        K: Serialize,
    {
        let key_count = keys.len();
        let serialized_keys = keys
            .into_iter()
            .filter_map(|key| serde_json::to_string(&key).ok())
            .collect();
        Self {
            tool_name,
            serialized_keys,
            key_count,
        }
    }

    fn has_keys(&self) -> bool {
        self.key_count > 0
    }

    fn all_keys_serialized(&self) -> bool {
        self.serialized_keys.len() == self.key_count
    }

    async fn is_approved_for_session(&self, session: &Session) -> bool {
        if !self.has_keys() || !self.all_keys_serialized() {
            return false;
        }

        let store = session.services.tool_approvals.lock().await;
        self.serialized_keys.iter().all(|key| {
            matches!(
                store.get_serialized(key),
                Some(ReviewDecision::ApprovedForSession)
            )
        })
    }

    async fn record_outcome(&self, session: &Session, outcome: &ApprovalOutcome) {
        if !self.has_keys() {
            return;
        }

        session.services.session_telemetry.counter(
            "codex.approval.requested",
            /*inc*/ 1,
            &[
                ("tool", self.tool_name),
                ("approved", outcome.decision.to_opaque_string()),
            ],
        );

        if matches!(outcome.decision, ReviewDecision::ApprovedForSession) {
            let mut store = session.services.tool_approvals.lock().await;
            for key in &self.serialized_keys {
                store.put_serialized(key.clone(), ReviewDecision::ApprovedForSession);
            }
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ApprovalRequest {
    pub intent: ApprovalIntent,
    pub retry_reason: Option<String>,
    pub cache: Option<ApprovalCache>,
}

#[derive(Clone, Debug)]
pub(crate) enum ApprovalIntent {
    Shell(ShellApprovalRequest),
    UnifiedExec(UnifiedExecApprovalRequest),
    ApplyPatch(ApplyPatchApprovalRequest),
    NetworkAccess(NetworkAccessApprovalRequest),
    #[cfg(unix)]
    Execve(ExecveApprovalRequest),
}

#[derive(Clone, Debug)]
pub(crate) struct ShellApprovalRequest {
    pub call_id: String,
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub sandbox_permissions: crate::sandboxing::SandboxPermissions,
    pub additional_permissions: Option<PermissionProfile>,
    pub justification: Option<String>,
    pub reason: Option<String>,
    pub network_approval_context: Option<NetworkApprovalContext>,
    pub proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
}

#[derive(Clone, Debug)]
pub(crate) struct UnifiedExecApprovalRequest {
    pub call_id: String,
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub sandbox_permissions: crate::sandboxing::SandboxPermissions,
    pub additional_permissions: Option<PermissionProfile>,
    pub justification: Option<String>,
    pub tty: bool,
    pub reason: Option<String>,
    pub network_approval_context: Option<NetworkApprovalContext>,
    pub proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
}

#[derive(Clone, Debug)]
pub(crate) struct ApplyPatchApprovalRequest {
    pub call_id: String,
    pub cwd: PathBuf,
    pub files: Vec<AbsolutePathBuf>,
    pub patch: String,
    pub changes: HashMap<PathBuf, FileChange>,
    pub reason: Option<String>,
    pub grant_root: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub(crate) struct NetworkAccessApprovalRequest {
    pub call_id: String,
    pub turn_id: String,
    pub target: String,
    pub host: String,
    pub protocol: NetworkApprovalProtocol,
    pub port: u16,
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub reason: Option<String>,
    pub network_approval_context: NetworkApprovalContext,
}

#[cfg(unix)]
#[derive(Clone, Debug)]
pub(crate) struct ExecveApprovalRequest {
    pub call_id: String,
    pub approval_id: Option<String>,
    pub source: GuardianCommandSource,
    pub program: String,
    pub argv: Vec<String>,
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub additional_permissions: Option<PermissionProfile>,
    pub reason: Option<String>,
    pub available_decisions: Vec<ReviewDecision>,
}

pub(crate) async fn request_approval(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    request: ApprovalRequest,
) -> ApprovalOutcome {
    let routes_to_guardian = routes_approval_to_guardian(turn);
    if routes_to_guardian {
        return request_guardian_approval(session, turn, request).await;
    }

    let ApprovalRequest {
        intent,
        retry_reason,
        cache,
    } = request;

    if should_consult_session_cache(routes_to_guardian, retry_reason.as_ref(), cache.as_ref())
        && let Some(cache) = &cache
        && cache.is_approved_for_session(session).await
    {
        return ApprovalOutcome::new(ReviewDecision::ApprovedForSession);
    }

    let decision = request_user_approval(session, turn, intent).await;
    let outcome = ApprovalOutcome::new(decision);

    if let Some(cache) = &cache {
        cache.record_outcome(session, &outcome).await;
    }

    outcome
}

fn should_consult_session_cache(
    routes_to_guardian: bool,
    retry_reason: Option<&String>,
    cache: Option<&ApprovalCache>,
) -> bool {
    !routes_to_guardian && retry_reason.is_none() && cache.is_some()
}

async fn request_guardian_approval(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    request: ApprovalRequest,
) -> ApprovalOutcome {
    let guardian_review_id = new_guardian_review_id();
    let decision = review_approval_request(
        session,
        turn,
        guardian_review_id.clone(),
        request.intent.into_guardian_request(),
        request.retry_reason,
    )
    .await;
    ApprovalOutcome {
        decision,
        guardian_review_id: Some(guardian_review_id),
    }
}

async fn request_user_approval(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    intent: ApprovalIntent,
) -> ReviewDecision {
    match intent.into_user_request() {
        UserApprovalRequest::Command(command) => {
            let CommandApprovalRequest {
                call_id,
                approval_id,
                command,
                cwd,
                reason,
                network_approval_context,
                proposed_execpolicy_amendment,
                additional_permissions,
                available_decisions,
            } = command;
            session
                .request_command_approval(
                    turn,
                    call_id,
                    approval_id,
                    command,
                    cwd,
                    reason,
                    network_approval_context,
                    proposed_execpolicy_amendment,
                    additional_permissions,
                    available_decisions,
                )
                .await
        }
        UserApprovalRequest::Patch(patch) => {
            let PatchApprovalRequest {
                call_id,
                changes,
                reason,
                grant_root,
            } = patch;
            session
                .request_patch_approval(turn, call_id, changes, reason, grant_root)
                .await
                .await
                .unwrap_or_default()
        }
    }
}

#[derive(Debug, PartialEq)]
enum UserApprovalRequest {
    Command(CommandApprovalRequest),
    Patch(PatchApprovalRequest),
}

#[derive(Debug, PartialEq)]
struct CommandApprovalRequest {
    call_id: String,
    approval_id: Option<String>,
    command: Vec<String>,
    cwd: PathBuf,
    reason: Option<String>,
    network_approval_context: Option<NetworkApprovalContext>,
    proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
    additional_permissions: Option<PermissionProfile>,
    available_decisions: Option<Vec<ReviewDecision>>,
}

#[derive(Debug, PartialEq)]
struct PatchApprovalRequest {
    call_id: String,
    changes: HashMap<PathBuf, FileChange>,
    reason: Option<String>,
    grant_root: Option<PathBuf>,
}

impl ApprovalIntent {
    fn into_guardian_request(self) -> GuardianApprovalRequest {
        match self {
            Self::Shell(shell) => GuardianApprovalRequest::Shell {
                id: shell.call_id,
                command: shell.command,
                cwd: shell.cwd,
                sandbox_permissions: shell.sandbox_permissions,
                additional_permissions: shell.additional_permissions,
                justification: shell.justification,
            },
            Self::UnifiedExec(exec) => GuardianApprovalRequest::ExecCommand {
                id: exec.call_id,
                command: exec.command,
                cwd: exec.cwd,
                sandbox_permissions: exec.sandbox_permissions,
                additional_permissions: exec.additional_permissions,
                justification: exec.justification,
                tty: exec.tty,
            },
            Self::ApplyPatch(patch) => GuardianApprovalRequest::ApplyPatch {
                id: patch.call_id,
                cwd: patch.cwd,
                files: patch.files,
                patch: patch.patch,
            },
            Self::NetworkAccess(network) => GuardianApprovalRequest::NetworkAccess {
                id: network.call_id,
                turn_id: network.turn_id,
                target: network.target,
                host: network.host,
                protocol: network.protocol,
                port: network.port,
            },
            #[cfg(unix)]
            Self::Execve(execve) => GuardianApprovalRequest::Execve {
                id: execve.call_id,
                source: execve.source,
                program: execve.program,
                argv: execve.argv,
                cwd: execve.cwd,
                additional_permissions: execve.additional_permissions,
            },
        }
    }

    fn into_user_request(self) -> UserApprovalRequest {
        match self {
            Self::Shell(shell) => UserApprovalRequest::Command(CommandApprovalRequest {
                call_id: shell.call_id,
                approval_id: None,
                command: shell.command,
                cwd: shell.cwd,
                reason: shell.reason,
                network_approval_context: shell.network_approval_context,
                proposed_execpolicy_amendment: shell.proposed_execpolicy_amendment,
                additional_permissions: shell.additional_permissions,
                available_decisions: None,
            }),
            Self::UnifiedExec(exec) => UserApprovalRequest::Command(CommandApprovalRequest {
                call_id: exec.call_id,
                approval_id: None,
                command: exec.command,
                cwd: exec.cwd,
                reason: exec.reason,
                network_approval_context: exec.network_approval_context,
                proposed_execpolicy_amendment: exec.proposed_execpolicy_amendment,
                additional_permissions: exec.additional_permissions,
                available_decisions: None,
            }),
            Self::ApplyPatch(patch) => UserApprovalRequest::Patch(PatchApprovalRequest {
                call_id: patch.call_id,
                changes: patch.changes,
                reason: patch.reason,
                grant_root: patch.grant_root,
            }),
            Self::NetworkAccess(network) => UserApprovalRequest::Command(CommandApprovalRequest {
                call_id: network.call_id,
                approval_id: None,
                command: network.command,
                cwd: network.cwd,
                reason: network.reason,
                network_approval_context: Some(network.network_approval_context),
                proposed_execpolicy_amendment: None,
                additional_permissions: None,
                available_decisions: None,
            }),
            #[cfg(unix)]
            Self::Execve(execve) => UserApprovalRequest::Command(CommandApprovalRequest {
                call_id: execve.call_id,
                approval_id: execve.approval_id,
                command: execve.command,
                cwd: execve.cwd,
                reason: execve.reason,
                network_approval_context: None,
                proposed_execpolicy_amendment: None,
                additional_permissions: execve.additional_permissions,
                available_decisions: Some(execve.available_decisions),
            }),
        }
    }
}

#[cfg(test)]
#[path = "approval_router_tests.rs"]
mod tests;
