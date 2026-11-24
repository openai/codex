//! Selects Guardian answer evidence from the review's history snapshot and retains reviews.
//! Retained answers survive restart even while incompatible checkpoints use legacy review.

use crate::GuardianEvidenceChanges;
use codex_thread_context_ids::ThreadContextIds;
use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;

use codex_extension_api::ConversationHistorySnapshot;
pub use codex_guardian_context::GuardianAuthorizationVersion;
use codex_guardian_context::GuardianRootMessage;
use codex_guardian_context::MAX_PREVIOUS_REVIEWS;
use codex_protocol::ThreadId;
use codex_protocol::protocol::GuardianAssessmentEvent;
use serde_json::json;

const MAX_TRUSTED_SKILLS: usize = 16;
const MAX_TRUSTED_SKILL_PATHS_BYTES: usize = 2_048;

/// Root authorization state, selected messages and request-local originals from one snapshot.
#[derive(Debug, PartialEq)]
pub struct GuardianRootSnapshot {
    /// Authoritative root from which this evidence was captured.
    pub root_thread_id: ThreadId,
    /// Distinguishes a history reset from additional authorization in the same history.
    pub history_reset_version: u64,
    pub authorization_version: GuardianAuthorizationVersion,
    /// Confirmed assistant context is relevant to review but cannot authorize an action.
    pub review_context_revision: u64,
    pub messages: Vec<GuardianRootMessage>,
    /// Request-local originals retained before the bounded compatibility projection.
    pub request_candidates: Option<codex_guardian_context::RootConversationInput>,
    pub trusted_skill_paths: Vec<String>,
}

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
    /// Shared notifications for committed history and verified skill changes.
    pub changes: GuardianEvidenceChanges,
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
                    root_runtime_id: None,
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
                    root_runtime_id: None,
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
        let changed_turn = state.trusted_skill_turn_id.as_deref() != Some(turn_id);
        if changed_turn {
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
            if changed_turn {
                self.changes.notify();
            }
            return;
        }
        state.trusted_skill_paths.insert(path);
        self.changes.notify();
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
    pub fn record(
        &self,
        assessment: &GuardianAssessmentEvent,
        action: &str,
        authorization_version: GuardianAuthorizationVersion,
        review_context_revision: u64,
        root_versions: Option<(GuardianAuthorizationVersion, u64)>,
        context_ids: &ThreadContextIds,
    ) {
        let Some(completed_at_ms) = assessment.completed_at_ms else {
            return;
        };
        let (root_authorization_version, root_review_context_revision) = root_versions.unzip();
        let review = Arc::new(GuardianReviewEvidenceRecord {
            delivery_id: context_ids.next_item("guardian_review"),
            completed_at_ms,
            authorization_version,
            review_context_revision,
            root_authorization_version,
            root_review_context_revision,
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
    /// Identifies this immutable completion even if the same action is reviewed again.
    pub delivery_id: codex_protocol::ResponseItemId,
    pub authorization_version: GuardianAuthorizationVersion,
    pub review_context_revision: u64,
    pub root_authorization_version: Option<GuardianAuthorizationVersion>,
    pub root_review_context_revision: Option<u64>,
    completed_at_ms: i64,
    pub correlation: serde_json::Value,
    pub decision: serde_json::Value,
    pub action: String,
    pub rationale: Option<String>,
}
