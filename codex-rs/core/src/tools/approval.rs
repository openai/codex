//! Shared approval coordination helpers used across tool and non-tool flows.

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::guardian::GuardianApprovalRequest;
use crate::guardian::new_guardian_review_id;
use crate::guardian::review_approval_request;
use crate::guardian::routes_approval_to_guardian;
use crate::state::SessionServices;
use codex_protocol::protocol::ReviewDecision;
use futures::Future;
use serde::Serialize;
use std::collections::HashMap;
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
        let serialized_key = serde_json::to_string(key).ok()?;
        self.map.get(&serialized_key).cloned()
    }

    pub fn put<K>(&mut self, key: K, value: ReviewDecision)
    where
        K: Serialize,
    {
        if let Ok(serialized_key) = serde_json::to_string(&key) {
            self.map.insert(serialized_key, value);
        }
    }
}

pub(crate) struct ApprovalOutcome {
    pub decision: ReviewDecision,
    pub guardian_review_id: Option<String>,
}

pub(crate) fn guardian_review_id_for_turn(turn: &TurnContext) -> Option<String> {
    routes_approval_to_guardian(turn).then(new_guardian_review_id)
}

/// Returns an approve-for-session decision when every key is already cached,
/// otherwise calls `fetch` and stores any new session approval per key.
pub(crate) async fn with_cached_approval<K, F, Fut>(
    services: &SessionServices,
    tool_name: &str,
    keys: Vec<K>,
    fetch: F,
) -> ReviewDecision
where
    K: Serialize,
    F: FnOnce() -> Fut,
    Fut: Future<Output = ReviewDecision>,
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
        return ReviewDecision::ApprovedForSession;
    }

    let decision = fetch().await;

    services.session_telemetry.counter(
        "codex.approval.requested",
        /*inc*/ 1,
        &[
            ("tool", tool_name),
            ("approved", decision.to_opaque_string()),
        ],
    );

    if matches!(decision, ReviewDecision::ApprovedForSession) {
        let mut store = services.tool_approvals.lock().await;
        for key in keys {
            store.put(key, ReviewDecision::ApprovedForSession);
        }
    }

    decision
}

pub(crate) async fn request_approval<F, UserFut>(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    guardian_review_id: Option<String>,
    guardian_request: GuardianApprovalRequest,
    retry_reason: Option<String>,
    request_user: F,
) -> ApprovalOutcome
where
    F: FnOnce() -> UserFut,
    UserFut: Future<Output = ReviewDecision>,
{
    if let Some(review_id) = guardian_review_id.clone() {
        return ApprovalOutcome {
            decision: review_approval_request(
                session,
                turn,
                review_id,
                guardian_request,
                retry_reason,
            )
            .await,
            guardian_review_id,
        };
    }

    ApprovalOutcome {
        decision: request_user().await,
        guardian_review_id: None,
    }
}

pub(crate) async fn request_approval_for_turn<F, UserFut>(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    guardian_request: GuardianApprovalRequest,
    retry_reason: Option<String>,
    request_user: F,
) -> ApprovalOutcome
where
    F: FnOnce() -> UserFut,
    UserFut: Future<Output = ReviewDecision>,
{
    request_approval(
        session,
        turn,
        guardian_review_id_for_turn(turn),
        guardian_request,
        retry_reason,
        request_user,
    )
    .await
}
