use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;

use codex_extension_api::ConversationHistorySnapshot;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::GuardianAssessmentEvent;
use serde_json::json;

use super::ContextualUserFragment;
use crate::codex_thread::GuardianAuthorizationVersion;

const MAX_RETAINED_REVIEWS: usize = 8;

/// Trusted user answers and completed reviews retained only for this thread's Guardian reviewers.
///
/// This runtime-only evidence is never inserted into the agent's conversation or
/// inherited by another thread. Authorization changes make stale records ineligible.
#[derive(Debug, Default)]
pub struct GuardianReviewEvidence(Mutex<GuardianReviewEvidenceState>);

#[derive(Debug, Default)]
struct GuardianReviewEvidenceState {
    reviews: VecDeque<Arc<GuardianReviewEvidenceRecord>>,
    user_inputs: VecDeque<(String, String)>,
    user_input_response_count: usize,
}

impl GuardianReviewEvidence {
    /// Records a bounded user answer before post-tool hooks can replace or reject its output.
    pub(crate) fn record_user_input(&self, call_id: &str, fragment: String) {
        let mut state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        state.user_input_response_count = state.user_input_response_count.saturating_add(1);
        state.user_inputs.push_back((call_id.to_owned(), fragment));
        while state.user_inputs.len() > MAX_RETAINED_REVIEWS {
            state.user_inputs.pop_front();
        }
    }

    /// Captures history changes and host-recorded user answers for one reviewer decision.
    pub fn authorization_version(
        &self,
        history: &dyn ConversationHistorySnapshot,
    ) -> GuardianAuthorizationVersion {
        let user_input_response_count = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .user_input_response_count;
        GuardianAuthorizationVersion {
            user_input_response_count,
            ..GuardianAuthorizationVersion::from_history(history)
        }
    }

    /// Returns bounded answers whose original, host-observed tool calls remain in history.
    pub fn user_input_fragments(&self, history: &dyn ConversationHistorySnapshot) -> Vec<String> {
        let state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        state
            .user_inputs
            .iter()
            .filter(|(recorded_call_id, _)| {
                history.items().any(|item| {
                    matches!(
                        item,
                        ResponseItem::FunctionCall { call_id, .. }
                            if call_id == recorded_call_id
                    )
                })
            })
            .map(|(_, fragment)| fragment.clone())
            .collect()
    }

    pub(crate) fn user_input_for_call(&self, call_id: &str) -> Option<String> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .user_inputs
            .iter()
            .find_map(|(recorded_call_id, fragment)| {
                (recorded_call_id == call_id).then(|| fragment.clone())
            })
    }

    /// Records a genuine allow/deny assessment, not a timeout or fail-closed error.
    pub(crate) fn record(
        &self,
        assessment: &GuardianAssessmentEvent,
        action: &str,
        authorization_version: GuardianAuthorizationVersion,
        root_authorization_version: Option<GuardianAuthorizationVersion>,
    ) {
        let Some(completed_at_ms) = assessment.completed_at_ms else {
            return;
        };
        let review = Arc::new(GuardianReviewEvidenceRecord {
            completed_at_ms,
            authorization_version,
            root_authorization_version,
            correlation: json!({
                "review_id": assessment.id,
                "turn_id": assessment.turn_id,
                "target_item_id": assessment.target_item_id,
                "completed_at_ms": completed_at_ms,
            }),
            decision: json!({
                "status": assessment.status,
                "risk_level": assessment.risk_level,
                "user_authorization": assessment.user_authorization,
            }),
            action: action.to_owned(),
            rationale: assessment.rationale.clone(),
        });
        let mut state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        state.reviews.push_back(review);
        state
            .reviews
            .make_contiguous()
            .sort_by_key(|review| review.completed_at_ms);
        while state.reviews.len() > MAX_RETAINED_REVIEWS {
            state.reviews.pop_front();
        }
    }

    /// Freezes the latest completed reviews, oldest first, for one classifier sample.
    pub fn snapshot(&self) -> Vec<Arc<GuardianReviewEvidenceRecord>> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .reviews
            .iter()
            .cloned()
            .collect()
    }
}

/// Structured synchronous-review evidence retained for Guardian V2 classification.
#[derive(Debug)]
pub struct GuardianReviewEvidenceRecord {
    pub authorization_version: GuardianAuthorizationVersion,
    pub root_authorization_version: Option<GuardianAuthorizationVersion>,
    completed_at_ms: i64,
    pub correlation: serde_json::Value,
    pub decision: serde_json::Value,
    pub action: String,
    pub rationale: Option<String>,
}

/// A bounded, host-supplied sync-review record for async classifier input only.
#[derive(Clone, Debug)]
pub struct GuardianReviewEvidenceFragment {
    body: String,
}

impl GuardianReviewEvidenceFragment {
    /// Creates a trusted fragment from classifier-bounded review evidence.
    pub fn new(body: String) -> Self {
        Self { body }
    }
}

impl ContextualUserFragment for GuardianReviewEvidenceFragment {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("guardian.review_evidence".to_string())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<guardian_sync_review>", "</guardian_sync_review>")
    }

    fn body(&self) -> String {
        self.body.clone()
    }
}
