use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use std::any::Any;

use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPane;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::bottom_pane::scroll_state::ScrollState;
use crate::history_cell::AgentMessageCell;
use crate::key_hint;
use crate::render::line_utils::prefix_lines;
use crate::render::line_utils::push_owned_lines;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_line;
use codex_core::protocol::ReviewFinding;
use codex_core::review_format;

pub(crate) struct ReviewSelectionView {
    title: String,
    comments: Vec<ReviewFinding>,
    selected: Vec<bool>,
    state: ScrollState,
    complete: bool,
    app_event_tx: AppEventSender,
}

impl ReviewSelectionView {
    const TITLE_ROWS: u16 = 1;
    const TOP_SPACER_ROWS: u16 = 1;
    const BOTTOM_RESERVED_ROWS: u16 = 3;

    pub fn new(title: String, comments: Vec<ReviewFinding>, app_event_tx: AppEventSender) -> Self {
        let len = comments.len();
        Self {
            title,
            comments,
            selected: vec![true; len],
            state: ScrollState::new(),
            complete: false,
            app_event_tx,
        }
        .init_selection()
    }

    fn desired_rows(&self, width: u16) -> u16 {
        if width == 0 {
            return 0;
        }
        let wrap_width = Self::wrap_width_for(width);
        let total_lines: usize = self.compute_heights(wrap_width).into_iter().sum();
        let total = Self::base_rows() as usize + total_lines;
        total.min(u16::MAX as usize) as u16
    }

    fn toggle_current(&mut self) {
        if let Some(idx) = self.state.selected_idx
            && let Some(v) = self.selected.get_mut(idx)
        {
            *v = !*v;
        }
    }

    fn move_up(&mut self) {
        let len = self.comments.len();
        self.state.move_up_wrap(len);
        self.state.ensure_visible(len, len);
    }

    fn move_down(&mut self) {
        let len = self.comments.len();
        self.state.move_down_wrap(len);
        self.state.ensure_visible(len, len);
    }

    fn accept(&mut self) {
        use crate::app_event::AppEvent;

        let selected: Vec<&ReviewFinding> = self
            .comments
            .iter()
            .enumerate()
            .filter_map(|(i, comment)| {
                self.selected
                    .get(i)
                    .copied()
                    .unwrap_or(false)
                    .then_some(comment)
            })
            .collect();

        if selected.is_empty() {
            self.complete = true;
            return;
        }

        let message_text =
            review_format::format_review_findings_block(&self.comments, Some(&self.selected));
        let message_lines: Vec<Line<'static>> = message_text
            .lines()
            .map(|s| Line::from(s.to_string()))
            .collect();
        let agent_cell = AgentMessageCell::new(message_lines, true);
        self.app_event_tx
            .send(AppEvent::InsertHistoryCell(Box::new(agent_cell)));

        let mut user_message = String::new();
        user_message.push_str(if selected.len() == 1 {
            "Please fix this review comment:\n"
        } else {
            "Please fix these review comments:\n"
        });
        for comment in &selected {
            let title = &comment.title;
            let location = Self::format_location(comment);
            user_message.push_str(&format!("\n- {title} — {location}\n"));
            for body_line in comment.body.lines() {
                if body_line.is_empty() {
                    user_message.push_str("  \n");
                } else {
                    user_message.push_str(&format!("  {body_line}\n"));
                }
            }
        }
        self.app_event_tx.send(AppEvent::SubmitUserText(
            user_message.trim_end().to_string(),
        ));
        self.complete = true;
    }

    fn init_selection(mut self) -> Self {
        let len = self.comments.len();
        if len > 0 {
            self.state.selected_idx = Some(0);
            // Default to top when opening; render pass will ensure visibility.
            self.state.ensure_visible(len, len);
        }
        self
    }

    fn wrap_width_for(width: u16) -> usize {
        width.saturating_sub(2).max(1) as usize
    }

    fn base_rows() -> u16 {
        Self::TITLE_ROWS + Self::TOP_SPACER_ROWS + Self::BOTTOM_RESERVED_ROWS
    }

