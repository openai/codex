//! Render Find query input and navigation hints without owning search transitions.

use super::*;
use crate::bottom_pane::TextAreaState;
use ratatui::text::Span;
use ratatui::widgets::StatefulWidgetRef;
use unicode_width::UnicodeWidthStr;

impl TranscriptView {
    /// Project the editor and its caret from the same one-row viewport into the existing footer.
    pub(crate) fn search_footer(&self, width: u16) -> Option<(Line<'static>, u16)> {
        if !self.is_search_editing() || width == 0 {
            return None;
        }
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width, /*height*/ 1);
        let mut buffer = Buffer::empty(area);
        let prefix_width = width.saturating_sub(/*rhs*/ 1).min(/*other*/ 6);
        Line::from("Find: ").dim().render(
            Rect::new(/*x*/ 0, /*y*/ 0, prefix_width, /*height*/ 1),
            &mut buffer,
        );
        let query_area = Rect::new(
            prefix_width,
            /*y*/ 0,
            width - prefix_width,
            /*height*/ 1,
        );
        let mut state = TextAreaState::default();
        StatefulWidgetRef::render_ref(&&self.search.editor, query_area, &mut buffer, &mut state);
        let (cursor_column, _) = self
            .search
            .editor
            .cursor_pos_with_state(query_area, state)?;
        let mut spans = Vec::new();
        let mut column = 0;
        while column < width {
            let cell = &buffer[(column, 0)];
            spans.push(Span::styled(cell.symbol().to_string(), cell.style()));
            column += cell.symbol().width().max(/*other*/ 1) as u16;
        }
        Some((Line::from(spans), cursor_column))
    }
}

impl Search {
    pub(in crate::transcript_view) fn status_line(
        &self,
        width: u16,
        history: TranscriptHistoryState,
    ) -> Line<'static> {
        let previous = crate::key_hint::ctrl(crossterm::event::KeyCode::Char('p')).display_label();
        let next = crate::key_hint::ctrl(crossterm::event::KeyCode::Char('n')).display_label();
        let retry_hint = format!("{previous} retry");
        let unavailable_hint = format!("History unavailable · {retry_hint}");
        let next_hint = format!("enter/{previous} older · {next} newer");
        let exhausted_hint = format!("No more matches · {next_hint}");
        let (status, compact) = match self.progress {
            Progress::Idle | Progress::Found if self.is_reading() => {
                ("ctrl+p older · ctrl+n newer", "ctrl+p older")
            }
            Progress::Idle => ("Type to find", "Type to find"),
            Progress::Restart | Progress::Scanning(_) => ("Searching…", "Searching…"),
            Progress::AwaitingHistory if history == TranscriptHistoryState::Failed => {
                (unavailable_hint.as_str(), retry_hint.as_str())
            }
            Progress::AwaitingHistory => ("Searching earlier history…", "Loading…"),
            Progress::Found => (next_hint.as_str(), "enter older"),
            Progress::Exhausted if self.is_reading() && self.current.is_some() => (
                "No more matches · ctrl+p older · ctrl+n newer",
                "ctrl+p older",
            ),
            Progress::Exhausted if self.current.is_some() => {
                (exhausted_hint.as_str(), "enter older")
            }
            Progress::Exhausted => ("No matches", "No matches"),
        };
        let limit = if self.query_truncated {
            " · query limited to 4 KiB"
        } else {
            ""
        };
        if self.is_reading() {
            return crate::footer_hint::first_fitting_line(
                [
                    format!("{status} · esc latest"),
                    format!("{compact} · esc latest"),
                    "esc latest".to_owned(),
                ]
                .map(|hint| crate::transcript_view::footer::navigation_line(&hint)),
                width,
            );
        }
        crate::footer_hint::first_fitting_line(
            [
                format!("{status} · full transcript · esc close{limit}"),
                format!("{status} · esc close{limit}"),
                format!("{compact} · esc close"),
                format!("{compact} · esc"),
                "esc close".to_owned(),
            ]
            .map(|hint| crate::transcript_view::footer::navigation_line(&hint)),
            width,
        )
    }
}
