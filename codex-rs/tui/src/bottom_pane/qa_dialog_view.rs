use std::cell::RefCell;
use std::collections::VecDeque;

use codex_core::protocol::Op;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputEvent;
use codex_protocol::request_user_input::RequestUserInputQuestion;
use codex_protocol::request_user_input::RequestUserInputResponse;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::StatefulWidgetRef;
use ratatui::widgets::Widget;
use textwrap::wrap;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::key_hint;
use crate::render::renderable::Renderable;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::textarea::TextArea;
use super::textarea::TextAreaState;

const ANSWER_PLACEHOLDER: &str = "Type your answer (optional)";
const QUESTION_PREFIX: &str = "Q: ";
const ANSWER_PREFIX: &str = "A: ";

pub(crate) struct QaDialogView {
    app_event_tx: AppEventSender,
    request: RequestUserInputEvent,
    queue: VecDeque<RequestUserInputEvent>,
    textarea: TextArea,
    textarea_state: RefCell<TextAreaState>,
    answers: Vec<String>,
    current_idx: usize,
    complete: bool,
}

struct LayoutSections {
    progress_area: Rect,
    header_area: Rect,
    question_area: Rect,
    answer_title_area: Rect,
    answer_area: Rect,
    footer_area: Rect,
    question_lines: Vec<Line<'static>>,
}

impl QaDialogView {
    pub(crate) fn new(request: RequestUserInputEvent, app_event_tx: AppEventSender) -> Self {
        let mut view = Self {
            app_event_tx,
            request,
            queue: VecDeque::new(),
            textarea: TextArea::new(),
            textarea_state: RefCell::new(TextAreaState::default()),
            answers: Vec::new(),
            current_idx: 0,
            complete: false,
        };
        view.reset_for_request();
        view
    }

    pub(crate) fn can_render(request: &RequestUserInputEvent) -> bool {
        !request.questions.is_empty()
            && request.questions.iter().all(|question| {
                question
                    .options
                    .as_ref()
                    .is_none_or(std::vec::Vec::is_empty)
            })
    }

    fn reset_for_request(&mut self) {
        self.answers = vec![String::new(); self.request.questions.len()];
        self.current_idx = 0;
        self.load_current_answer();
        *self.textarea_state.borrow_mut() = TextAreaState::default();
        self.complete = false;
    }

    fn question_count(&self) -> usize {
        self.request.questions.len()
    }

    fn current_index(&self) -> usize {
        self.current_idx
    }

    fn question(&self) -> Option<&RequestUserInputQuestion> {
        self.request.questions.get(self.current_index())
    }

    fn save_current_answer(&mut self) {
        let idx = self.current_idx;
        if let Some(slot) = self.answers.get_mut(idx) {
            *slot = self.textarea.text().to_string();
        }
    }

    fn load_current_answer(&mut self) {
        let idx = self.current_idx;
        let answer = self.answers.get(idx).cloned().unwrap_or_default();
        self.textarea.set_text_clearing_elements(&answer);
        self.textarea.set_cursor(answer.len());
    }