    fn dim_prefix_span() -> Span<'static> {
        "▌ ".dim()
    }

    // Note: we render dim prefix lines inline where needed for clarity.
    fn format_location(item: &ReviewFinding) -> String {
        let path = item.code_location.absolute_file_path.display();
        let start = item.code_location.line_range.start;
        let end = item.code_location.line_range.end;
        format!("{path}:{start}-{end}")
    }

    fn title_with_priority(item: &ReviewFinding) -> String {
        let t = item.title.as_str();
        if t.trim_start().starts_with('[') {
            t.to_string()
        } else {
            let priority = item.priority;
            format!("[P{priority}] {t}")
        }
    }

    fn header_text(item: &ReviewFinding) -> String {
        let title_with_priority = Self::title_with_priority(item);
        let loc = Self::format_location(item);
        format!("{title_with_priority} — {loc}")
    }

    /// Build the item's prefix (selection marker + checkbox) and return the
    /// prefix as a styled Line plus its display width in characters for
    /// subsequent indent alignment.
    fn header_prefix(is_selected: bool, checked: bool) -> (Line<'static>, usize) {
        let selected_marker = if is_selected { '>' } else { ' ' };
        let width_hint = format!("{selected_marker} [x] ").chars().count();
        let line = if checked {
            Line::from(vec![
                Span::from(format!("{selected_marker} [")),
                "x".cyan(),
                Span::from("] "),
            ])
        } else {
            Line::from(format!("{selected_marker} [ ] "))
        };
        (line, width_hint)
    }

    fn measure_item_lines(&self, idx: usize, wrap_width: usize) -> usize {
        if idx >= self.comments.len() {
            return 0;
        }
        let item = &self.comments[idx];

        // Compute header (title + location) wrapped height with indent for marker + checkbox.
        let is_selected = self.state.selected_idx == Some(idx);
        let is_checked = self.selected.get(idx).copied().unwrap_or(false);
        let (prefix_line, prefix_width) = Self::header_prefix(is_selected, is_checked);
        let header_line = Line::from(Self::header_text(item));
        let header_subseq = " ".repeat(prefix_width);
        let header_opts = RtOptions::new(wrap_width)
            .initial_indent(prefix_line)
            .subsequent_indent(Line::from(header_subseq));
        let header_len = word_wrap_line(&header_line, header_opts).len();

        // Compute body wrapped height (no preview cap; show full body).
        let body_line = Line::from(item.body.as_str());
        let body_opts = RtOptions::new(wrap_width)
            .initial_indent(Line::from("    "))
            .subsequent_indent(Line::from("    "));
        let body_len = word_wrap_line(&body_line, body_opts).len();

        let spacer = if idx + 1 < self.comments.len() { 1 } else { 0 };
        header_len + body_len + spacer
    }

    fn compute_heights(&self, wrap_width: usize) -> Vec<usize> {
        (0..self.comments.len())
            .map(|i| self.measure_item_lines(i, wrap_width))
            .collect()
    }

    fn choose_start_for_visibility(
        &self,
        current_start: usize,
        selected_idx: usize,
        heights: &[usize],
        window_rows: usize,
    ) -> usize {
        if heights.is_empty() || window_rows == 0 {
            return 0;
        }
        let n = heights.len();
        let sel = selected_idx.min(n - 1);
        let start = current_start.min(n - 1);

        // Check visibility from current start.
        let mut sum = 0usize;
        let mut end = start;
        while end < n {
            let h = heights[end];
            if sum.saturating_add(h) > window_rows {
                break;
            }
            sum += h;
            end += 1;
        }
        if sel >= start && sel < end {
            return start;
        }

        // Slide window so sel is visible; include as many items above as fit.
        let mut acc = heights[sel];
        let mut s = sel;
        while s > 0 {
            let h = heights[s - 1];
            if acc.saturating_add(h) > window_rows {
                break;
            }
            acc += h;
            s -= 1;
        }
        s
    }
}

impl BottomPaneView for ReviewSelectionView {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn handle_key_event(&mut self, _pane: &mut BottomPane, key_event: KeyEvent) {
        match key_event {
            KeyEvent {
                code: KeyCode::Up, ..
            } => self.move_up(),
            KeyEvent {
                code: KeyCode::Down,
                ..
            } => self.move_down(),
            KeyEvent {
                code: KeyCode::Char(' '),
                ..
            } => self.toggle_current(),
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.complete = true;
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.accept(),
            _ => {}
        }
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn on_ctrl_c(&mut self, _pane: &mut BottomPane) -> CancellationEvent {
        self.complete = true;
        CancellationEvent::Handled
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.desired_rows(width)
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let render_prefixed_blank_line = |x: u16, y: u16, width: u16, buf: &mut Buffer| {
            if width == 0 {
                return;
            }
            let rest_width = width.saturating_sub(2);
            let mut spans = vec![Self::dim_prefix_span()];
            if rest_width > 0 {
                spans.push(" ".repeat(rest_width as usize).into());
            }
            Paragraph::new(Line::from(spans)).render(
                Rect {
                    x,
                    y,
                    width,
                    height: 1,
                },
                buf,
            );
        };

        // Title
        let title_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        };
        Paragraph::new(Line::from(vec![
            Self::dim_prefix_span(),
            self.title.as_str().bold(),
        ]))
        .render(title_area, buf);
        if area.height > Self::TITLE_ROWS {
            render_prefixed_blank_line(
                area.x,
                area.y.saturating_add(Self::TITLE_ROWS),
                area.width,
                buf,
            );
        }

