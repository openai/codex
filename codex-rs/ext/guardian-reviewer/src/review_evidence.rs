//! Selects Guardian answer evidence from the review's history snapshot and retains reviews.
//! Retained answers survive restart even while incompatible checkpoints use legacy review.

use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;

use codex_extension_api::ConversationHistorySnapshot;
use codex_guardian_context::GuardianRootMessage;
use codex_guardian_context::MAX_PREVIOUS_REVIEWS;
use codex_guardian_context::PreparedRootContext;
use codex_guardian_context::RootConversationInput;
use codex_protocol::ThreadId;
use codex_protocol::protocol::GuardianAssessmentEvent;
use serde_json::json;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;

const MAX_TRUSTED_SKILLS: usize = 16;
const MAX_TRUSTED_SKILL_PATHS_BYTES: usize = 2_048;

/// Authorization state that changes on genuine user input or history resets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GuardianAuthorizationVersion {
    /// User-message/reset revision, preserved across compaction and internal context.
    pub user_message_revision: u64,
    /// False when required retained answers or root instructions are unavailable.
    pub retained_context_complete: bool,
}

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
}

#[derive(Debug, Default)]
struct GuardianReviewEvidenceState {
    reviews: VecDeque<Arc<GuardianReviewEvidenceRecord>>,
    trusted_skill_turn_id: Option<String>,
    trusted_skill_paths: BTreeSet<String>,
}

impl GuardianReviewEvidence {
    /// Projects prepared root evidence without accessing the host runtime.
    pub fn root_snapshot(
        &self,
        root_thread_id: ThreadId,
        history_reset_version: u64,
        history: &dyn ConversationHistorySnapshot,
        prepared: PreparedRootContext,
        request_candidates: RootConversationInput,
    ) -> GuardianRootSnapshot {
        let mut authorization_version = self.authorization_version(history);
        if prepared.missing_root_instructions {
            authorization_version.retained_context_complete = false;
        }
        let mut review_context_revision = history.guardian_review_context_revision();
        if let Some(selected) = &prepared.handoff_selection {
            let mut revision = DefaultHasher::new();
            review_context_revision.hash(&mut revision);
            selected.hash(&mut revision);
            review_context_revision = revision.finish();
        }
        let trusted_skill_paths = prepared
            .latest_user_turn_id
            .as_deref()
            .map(|turn_id| self.trusted_skill_paths(turn_id))
            .unwrap_or_default();
        GuardianRootSnapshot {
            root_thread_id,
            history_reset_version,
            authorization_version,
            review_context_revision,
            messages: prepared.messages,
            trusted_skill_paths,
            request_candidates: Some(request_candidates),
        }
    }

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
    pub fn record(
        &self,
        assessment: &GuardianAssessmentEvent,
        action: &str,
        authorization_version: GuardianAuthorizationVersion,
        review_context_revision: u64,
        root_authorization_version: Option<GuardianAuthorizationVersion>,
        root_review_context_revision: Option<u64>,
    ) {
        let Some(completed_at_ms) = assessment.completed_at_ms else {
            return;
        };
        let review = Arc::new(GuardianReviewEvidenceRecord {
            delivery_id: codex_protocol::ResponseItemId::new("guardian_review"),
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