    fn question_lines(&self, width: u16) -> Vec<Line<'static>> {
        let Some(question) = self.question() else {
            return vec![Line::from("No question".dim())];
        };
        let prefix_width = QUESTION_PREFIX.len();
        let wrap_width = width.max(1) as usize;
        if wrap_width <= prefix_width {
            return vec![Line::from(QUESTION_PREFIX.dim())];
        }
        let wrapped = wrap(&question.question, wrap_width - prefix_width);
        if wrapped.is_empty() {
            return vec![Line::from(QUESTION_PREFIX.dim())];
        }
        let indent = " ".repeat(prefix_width);
        wrapped
            .into_iter()
            .enumerate()
            .map(|(idx, line)| {
                let text = line.into_owned();
                if idx == 0 {
                    Line::from(vec![QUESTION_PREFIX.cyan().bold(), text.into()])
                } else {
                    Line::from(vec![indent.clone().into(), text.into()])
                }
            })
            .collect()
    }

    fn answer_input_height(&self, width: u16) -> u16 {
        let usable_width = width.saturating_sub(2);
        let text_height = self.textarea.desired_height(usable_width).clamp(1, 6);
        text_height.saturating_add(2).clamp(3, 8)
    }

    fn layout_sections(&self, area: Rect) -> LayoutSections {
        let question_lines = self.question_lines(area.width);
        let progress_height = if area.height == 0 { 0u16 } else { 1u16 };
        let header_height = if area.height == 0 { 0u16 } else { 1u16 };
        let max_question_height = area
            .height
            .saturating_sub(progress_height.saturating_add(header_height));
        let question_height = (question_lines.len() as u16).min(max_question_height);
        let footer_height = if area.height == 0 { 0u16 } else { 1u16 };
        let mut answer_title_height = if area.height == 0 { 0u16 } else { 1u16 };
        let mut answer_height = self.answer_input_height(area.width);

        let mut cursor_y = area.y;
        let progress_area = Rect {
            x: area.x,
            y: cursor_y,
            width: area.width,
            height: progress_height,
        };
        cursor_y = cursor_y.saturating_add(progress_height);
        let header_area = Rect {
            x: area.x,
            y: cursor_y,
            width: area.width,
            height: header_height,
        };
        cursor_y = cursor_y.saturating_add(header_height);
        let question_area = Rect {
            x: area.x,
            y: cursor_y,
            width: area.width,
            height: question_height,
        };
        cursor_y = cursor_y.saturating_add(question_height);

        let remaining = area.height.saturating_sub(cursor_y.saturating_sub(area.y));
        if remaining <= footer_height {
            answer_title_height = 0;
            answer_height = 0;
        } else {
            let max_answer = remaining
                .saturating_sub(footer_height)
                .saturating_sub(answer_title_height);
            if max_answer == 0 {
                answer_title_height = 0;
                answer_height = remaining.saturating_sub(footer_height).min(1);
            } else {
                answer_height = answer_height.min(max_answer).max(1);
            }
        }

        let answer_title_area = Rect {
            x: area.x,
            y: cursor_y,
            width: area.width,
            height: answer_title_height,
        };
        cursor_y = cursor_y.saturating_add(answer_title_height);
        let answer_area = Rect {
            x: area.x,
            y: cursor_y,
            width: area.width,
            height: answer_height,
        };
        cursor_y = cursor_y.saturating_add(answer_height);
        let footer_area = Rect {
            x: area.x,
            y: cursor_y,
            width: area.width,
            height: area.height.saturating_sub(cursor_y.saturating_sub(area.y)),
        };

        LayoutSections {
            progress_area,
            header_area,
            question_area,
            answer_title_area,
            answer_area,
            footer_area,
            question_lines,
        }
    }

    fn submit_answers(&mut self) {
        let mut answers = std::collections::HashMap::new();
        self.save_current_answer();
        for (idx, question) in self.request.questions.iter().enumerate() {
            let answer_text = self.answers.get(idx).map(|text| text.trim()).unwrap_or("");
            let mut answer_list = Vec::new();
            if !answer_text.is_empty() {
                answer_list.push(format!("user_note: {answer_text}"));
            }
            answers.insert(
                question.id.clone(),
                RequestUserInputAnswer {
                    answers: answer_list,
                },
            );
        }
        self.app_event_tx
            .send(AppEvent::CodexOp(Op::UserInputAnswer {
                id: self.request.turn_id.clone(),
                response: RequestUserInputResponse { answers },
            }));
        if let Some(next) = self.queue.pop_front() {
            self.request = next;
            self.reset_for_request();
        } else {
            self.complete = true;
        }
    }

    fn move_question(&mut self, forward: bool) {
        if self.question_count() == 0 {
            return;
        }
        self.save_current_answer();
        if forward {
            if self.current_index() + 1 < self.question_count() {
                self.current_idx = self.current_idx.saturating_add(1);
            }
        } else if self.current_index() > 0 {
            self.current_idx = self.current_idx.saturating_sub(1);
        }
        self.load_current_answer();
        *self.textarea_state.borrow_mut() = TextAreaState::default();
    }

    fn go_next_or_submit(&mut self) {
        if self.current_index() + 1 >= self.question_count() {
            self.submit_answers();
        } else {
            self.move_question(true);
        }
    }

    fn render_answer_input(&self, area: Rect, buf: &mut Buffer) {
        if area.width < 2 || area.height == 0 {
            return;
        }
        if area.height < 3 {
            let prefix_width = ANSWER_PREFIX.len() as u16;
            if area.width <= prefix_width {
                Paragraph::new(Line::from(ANSWER_PREFIX.dim())).render(area, buf);
                return;
            }
            Paragraph::new(Line::from(ANSWER_PREFIX.dim())).render(
                Rect {
                    x: area.x,
                    y: area.y,
                    width: prefix_width,
                    height: 1,
                },
                buf,
            );
            let textarea_rect = Rect {
                x: area.x.saturating_add(prefix_width),
                y: area.y,
                width: area.width.saturating_sub(prefix_width),
                height: 1,
            };
            let mut state = self.textarea_state.borrow_mut();
            Clear.render(textarea_rect, buf);
            StatefulWidgetRef::render_ref(&(&self.textarea), textarea_rect, buf, &mut state);
            if self.textarea.text().is_empty() {
                Paragraph::new(Line::from(ANSWER_PLACEHOLDER.dim())).render(textarea_rect, buf);
            }
            return;
        }
        let top_border = format!("+{}+", "-".repeat(area.width.saturating_sub(2) as usize));
        let bottom_border = top_border.clone();
        Paragraph::new(Line::from(top_border)).render(
            Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1,
            },
            buf,
        );
        Paragraph::new(Line::from(bottom_border)).render(
            Rect {
                x: area.x,
                y: area.y.saturating_add(area.height.saturating_sub(1)),
                width: area.width,
                height: 1,
            },
            buf,
        );
        for row in 1..area.height.saturating_sub(1) {
            Line::from(vec![
                "|".into(),
                " ".repeat(area.width.saturating_sub(2) as usize).into(),
                "|".into(),
            ])
            .render(
                Rect {
                    x: area.x,
                    y: area.y.saturating_add(row),
                    width: area.width,
                    height: 1,
                },
                buf,
            );
        }
        let text_area_height = area.height.saturating_sub(2);
        let textarea_rect = Rect {
            x: area.x.saturating_add(1),
            y: area.y.saturating_add(1),
            width: area.width.saturating_sub(2),
            height: text_area_height,
        };
        let mut state = self.textarea_state.borrow_mut();
        Clear.render(textarea_rect, buf);
        StatefulWidgetRef::render_ref(&(&self.textarea), textarea_rect, buf, &mut state);
        if self.textarea.text().is_empty() {
            Paragraph::new(Line::from(ANSWER_PLACEHOLDER.dim())).render(textarea_rect, buf);
        }
    }

    fn cursor_pos_impl(&self, area: Rect) -> Option<(u16, u16)> {
        let sections = self.layout_sections(area);
        let input_area = sections.answer_area;
        if input_area.height == 0 || input_area.width == 0 {
            return None;
        }
        if input_area.height < 3 {
            let prefix_width = ANSWER_PREFIX.len() as u16;
            if input_area.width <= prefix_width {
                return None;
            }
            let textarea_rect = Rect {
                x: input_area.x.saturating_add(prefix_width),
                y: input_area.y,
                width: input_area.width.saturating_sub(prefix_width),
                height: 1,
            };
            let state = *self.textarea_state.borrow();
            return self.textarea.cursor_pos_with_state(textarea_rect, state);
        }
        let text_area_height = input_area.height.saturating_sub(2);
        let textarea_rect = Rect {
            x: input_area.x.saturating_add(1),
            y: input_area.y.saturating_add(1),
            width: input_area.width.saturating_sub(2),
            height: text_area_height,
        };
        let state = *self.textarea_state.borrow();
        self.textarea.cursor_pos_with_state(textarea_rect, state)
    }
}

