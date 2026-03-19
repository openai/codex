use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;
use unicode_width::UnicodeWidthStr;

use crate::render::Insets;
use crate::render::RectExt;

use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::measure_rows_height;
use super::selection_popup_common::render_rows;

pub(crate) struct DraftCompletionPopup {
    request_id: u64,
    waiting: bool,
    suggestions: Vec<String>,
    error_message: Option<String>,
    state: ScrollState,
}

impl DraftCompletionPopup {
    pub(crate) fn new(request_id: u64) -> Self {
        let mut state = ScrollState::new();
        state.selected_idx = Some(0);
        Self {
            request_id,
            waiting: true,
            suggestions: Vec::new(),
            error_message: None,
            state,
        }
    }

    pub(crate) fn request_id(&self) -> u64 {
        self.request_id
    }

    pub(crate) fn set_suggestions(&mut self, suggestions: Vec<String>) {
        self.waiting = false;
        self.suggestions = suggestions;
        self.error_message = None;
        let len = self.suggestions.len();
        self.state.clamp_selection(len);
        self.state.ensure_visible(len, len.clamp(1, MAX_POPUP_ROWS));
    }

    pub(crate) fn set_error_message(&mut self, error_message: String) {
        self.waiting = false;
        self.suggestions.clear();
        self.error_message = Some(error_message);
        self.state.selected_idx = None;
        self.state.scroll_top = 0;
    }

    pub(crate) fn move_up(&mut self) {
        let len = self.suggestions.len();
        self.state.move_up_wrap(len);
        self.state.ensure_visible(len, len.clamp(1, MAX_POPUP_ROWS));
    }

    pub(crate) fn move_down(&mut self) {
        let len = self.suggestions.len();
        self.state.move_down_wrap(len);
        self.state.ensure_visible(len, len.clamp(1, MAX_POPUP_ROWS));
    }

    pub(crate) fn selected_suggestion(&self) -> Option<&str> {
        self.state
            .selected_idx
            .and_then(|idx| self.suggestions.get(idx))
            .map(String::as_str)
    }

    pub(crate) fn calculate_required_height(&self, width: u16) -> u16 {
        if let Some(message) = self.error_message.as_deref() {
            let wrapped_height =
                wrapped_message_lines(message, width.saturating_sub(2)).len() as u16;
            return wrapped_height.max(1);
        }
        let rows = self.rows();
        measure_rows_height(&rows, &self.state, MAX_POPUP_ROWS, width.saturating_sub(2))
    }

    fn rows(&self) -> Vec<GenericDisplayRow> {
        self.suggestions
            .iter()
            .map(|suggestion| GenericDisplayRow {
                name: suggestion.trim_start().to_string(),
                name_prefix_spans: Vec::new(),
                match_indices: None,
                display_shortcut: None,
                description: None,
                category_tag: None,
                wrap_indent: None,
                is_disabled: false,
                disabled_reason: None,
            })
            .collect()
    }
}

impl WidgetRef for &DraftCompletionPopup {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        if let Some(message) = self.error_message.as_deref() {
            render_wrapped_message(area, buf, message);
            return;
        }

        let rows = self.rows();
        let empty_message = if self.waiting {
            "loading..."
        } else {
            "no suggestions"
        };
        render_rows(
            area.inset(Insets::tlbr(0, 2, 0, 0)),
            buf,
            &rows,
            &self.state,
            MAX_POPUP_ROWS,
            empty_message,
        );
    }
}

fn render_wrapped_message(area: Rect, buf: &mut Buffer, message: &str) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let lines = wrapped_message_lines(message, area.width);
    for (idx, line) in lines.into_iter().take(area.height as usize).enumerate() {
        Line::from(vec![Span::from(line).dim().italic()]).render(
            Rect {
                x: area.x,
                y: area.y + idx as u16,
                width: area.width,
                height: 1,
            },
            buf,
        );
    }
}

fn wrapped_message_lines(message: &str, width: u16) -> Vec<String> {
    let width = width.max(1) as usize;
    textwrap::wrap(message, width)
        .into_iter()
        .map(|line| {
            let text = line.into_owned();
            if UnicodeWidthStr::width(text.as_str()) > width {
                let trimmed = text.trim().to_string();
                if trimmed.is_empty() { text } else { trimmed }
            } else {
                text
            }
        })
        .collect()
}
