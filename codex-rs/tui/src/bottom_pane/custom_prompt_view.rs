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
use uuid::Uuid;

use crate::key_hint;
use crate::key_hint::has_ctrl_or_alt;
use crate::keymap::KeymapContextSet;
use crate::keymap::RuntimeKeymap;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::render::renderable::Renderable;

use super::popup_consts::accept_cancel_hint_line;
use super::popup_consts::standard_popup_hint_line;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::bottom_pane_view::ViewCompletion;
use super::footer::max_left_width_for_right;
use super::footer::render_context_right;
use super::paste_burst::PasteBurst;
use super::textarea::TextArea;
use super::textarea::TextAreaState;

/// Callback invoked when the user submits a custom prompt.
pub(crate) type PromptSubmitted = Box<dyn Fn(String) + Send + Sync>;

/// Correlates a pending generated prefill with its eventual display label.
struct PendingTextSuggestion {
    request_id: Uuid,
    applied_label: String,
}

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
    pending_suggestion: Option<PendingTextSuggestion>,
    user_edited: bool,
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
            pending_suggestion: None,
            user_edited: false,
        }
    }

    /// Apply the same editor and Vim bindings used by the main composer.
    pub(crate) fn set_keymap_bindings(&mut self, keymap: &RuntimeKeymap) {
        self.textarea.set_keymap_bindings(keymap);
    }

    /// Enable Vim editing while keeping the prompt ready for immediate input.
    pub(crate) fn enable_vim_in_insert_mode(&mut self) {
        self.textarea.set_vim_enabled(/*enabled*/ true);
        self.textarea.enter_vim_insert_mode();
    }

    /// Opt this prompt into one correlated background text suggestion.
    pub(crate) fn with_text_suggestion(
        mut self,
        request_id: Uuid,
        loading_label: String,
        applied_label: String,
    ) -> Self {
        self.context_label = Some(loading_label);
        self.pending_suggestion = Some(PendingTextSuggestion {
            request_id,
            applied_label,
        });
        self
    }

    fn handle_key_event_at(&mut self, key_event: KeyEvent, now: Instant) {
        let previous_text = self
            .pending_suggestion
            .as_ref()
            .filter(|_| !self.user_edited)
            .map(|_| self.textarea.text().to_string());

        if self.textarea.is_vim_operator_pending() {
            self.textarea.input(key_event);
            self.user_edited |= previous_text.is_some_and(|text| self.textarea.text() != text);
            return;
        }

        match key_event {
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                if self.textarea.should_handle_vim_insert_escape(key_event) {
                    self.textarea.input(key_event);
                    self.paste_burst.clear_after_explicit_paste();
                } else {
                    self.on_ctrl_c();
                }
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers,
                ..
            } => {
                if self.textarea.allows_paste_burst()
                    && self.paste_burst.direct_insert_newline_should_insert(now)
                {
                    self.paste_burst.extend_window(now);
                    self.textarea.insert_str("\n");
                    self.user_edited = true;
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
                let paste_like_burst = self.paste_burst.on_plain_char_no_hold(now).is_some();
                self.textarea.input(key_event);
                if paste_like_burst {
                    self.paste_burst.extend_window(now);
                }
            }
            KeyEvent {
                code: KeyCode::Tab,
                modifiers,
                ..
            } if !has_ctrl_or_alt(modifiers) && self.textarea.allows_paste_burst() => {
                let in_paste_burst = self.paste_burst.direct_insert_newline_should_insert(now);
                self.textarea.input(key_event);
                if in_paste_burst {
                    self.paste_burst.extend_window(now);
                }
            }
            other => {
                self.textarea.input(other);
                self.paste_burst.clear_after_explicit_paste();
            }
        }

        self.user_edited |= previous_text.is_some_and(|text| self.textarea.text() != text);
    }
}

impl BottomPaneView for CustomPromptView {
    fn keymap_contexts(&self) -> KeymapContextSet {
        KeymapContextSet::new(self.textarea.keymap_context())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        self.handle_key_event_at(key_event, Instant::now());
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.completion = Some(ViewCompletion::Cancelled);
        CancellationEvent::Handled
    }

    fn prefer_esc_to_handle_key_event(&self) -> bool {
        self.textarea.uses_vim_insert_cursor() || self.textarea.is_vim_operator_pending()
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
        self.user_edited = true;
        self.paste_burst.clear_after_explicit_paste();
        true
    }

    fn apply_text_suggestion(&mut self, request_id: Uuid, suggestion: Option<&str>) -> bool {
        if self.completion.is_some() {
            return false;
        }

        let Some(pending) = self
            .pending_suggestion
            .take_if(|pending| pending.request_id == request_id)
        else {
            return false;
        };

        self.context_label = None;

        if !self.user_edited
            && let Some(suggestion) = suggestion
        {
            self.textarea.set_text_clearing_elements(suggestion);
            self.textarea.set_cursor(suggestion.len());
            self.context_label = Some(pending.applied_label);
        }

        true
    }
}

#[cfg(test)]
#[path = "custom_prompt_view_tests.rs"]
mod tests;

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
            let mut hint_line = if self.textarea.uses_vim_insert_cursor() {
                accept_cancel_hint_line(
                    Some(key_hint::plain(KeyCode::Enter).into()),
                    "to confirm",
                    Some(key_hint::plain(KeyCode::Esc).into()),
                    "to enter normal mode",
                )
            } else {
                standard_popup_hint_line()
            };
            let hint_area = Rect {
                x: area.x,
                y: hint_y,
                width: area.width,
                height: 1,
            };
            let mut vim_mode = self.textarea.vim_mode_indicator_span().map(Line::from);
            let available_hint_width = vim_mode
                .as_ref()
                .and_then(|line| max_left_width_for_right(hint_area, line.width() as u16))
                .filter(|width| *width > 0);
            if vim_mode.is_some() && available_hint_width.is_none() {
                vim_mode = None;
            }
            if let Some(width) = available_hint_width {
                hint_line = truncate_line_with_ellipsis_if_overflow(hint_line, width as usize);
            }
            Paragraph::new(hint_line).render(hint_area, buf);
            if let Some(line) = vim_mode {
                render_context_right(hint_area, buf, &line);
            }
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

    fn cursor_style(&self, _area: Rect) -> crossterm::cursor::SetCursorStyle {
        if self.textarea.uses_vim_insert_cursor() {
            crossterm::cursor::SetCursorStyle::SteadyBar
        } else {
            crossterm::cursor::SetCursorStyle::DefaultUserShape
        }
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
