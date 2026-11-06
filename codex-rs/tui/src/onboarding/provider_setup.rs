//! Single-line fields shared by provider settings and the cloud setup wizards.

use super::auth::AuthModeWidget;
use super::auth::SignInState;
use crate::bottom_pane::TextArea;
use codex_app_server_protocol::RequestId;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;
use unicode_segmentation::UnicodeSegmentation;

// The sign-in state is shared with request tasks. Each field owns its editing history,
// including killed text, so yanking in another field cannot reveal a credential.
#[derive(Clone)]
pub(crate) struct ProviderInput(Arc<Mutex<TextArea>>);

pub(crate) enum FieldDisplay {
    Focused,
    Unfocused,
    FocusedSecret,
    UnfocusedSecret,
}

impl Default for ProviderInput {
    fn default() -> Self {
        Self::from("")
    }
}

impl From<&str> for ProviderInput {
    fn from(text: &str) -> Self {
        let mut editor = TextArea::new_single_line();
        editor.insert_str(text);
        Self(Arc::new(Mutex::new(editor)))
    }
}

impl ProviderInput {
    pub(crate) fn editor(&self) -> MutexGuard<'_, TextArea> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn input(&mut self, event: KeyEvent) {
        self.editor().input(event);
    }

    pub(crate) fn paste(&mut self, pasted: &str) {
        let text: String = pasted.chars().filter(|c| !c.is_control()).collect();
        self.editor().insert_str(&text);
    }

    pub(crate) fn value(&self) -> Option<String> {
        let editor = self.editor();
        let text = editor.text().trim();
        (!text.is_empty()).then(|| text.to_owned())
    }

    pub(crate) fn line(
        &self,
        label: &str,
        placeholder: &str,
        display: FieldDisplay,
        width: u16,
    ) -> Line<'static> {
        let editor = self.editor();
        let mut line = Line::from(label.to_owned());
        if editor.text().is_empty() && !placeholder.is_empty() {
            line.push_span(placeholder.to_owned().dim());
            return line;
        }
        let focused = matches!(display, FieldDisplay::Focused | FieldDisplay::FocusedSecret);
        let field_width = usize::from(width).saturating_sub(line.width());
        // When the label occupies a whole row, wrapping moves the value to the next row.
        let mut remaining = if field_width == 0 {
            usize::from(width)
        } else {
            field_width
        };
        let start = if focused {
            editor.single_line_viewport(remaining as u16).0
        } else {
            0
        };
        let mut content = editor.text()[start..].grapheme_indices(true).peekable();
        while let Some((offset, grapheme)) = content.next() {
            let cell = match display {
                FieldDisplay::Focused | FieldDisplay::Unfocused => grapheme,
                FieldDisplay::FocusedSecret if content.peek().is_none() => grapheme,
                FieldDisplay::FocusedSecret | FieldDisplay::UnfocusedSecret => "•",
            };
            let span = Span::raw(cell.to_owned());
            let span = if focused { span.cyan() } else { span };
            let Some(rest) = remaining.checked_sub(span.width()) else {
                break;
            };
            remaining = rest;
            line.push_span(if focused && start + offset == editor.cursor() {
                span.reversed()
            } else {
                span
            });
        }
        if focused && remaining > 0 && editor.cursor() == editor.text().len() {
            line.push_span(" ".reversed());
        }
        line
    }
}

/// Cycle focus among the inputs and actions in a provider form.
pub(crate) fn move_provider_field(key: KeyEvent, selected: &mut usize, count: usize) -> bool {
    *selected = match (key.code, key.modifiers) {
        (KeyCode::Up | KeyCode::BackTab, KeyModifiers::NONE)
        | (KeyCode::Tab | KeyCode::BackTab, KeyModifiers::SHIFT) => (*selected + count - 1) % count,
        (KeyCode::Down | KeyCode::Tab, KeyModifiers::NONE) => (*selected + 1) % count,
        _ => return false,
    };
    true
}

impl AuthModeWidget {
    pub(super) fn spawn_provider_setup(
        &self,
        request_id: RequestId,
        retry: SignInState,
        request: impl Future<Output = Result<(), String>> + Send + 'static,
    ) {
        let sign_in_state = self.sign_in_state.clone();
        let error = self.error.clone();
        let request_frame = self.request_frame.clone();
        tokio::spawn(async move {
            let result = request.await;
            let mut state = sign_in_state
                .write()
                .unwrap_or_else(PoisonError::into_inner);
            let configured = match &*state {
                SignInState::Bedrock(bedrock) if bedrock.is_configuring(&request_id) => {
                    SignInState::BedrockConfigured
                }
                SignInState::Gcp(gcp) if gcp.is_configuring(&request_id) => {
                    SignInState::GcpConfigured
                }
                _ => return,
            };
            let (next, message) = match result {
                Ok(()) => (configured, None),
                Err(err) => (retry, Some(err)),
            };
            *state = next;
            *error.write().unwrap_or_else(PoisonError::into_inner) = message;
            drop(state);
            request_frame.schedule_frame();
        });
    }
}
