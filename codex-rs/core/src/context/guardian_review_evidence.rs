//! Selects Guardian answer evidence from the review's history snapshot and retains reviews.
//! Retained answers survive restart even while incompatible checkpoints use legacy review.

use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;

use codex_extension_api::ConversationHistorySnapshot;
use codex_guardian_context::MAX_PREVIOUS_REVIEWS;
use codex_protocol::models::ContentItemKind;
use codex_protocol::protocol::GuardianAssessmentEvent;
use serde_json::json;

use super::ContextualUserFragment;
use crate::codex_thread::GuardianAuthorizationVersion;

const MAX_TRUSTED_SKILLS: usize = 16;
const MAX_TRUSTED_SKILL_PATHS_BYTES: usize = 2_048;

/// Selected answer fragments and the authorization state they describe.
pub struct GuardianUserInputSnapshot {
    pub fragments: Vec<String>,
    pub authorization_version: GuardianAuthorizationVersion,
}

/// Selected answer evidence, verified skill paths, and completed Guardian reviews.
///
/// This runtime-only evidence is never inserted into the agent's conversation.
/// Only bounded, turn-matched skill paths are exposed to delegated workers;
/// completed reviews remain thread-local, and authorization changes invalidate stale records.
#[derive(Debug, Default)]
pub struct GuardianReviewEvidence {
    state: Mutex<GuardianReviewEvidenceState>,
}

#[derive(Debug, Default)]
struct GuardianReviewEvidenceState {
    reviews: VecDeque<Arc<GuardianReviewEvidenceRecord>>,
    trusted_skill_turn_id: Option<String>,
    trusted_skill_paths: BTreeSet<String>,
}

impl GuardianReviewEvidence {
    /// Reads the selected answer path against the caller's action-time history snapshot.
    pub fn user_input_snapshot(
        &self,
        history: &dyn ConversationHistorySnapshot,
    ) -> GuardianUserInputSnapshot {
        match history.retained_context() {
            Some(context) => {
                let answers = codex_guardian_context::render_verified_answers(context);
                let authorization_version = GuardianAuthorizationVersion {
                    user_message_revision: history.user_message_revision(),
                    retained_context_complete: answers.complete,
                };
                GuardianUserInputSnapshot {
                    fragments: answers.fragments,
                    authorization_version,
                }
            }
            None => GuardianUserInputSnapshot {
                fragments: Vec::new(),
                authorization_version: GuardianAuthorizationVersion {
                    user_message_revision: history.user_message_revision(),
                    retained_context_complete: true,
                },
            },
        }
    }

    pub fn authorization_version(
        &self,
        history: &dyn ConversationHistorySnapshot,
    ) -> GuardianAuthorizationVersion {
        self.user_input_snapshot(history).authorization_version
    }

    /// Records a bounded, verified user-owned skill path for one host-owned turn.
    pub fn record_trusted_skill(&self, turn_id: &str, path: String) {
        if turn_id.is_empty() {
            return;
        }
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.trusted_skill_turn_id.as_deref() != Some(turn_id) {
            state.trusted_skill_turn_id = Some(turn_id.to_owned());
            state.trusted_skill_paths.clear();
        }
        if state.trusted_skill_paths.contains(&path)
            || state.trusted_skill_paths.len() >= MAX_TRUSTED_SKILLS
            || state
                .trusted_skill_paths
                .iter()
                .map(String::len)
                .sum::<usize>()
                .saturating_add(path.len())
                > MAX_TRUSTED_SKILL_PATHS_BYTES
        {
            return;
        }
        state.trusted_skill_paths.insert(path);
    }

    /// Returns verified skill paths only for their original host-owned turn.
    pub fn trusted_skill_paths(&self, turn_id: &str) -> Vec<String> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.trusted_skill_turn_id.as_deref() != Some(turn_id) {
            return Vec::new();
        }
        state.trusted_skill_paths.iter().cloned().collect()
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
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.reviews.push_back(review);
        state
            .reviews
            .make_contiguous()
            .sort_by_key(|review| review.completed_at_ms);
        while state.reviews.len() > MAX_PREVIOUS_REVIEWS {
            state.reviews.pop_front();
        }
    }

    /// Freezes the latest completed reviews, oldest first, for one classifier sample.
    pub fn snapshot(&self) -> Vec<Arc<GuardianReviewEvidenceRecord>> {
        self.state
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