        // Rows area
        // Rows area: reserve 2 rows at top (title + top hint)
        // and 3 rows at bottom for two spacers + key hint.
        let rows_area = Rect {
            x: area.x,
            y: area
                .y
                .saturating_add(Self::TITLE_ROWS + Self::TOP_SPACER_ROWS),
            width: area.width,
            height: area.height.saturating_sub(Self::base_rows()),
        };
        let wrap_width = Self::wrap_width_for(rows_area.width);
        let heights = self.compute_heights(wrap_width);
        let window_rows = rows_area.height as usize;
        let selected_idx = self
            .state
            .selected_idx
            .unwrap_or(0)
            .min(self.comments.len().saturating_sub(1));
        let start = self.choose_start_for_visibility(
            self.state.scroll_top,
            selected_idx,
            &heights,
            window_rows,
        );
        let mut y = rows_area.y;
        let mut idx = start;
        while idx < self.comments.len() && y < rows_area.y.saturating_add(rows_area.height) {
            let item = &self.comments[idx];

            // Header: marker + checkbox + wrapped title/location
            let is_selected = self.state.selected_idx == Some(idx);
            let is_checked = self.selected.get(idx).copied().unwrap_or(false);
            let (header_prefix_line, header_subseq_len) =
                Self::header_prefix(is_selected, is_checked);
            let header_subseq = " ".repeat(header_subseq_len);
            let header_opts = RtOptions::new(wrap_width)
                .initial_indent(header_prefix_line)
                .subsequent_indent(Line::from(header_subseq));
            let name_line = Line::from(Self::header_text(item));
            let header_wrapped = word_wrap_line(&name_line, header_opts);
            let mut header_owned: Vec<Line<'static>> = Vec::new();
            push_owned_lines(&header_wrapped, &mut header_owned);
            let header_prefixed = prefix_lines(
                header_owned,
                Self::dim_prefix_span(),
                Self::dim_prefix_span(),
            );
            for l in header_prefixed {
                if y >= rows_area.y.saturating_add(rows_area.height) {
                    break;
                }
                Paragraph::new(l).render(
                    Rect {
                        x: rows_area.x,
                        y,
                        width: rows_area.width,
                        height: 1,
                    },
                    buf,
                );
                y = y.saturating_add(1);
            }

            // Body: fully wrapped (dim), no preview cap
            if y >= rows_area.y.saturating_add(rows_area.height) {
                break;
            }
            let body_line = Line::from(item.body.as_str().dim());
            let body_opts = RtOptions::new(wrap_width)
                .initial_indent(Line::from("    "))
                .subsequent_indent(Line::from("    "));
            let body_wrapped = word_wrap_line(&body_line, body_opts);
            let mut body_owned: Vec<Line<'static>> = Vec::new();
            push_owned_lines(&body_wrapped, &mut body_owned);
            let body_prefixed =
                prefix_lines(body_owned, Self::dim_prefix_span(), Self::dim_prefix_span());
            for l in body_prefixed {
                if y >= rows_area.y.saturating_add(rows_area.height) {
                    break;
                }
                Paragraph::new(l).render(
                    Rect {
                        x: rows_area.x,
                        y,
                        width: rows_area.width,
                        height: 1,
                    },
                    buf,
                );
                y = y.saturating_add(1);
            }

            // Spacer line between items (not after the last item).
            if idx + 1 < self.comments.len() && y < rows_area.y.saturating_add(rows_area.height) {
                render_prefixed_blank_line(rows_area.x, y, rows_area.width, buf);
                y = y.saturating_add(1);
            }

            idx += 1;
        }

