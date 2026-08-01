//! Transient single-key and chord capture for the `/keymap` shortcut editor.

use super::key_event_to_config_key_spec;
use crate::app_event::AppEvent;
use crate::app_event::KeymapCaptureMode;
use crate::app_event::KeymapEditIntent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPaneView;
use crate::bottom_pane::CancellationEvent;
use crate::render::renderable::Renderable;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

/// Bottom-pane view that captures one key or a two-stroke `/keymap` chord.
///
/// The view is deliberately transient: it renders instructions, captures the
/// requested number of strokes, and emits their canonical config representation
/// to the app layer. It does not mutate config itself, because mutation needs
/// the latest runtime keymap to detect conflicts and stale selections.
pub(crate) struct KeymapCaptureView {
    context: String,
    action: String,
    intent: KeymapEditIntent,
    capture_mode: KeymapCaptureMode,
    first_stroke: Option<String>,
    label: String,
    current_binding: String,
    app_event_tx: AppEventSender,
    complete: bool,
    error_message: Option<String>,
}

impl KeymapCaptureView {
    pub(super) fn new(
        context: String,
        action: String,
        intent: KeymapEditIntent,
        capture_mode: KeymapCaptureMode,
        label: String,
        current_binding: String,
        app_event_tx: AppEventSender,
    ) -> Self {
        Self {
            context,
            action,
            intent,
            capture_mode,
            first_stroke: None,
            label,
            current_binding,
            app_event_tx,
            complete: false,
            error_message: None,
        }
    }

    fn lines(&self, width: u16) -> Vec<Line<'static>> {
        let wrap_width = usize::from(width.max(1));
        let mut lines = vec![
            Line::from("Remap Shortcut".bold()),
            Line::from(vec![
                "Action: ".dim(),
                self.label.clone().into(),
                "  ".into(),
                format!("{}.{}", self.context, self.action).dim(),
            ]),
            Line::from(vec!["Current: ".dim(), self.current_binding.clone().cyan()]),
        ];

        let instructions = match (self.capture_mode, self.first_stroke.as_deref()) {
            (KeymapCaptureMode::SingleKey, _) => "Press the new key now. Esc cancels.".to_string(),
            (KeymapCaptureMode::Chord, None) => {
                "Press the first key, then the second. Esc cancels.".to_string()
            }
            (KeymapCaptureMode::Chord, Some(first)) => {
                format!("First key: {first}. Press the second key. Esc cancels.")
            }
        };
        lines.extend(
            textwrap::wrap(&instructions, wrap_width)
                .into_iter()
                .map(|line| Line::from(line.into_owned().dim())),
        );

        if let Some(error) = &self.error_message {
            lines.push(Line::from(""));
            let options = textwrap::Options::new(wrap_width)
                .initial_indent("Error: ")
                .subsequent_indent("       ");
            lines.extend(
                textwrap::wrap(error, options)
                    .into_iter()
                    .map(|line| Line::from(line.into_owned().red())),
            );
        }

        lines
    }
}

impl Renderable for KeymapCaptureView {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.lines(area.width)).render(area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.lines(width).len() as u16
    }
}

impl BottomPaneView for KeymapCaptureView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Release
            || (self.capture_mode == KeymapCaptureMode::Chord
                && key_event.kind != KeyEventKind::Press)
        {
            return;
        }

        if crate::key_hint::plain(KeyCode::Esc).is_press(key_event) {
            self.complete = true;
            return;
        }

        match key_event_to_config_key_spec(key_event) {
            Ok(key) => {
                self.error_message = None;
                let key = if self.capture_mode == KeymapCaptureMode::Chord {
                    let Some(first_stroke) = self.first_stroke.take() else {
                        self.first_stroke = Some(key);
                        return;
                    };
                    format!("{first_stroke} {key}")
                } else {
                    key
                };
                self.app_event_tx.send(AppEvent::KeymapCaptured {
                    context: self.context.clone(),
                    action: self.action.clone(),
                    key,
                    intent: self.intent.clone(),
                });
                self.complete = true;
            }
            Err(error) => {
                self.error_message = Some(error);
            }
        }
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.complete = true;
        CancellationEvent::Handled
    }

    fn prefer_esc_to_handle_key_event(&self) -> bool {
        true
    }
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;
