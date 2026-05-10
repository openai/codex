//! Confirmation flow for queued follow-up sends paused by usage limits.

use super::*;

impl ChatWidget {
    pub(super) fn pause_queued_sends_after_limit_error(&mut self) {
        if self.has_queued_follow_up_messages() {
            self.queued_sends_paused_after_usage_limit = true;
            self.refresh_pending_input_preview();
        }
    }

    pub(super) fn should_prompt_to_resume_queued_sends(&self) -> bool {
        self.queued_sends_paused_after_usage_limit
            && self.has_queued_follow_up_messages()
            && !self.is_user_turn_pending_or_running()
            && self.bottom_pane.composer_is_empty()
            && self.bottom_pane.no_modal_or_popup_active()
    }

    pub(super) fn show_resume_queued_sends_prompt(&mut self) {
        self.show_selection_view(SelectionViewParams {
            title: Some("Resume queued sends?".to_string()),
            subtitle: Some(
                "Queued inputs were paused after a usage limit was reached.".to_string(),
            ),
            footer_hint: Some(standard_popup_hint_line()),
            initial_selected_idx: Some(0),
            items: vec![
                SelectionItem {
                    name: "Keep paused".to_string(),
                    description: Some(
                        "Leave queued sends paused until you review them later.".to_string(),
                    ),
                    dismiss_on_select: true,
                    ..Default::default()
                },
                SelectionItem {
                    name: "Resume queued sends".to_string(),
                    description: Some("Continue sending queued inputs.".to_string()),
                    actions: vec![Box::new(|tx| {
                        tx.send(AppEvent::ResumeQueuedSends);
                    })],
                    dismiss_on_select: true,
                    ..Default::default()
                },
            ],
            ..Default::default()
        });
    }

    pub(crate) fn resume_queued_sends(&mut self) {
        self.queued_sends_paused_after_usage_limit = false;
        self.refresh_pending_input_preview();
        self.maybe_send_next_queued_input();
    }
}
