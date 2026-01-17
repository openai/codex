//! Collects free-form, multi-line custom prompt input in the bottom pane.
//!
//! The view owns a [`TextArea`] and its [`TextAreaState`], translating key and paste
//! events into text edits and rendering a compact popup with a title, optional
//! context line, and submission hint. It is intentionally lightweight: it only
//! accepts text, handles submission or cancellation, and reports completion back
//! to the bottom pane orchestrator. The input height is clamped to keep popups
//! compact, and Enter without modifiers is treated as a submit gesture rather
//! than a newline insertion.
//!
//! This module does not interpret the prompt contents or manage navigation
//! between bottom-pane modes; it focuses purely on input capture and rendering.

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::StatefulWidgetRef;
use ratatui::widgets::Widget;
use std::cell::RefCell;

use crate::render::renderable::Renderable;

use super::popup_consts::standard_popup_hint_line;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::textarea::TextArea;
use super::textarea::TextAreaState;

/// Callback invoked when the user submits a custom prompt.
///
/// The callback receives the trimmed prompt text and is expected to trigger the
/// bottom-pane transition out of the custom prompt mode.
pub(crate) type PromptSubmitted = Box<dyn Fn(String) + Send + Sync>;

/// Minimal multi-line text input view to collect custom review instructions.
///
/// The view owns the textarea widget and its mutable state, and it tracks a
/// completion flag used by the bottom pane to dismiss the popup. The submit
/// callback is invoked only when the trimmed text is non-empty, mirroring the
/// UI's expectation that empty submissions are ignored.
pub(crate) struct CustomPromptView {
    /// Title text rendered on the first line of the popup.
    title: String,
    /// Placeholder hint rendered when the input is empty.
    placeholder: String,
    /// Optional context line rendered between the title and input.
    context_label: Option<String>,
    /// Handler invoked once with the final, trimmed prompt text.
    on_submit: PromptSubmitted,

    /// Text input widget that owns the buffer and cursor logic.
    textarea: TextArea,
    /// Stateful widget data (selection, cursor, scroll) for the textarea.
    textarea_state: RefCell<TextAreaState>,
    /// Marker indicating the view has completed and should be dismissed.
    complete: bool,
}

impl CustomPromptView {
    /// Creates a new prompt view configured with labels and a submit callback.
    ///
    /// The view starts empty and not-complete; the caller is responsible for
    /// reacting to the submission callback or cancellation events.
    pub(crate) fn new(
        title: String,
        placeholder: String,
        context_label: Option<String>,
        on_submit: PromptSubmitted,
    ) -> Self {
        Self {
            title,
            placeholder,
            context_label,
            on_submit,
            textarea: TextArea::new(),
            textarea_state: RefCell::new(TextAreaState::default()),
            complete: false,
        }
    }
}

impl BottomPaneView for CustomPromptView {
    /// Handles key input by editing the textarea or submitting/canceling.
    ///
    /// The handler treats Enter without modifiers as a submission attempt,
    /// defers other Enter variants to the textarea for newline insertion, and
    /// routes `Esc` to the same cancellation path as `Ctrl-C`.
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event {
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.on_ctrl_c();
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                let text = self.textarea.text().trim().to_string();
                if !text.is_empty() {
                    (self.on_submit)(text);
                    self.complete = true;
                }
            }
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                self.textarea.input(key_event);
            }
            other => {
                self.textarea.input(other);
            }
        }
    }

    /// Marks the view as complete and reports a handled cancellation.
    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.complete = true;
        CancellationEvent::Handled
    }

    /// Returns whether the view has finished (submitted or canceled).
    fn is_complete(&self) -> bool {
        self.complete
    }

    /// Inserts pasted text into the textarea if it is non-empty.
    ///
    /// Empty paste payloads are treated as no-ops so the bottom pane can keep
    /// other handlers in play.
    fn handle_paste(&mut self, pasted: String) -> bool {
        if pasted.is_empty() {
            return false;
        }
        self.textarea.insert_str(&pasted);
        true
    }
}

impl Renderable for CustomPromptView {
    /// Computes the height needed to render title, optional context, input, and hints.
    fn desired_height(&self, width: u16) -> u16 {
        let extra_top: u16 = if self.context_label.is_some() { 1 } else { 0 };
        1u16 + extra_top + self.input_height(width) + 3u16
    }