        let pane_bottom = area.y.saturating_add(area.height);
        if y < pane_bottom {
            render_prefixed_blank_line(rows_area.x, y, rows_area.width, buf);
            y = y.saturating_add(1);
        }

        // Hint with blue keys, matching chat input hint styling.
        let hint_spans: Vec<Span<'static>> = vec![
            Self::dim_prefix_span(),
            key_hint::plain("Enter"),
            "=fix selected issues".dim(),
            "  ".into(),
            key_hint::plain("Space"),
            "=toggle".dim(),
            "  ".into(),
            key_hint::plain("↑/↓"),
            "=scroll".dim(),
            "  ".into(),
            key_hint::plain("Esc"),
            "=cancel".dim(),
        ];
        if y < pane_bottom {
            Paragraph::new(Line::from(hint_spans)).render(
                Rect {
                    x: rows_area.x,
                    y,
                    width: rows_area.width,
                    height: 1,
                },
                buf,
            );
            y = y.saturating_add(1);
        }

        while y < pane_bottom {
            render_prefixed_blank_line(rows_area.x, y, rows_area.width, buf);
            y = y.saturating_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use codex_core::protocol::ReviewCodeLocation;
    use codex_core::protocol::ReviewFinding;
    use codex_core::protocol::ReviewLineRange;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    use tokio::sync::mpsc::unbounded_channel;

    /// Accepting the view submits a user message summarizing checked findings
    /// and renders history output that reflects each checkbox state.
    #[test]
    fn accept_emits_history_and_user_message_for_selected_findings() {
        let (sender, mut rx) = test_sender();
        let findings = vec![
            finding("Leak fix", "Close the file handle", 4),
            finding("Rename", "Use snake case", 10),
        ];
        let mut view =
            ReviewSelectionView::new("Select review comments".to_string(), findings, sender);

        // Deselect the second finding so only the first is submitted.
        view.move_down();
        view.toggle_current();

        view.accept();

        let mut history_lines = Vec::new();
        let mut user_messages = Vec::new();
        while let Ok(event) = rx.try_recv() {
            match event {
                AppEvent::InsertHistoryCell(cell) => {
                    history_lines.push(lines_to_strings(cell.display_lines(120)));
                }
                AppEvent::SubmitUserText(text) => {
                    user_messages.push(text);
                }
                _ => {}
            }
        }

        assert!(
            view.complete,
            "view should close after accepting selections"
        );
        assert_eq!(
            user_messages,
            vec![String::from(
                "Please fix this review comment:\n\n- Leak fix — src/lib.rs:4-5\n  Close the file handle",
            ),]
        );

        assert_eq!(
            history_lines,
            vec![vec![
                String::from("> Full review comments:"),
                String::from("  "),
                String::from("  - [x] Leak fix — src/lib.rs:4-5"),
                String::from("    Close the file handle"),
                String::from("  "),
                String::from("  - [ ] Rename — src/lib.rs:10-11"),
                String::from("    Use snake case"),
            ]]
        );
    }

    /// Accepting with every finding unchecked completes and leaves the event
    /// stream untouched.
    #[test]
    fn accept_with_no_selected_findings_completes_without_emitting_events() {
        let (sender, mut rx) = test_sender();
        let findings = vec![finding("First", "body", 1), finding("Second", "body", 3)];
        let mut view =
            ReviewSelectionView::new("Select review comments".to_string(), findings, sender);

        // Deselect both findings so none remain checked.
        view.toggle_current();
        view.move_down();
        view.toggle_current();

        view.accept();

        assert!(view.complete);
        assert!(
            rx.try_recv().is_err(),
            "no events should be emitted when nothing is selected"
        );
    }

    fn test_sender() -> (
        AppEventSender,
        tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    ) {
        let (tx, rx) = unbounded_channel();
        (AppEventSender::new(tx), rx)
    }

    fn finding(title: &str, body: &str, start: u32) -> ReviewFinding {
        ReviewFinding {
            title: title.to_string(),
            body: body.to_string(),
            confidence_score: 0.75,
            priority: 1,
            code_location: ReviewCodeLocation {
                absolute_file_path: PathBuf::from("src/lib.rs"),
                line_range: ReviewLineRange {
                    start,
                    end: start + 1,
                },
            },
        }
    }

    fn lines_to_strings(lines: Vec<Line<'static>>) -> Vec<String> {
        lines
            .into_iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.to_string())
                    .collect::<String>()
            })
            .collect()
    }
}
