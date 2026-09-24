//! A suggestion belongs to one live completed turn and never becomes draft text implicitly.
//! The composer owns cancellation; dropping it also cancels requests during thread switches.

use codex_protocol::ThreadId;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

// Desktop FALLBACK_PROMPT, fallback-1. Keep the source text unchanged.
pub(crate) const PROMPT: &str = include_str!("../assets/prompt_suggestion.txt");

#[derive(Clone, Debug)]
pub(crate) struct SuggestionRequest {
    pub(crate) thread_id: ThreadId,
    pub(crate) turn_id: String,
    pub(crate) summary: codex_protocol::config_types::ReasoningSummary,
    pub(crate) id: Uuid,
    pub(crate) cancellation: CancellationToken,
    pub(crate) generation_finished: CancellationToken,
}

pub(crate) struct PromptSuggestion {
    pub(crate) request: SuggestionRequest,
    pub(crate) text: Option<String>,
}

impl Drop for PromptSuggestion {
    fn drop(&mut self) {
        self.request.cancellation.cancel();
    }
}

pub(crate) fn parse_suggestion(response: &str) -> Option<String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Response {
        suggestion: serde_json::Value,
    }
    let response = serde_json::from_str::<Response>(response).ok()?;
    let text = response.suggestion.as_str()?.trim();
    (!text.is_empty()
        && text.chars().count() <= 240
        && !text
            .chars()
            .any(|ch| ch.is_control() || matches!(ch, '\u{2028}' | '\u{2029}')))
    .then(|| text.to_string())
}

#[cfg(test)]
#[path = "prompt_suggestions_tests.rs"]
mod tests;
