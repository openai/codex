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

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::list_selection_view::ListSelectionView;
use super::list_selection_view::SelectionViewParams;
use super::popup_consts::standard_popup_hint_line;
use super::textarea::TextArea;
use super::textarea::TextAreaState;

pub(crate) struct CancelableSelectionView {
    inner: ListSelectionView,
    on_cancel: Box<dyn Fn() + Send + Sync>,
}

impl CancelableSelectionView {
    pub(crate) fn new(
        params: SelectionViewParams,
        app_event_tx: crate::app_event_sender::AppEventSender,
        on_cancel: Box<dyn Fn() + Send + Sync>,
    ) -> Self {
        Self {
            inner: ListSelectionView::new(params, app_event_tx),
            on_cancel,
        }
    }
}

impl BottomPaneView for CancelableSelectionView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if matches!(key_event.code, KeyCode::Esc) {
            self.on_ctrl_c();
            return;
        }
        self.inner.handle_key_event(key_event);
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        (self.on_cancel)();
        self.inner.on_ctrl_c()
    }

    fn is_complete(&self) -> bool {
        self.inner.is_complete()
    }

    fn handle_paste(&mut self, pasted: String) -> bool {
        self.inner.handle_paste(pasted)
    }
}

impl Renderable for CancelableSelectionView {
    fn desired_height(&self, width: u16) -> u16 {
        self.inner.desired_height(width)
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.inner.render(area, buf);
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        self.inner.cursor_pos(area)
    }
}

pub(crate) struct AskUserQuestionTextView {
    title: String,
    prompt: String,
    placeholder: String,
    allow_empty: bool,
    on_submit: Box<dyn Fn(String) + Send + Sync>,
    on_cancel: Box<dyn Fn() + Send + Sync>,

    textarea: TextArea,
    textarea_state: RefCell<TextAreaState>,
    complete: bool,
}

impl AskUserQuestionTextView {
    pub(crate) fn new(
        title: String,
        prompt: String,
        placeholder: String,
        allow_empty: bool,
        on_submit: Box<dyn Fn(String) + Send + Sync>,
        on_cancel: Box<dyn Fn() + Send + Sync>,
    ) -> Self {
        Self {
            title,
            prompt,
            placeholder,
            allow_empty,
            on_submit,
            on_cancel,
            textarea: TextArea::new(),
            textarea_state: RefCell::new(TextAreaState::default()),
            complete: false,
        }
    }

    fn input_height(&self, width: u16) -> u16 {
        let min_height = 3u16;
        let max_height = 8u16;
        let needed = self.textarea.desired_height(width.saturating_sub(2));
        needed.clamp(min_height, max_height)
    }
}

impl BottomPaneView for AskUserQuestionTextView {
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
                if self.allow_empty || !text.is_empty() {
                    (self.on_submit)(text);
                    self.complete = true;
                }
            }
            other => {
                self.textarea.input(other);
            }
        }
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        (self.on_cancel)();
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
        true
    }
}

impl Renderable for AskUserQuestionTextView {
    fn desired_height(&self, width: u16) -> u16 {
        2u16 + self.input_height(width) + 3u16
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let input_height = self.input_height(area.width);

        let title_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        };
        Paragraph::new(Line::from(vec![gutter(), self.title.clone().bold()]))
            .render(title_area, buf);

        let prompt_area = Rect {
            x: area.x,
            y: area.y.saturating_add(1),
            width: area.width,
            height: 1,
        };
        Paragraph::new(Line::from(vec![gutter(), self.prompt.clone().into()]))
            .render(prompt_area, buf);

        let input_area = Rect {
            x: area.x,
            y: area.y.saturating_add(2),
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
        if area.height < 3 || area.width <= 2 {
            return None;
        }
        let input_height = self.input_height(area.width);
        let text_area_height = input_height.saturating_sub(1);
        if text_area_height == 0 {
            return None;
        }
        let textarea_rect = Rect {
            x: area.x.saturating_add(2),
            y: area.y.saturating_add(3),
            width: area.width.saturating_sub(2),
            height: text_area_height,
        };
        self.textarea.cursor_pos(textarea_rect)
    }
}

fn gutter() -> Span<'static> {
    "  ".into()
}
