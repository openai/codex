//! Safety-buffering status and retry UI for active turns.

use super::*;
use codex_app_server_protocol::ModelSafetyBufferingUpdatedNotification;

const SAFETY_BUFFERING_PROMPT_VIEW_ID: &str = "safety-buffering-prompt";
const SAFETY_BUFFERING_LEARN_MORE_URL: &str = "https://help.openai.com/en/articles/20001326";

const SAFETY_BUFFERING_MESSAGE_WITH_RETRY: &str = "This request requires additional safety checks, which can take extra time. Hang tight or retry with a faster model for a quicker response, though it may be less capable of handling complex requests.";
const SAFETY_BUFFERING_MESSAGE_WITHOUT_RETRY: &str =
    "This request requires additional safety checks, which can take extra time.";

#[derive(Debug)]
struct ActiveSafetyBuffering {
    turn_id: String,
    last_prompt_had_retry: bool,
    agent_message_started: bool,
}

#[derive(Debug, Default)]
pub(super) struct SafetyBufferingState {
    submitted_turn: Option<(String, AppCommand)>,
    active: Option<ActiveSafetyBuffering>,
}

impl ChatWidget {
    pub(crate) fn record_safety_buffering_turn(&mut self, turn_id: String, turn: &AppCommand) {
        self.safety_buffering.submitted_turn = Some((turn_id, turn.clone()));
    }

    pub(super) fn reset_safety_buffering_for_turn_start(&mut self) {
        self.bottom_pane
            .dismiss_view_by_id(SAFETY_BUFFERING_PROMPT_VIEW_ID);
        self.safety_buffering.active = None;
    }

    pub(crate) fn clear_safety_buffering(&mut self) {
        self.bottom_pane
            .dismiss_view_by_id(SAFETY_BUFFERING_PROMPT_VIEW_ID);
        self.safety_buffering = SafetyBufferingState::default();
    }

    pub(super) fn mark_safety_buffering_agent_message_started(&mut self) {
        if let Some(active) = self.safety_buffering.active.as_mut() {
            active.agent_message_started = true;
        }
    }

    pub(super) fn safety_buffering_is_waiting(&self) -> bool {
        self.safety_buffering
            .active
            .as_ref()
            .is_some_and(|active| !active.agent_message_started)
    }

    pub(crate) fn can_retry_safety_buffered_turn(&self, turn_id: &str) -> bool {
        self.turn_lifecycle.agent_turn_running
            && self
                .safety_buffering
                .active
                .as_ref()
                .is_some_and(|active| active.turn_id == turn_id && !active.agent_message_started)
    }

    pub(crate) fn prepare_safety_buffering_retry(&mut self) {
        let cancel_edit = std::mem::take(&mut self.cancel_edit);
        self.last_rendered_user_message_display = None;
        self.finalize_turn();
        self.cancel_edit = cancel_edit;
        self.input_queue.user_turn_pending_start = true;
    }

    pub(crate) fn fail_safety_buffering_retry(&mut self) {
        self.input_queue.user_turn_pending_start = false;
        self.clear_safety_buffering();
        let prompt = self.cancel_edit.prompt.take();
        self.clear_cancel_edit();
        if let Some(prompt) = prompt {
            self.restore_user_message_to_composer(prompt);
        }
    }

    pub(super) fn on_model_safety_buffering_updated(
        &mut self,
        notification: ModelSafetyBufferingUpdatedNotification,
        replay_kind: Option<ReplayKind>,
    ) {
        let ModelSafetyBufferingUpdatedNotification {
            turn_id,
            show_buffering_ui,
            faster_model,
            ..
        } = notification;
        if matches!(replay_kind, Some(ReplayKind::ResumeInitialMessages))
            || !self.turn_lifecycle.agent_turn_running
            || self.turn_lifecycle.last_turn_id.as_deref() != Some(turn_id.as_str())
        {
            return;
        }
        if !show_buffering_ui {
            if self
                .safety_buffering
                .active
                .as_ref()
                .is_some_and(|active| active.turn_id == turn_id)
            {
                self.bottom_pane
                    .dismiss_view_by_id(SAFETY_BUFFERING_PROMPT_VIEW_ID);
                self.safety_buffering.active = None;
                self.restore_reasoning_status_header();
            }
            return;
        }

        let retry_turn = self
            .safety_buffering
            .submitted_turn
            .as_ref()
            .filter(|(submitted_turn_id, _)| replay_kind.is_none() && submitted_turn_id == &turn_id)
            .map(|(_, turn)| turn.clone());
        let thread_id = self.thread_id;
        let can_offer_retry = faster_model.is_some() && retry_turn.is_some() && thread_id.is_some();
        let previous_active = self
            .safety_buffering
            .active
            .as_ref()
            .filter(|active| active.turn_id == turn_id);
        let should_show_prompt =
            previous_active.is_none_or(|active| active.last_prompt_had_retry != can_offer_retry);
        let agent_message_started =
            previous_active.is_some_and(|active| active.agent_message_started);
        self.safety_buffering.active = Some(ActiveSafetyBuffering {
            turn_id: turn_id.clone(),
            last_prompt_had_retry: can_offer_retry,
            agent_message_started,
        });

        let message = if can_offer_retry {
            SAFETY_BUFFERING_MESSAGE_WITH_RETRY
        } else {
            SAFETY_BUFFERING_MESSAGE_WITHOUT_RETRY
        };
        self.bottom_pane.ensure_status_indicator();
        self.set_status(
            "Working".to_string(),
            Some(message.to_string()),
            StatusDetailsCapitalization::Preserve,
            /*details_max_lines*/ 6,
        );

        if !should_show_prompt {
            return;
        }
        self.bottom_pane
            .dismiss_view_by_id(SAFETY_BUFFERING_PROMPT_VIEW_ID);

        let header = ColumnRenderable::with(vec![
            Box::new(Line::from("Additional safety checks").bold()) as Box<dyn Renderable>,
            Box::new(Paragraph::new(Line::from(message).dim()).wrap(Wrap { trim: false })),
        ]);
        let mut items = Vec::new();
        if let (Some(faster_model), Some(turn), Some(thread_id)) =
            (faster_model, retry_turn, thread_id)
        {
            items.push(SelectionItem {
                name: "Retry with a faster model".to_string(),
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::RetrySafetyBufferedTurn {
                        thread_id,
                        turn_id: turn_id.clone(),
                        model: faster_model.clone(),
                        turn: turn.clone(),
                    });
                })],
                dismiss_on_select: true,
                ..Default::default()
            });
        }
        items.extend([
            SelectionItem {
                name: "Keep waiting".to_string(),
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Learn more".to_string(),
                actions: vec![Box::new(|tx| {
                    tx.send(AppEvent::OpenUrlInBrowser {
                        url: SAFETY_BUFFERING_LEARN_MORE_URL.to_string(),
                    });
                })],
                ..Default::default()
            },
        ]);
        self.bottom_pane.show_selection_view(SelectionViewParams {
            view_id: Some(SAFETY_BUFFERING_PROMPT_VIEW_ID),
            header: Box::new(header),
            items,
            ..Default::default()
        });
    }
}
