//! Shared approval coordination primitives used across tool and non-tool flows.
//!
//! This module centralizes:
//! - session-scoped approval caching for "approve for session" decisions
//! - routing approval prompts to the user or guardian reviewer
//! - dispatching typed user/guardian approval prompts
//! - returning prompt metadata needed by caller-specific result handling

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::guardian::GuardianApprovalRequest;
use crate::guardian::new_guardian_review_id;
use crate::guardian::review_approval_request;
use crate::guardian::routes_approval_to_guardian;
use crate::hook_runtime::run_permission_request_hooks;
use crate::state::SessionServices;
use codex_protocol::approvals::ExecPolicyAmendment;
use codex_protocol::approvals::NetworkApprovalContext;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::FileChange;
use codex_protocol::protocol::ReviewDecision;
use futures::Future;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Default, Debug)]
pub(crate) struct ApprovalStore {
    map: HashMap<String, ReviewDecision>,
}

impl ApprovalStore {
    pub fn get<K>(&self, key: &K) -> Option<ReviewDecision>
    where
        K: Serialize,
    {
        let s = serde_json::to_string(key).ok()?;
        self.map.get(&s).cloned()
    }

    pub fn put<K>(&mut self, key: K, value: ReviewDecision)
    where
        K: Serialize,
    {
        if let Ok(s) = serde_json::to_string(&key) {
            self.map.insert(s, value);
        }
    }
}

#[derive(Debug)]
pub(crate) enum ApprovalCache<K> {
    None,
    SessionApproveOnly {
        tool_name: &'static str,
        keys: Vec<K>,
    },
}

#[derive(Debug)]
pub(crate) struct ApprovalOutcome {
    pub decision: ReviewDecision,
    pub guardian_review_id: Option<String>,
}

#[derive(Debug)]
pub(crate) struct CommandApprovalRequest {
    pub call_id: String,
    pub approval_id: Option<String>,
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub reason: Option<String>,
    pub network_approval_context: Option<NetworkApprovalContext>,
    pub proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
    pub additional_permissions: Option<PermissionProfile>,
    pub available_decisions: Option<Vec<ReviewDecision>>,
}

#[derive(Debug)]
pub(crate) struct PatchApprovalRequest {
    pub call_id: String,
    pub changes: HashMap<PathBuf, FileChange>,
    pub reason: Option<String>,
    pub grant_root: Option<PathBuf>,
}

#[derive(Debug)]
pub(crate) enum UserApprovalRequest {
    Command(CommandApprovalRequest),
    Patch(PatchApprovalRequest),
}

#[derive(Debug)]
pub(crate) struct GuardianApproval {
    pub request: GuardianApprovalRequest,
    pub retry_reason: Option<String>,
}

impl GuardianApproval {
    pub(crate) fn new(request: GuardianApprovalRequest, retry_reason: Option<String>) -> Self {
        Self {
            request,
            retry_reason,
        }
    }
}

#[derive(Debug)]
pub(crate) struct PermissionRequestHook {
    pub tool_name: &'static str,
    pub tool_input: Value,
    pub codex_permission_context: Value,
}

#[derive(Debug)]
pub(crate) struct ApprovalPlan<K> {
    pub cache: ApprovalCache<K>,
    pub user: UserApprovalRequest,
    pub guardian: GuardianApproval,
    pub hook: PermissionRequestHook,
}

pub(crate) fn guardian_review_id_for_turn(turn: &crate::codex::TurnContext) -> Option<String> {
    routes_approval_to_guardian(turn).then(new_guardian_review_id)
}

/// Takes a vector of approval keys and returns a ReviewDecision.
/// There will be one key in most cases, but apply_patch can modify multiple files at once.
///
/// - If all keys are already approved for session, we skip prompting.
/// - If the user approves for session, we store the decision for each key individually
///   so future requests touching any subset can also skip prompting.
pub(crate) async fn with_cached_approval<K, F, Fut>(
    services: &SessionServices,
    tool_name: &str,
    keys: Vec<K>,
    fetch: F,
) -> ApprovalOutcome
where
    K: Serialize,
    F: FnOnce() -> Fut,
    Fut: Future<Output = ApprovalOutcome>,
{
    if keys.is_empty() {
        return fetch().await;
    }

    let already_approved = {
        let store = services.tool_approvals.lock().await;
        keys.iter()
            .all(|key| matches!(store.get(key), Some(ReviewDecision::ApprovedForSession)))
    };

    if already_approved {
        return ApprovalOutcome {
            decision: ReviewDecision::ApprovedForSession,
            guardian_review_id: None,
        };
    }

    let outcome = fetch().await;

    services.session_telemetry.counter(
        "codex.approval.requested",
        /*inc*/ 1,
        &[
            ("tool", tool_name),
            ("approved", outcome.decision.to_opaque_string()),
        ],
    );

    if matches!(outcome.decision, ReviewDecision::ApprovedForSession) {
        let mut store = services.tool_approvals.lock().await;
        for key in keys {
            store.put(key, ReviewDecision::ApprovedForSession);
        }
    }

    outcome
}

async fn dispatch_user_approval(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    request: UserApprovalRequest,
) -> ReviewDecision {
    match request {
        UserApprovalRequest::Command(request) => {
            session
                .request_command_approval(
                    turn.as_ref(),
                    request.call_id,
                    request.approval_id,
                    request.command,
                    request.cwd,
                    request.reason,
                    request.network_approval_context,
                    request.proposed_execpolicy_amendment,
                    request.additional_permissions,
                    request.available_decisions,
                )
                .await
        }
        UserApprovalRequest::Patch(request) => {
            let rx_approve = session
                .request_patch_approval(
                    turn.as_ref(),
                    request.call_id,
                    request.changes,
                    request.reason,
                    request.grant_root,
                )
                .await;
            rx_approve.await.unwrap_or_default()
        }
    }
}

async fn request_uncached_approval(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    guardian_review_id: Option<String>,
    user: UserApprovalRequest,
    guardian: GuardianApproval,
    hook: PermissionRequestHook,
) -> ApprovalOutcome {
    if let Some(decision) = run_permission_request_hooks(session, turn, hook).await {
        return ApprovalOutcome {
            decision,
            guardian_review_id: None,
        };
    }

    if let Some(review_id) = guardian_review_id.clone() {
        return ApprovalOutcome {
            decision: review_approval_request(
                session,
                turn,
                review_id,
                guardian.request,
                guardian.retry_reason,
            )
            .await,
            guardian_review_id,
        };
    }

    ApprovalOutcome {
        decision: dispatch_user_approval(session, turn, user).await,
        guardian_review_id: None,
    }
}

pub(crate) async fn request_approval<K>(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    guardian_review_id: Option<String>,
    plan: ApprovalPlan<K>,
) -> ApprovalOutcome
where
    K: Serialize,
{
    let ApprovalPlan {
        cache,
        user,
        guardian,
        hook,
    } = plan;
    match cache {
        ApprovalCache::None => {
            request_uncached_approval(session, turn, guardian_review_id, user, guardian, hook).await
        }
        ApprovalCache::SessionApproveOnly { tool_name, keys } => {
            with_cached_approval(&session.services, tool_name, keys, || {
                request_uncached_approval(session, turn, guardian_review_id, user, guardian, hook)
            })
            .await
        }
    }
}

pub(crate) async fn request_approval_for_turn<K>(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    plan: ApprovalPlan<K>,
) -> ApprovalOutcome
where
    K: Serialize,
{
    request_approval(session, turn, guardian_review_id_for_turn(turn), plan).await
}
