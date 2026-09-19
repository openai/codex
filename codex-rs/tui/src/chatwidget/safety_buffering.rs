//! Safety-buffering status and retry confirmation UI for active turns.
//! Both views share an ID so response streaming, completion, or buffering updates dismiss either one.

use super::*;
use crate::wrapping::word_wrap_lines;
use codex_app_server_protocol::ModelSafetyBufferingUpdatedNotification;

const SAFETY_BUFFERING_PROMPT_VIEW_ID: &str = "safety-buffering-prompt";
const SAFETY_BUFFERING_LEARN_MORE_URL: &str = "https://help.openai.com/en/articles/20001326";

const SAFETY_BUFFERING_HEADER: &str = "Giving this request a little extra thought";
const SAFETY_BUFFERING_MESSAGE_WITH_RETRY: &str = "If you'd rather not wait, retry with a faster model. It may be less capable of handling complex requests.";
const SAFETY_BUFFERING_FOOTER: &str = "No action is required. Codex will keep waiting, and this menu will close when the response is ready.";

struct SafetyBufferingHeader(Vec<Line<'static>>);

impl Renderable for SafetyBufferingHeader {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        Renderable::render(
            &Paragraph::new(word_wrap_lines(&self.0, usize::from(area.width))),
            area,
            buf,
        );
    }

    fn desired_height(&self, width: u16) -> u16 {
        word_wrap_lines(&self.0, usize::from(width)).len() as u16
    }
}

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
        if let Some(active) = self.safety_buffering.active.as_mut()
            && !active.agent_message_started
        {
            active.agent_message_started = true;
            self.bottom_pane
                .dismiss_view_by_id(SAFETY_BUFFERING_PROMPT_VIEW_ID);
            self.restore_reasoning_status_header();
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

    pub(crate) fn prepare_safety_buffered_retry_submission(&mut self, prompt: UserMessage) {
        self.last_rendered_user_message_display = None;
        self.finalize_turn();
        self.safety_buffering_prompt = Some(prompt);
        self.input_queue.user_turn_pending_start = true;
    }

    pub(crate) fn commit_safety_buffered_retry_submission(&mut self, display: UserMessageDisplay) {
        self.on_user_message_display(display);
    }

    pub(crate) fn cancel_safety_buffered_retry_submission(&mut self) {
        self.input_queue.user_turn_pending_start = false;
        self.clear_safety_buffering();
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
            || self
                .safety_buffering
                .active
                .as_ref()
                .is_some_and(|active| active.turn_id == turn_id && active.agent_message_started)
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

        let retry_turn = if self.side_conversation_active() {
            None
        } else {
            self.safety_buffering
                .submitted_turn
                .as_ref()
                .filter(|(submitted_turn_id, _)| {
                    replay_kind.is_none() && submitted_turn_id == &turn_id
                })
                .map(|(_, turn)| turn.clone())
        };
        let thread_id = self.thread_id;
        let retry_prompt = self.safety_buffering_prompt.clone();
        let can_offer_retry = faster_model.is_some()
            && retry_turn.is_some()
            && retry_prompt.is_some()
            && thread_id.is_some();
        let previous_active = self
            .safety_buffering
            .active
            .as_ref()
            .filter(|active| active.turn_id == turn_id);
        let should_show_prompt =
            previous_active.is_none_or(|active| active.last_prompt_had_retry != can_offer_retry);
        self.safety_buffering.active = Some(ActiveSafetyBuffering {
            turn_id: turn_id.clone(),
            last_prompt_had_retry: can_offer_retry,
            agent_message_started: false,
        });

        let status_details = if can_offer_retry {
            format!("{SAFETY_BUFFERING_HEADER} {SAFETY_BUFFERING_MESSAGE_WITH_RETRY}")
        } else {
            SAFETY_BUFFERING_HEADER.to_string()
        };
        self.bottom_pane.ensure_status_indicator();
        self.set_status(
            "Working".to_string(),
            Some(status_details),
            StatusDetailsCapitalization::Preserve,
            /*details_max_lines*/ 6,
        );

        if !should_show_prompt {
            return;
        }
        self.bottom_pane
            .dismiss_view_by_id(SAFETY_BUFFERING_PROMPT_VIEW_ID);

        let mut header = vec![Line::from(SAFETY_BUFFERING_HEADER).bold()];
        if can_offer_retry {
            header.push(Line::from(SAFETY_BUFFERING_MESSAGE_WITH_RETRY).dim());
        }
        let mut items = Vec::new();
        if let (Some(faster_model), Some(turn), Some(prompt), Some(thread_id)) =
            (faster_model, retry_turn, retry_prompt, thread_id)
        {
            items.push(SelectionItem {
                name: "Retry with a faster model".to_string(),
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::ConfirmSafetyBufferedRetry {
                        thread_id,
                        turn_id: turn_id.clone(),
                        model: faster_model.clone(),
                        turn: turn.clone(),
                        prompt: prompt.clone(),
                    });
                })],
                dismiss_on_select: true,
                ..Default::default()
            });
        }
        items.extend([
            SelectionItem {
                name: "Dismiss and keep waiting".to_string(),
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
            appearance: crate::bottom_pane::SelectionAppearance::Picker,
            view_id: Some(SAFETY_BUFFERING_PROMPT_VIEW_ID),
            header: Box::new(SafetyBufferingHeader(header)),
            footer_note: Some(Line::from(SAFETY_BUFFERING_FOOTER).dim()),
            footer_hint: Some(Line::default()),
            items,
            ..SelectionViewParams::picker()
        });
    }

    pub(crate) fn confirm_safety_buffered_retry(
        &mut self,
        thread_id: ThreadId,
        turn_id: String,
        model: String,
        turn: AppCommand,
        prompt: UserMessage,
    ) {
        if self.thread_id != Some(thread_id) || !self.can_retry_safety_buffered_turn(&turn_id) {
            return;
        }

        let model_name = self
            .model_catalog
            .try_list_models()
            .ok()
            .and_then(|models| models.into_iter().find(|preset| preset.model == model))
            .map(|preset| preset.display_name)
            .unwrap_or_else(|| model.clone());
        self.bottom_pane
            .dismiss_view_by_id(SAFETY_BUFFERING_PROMPT_VIEW_ID);
        self.bottom_pane.show_selection_view(SelectionViewParams {
            appearance: crate::bottom_pane::SelectionAppearance::Picker,
            view_id: Some(SAFETY_BUFFERING_PROMPT_VIEW_ID),
            header: Box::new(SafetyBufferingHeader(vec![
                    "Stop this attempt and retry?".bold().into(),
                    Line::default(),
                    "This will stop the current attempt and retry in a new thread. Any file changes or other actions already taken will remain.".dim().into(),
                    Line::default(),
                    format!("Your message will be sent again using {model_name}, which may be less capable on complex tasks.").dim().into(),
                ])),
            footer_hint: Some(Line::default()),
            items: vec![
                SelectionItem {
                    name: "Keep waiting".to_string(),
                    dismiss_on_select: true,
                    ..Default::default()
                },
                SelectionItem {
                    name: "Stop and retry".to_string(),
                    actions: vec![Box::new(move |tx| {
                        tx.send(AppEvent::RetrySafetyBufferedTurn {
                            thread_id,
                            turn_id: turn_id.clone(),
                            model: model.clone(),
                            turn: turn.clone(),
                            prompt: prompt.clone(),
                        });
                    })],
                    dismiss_on_select: true,
                    require_explicit_confirmation: true,
                    ..Default::default()
                },
            ],
            ..SelectionViewParams::picker()
        });
    }
}
