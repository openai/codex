//! Detailed transcript overlay with entry-based scrolling and the existing pager chrome.
//!
//! The overlay owns committed cells and input bindings. Its viewport displays current content
//! and keeps readers on the same entry through pagination and stream consolidation.

use super::*;
use crate::transcript_view::TranscriptView;

pub(crate) struct TranscriptOverlay {
    view: TranscriptView,
    cells: Vec<Arc<dyn HistoryCell>>,
    keymap: PagerKeymap,
    highlight_cell: Option<usize>,
    reveal_highlight: bool,
    content_area: Rect,
    is_done: bool,
}

impl TranscriptOverlay {
    pub(crate) fn new(cells: Vec<Arc<dyn HistoryCell>>, keymap: PagerKeymap) -> Self {
        Self {
            view: TranscriptView::default(),
            cells,
            keymap,
            highlight_cell: None,
            reveal_highlight: false,
            content_area: Rect::default(),
            is_done: false,
        }
    }

    pub(crate) fn render(&mut self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let top_height = area.height.saturating_sub(/*rhs*/ 3);
        let top = Rect::new(area.x, area.y, area.width, top_height);
        self.content_area = Rect::new(
            area.x,
            area.y.saturating_add(/*rhs*/ 1),
            area.width,
            top_height.saturating_sub(/*rhs*/ 2),
        )
        .intersection(area);
        let mut rendered =
            self.view
                .render(self.content_area, buf, &self.cells, self.highlight_cell);
        if std::mem::take(&mut self.reveal_highlight)
            && let Some(index) = self.highlight_cell
        {
            self.view.ensure_entry_visible(&self.cells, index);
            rendered = self
                .view
                .render(self.content_area, buf, &self.cells, self.highlight_cell);
        }
        if self.view.history == TranscriptHistoryState::LoadingBeginning
            && self.view.is_following()
            && !self.content_area.is_empty()
        {
            // Home may arrive before the first draw; retain the first real viewport.
            self.view.scroll(&self.cells, /*rows*/ 0);
        }
        if area.width > 0 {
            for y in self.content_area.y + rendered..self.content_area.bottom() {
                "~".render(Rect::new(area.x, y, /*width*/ 1, /*height*/ 1), buf);
            }
        }
        let header = Rect::new(area.x, area.y, area.width, area.height.min(/*other*/ 1));
        Span::from("/ ".repeat(usize::from(area.width) / 2))
            .dim()
            .render(header, buf);
        "/ T R A N S C R I P T".dim().render(header, buf);
        self.render_history_state(top, buf);
        let separator = Rect::new(
            area.x,
            self.content_area.bottom(),
            area.width,
            /*height*/ 1,
        )
        .intersection(area);
        "─"
            .repeat(usize::from(separator.width))
            .dim()
            .render(separator, buf);
        // An exact intermediate percentage would require laying out every offscreen cell.
        // Only the loaded tail has a known percentage without defeating bounded rendering.
        if self.view.is_following()
            && !self.view.history.has_unloaded_history()
            && separator.width >= 7
        {
            " 100% ".dim().render(
                Rect::new(
                    separator.right() - 7,
                    separator.y,
                    /*width*/ 6,
                    separator.height,
                ),
                buf,
            );
        }
        let footer =
            Rect::new(area.x, area.y + top_height, area.width, /*height*/ 3).intersection(area);
        self.render_hints(footer, buf);
    }

