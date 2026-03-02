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
pub(crate) type PromptSubmitted = Box<dyn Fn(String) -> Result<(), String> + Send + Sync>;
pub(crate) type PromptCancelled = Option<Box<dyn Fn() + Send + Sync>>;

/// Minimal multi-line text input view to collect custom review instructions.
pub(crate) struct CustomPromptView {
    title: String,
    placeholder: String,
    context_label: Option<String>,
    on_submit: PromptSubmitted,
    on_cancel: PromptCancelled,

    // UI state
    textarea: TextArea,
    textarea_state: RefCell<TextAreaState>,
    error_message: Option<String>,
    complete: bool,
}

impl CustomPromptView {
    pub(crate) fn new(
        title: String,
        placeholder: String,
        context_label: Option<String>,
        on_submit: PromptSubmitted,
        on_cancel: PromptCancelled,
    ) -> Self {
        Self {
            title,
            placeholder,
            context_label,
            on_submit,
            on_cancel,
            textarea: TextArea::new(),
            textarea_state: RefCell::new(TextAreaState::default()),
            error_message: None,
            complete: false,
        }
    }
}

impl BottomPaneView for CustomPromptView {
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
                    match (self.on_submit)(text) {
                        Ok(()) => self.complete = true,
                        Err(err) => self.error_message = Some(err),
                    }
                }
            }
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                self.error_message = None;
                self.textarea.input(key_event);
            }
            other => {
                self.error_message = None;
                self.textarea.input(other);
            }
        }
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        if let Some(on_cancel) = &self.on_cancel {
            on_cancel();
        }
        self.complete = true;
        CancellationEvent::Handled
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn handle_paste(&mut self, pasted: String) -> bool {
        if pasted.is_empty() {
            return false;
        }
        self.error_message = None;
        self.textarea.insert_str(&pasted);
        true
    }
}

impl Renderable for CustomPromptView {
    fn desired_height(&self, width: u16) -> u16 {
        let extra_top: u16 = if self.context_label.is_some() || self.error_message.is_some() {
            1
        } else {
            0
        };
        1u16 + extra_top + self.input_height(width) + 3u16
    }

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
        if let Some(error_message) = &self.error_message {
            let context_area = Rect {
                x: area.x,
                y: input_y,
                width: area.width,
                height: 1,
            };
            let spans: Vec<Span<'static>> = vec![gutter(), error_message.clone().red()];
            Paragraph::new(Line::from(spans)).render(context_area, buf);
            input_y = input_y.saturating_add(1);
        } else if let Some(context_label) = &self.context_label {
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

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        if area.height < 2 || area.width <= 2 {
            return None;
        }
        let text_area_height = self.input_height(area.width).saturating_sub(1);
        if text_area_height == 0 {
            return None;
        }
        let extra_offset: u16 = if self.error_message.is_some() || self.context_label.is_some() {
            1
        } else {
            0
        };
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
    fn input_height(&self, width: u16) -> u16 {
        let usable_width = width.saturating_sub(2);
        let text_height = self.textarea.desired_height(usable_width).clamp(1, 8);
        text_height.saturating_add(1).min(9)
    }
}

fn gutter() -> Span<'static> {
    "▌ ".cyan()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    #[test]
    fn custom_prompt_cursor_uses_error_row_offset() {
        let mut view = CustomPromptView::new(
            "Set max fix attempts".to_string(),
            "Type a positive integer and press Enter".to_string(),
            None,
            Box::new(|value: String| match value.parse::<u32>() {
                Ok(value) if value > 0 => Ok(()),
                Ok(_) | Err(_) => Err("Enter a positive integer.".to_string()),
            }),
            None,
        );

        view.handle_key_event(KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE));
        view.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        let prompt_area = Rect::new(0, 0, 80, view.desired_height(80));
        let (_x, y) = view
            .cursor_pos(prompt_area)
            .expect("cursor should be positioned when popup is active");
        assert_eq!(y, 3);
    }

    #[test]
    fn custom_prompt_cursor_uses_context_row_offset() {
        let mut view = CustomPromptView::new(
            "Custom prompt".to_string(),
            "Type your input".to_string(),
            Some("Optional context".to_string()),
            Box::new(|_| Ok(())),
            None,
        );

        view.handle_key_event(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));

        let prompt_area = Rect::new(0, 0, 80, view.desired_height(80));
        let (_x, y) = view
            .cursor_pos(prompt_area)
            .expect("cursor should be positioned when popup is active");
        assert_eq!(y, 3);
    }

    #[test]
    fn custom_prompt_cursor_uses_basic_row_offset() {
        let mut view = CustomPromptView::new(
            "Custom prompt".to_string(),
            "Type your input".to_string(),
            None,
            Box::new(|_| Ok(())),
            None,
        );

        view.handle_key_event(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));

        let prompt_area = Rect::new(0, 0, 80, view.desired_height(80));
        let (_x, y) = view
            .cursor_pos(prompt_area)
            .expect("cursor should be positioned when popup is active");
        assert_eq!(y, 2);
    }
}
