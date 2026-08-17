use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::PoisonError;

use codex_protocol::user_input::UserInput;
use codex_utils_string::take_bytes_at_char_boundary;

use super::ShadowQuery;

const MAX_PRIOR_REQUESTS: usize = 2;
const MAX_REQUEST_BYTES: usize = 2 * 1024;
const MAX_QUERY_BYTES: usize = 4 * 1024;
const MAX_RECENT_SKILLS: usize = 50;

/// Shadow-only relevance history. New and reconstructed thread runtimes start cold.
#[derive(Default)]
pub(crate) struct ShadowTaskContext(Mutex<TaskContextState>);

#[derive(Default)]
struct TaskContextState {
    prior_requests: VecDeque<ShadowQuery>,
    recent_skills: VecDeque<String>,
    pending: Option<PendingTurn>,
}

struct PendingTurn {
    id: String,
    request: Option<ShadowQuery>,
    recent_skills: VecDeque<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct TaskContextSnapshot {
    pub(super) query: ShadowQuery,
    pub(super) recent_skills: Vec<String>,
}

impl ShadowTaskContext {
    pub(super) fn begin_turn(
        &self,
        turn_id: &str,
        current: &ShadowQuery,
        inputs: &[UserInput],
    ) -> TaskContextSnapshot {
        let mut state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if state.pending.as_ref().is_none_or(|turn| turn.id != turn_id) {
            if let Some(previous) = state.pending.take() {
                if let Some(request) = previous.request {
                    state
                        .prior_requests
                        .retain(|prior| prior.text != request.text);
                    state.prior_requests.push_front(request);
                    state.prior_requests.truncate(MAX_PRIOR_REQUESTS);
                }
                for resource in previous.recent_skills.into_iter().rev() {
                    remember_skill(&mut state.recent_skills, resource);
                }
            }
            state.pending = Some(PendingTurn {
                id: turn_id.to_string(),
                request: None,
                recent_skills: VecDeque::new(),
            });
        }

        let retained = take_bytes_at_char_boundary(&current.text, MAX_REQUEST_BYTES);
        let text = take_bytes_at_char_boundary(&current.text, MAX_QUERY_BYTES);
        let mut query = ShadowQuery {
            text: text.to_string(),
            truncated: current.truncated || text.len() < current.text.len(),
        };
        for prior in &state.prior_requests {
            if prior.text == retained {
                continue;
            }
            if !query.text.is_empty() && query.text.len() < MAX_QUERY_BYTES {
                query.text.push('\n');
            }
            let part = take_bytes_at_char_boundary(
                &prior.text,
                MAX_QUERY_BYTES.saturating_sub(query.text.len()),
            );
            query.text.push_str(part);
            query.truncated |= prior.truncated || part.len() < prior.text.len();
            if part.len() < prior.text.len() {
                break;
            }
        }
        let recent_skills = state.recent_skills.iter().cloned().collect();
        if is_substantive(&current.text, inputs)
            && let Some(pending) = state.pending.as_mut()
        {
            pending.request = Some(ShadowQuery {
                text: retained.to_string(),
                truncated: current.truncated || retained.len() < current.text.len(),
            });
        }
        TaskContextSnapshot {
            query,
            recent_skills,
        }
    }

    /// Records relevance evidence for future turns, never the active turn's predictions.
    pub(super) fn record(&self, turn_id: &str, resource: String) {
        let mut state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(pending) = state.pending.as_mut().filter(|turn| turn.id == turn_id) {
            remember_skill(&mut pending.recent_skills, resource);
        }
    }
}

fn remember_skill(skills: &mut VecDeque<String>, resource: String) {
    skills.retain(|previous| previous != &resource);
    skills.push_front(resource);
    skills.truncate(MAX_RECENT_SKILLS);
}

fn is_substantive(text: &str, inputs: &[UserInput]) -> bool {
    if inputs.iter().any(|input| {
        matches!(input, UserInput::Skill { name, .. } | UserInput::Mention { name, .. } if !name.trim().is_empty())
    }) {
        return true;
    }
    let normalized = text
        .split(|character: char| !character.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ");
    !matches!(
        normalized.as_str(),
        "" | "yes"
            | "yep"
            | "yeah"
            | "ok"
            | "okay"
            | "sure"
            | "go"
            | "go ahead"
            | "continue"
            | "please continue"
            | "proceed"
            | "do it"
            | "do that"
            | "try again"
            | "retry"
            | "thanks"
            | "thank you"
    )
}

#[cfg(test)]
#[path = "task_context_tests.rs"]
mod tests;
