//! Collects a feedback note with audience-specific disclosure and independent log consent.
//! The editor and submission controls remain visible when the disclosure must scroll.
//! Unbracketed paste bursts insert newlines; Enter submits after the burst ends.

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
use std::cell::Cell;
use std::cell::RefCell;
use std::time::Instant;

use crate::app_event::AppEvent;
use crate::app_event::FeedbackCategory;
use crate::app_event_sender::AppEventSender;
use crate::key_hint;
use crate::key_hint::has_ctrl_or_alt;
use crate::render::renderable::Renderable;
use crate::terminal_hyperlinks::mark_underlined_hyperlink;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_lines;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::feedback_view::FeedbackAudience;
use super::paste_burst::PasteBurst;
use super::popup_consts::accept_cancel_hint_line;
use super::textarea::TextArea;
use super::textarea::TextAreaState;

const FEEDBACK_POLICY_URL: &str = "https://help.openai.com/en/articles/5722486-how-your-data-is-used-to-improve-model-performance";
const EMPLOYEE_FEEDBACK_FAQ_URL: &str = "https://go/codex-employee-feedback-faq";

enum IntroContent {
    Full,
    DisclosureOnly,
}

/// Minimal input overlay to collect an optional feedback note, then submit it
/// through the app-server-managed feedback flow.
pub(crate) struct FeedbackNoteView {
    category: FeedbackCategory,
    turn_id: Option<String>,
    app_event_tx: AppEventSender,
    include_logs: bool,
    feedback_audience: FeedbackAudience,

    // UI state
    textarea: TextArea,
    textarea_state: RefCell<TextAreaState>,
    paste_burst: PasteBurst,
    intro_scroll: Cell<usize>,
    intro_page_height: Cell<usize>,
    complete: bool,
}

impl FeedbackNoteView {
    pub(crate) fn new(
        category: FeedbackCategory,
        turn_id: Option<String>,
        app_event_tx: AppEventSender,
        include_logs: bool,
        feedback_audience: FeedbackAudience,
    ) -> Self {
        Self {
            category,
            turn_id,
            app_event_tx,
            include_logs,
            feedback_audience,
            textarea: TextArea::new(),
            textarea_state: RefCell::new(TextAreaState::default()),
            paste_burst: PasteBurst::default(),
            intro_scroll: Cell::new(0),
            intro_page_height: Cell::new(1),
            complete: false,
        }
    }

    fn submit(&mut self) {
        let note = self.textarea.text().trim().to_string();
        let reason = if note.is_empty() { None } else { Some(note) };
        self.app_event_tx.send(AppEvent::SubmitFeedback {
            category: self.category,
            reason,
            turn_id: self.turn_id.clone(),
            include_logs: self.include_logs,
        });
        self.complete = true;
    }

    fn handle_key_event_at(&mut self, key_event: KeyEvent, now: Instant) {
        match key_event {
            KeyEvent {
                code: KeyCode::PageUp,
                ..
            } => {
                self.intro_scroll.set(
                    self.intro_scroll
                        .get()
                        .saturating_sub(self.intro_page_height.get()),
                );
            }
            KeyEvent {
                code: KeyCode::PageDown,
                ..
            } => {
                self.intro_scroll.set(
                    self.intro_scroll
                        .get()
                        .saturating_add(self.intro_page_height.get()),
                );
            }
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
                if self.paste_burst.direct_insert_newline_should_insert(now) {
                    self.paste_burst.extend_window(now);
                    self.textarea.insert_str("\n");
                } else if modifiers == KeyModifiers::NONE {
                    self.submit();
                } else {
                    self.textarea.input(key_event);
                }
            }
            KeyEvent {
                code: KeyCode::Char(_),
                modifiers,
                ..
            } if !has_ctrl_or_alt(modifiers) => {
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
            } if !has_ctrl_or_alt(modifiers) => {
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
    }
}

impl BottomPaneView for FeedbackNoteView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        self.handle_key_event_at(key_event, Instant::now());
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
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
        self.textarea.insert_str(&pasted);
        self.paste_burst.clear_after_explicit_paste();
        true
    }
}

