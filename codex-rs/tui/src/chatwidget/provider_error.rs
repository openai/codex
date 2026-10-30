//! Provider admission failures keep history readable and drafts recoverable.

use super::*;
use crate::render::renderable::ColumnRenderable;
use crate::wrapping::RtOptions;
use crate::wrapping::adaptive_wrap_lines;
use crate::wrapping::word_wrap_lines;
use codex_app_server_protocol::ModelProviderError;
use ratatui::widgets::Block;
use ratatui::widgets::Padding;

pub(super) struct ProviderErrorNotice {
    pub(super) title: &'static str,
    pub(super) message: String,
    pub(super) configuration_location: Option<String>,
    pub(super) can_continue: bool,
}

impl Renderable for ProviderErrorNotice {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.as_renderable(area.width).render(area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.as_renderable(width).desired_height(width)
    }
}

impl ProviderErrorNotice {
    fn as_renderable(&self, width: u16) -> ColumnRenderable<'_> {
        let content_width = usize::from(width.saturating_sub(/*rhs*/ 4));
        let mut lines = word_wrap_lines(
            &[Line::from(vec!["🔒  ".into(), self.title.bold()])],
            content_width,
        );
        let mut description = vec![Line::from(self.message.as_str().dim())];
        if let Some(location) = &self.configuration_location {
            description.push(Line::from(location.as_str().dim()));
        }
        lines.extend(adaptive_wrap_lines(
            &description,
            RtOptions::new(content_width)
                .initial_indent("    ".into())
                .subsequent_indent("    ".into()),
        ));
        let mut notice = ColumnRenderable::new();
        notice.push(
            Paragraph::new(lines).block(
                Block::default()
                    .style(crate::style::user_message_style())
                    .padding(Padding::new(
                        /*left*/ 2, /*right*/ 2, /*top*/ 1, /*bottom*/ 1,
                    )),
            ),
        );
        let mut shortcuts = Vec::new();
        if self.can_continue {
            shortcuts.extend([
                crate::key_hint::plain(KeyCode::Char('c')).into(),
                " continue in new chat · ".into(),
            ]);
        }
        shortcuts.extend([
            crate::key_hint::plain(KeyCode::Char('n')).into(),
            " start fresh · ".into(),
            crate::key_hint::plain(KeyCode::Esc).into(),
            " other chats".into(),
        ]);
        let footer = Line::from(shortcuts).style(crate::style::secondary_text_style());
        notice.push(
            Paragraph::new(word_wrap_lines(
                &[footer],
                usize::from(width.saturating_sub(/*rhs*/ 2)),
            ))
            .inset(Insets::tlbr(
                /*top*/ 0, /*left*/ 2, /*bottom*/ 0, /*right*/ 0,
            )),
        );
        notice
    }
}

impl ChatWidget {
    pub(super) fn provider_error_message(&self, error: &ModelProviderError) -> String {
        let provider_name = |id: &str| {
            self.config
                .model_providers
                .get(id)
                .filter(|provider| !provider.name.trim().is_empty())
                .map(|provider| provider.name.clone())
                .unwrap_or_else(|| id.to_string())
        };
        match error {
            ModelProviderError::ModelProviderMismatch {
                model_provider,
                current_model_provider,
            } => {
                let provider = provider_name(model_provider);
                let current = provider_name(current_model_provider);
                format!(
                    "This chat uses {provider}, but Codex now uses {current}, so this chat is read-only."
                )
            }
            ModelProviderError::ModelProviderNotConfigured { model_provider } => {
                let provider = provider_name(model_provider);
                format!(
                    "This chat uses model provider “{provider}”, which is no longer configured. Restore this provider’s configuration to continue."
                )
            }
            ModelProviderError::ModelProviderPolicyConflict {
                model_provider,
                required_model_provider,
            } if model_provider != required_model_provider => {
                let provider = provider_name(model_provider);
                let required = provider_name(required_model_provider);
                format!(
                    "This chat uses model provider “{provider}”, but your organization requires “{required}”. Contact your administrator or start a new chat using the required provider."
                )
            }
            ModelProviderError::ModelProviderPolicyConflict { .. } => error.to_string(),
        }
    }

    pub(crate) fn has_provider_error(&self) -> bool {
        self.provider_error.is_some()
    }

    pub(crate) fn show_provider_error(
        &mut self,
        error: ModelProviderError,
        rejected_message_id: Option<&str>,
    ) {
        self.provider_error = Some(error);
        self.blocks_direct_input = true;
        self.input_queue.suppress_queue_autosend = true;
        self.input_queue.recovered_queue = true;
        if let Some(client_id) = rejected_message_id {
            if self.input_queue.user_turn_pending_start
                && self.input_queue.pending_user_message_client_id.as_deref() == Some(client_id)
            {
                self.input_queue.user_turn_pending_start = false;
                if let Some(prompt) = self.safety_buffering_prompt.take() {
                    self.input_queue
                        .queued_user_messages
                        .push_front(QueuedUserMessage {
                            source: self.safety_buffering_source,
                            ..QueuedUserMessage::from(prompt)
                        });
                    self.input_queue
                        .queued_user_message_history_records
                        .push_front(UserMessageHistoryRecord::UserMessageText);
                }
            } else if let Some(index) = self
                .input_queue
                .pending_steers
                .iter()
                .position(|steer| steer.client_id == client_id)
                && let Some(steer) = self.input_queue.pending_steers.remove(index)
            {
                self.input_queue
                    .rejected_steers_queue
                    .push_back(steer.user_message);
                self.input_queue
                    .rejected_steer_sources
                    .push_back(steer.source);
                self.input_queue
                    .rejected_steer_history_records
                    .push_back(steer.history_record);
            }
            self.update_task_running_state();
        }
        self.bottom_pane
            .set_composer_input_enabled(/*enabled*/ false, /*placeholder*/ None);
        self.refresh_pending_input_preview();
        self.request_redraw();
    }
}