    pub(crate) fn handle_event(&mut self, tui: &mut tui::Tui, event: TuiEvent) -> Result<()> {
        match event {
            TuiEvent::Key(key) => {
                if self.keymap.close.is_pressed(key) || self.keymap.close_transcript.is_pressed(key)
                {
                    self.is_done = true;
                } else if self.navigate(key) {
                    tui.frame_requester()
                        .schedule_frame_in(crate::tui::TARGET_FRAME_INTERVAL);
                }
                Ok(())
            }
            TuiEvent::Draw | TuiEvent::Resume | TuiEvent::Resize(_) | TuiEvent::FocusGained => {
                tui.draw(u16::MAX, |frame| self.render(frame.area(), frame.buffer))?;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub(crate) fn set_history_state(
        &mut self,
        state: TranscriptHistoryState,
    ) -> TranscriptHistoryState {
        let previous = std::mem::replace(&mut self.view.history, state);
        if state == TranscriptHistoryState::LoadingBeginning
            && previous != TranscriptHistoryState::LoadingBeginning
            && !self.content_area.is_empty()
        {
            // App marks loading before forwarding Home, so record the current position here.
            self.view.scroll(&self.cells, /*rows*/ 0);
        }
        if previous == TranscriptHistoryState::LoadingBeginning
            && state == TranscriptHistoryState::Complete
        {
            self.view.jump_to_entry(&self.cells, /*index*/ 0);
        }
        previous
    }

    pub(crate) fn should_load_older(&mut self, key: KeyEvent) -> bool {
        self.should_load_from_start(key)
            || (self.view.near_start(&self.cells)
                && [
                    &self.keymap.scroll_up,
                    &self.keymap.page_up,
                    &self.keymap.half_page_up,
                ]
                .iter()
                .any(|bindings| bindings.is_pressed(key)))
    }

    pub(crate) fn should_load_from_start(&self, key: KeyEvent) -> bool {
        self.keymap.jump_top.is_pressed(key)
    }

    pub(crate) fn insert_cell(&mut self, cell: Arc<dyn HistoryCell>) {
        let index = self.cells.len();
        self.view
            .replace_range(&self.cells, index..index + 1, &cell);
        self.cells.push(cell);
    }

    /// Prepends history and returns the insertion index for the canonical transcript.
    pub(crate) fn prepend(&mut self, cells: Vec<Arc<dyn HistoryCell>>) -> usize {
        if cells.is_empty() {
            return 0;
        }
        let index = self
            .cells
            .iter()
            .rposition(|cell| {
                cell.as_any().is::<SessionInfoCell>()
                    || cell
                        .as_any()
                        .is::<crate::history_cell::SessionHeaderHistoryCell>()
            })
            .map_or(/*default*/ 0, |index| index + 1);
        let added = cells.len();
        self.cells.splice(index..index, cells);
        if let Some(highlight) = self
            .highlight_cell
            .as_mut()
            .filter(|highlight| **highlight >= index)
        {
            *highlight += added;
        }
        self.view.history_changed(&self.cells);
        index
    }

    pub(crate) fn replace_cells(&mut self, cells: Vec<Arc<dyn HistoryCell>>) {
        self.cells = cells;
        self.view.history_changed(&self.cells);
        self.highlight_cell = self
            .highlight_cell
            .filter(|index| *index < self.cells.len());
    }

    pub(crate) fn consolidate_cells(
        &mut self,
        range: std::ops::Range<usize>,
        consolidated: Arc<dyn HistoryCell>,
    ) {
        let end = range.end.min(self.cells.len());
        let start = range.start.min(end);
        if start == end {
            return;
        }
        self.view
            .replace_range(&self.cells, start..end, &consolidated);
        self.highlight_cell = self.highlight_cell.map(|index| {
            if index < start {
                index
            } else if index < end {
                start
            } else {
                index - (end - start - 1)
            }
        });
        self.cells.splice(start..end, [consolidated]);
    }

    pub(crate) fn sync_live_tail(
        &mut self,
        width: u16,
        key: Option<ActiveCellTranscriptKey>,
        compute_lines: impl FnOnce(u16) -> Option<Vec<HyperlinkLine>>,
    ) {
        self.view.sync_live_tail(width, key, compute_lines);
    }

    /// Explicit prompt navigation supersedes Home; passive highlight restoration does not.
    pub(crate) fn cancel_pending_jump(&mut self) {
        self.view.cancel_beginning();
    }

    pub(crate) fn set_highlight_cell(&mut self, cell: Option<usize>) {
        self.highlight_cell = cell.filter(|index| *index < self.cells.len());
        self.reveal_highlight = self.highlight_cell.is_some();
    }

    pub(crate) fn scroll(&mut self, rows: isize) {
        if std::mem::take(&mut self.reveal_highlight)
            && let Some(index) = self.highlight_cell
        {
            self.view.ensure_entry_visible(&self.cells, index);
        }
        self.view.scroll(&self.cells, rows);
    }

    fn navigate(&mut self, key: KeyEvent) -> bool {
        if self.keymap.jump_top.is_pressed(key) {
            if self.content_area.is_empty() && self.view.history.has_unloaded_history() {
                self.set_history_state(TranscriptHistoryState::LoadingBeginning);
            } else {
                self.view.jump_to_beginning(&self.cells);
            }
            return true;
        }
        if self.keymap.jump_bottom.is_pressed(key) {
            self.view.jump_to_latest();
            return true;
        }
        let page = self.content_area.height.max(/*other*/ 1) as isize;
        let half = (page + 1) / 2;
        let delta = [
            (&self.keymap.scroll_up, -1),
            (&self.keymap.scroll_down, 1),
            (&self.keymap.page_up, -page),
            (&self.keymap.page_down, page),
            (&self.keymap.half_page_up, -half),
            (&self.keymap.half_page_down, half),
        ]
        .into_iter()
        .find_map(|(bindings, delta)| bindings.is_pressed(key).then_some(delta));
        let Some(delta) = delta else {
            return false;
        };
        self.view.cancel_beginning();
        if !self.content_area.is_empty() {
            self.scroll(delta);
        }
        true
    }

    pub(crate) fn live_tail_visible(&self) -> bool {
        self.view.live_tail_visible()
    }

    pub(crate) fn is_done(&self) -> bool {
        self.is_done
    }

    fn render_hints(&self, area: Rect, buf: &mut Buffer) {
        let line1 = Rect::new(area.x, area.y, area.width, /*height*/ 1).intersection(area);
        let line2 = Rect::new(
            area.x,
            area.y.saturating_add(/*rhs*/ 1),
            area.width,
            /*height*/ 1,
        )
        .intersection(area);
        render_navigation_hints(line1, buf, &self.keymap);

        let mut pairs: Vec<(Vec<ShortcutHint>, &str)> = vec![(
            first_or_empty(&self.keymap, "close", &self.keymap.close),
            "close",
        )];
        if self.highlight_cell.is_some() {
            pairs.push((
                vec![
                    key_hint::plain(KeyCode::Esc).into(),
                    key_hint::plain(KeyCode::Left).into(),
                ],
                "to edit prev",
            ));
            pairs.push((vec![key_hint::plain(KeyCode::Right).into()], "to edit next"));
            pairs.push((
                vec![key_hint::plain(KeyCode::Enter).into()],
                "to edit message",
            ));
        } else {
            pairs.push((vec![key_hint::plain(KeyCode::Esc).into()], "to edit prev"));
        }
        render_key_hints(line2, buf, &pairs);
    }

    fn render_history_state(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }
        let label = match self.view.history {
            TranscriptHistoryState::Idle => return,
            TranscriptHistoryState::LoadingOlder | TranscriptHistoryState::LoadingBeginning => {
                " loading older history... "
            }
            TranscriptHistoryState::Partial => " partial history | PgUp for earlier ",
            TranscriptHistoryState::Failed => " history unavailable | PgUp to retry ",
            TranscriptHistoryState::Complete => " start of history ",
        };
        let width = (label.chars().count() as u16).min(area.width);
        let status_area = Rect::new(
            area.right().saturating_sub(width),
            area.y,
            width,
            /*height*/ 1,
        );
        Span::from(label).dim().render(status_area, buf);
    }
}

#[cfg(test)]
#[path = "transcript_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "anchor_tests.rs"]
mod anchor_tests;
