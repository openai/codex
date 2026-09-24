//! Suggests after successful live turns with known settings. The composer owns cancellation;
//! request identity prevents late results from replacing a newer suggestion.

use super::*;
use crate::prompt_suggestions::SuggestionRequest;
use tokio_util::sync::CancellationToken;

impl ChatWidget {
    pub(crate) fn has_prompt_suggestion(&self) -> bool {
        self.bottom_pane.has_prompt_suggestion()
    }

    pub(crate) fn clear_prompt_suggestion(&mut self) {
        self.bottom_pane.clear_prompt_suggestion();
    }

    pub(super) fn suggest_next_prompt(&mut self, turn_id: String) {
        if !self.local_settings.tui.prompt_suggestions
            || self.is_user_turn_pending_or_running()
            || self.external_writer_view
            || self.blocks_direct_input
            || self.side_conversation_active()
            || self.has_queued_follow_up_messages()
        {
            return;
        }
        let Some(thread_id) = self.thread_id() else {
            return;
        };
        let Some(summary) = self.prompt_suggestion_summary else {
            return;
        };
        let request = SuggestionRequest {
            thread_id,
            turn_id,
            summary,
            id: uuid::Uuid::new_v4(),
            cancellation: CancellationToken::new(),
            generation_finished: CancellationToken::new(),
        };
        self.bottom_pane.set_prompt_suggestion(request.clone());
        self.app_event_tx
            .send(AppEvent::GeneratePromptSuggestion(request));
    }

    pub(crate) fn apply_prompt_suggestion(
        &mut self,
        request: &SuggestionRequest,
        text: Option<String>,
    ) {
        if self.thread_id() == Some(request.thread_id) && self.local_settings.tui.prompt_suggestions
        {
            self.bottom_pane.apply_prompt_suggestion(request, text);
        }
    }
}

#[cfg(test)]
#[path = "prompt_suggestions_tests.rs"]
mod tests;