impl BottomPaneView for QaDialogView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Release {
            return;
        }
        match key_event {
            KeyEvent {
                code: KeyCode::PageUp,
                ..
            } => {
                self.move_question(false);
            }
            KeyEvent {
                code: KeyCode::PageDown,
                ..
            } => {
                self.move_question(true);
            }
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
                self.go_next_or_submit();
            }
            other => {
                self.textarea.input(other);
            }
        }
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.app_event_tx.send(AppEvent::CodexOp(Op::Interrupt));
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

    fn try_consume_user_input_request(
        &mut self,
        request: RequestUserInputEvent,
    ) -> Option<RequestUserInputEvent> {
        if Self::can_render(&request) {
            self.queue.push_back(request);
            None
        } else {
            Some(request)
        }
    }
}

impl Renderable for QaDialogView {
    fn desired_height(&self, width: u16) -> u16 {
        if width == 0 {
            return 0;
        }
        let progress_height = 1u16;
        let header_height = 1u16;
        let question_height = self.question_lines(width).len() as u16;
        let answer_title_height = 1u16;
        let answer_height = self.answer_input_height(width);
        let footer_height = 1u16;
        progress_height
            .saturating_add(header_height)
            .saturating_add(question_height)
            .saturating_add(answer_title_height)
            .saturating_add(answer_height)
            .saturating_add(footer_height)
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        Clear.render(area, buf);
        let sections = self.layout_sections(area);

        if sections.progress_area.height > 0 {
            let progress_line = if self.question_count() > 0 {
                let idx = self.current_index() + 1;
                let total = self.question_count();
                Line::from(format!("Question {idx}/{total}").dim())
            } else {
                Line::from("No questions".dim())
            };
            Paragraph::new(progress_line).render(sections.progress_area, buf);
        }

        if sections.header_area.height > 0 {
            let header = self
                .question()
                .map(|q| q.header.trim())
                .filter(|text| !text.is_empty());
            let header_line = if let Some(text) = header {
                Line::from(text.to_string().bold())
            } else {
                Line::from("Question".dim())
            };
            Paragraph::new(header_line).render(sections.header_area, buf);
        }

        let question_y = sections.question_area.y;
        for (offset, line) in sections.question_lines.iter().enumerate() {
            if question_y.saturating_add(offset as u16)
                >= sections.question_area.y + sections.question_area.height
            {
                break;
            }
            Paragraph::new(line.clone()).render(
                Rect {
                    x: sections.question_area.x,
                    y: question_y.saturating_add(offset as u16),
                    width: sections.question_area.width,
                    height: 1,
                },
                buf,
            );
        }

        if sections.answer_title_area.height > 0 {
            Paragraph::new(Line::from("Answer".cyan().bold()))
                .render(sections.answer_title_area, buf);
        }

        if sections.answer_area.height > 0 {
            self.render_answer_input(sections.answer_area, buf);
        }

        if sections.footer_area.height > 0 {
            let enter_action = if self.current_index() + 1 >= self.question_count() {
                "submit"
            } else {
                "next"
            };
            let mut hint_spans = vec![
                key_hint::plain(KeyCode::Enter).into(),
                format!(" {enter_action}").into(),
                " | ".into(),
            ];
            if self.question_count() > 1 {
                hint_spans.extend(vec![
                    key_hint::plain(KeyCode::PageUp).into(),
                    " prev | ".into(),
                    key_hint::plain(KeyCode::PageDown).into(),
                    " next | ".into(),
                ]);
            }
            hint_spans.extend(vec![
                key_hint::plain(KeyCode::Esc).into(),
                " interrupt".into(),
            ]);
            let hint = Line::from(hint_spans);
            Paragraph::new(hint.dim()).render(sections.footer_area, buf);
        }
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        self.cursor_pos_impl(area)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::renderable::Renderable;
    use codex_protocol::request_user_input::RequestUserInputQuestion;
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use tokio::sync::mpsc::unbounded_channel;

    fn test_sender() -> (
        AppEventSender,
        tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    ) {
        let (tx_raw, rx) = unbounded_channel::<AppEvent>();
        (AppEventSender::new(tx_raw), rx)
    }

    fn request_event(turn_id: &str, question: RequestUserInputQuestion) -> RequestUserInputEvent {
        RequestUserInputEvent {
            call_id: "call-1".to_string(),
            turn_id: turn_id.to_string(),
            questions: vec![question],
        }
    }

    fn question(id: &str) -> RequestUserInputQuestion {
        RequestUserInputQuestion {
            id: id.to_string(),
            header: "Clarify".to_string(),
            question: "What should the modal title be?".to_string(),
            options: None,
        }
    }

    fn question_short(id: &str, text: &str) -> RequestUserInputQuestion {
        RequestUserInputQuestion {
            id: id.to_string(),
            header: "Clarify".to_string(),
            question: text.to_string(),
            options: None,
        }
    }

    fn snapshot_buffer(buf: &Buffer) -> String {
        let mut lines = Vec::new();
        for y in 0..buf.area().height {
            let mut row = String::new();
            for x in 0..buf.area().width {
                row.push(buf[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            lines.push(row);
        }
        lines.join("\n")
    }

    fn render_snapshot(view: &QaDialogView, area: Rect) -> String {
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);
        snapshot_buffer(&buf)
    }

    #[test]
    fn submits_user_note_answer() {
        let (tx, mut rx) = test_sender();
        let mut view = QaDialogView::new(request_event("turn-1", question("q1")), tx);
        view.textarea.insert_str("Use `Confirm changes`.");

        view.submit_answers();

        let event = rx.try_recv().expect("expected AppEvent");
        let AppEvent::CodexOp(Op::UserInputAnswer { response, .. }) = event else {
            panic!("expected UserInputAnswer");
        };
        let answer = response.answers.get("q1").expect("answer missing");
        assert_eq!(
            answer.answers,
            vec!["user_note: Use `Confirm changes`.".to_string()]
        );
    }

    #[test]
    fn submits_empty_answer_when_blank() {
        let (tx, mut rx) = test_sender();
        let mut view = QaDialogView::new(request_event("turn-2", question("q1")), tx);

        view.submit_answers();

        let event = rx.try_recv().expect("expected AppEvent");
        let AppEvent::CodexOp(Op::UserInputAnswer { response, .. }) = event else {
            panic!("expected UserInputAnswer");
        };
        let answer = response.answers.get("q1").expect("answer missing");
        assert_eq!(answer.answers, Vec::<String>::new());
    }

    #[test]
    fn multi_question_flow_submits_all_answers() {
        let (tx, mut rx) = test_sender();
        let request = RequestUserInputEvent {
            call_id: "call-1".to_string(),
            turn_id: "turn-3".to_string(),
            questions: vec![
                question_short("q1", "First question?"),
                question_short("q2", "Second question?"),
            ],
        };
        let mut view = QaDialogView::new(request, tx);
        view.textarea.insert_str("First answer");
        view.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(rx.try_recv().is_err());

        view.textarea.insert_str("Second answer");
        view.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        let event = rx.try_recv().expect("expected AppEvent");
        let AppEvent::CodexOp(Op::UserInputAnswer { response, .. }) = event else {
            panic!("expected UserInputAnswer");
        };
        assert_eq!(
            response.answers.get("q1").expect("answer missing").answers,
            vec!["user_note: First answer".to_string()]
        );
        assert_eq!(
            response.answers.get("q2").expect("answer missing").answers,
            vec!["user_note: Second answer".to_string()]
        );
    }

    #[test]
    fn qa_dialog_snapshot() {
        let (tx, _rx) = test_sender();
        let view = QaDialogView::new(request_event("turn-1", question("q1")), tx);
        let area = Rect::new(0, 0, 60, 9);
        insta::assert_snapshot!("qa_dialog_snapshot", render_snapshot(&view, area));
    }
}
