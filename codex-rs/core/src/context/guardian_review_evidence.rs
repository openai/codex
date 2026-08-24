use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;

use codex_protocol::protocol::GuardianAssessmentEvent;
use serde_json::json;

use super::ContextualUserFragment;
use crate::codex_thread::GuardianAuthorizationVersion;
use codex_protocol::models::ContentItemKind;

const MAX_RETAINED_REVIEWS: usize = 8;

/// Completed synchronous reviews retained only for this thread's async classifier.
///
/// This runtime-only evidence is never inserted into the agent's conversation or
/// inherited by another thread. Authorization changes make stale records ineligible.
#[derive(Debug, Default)]
pub struct GuardianReviewEvidence(Mutex<VecDeque<Arc<GuardianReviewEvidenceRecord>>>);

impl GuardianReviewEvidence {
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
        let mut reviews = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        reviews.push_back(review);
        reviews
            .make_contiguous()
            .sort_by_key(|review| review.completed_at_ms);
        while reviews.len() > MAX_RETAINED_REVIEWS {
            reviews.pop_front();
        }
    }

    /// Freezes the latest completed reviews, oldest first, for one classifier sample.
    pub fn snapshot(&self) -> Vec<Arc<GuardianReviewEvidenceRecord>> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
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
