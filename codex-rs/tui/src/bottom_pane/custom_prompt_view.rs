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
use std::time::Instant;

use crate::key_hint::has_ctrl_or_alt;
use crate::render::renderable::Renderable;

use super::popup_consts::standard_popup_hint_line;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::bottom_pane_view::ViewCompletion;
use super::paste_burst::CharDecision;
use super::paste_burst::FlushResult;
use super::paste_burst::PasteBurst;
use super::textarea::TextArea;
use super::textarea::TextAreaState;

/// Callback invoked when the user submits a custom prompt.
pub(crate) type PromptSubmitted = Box<dyn Fn(String) + Send + Sync>;

/// Minimal multi-line text input view to collect custom review instructions.
pub(crate) struct CustomPromptView {
    title: String,
    placeholder: String,
    context_label: Option<String>,
    on_submit: PromptSubmitted,

    // UI state
    textarea: TextArea,
    textarea_state: RefCell<TextAreaState>,
    paste_burst: PasteBurst,
    completion: Option<ViewCompletion>,
}

impl CustomPromptView {
    pub(crate) fn new(
        title: String,
        placeholder: String,
        initial_text: String,
        context_label: Option<String>,
        on_submit: PromptSubmitted,
    ) -> Self {
        let mut textarea = TextArea::new();
        if !initial_text.is_empty() {
            textarea.set_text_clearing_elements(&initial_text);
            textarea.set_cursor(initial_text.len());
        }

        Self {
            title,
            placeholder,
            context_label,
            on_submit,
            textarea,
            textarea_state: RefCell::new(TextAreaState::default()),
            paste_burst: PasteBurst::default(),
            completion: None,
        }
    }

    fn handle_key_event_at(&mut self, key_event: KeyEvent, now: Instant) {
        // Text is inserted immediately below; flushing only expires Enter suppression.
        self.flush_paste_burst_detector(now);
        match key_event {
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.on_ctrl_c();
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers,
                ..
            } => {
                if self
                    .paste_burst
                    .newline_should_insert_instead_of_submit(now)
                {
                    self.paste_burst.extend_window(now);
                    self.textarea.insert_str("\n");
                    return;
                }
                if modifiers == KeyModifiers::NONE {
                    let text = self.textarea.text().trim().to_string();
                    if !text.is_empty() {
                        (self.on_submit)(text);
                        self.completion = Some(ViewCompletion::Accepted);
                    }
                } else {
                    self.textarea.input(key_event);
                }
            }
            KeyEvent {
                code: KeyCode::Char(_),
                modifiers,
                ..
            } if !has_ctrl_or_alt(modifiers) && self.textarea.allows_paste_burst() => {
                let burst_retro_chars = match self.paste_burst.on_plain_char_no_hold(now) {
                    Some(CharDecision::BeginBuffer { retro_chars }) => {
                        Some(usize::from(retro_chars) + 1)
                    }
                    _ => None,
                };
                self.textarea.input(key_event);
                if let Some(retro_chars) = burst_retro_chars {
                    let cursor = self.textarea.cursor();
                    let before_cursor = &self.textarea.text()[..cursor];
                    let _ = self
                        .paste_burst
                        .decide_begin_buffer(now, before_cursor, retro_chars);
                }
            }
            other => {
                self.textarea.input(other);
                self.paste_burst.clear_after_explicit_paste();
            }
        }
    }

    fn flush_paste_burst_detector(&mut self, now: Instant) -> bool {
        let flushed = !matches!(self.paste_burst.flush_if_due(now), FlushResult::None);
        if flushed {
            self.paste_burst.clear_after_explicit_paste();
        }
        flushed
    }
}

impl BottomPaneView for CustomPromptView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        self.handle_key_event_at(key_event, Instant::now());
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.completion = Some(ViewCompletion::Cancelled);
        CancellationEvent::Handled
    }

    fn is_complete(&self) -> bool {
        self.completion.is_some()
    }

    fn completion(&self) -> Option<ViewCompletion> {
        self.completion
    }

    fn handle_paste(&mut self, pasted: String) -> bool {
        if pasted.is_empty() {
            return false;
        }
        self.textarea.insert_str(&pasted);
        self.paste_burst.clear_after_explicit_paste();
        true
    }

    fn flush_paste_burst_if_due(&mut self) -> bool {
        self.flush_paste_burst_detector(Instant::now())
    }

    fn is_in_paste_burst(&self) -> bool {
        self.paste_burst.is_active()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paste_burst_newline_does_not_submit() {
        let (submitted, submitted_rx) = std::sync::mpsc::channel();
        let mut view = CustomPromptView::new(
            "Edit goal".to_string(),
            "Type a goal objective and press Enter".to_string(),
            "existing".to_string(),
            /*context_label*/ None,
            Box::new(move |text| {
                submitted.send(text).expect("send submitted text");
            }),
        );
        let now = Instant::now();

        for (idx, ch) in " first".chars().enumerate() {
            view.handle_key_event_at(KeyEvent::from(KeyCode::Char(ch)), now + elapsed(idx));
        }
        view.handle_key_event_at(KeyEvent::from(KeyCode::Enter), now + elapsed(6));
        for (idx, ch) in "second".chars().enumerate() {
            view.handle_key_event_at(KeyEvent::from(KeyCode::Char(ch)), now + elapsed(7 + idx));
        }

        assert!(submitted_rx.try_recv().is_err());
        assert!(!view.is_complete());

        view.flush_paste_burst_detector(
            now + elapsed(40) + PasteBurst::recommended_active_flush_delay(),
        );
        view.handle_key_event_at(
            KeyEvent::from(KeyCode::Enter),
            now + elapsed(80) + PasteBurst::recommended_active_flush_delay(),
        );

        assert_eq!(
            submitted_rx.try_recv(),
            Ok("existing first\nsecond".to_string())
        );
        assert!(view.is_complete());
    }

    fn elapsed(ms: usize) -> std::time::Duration {
        std::time::Duration::from_millis(ms as u64)
    }
}

impl Renderable for CustomPromptView {
    fn desired_height(&self, width: u16) -> u16 {
        let extra_top: u16 = if self.context_label.is_some() { 1 } else { 0 };
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
    fn input_height(&self, width: u16) -> u16 {
        let usable_width = width.saturating_sub(2);
        let text_height = self.textarea.desired_height(usable_width).clamp(1, 8);
        text_height.saturating_add(1).min(9)
    }
}

fn gutter() -> Span<'static> {
    "▌ ".cyan()
}