    /// Renders the popup with title/context, textarea input, and hint line.
    ///
    /// Rendering proceeds in phases: title, optional context, guttered input,
    /// placeholder overlay, and finally the hint line. The input area reserves a
    /// one-line gutter to align with other bottom-pane popups and leaves a blank
    /// spacer line before the hint.
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let input_height = self.input_height(area.width);

        // Title line
        let title_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        };
        let title_spans: Vec<Span<'static>> = vec![gutter(), self.title.clone().bold()];
        Paragraph::new(Line::from(title_spans)).render(title_area, buf);

        // Optional context line
        let mut input_y = area.y.saturating_add(1);
        if let Some(context_label) = &self.context_label {
            let context_area = Rect {
                x: area.x,
                y: input_y,
                width: area.width,
                height: 1,
            };
            let spans: Vec<Span<'static>> = vec![gutter(), context_label.clone().cyan()];
            Paragraph::new(Line::from(spans)).render(context_area, buf);
            input_y = input_y.saturating_add(1);
        }

        // Input line
        let input_area = Rect {
            x: area.x,
            y: input_y,
            width: area.width,
            height: input_height,
        };
        if input_area.width >= 2 {
            for row in 0..input_area.height {
                Paragraph::new(Line::from(vec![gutter()])).render(
                    Rect {
                        x: input_area.x,
                        y: input_area.y.saturating_add(row),
                        width: 2,
                        height: 1,
                    },
                    buf,
                );
            }

            let text_area_height = input_area.height.saturating_sub(1);
            if text_area_height > 0 {
                if input_area.width > 2 {
                    let blank_rect = Rect {
                        x: input_area.x.saturating_add(2),
                        y: input_area.y,
                        width: input_area.width.saturating_sub(2),
                        height: 1,
                    };
                    Clear.render(blank_rect, buf);
                }
                let textarea_rect = Rect {
                    x: input_area.x.saturating_add(2),
                    y: input_area.y.saturating_add(1),
                    width: input_area.width.saturating_sub(2),
                    height: text_area_height,
                };
                let mut state = self.textarea_state.borrow_mut();
                StatefulWidgetRef::render_ref(&(&self.textarea), textarea_rect, buf, &mut state);
                if self.textarea.text().is_empty() {
                    Paragraph::new(Line::from(self.placeholder.clone().dim()))
                        .render(textarea_rect, buf);
                }
            }
        }

        let hint_blank_y = input_area.y.saturating_add(input_height);
        if hint_blank_y < area.y.saturating_add(area.height) {
            let blank_area = Rect {
                x: area.x,
                y: hint_blank_y,
                width: area.width,
                height: 1,
            };
            Clear.render(blank_area, buf);
        }

        let hint_y = hint_blank_y.saturating_add(1);
        if hint_y < area.y.saturating_add(area.height) {
            Paragraph::new(standard_popup_hint_line()).render(
                Rect {
                    x: area.x,
                    y: hint_y,
                    width: area.width,
                    height: 1,
                },
                buf,
            );
        }
    }

    /// Returns the cursor position inside the textarea, if it is visible.
    ///
    /// The cursor is offset by the title line, optional context line, and the
    /// gutter column that prefixes the input area.
    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        if area.height < 2 || area.width <= 2 {
            return None;
        }
        let text_area_height = self.input_height(area.width).saturating_sub(1);
        if text_area_height == 0 {
            return None;
        }
        let extra_offset: u16 = if self.context_label.is_some() { 1 } else { 0 };
        let top_line_count = 1u16 + extra_offset;
        let textarea_rect = Rect {
            x: area.x.saturating_add(2),
            y: area.y.saturating_add(top_line_count).saturating_add(1),
            width: area.width.saturating_sub(2),
            height: text_area_height,
        };
        let state = *self.textarea_state.borrow();
        self.textarea.cursor_pos_with_state(textarea_rect, state)
    }
}

impl CustomPromptView {
    /// Computes the input area height for the given width, capped to keep popups compact.
    ///
    /// The returned height includes the single-row gutter and clamps the text
    /// rows between 1 and 8, yielding a total height between 2 and 9.
    fn input_height(&self, width: u16) -> u16 {
        let usable_width = width.saturating_sub(2);
        let text_height = self.textarea.desired_height(usable_width).clamp(1, 8);
        text_height.saturating_add(1).min(9)
    }
}

/// Returns the colored gutter prefix used by popup lines.
fn gutter() -> Span<'static> {
    "▌ ".cyan()
}