impl Renderable for FeedbackNoteView {
    fn desired_height(&self, width: u16) -> u16 {
        self.intro_lines(width, IntroContent::Full).len() as u16 + self.input_height(width) + 2u16
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        if area.height < 2 || area.width <= 2 {
            return None;
        }
        let input_area = self.input_area(area);
        let text_area_height = input_area.height.saturating_sub(1);
        if text_area_height == 0 {
            return None;
        }
        let textarea_rect = Rect {
            x: area.x.saturating_add(2),
            y: input_area.y.saturating_add(1),
            width: area.width.saturating_sub(2),
            height: text_area_height,
        };
        let state = *self.textarea_state.borrow();
        self.textarea.cursor_pos_with_state(textarea_rect, state)
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        // Four rows leave room for one intro line; show the disclosure before submission.
        let intro_content = if area.height == 4 {
            IntroContent::DisclosureOnly
        } else {
            IntroContent::Full
        };
        let intro_lines = self.intro_lines(area.width, intro_content);
        let input_area = self.input_area(area);
        let intro_area = Rect {
            height: input_area.y.saturating_sub(area.y),
            ..area
        };
        let (_, placeholder) = feedback_title_and_placeholder(self.category);
        let input_height = input_area.height;
        let max_scroll = intro_lines
            .len()
            .saturating_sub(usize::from(intro_area.height));
        self.intro_scroll
            .set(self.intro_scroll.get().min(max_scroll));
        self.intro_page_height
            .set(usize::from(intro_area.height.max(1)));

        for (offset, line) in intro_lines
            .iter()
            .skip(self.intro_scroll.get())
            .take(usize::from(intro_area.height))
            .enumerate()
        {
            Paragraph::new(line.clone()).render(
                Rect {
                    x: area.x,
                    y: area.y.saturating_add(offset as u16),
                    width: area.width,
                    height: 1,
                },
                buf,
            );
        }
        let disclosure_url = match self.feedback_audience {
            FeedbackAudience::OpenAiEmployee => EMPLOYEE_FEEDBACK_FAQ_URL,
            FeedbackAudience::External => FEEDBACK_POLICY_URL,
        };
        mark_underlined_hyperlink(buf, intro_area, disclosure_url);

        // Input line
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
                    Paragraph::new(Line::from(placeholder.dim())).render(textarea_rect, buf);
                }
            }
        }

        let hint_blank_y = input_area.y.saturating_add(input_height);
        let hint_spacing = u16::from(area.height > 4);
        if hint_spacing > 0 && hint_blank_y < area.y.saturating_add(area.height) {
            let blank_area = Rect {
                x: area.x,
                y: hint_blank_y,
                width: area.width,
                height: 1,
            };
            Clear.render(blank_area, buf);
            if max_scroll > 0 {
                Paragraph::new(Line::from(vec![
                    key_hint::plain(KeyCode::PageUp).into(),
                    "/".into(),
                    key_hint::plain(KeyCode::PageDown).into(),
                    " scroll disclosure".into(),
                ]))
                .render(blank_area, buf);
            }
        }

        let hint_y = hint_blank_y.saturating_add(hint_spacing);
        if hint_y < area.y.saturating_add(area.height) {
            Paragraph::new(accept_cancel_hint_line(
                Some(key_hint::plain(KeyCode::Enter).into()),
                "to submit",
                Some(key_hint::plain(KeyCode::Esc).into()),
                "to cancel",
            ))
            .render(
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
}

impl FeedbackNoteView {
    fn input_area(&self, area: Rect) -> Rect {
        // Reserve the editor and submit/cancel hints before allocating disclosure rows.
        let footer_height = if area.height > 4 { 2 } else { 1 };
        let input_height = self
            .input_height(area.width)
            .min(area.height.saturating_sub(footer_height + 1).max(2))
            .min(area.height);
        let intro_height = (self.intro_lines(area.width, IntroContent::Full).len() as u16)
            .min(area.height.saturating_sub(input_height + footer_height));
        Rect {
            y: area.y.saturating_add(intro_height),
            height: input_height,
            ..area
        }
    }

    fn input_height(&self, width: u16) -> u16 {
        let usable_width = width.saturating_sub(2);
        let text_height = self.textarea.desired_height(usable_width).clamp(1, 8);
        text_height.saturating_add(1).min(9)
    }

    fn intro_lines(&self, width: u16, content: IntroContent) -> Vec<Line<'static>> {
        let (title, _) = feedback_title_and_placeholder(self.category);
        let mut lines = match content {
            IntroContent::Full => vec![title.bold().into(), "".into()],
            IntroContent::DisclosureOnly => Vec::new(),
        };
        match self.feedback_audience {
            FeedbackAudience::OpenAiEmployee => {
                lines.push(
                    "Your data may be used to improve our models and products"
                        .bold()
                        .into(),
                );
                lines.push(Line::from(vec![
                    "By submitting feedback, you agree that OpenAI can use your feedback for safety purposes and internal model training, as explained in more detail ".dim(),
                    "here".cyan().underlined(),
                    ". Please avoid sharing tented information under NDA, or privileged, HR-related, or sensitive personal information.".dim(),
                ]));
            }
            FeedbackAudience::External => lines.push(Line::from(vec![
                "Your feedback can be used to improve ChatGPT. ".dim(),
                "Learn more".cyan().underlined(),
                ".".dim(),
            ])),
        }
        word_wrap_lines(
            lines,
            RtOptions::new(usize::from(width.max(1)))
                .initial_indent(Line::from(gutter()))
                .subsequent_indent(Line::from(gutter())),
        )
    }
}

fn gutter() -> Span<'static> {
    "▌ ".cyan()
}

fn feedback_title_and_placeholder(category: FeedbackCategory) -> (String, String) {
    match category {
        FeedbackCategory::BadResult => (
            "Tell us more (bad result)".to_string(),
            "(optional) Write a short description to help us further".to_string(),
        ),
        FeedbackCategory::GoodResult => (
            "Tell us more (good result)".to_string(),
            "(optional) Write a short description to help us further".to_string(),
        ),
        FeedbackCategory::Bug => (
            "Tell us more (bug)".to_string(),
            "(optional) Write a short description to help us further".to_string(),
        ),
        FeedbackCategory::SafetyCheck => (
            "Tell us more (safety check)".to_string(),
            "(optional) Share what was refused and why it should have been allowed".to_string(),
        ),
        FeedbackCategory::Other => (
            "Tell us more (other)".to_string(),
            "(optional) Write a short description to help us further".to_string(),
        ),
    }
}

#[cfg(test)]
#[path = "feedback_note_view_tests.rs"]
mod tests;
