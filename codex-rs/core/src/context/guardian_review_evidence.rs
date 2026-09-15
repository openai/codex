//! Selects Guardian answer evidence from the review's history snapshot and retains reviews.
//! Legacy review uses a bounded runtime buffer while thread-owned review uses retained answers.
//! A migrating session captures both until its checkpoint can support thread-owned review.

use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;

use codex_extension_api::ConversationHistorySnapshot;
use codex_guardian_context::MAX_PREVIOUS_REVIEWS;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::GuardianAssessmentEvent;
use codex_protocol::request_user_input::RequestUserInputQuestion;
use codex_protocol::request_user_input::RequestUserInputResponse;
use serde_json::json;

use super::ContextualUserFragment;
use super::GuardianContextMode;
use crate::codex_thread::GuardianAuthorizationVersion;
use crate::codex_thread::GuardianRootMessage;
use crate::guardian::guardian_truncate_text;

const MAX_RETAINED_USER_INPUTS: usize = 8;
const MAX_TRUSTED_SKILLS: usize = 16;
const MAX_TRUSTED_SKILL_PATHS_BYTES: usize = 2_048;
const MAX_GUARDIAN_USER_INPUT_ANSWERS: usize = 8;
const MAX_GUARDIAN_USER_INPUT_TOKENS: usize = 900;

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
    user_inputs: VecDeque<(String, String)>,
    user_input_response_count: usize,
    trusted_skill_turn_id: Option<String>,
    trusted_skill_paths: BTreeSet<String>,
}

impl GuardianReviewEvidence {
    /// Preserves the legacy capture limits before hooks can replace the tool output.
    pub(crate) fn record_user_input(
        &self,
        history: &dyn ConversationHistorySnapshot,
        call_id: &str,
        questions: &[RequestUserInputQuestion],
        response: &RequestUserInputResponse,
    ) {
        if GuardianContextMode::from_history(history) != GuardianContextMode::Legacy {
            return;
        }
        let fragment = questions
            .iter()
            .filter_map(|question| {
                let response = response.answers.get(&question.id)?;
                let answers = response
                    .answers
                    .iter()
                    .filter(|answer| !answer.trim().is_empty())
                    .take(MAX_GUARDIAN_USER_INPUT_ANSWERS)
                    .cloned()
                    .collect::<Vec<_>>();
                if answers.is_empty() {
                    return None;
                }
                let mut question_text = question.question.clone();
                for option in question
                    .options
                    .iter()
                    .flatten()
                    .filter(|option| response.answers.contains(&option.label))
                    .take(MAX_GUARDIAN_USER_INPUT_ANSWERS)
                {
                    question_text.push_str(&format!("\n{}: {}", option.label, option.description));
                }
                Some(format!(
                    "{}{}",
                    GuardianRootMessage::Assistant(question_text).render(),
                    GuardianRootMessage::User(answers.join("\n")).render()
                ))
            })
            .take(MAX_GUARDIAN_USER_INPUT_ANSWERS)
            .collect::<String>();
        if fragment.is_empty() {
            return;
        }
        let fragment = guardian_truncate_text(&fragment, MAX_GUARDIAN_USER_INPUT_TOKENS).0;
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.user_input_response_count = state.user_input_response_count.saturating_add(1);
        state.user_inputs.push_back((call_id.to_owned(), fragment));
        while state.user_inputs.len() > MAX_RETAINED_USER_INPUTS {
            state.user_inputs.pop_front();
        }
    }

    /// Reads the selected answer path against the caller's action-time history snapshot.
    pub fn user_input_snapshot(
        &self,
        history: &dyn ConversationHistorySnapshot,
    ) -> GuardianUserInputSnapshot {
        match GuardianContextMode::from_history(history) {
            GuardianContextMode::ThreadOwned => {
                let answers = history
                    .retained_context()
                    .map(codex_guardian_context::render_verified_answers);
                let authorization_version = GuardianAuthorizationVersion {
                    user_message_revision: history.user_message_revision(),
                    user_input_response_count: 0,
                    retained_context_complete: answers
                        .as_ref()
                        .is_none_or(|answers| answers.complete),
                };
                GuardianUserInputSnapshot {
                    fragments: answers.map(|answers| answers.fragments).unwrap_or_default(),
                    authorization_version,
                }
            }
            GuardianContextMode::Legacy => {
                let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
                let fragments = state
                    .user_inputs
                    .iter()
                    .filter(|(call_id, _)| {
                        history.items().chain(history.review_items()).any(|item| {
                            matches!(item, ResponseItem::FunctionCall { call_id: id, .. } if id == call_id)
                        })
                    })
                    .map(|(_, fragment)| fragment.clone())
                    .collect();
                GuardianUserInputSnapshot {
                    fragments,
                    authorization_version: GuardianAuthorizationVersion {
                        user_message_revision: history.user_message_revision(),
                        user_input_response_count: state.user_input_response_count,
                        retained_context_complete: true,
                    },
                }
            }
        }
    }

    pub fn authorization_version(
        &self,
        history: &dyn ConversationHistorySnapshot,
    ) -> GuardianAuthorizationVersion {
        self.user_input_snapshot(history).authorization_version
    }

    pub(crate) fn user_input_for_call(
        &self,
        history: &dyn ConversationHistorySnapshot,
        call_id: &str,
    ) -> Option<String> {
        match GuardianContextMode::from_history(history) {
            GuardianContextMode::ThreadOwned => history
                .retained_context()?
                .verified_answers()
                .find(|answer| answer.call_id == call_id)
                .and_then(codex_guardian_context::render_verified_answer),
            GuardianContextMode::Legacy => self
                .state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .user_inputs
                .iter()
                .find_map(|(id, fragment)| (id == call_id).then(|| fragment.clone())),
        }
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
