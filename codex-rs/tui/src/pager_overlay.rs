use std::io::Result;
use std::sync::Arc;
use std::time::Duration;

use crate::history_cell::HistoryCell;
use crate::render::line_utils::push_owned_lines;
use crate::tui;
use crate::tui::TuiEvent;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::style::Styled;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;

pub(crate) enum Overlay {
    Transcript(TranscriptOverlay),
    Static(StaticOverlay),
}

impl Overlay {
    pub(crate) fn new_transcript(cells: Vec<Arc<dyn HistoryCell>>) -> Self {
        Self::Transcript(TranscriptOverlay::new(cells))
    }

    pub(crate) fn new_static_with_title(lines: Vec<Line<'static>>, title: String) -> Self {
        Self::Static(StaticOverlay::with_title(lines, title))
    }

    pub(crate) fn handle_event(&mut self, tui: &mut tui::Tui, event: TuiEvent) -> Result<()> {
        match self {
            Overlay::Transcript(o) => o.handle_event(tui, event),
            Overlay::Static(o) => o.handle_event(tui, event),
        }
    }

    pub(crate) fn is_done(&self) -> bool {
        match self {
            Overlay::Transcript(o) => o.is_done(),
            Overlay::Static(o) => o.is_done(),
        }
    }
}

// Common pager navigation hints rendered on the first line
const PAGER_KEY_HINTS: &[(&str, &str)] = &[
    ("↑/↓", "scroll"),
    ("PgUp/PgDn", "page"),
    ("Home/End", "jump"),
];

// Render a single line of key hints from (key, description) pairs.
fn render_key_hints(area: Rect, buf: &mut Buffer, pairs: &[(&str, &str)]) {
    let key_hint_style = Style::default().fg(Color::Cyan);
    let mut spans: Vec<Span<'static>> = vec![" ".into()];
    let mut first = true;
    for (key, desc) in pairs {
        if !first {
            spans.push("   ".into());
        }
        spans.push(Span::from(key.to_string()).set_style(key_hint_style));
        spans.push(" ".into());
        spans.push(Span::from(desc.to_string()));
        first = false;
    }
    Paragraph::new(vec![Line::from(spans).dim()]).render_ref(area, buf);
}

/// Generic widget for rendering a pager view.
struct PagerView {
    texts: Vec<Text<'static>>,
    scroll_offset: usize,
    title: String,
    wrap_cache: Option<WrapCache>,
    last_content_height: Option<usize>,
    /// If set, on next render ensure this chunk is visible.
    pending_scroll_chunk: Option<usize>,
}

impl PagerView {
    fn new(texts: Vec<Text<'static>>, title: String, scroll_offset: usize) -> Self {
        Self {
            texts,
            scroll_offset,
            title,
            wrap_cache: None,
            last_content_height: None,
            pending_scroll_chunk: None,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        self.render_header(area, buf);
        let content_area = self.scroll_area(area);
        self.update_last_content_height(content_area.height);
        self.ensure_wrapped(content_area.width);
        // If there is a pending request to scroll a specific chunk into view,
        // satisfy it now that wrapping is up to date for this width.
        if let (Some(idx), Some(cache)) =
            (self.pending_scroll_chunk.take(), self.wrap_cache.as_ref())
            && let Some(range) = cache.chunk_ranges.get(idx).cloned()
        {
            self.ensure_range_visible(range, content_area.height as usize, cache.wrapped.len());
        }
        // Compute page bounds without holding an immutable borrow on cache while mutating self
        let wrapped_len = self
            .wrap_cache
            .as_ref()
            .map(|c| c.wrapped.len())
            .unwrap_or(0);
        self.scroll_offset = self
            .scroll_offset
            .min(wrapped_len.saturating_sub(content_area.height as usize));
        let start = self.scroll_offset;
        let end = (start + content_area.height as usize).min(wrapped_len);

        let wrapped = self.cached();
        let page = &wrapped[start..end];
        self.render_content_page_prepared(content_area, buf, page);
        self.render_bottom_bar(area, content_area, buf, wrapped);
    }

    fn render_header(&self, area: Rect, buf: &mut Buffer) {
        Span::from("/ ".repeat(area.width as usize / 2))
            .dim()
            .render_ref(area, buf);
        let header = format!("/ {}", self.title);
        header.dim().render_ref(area, buf);
    }

    // Removed unused render_content_page (replaced by render_content_page_prepared)

    fn render_content_page_prepared(&self, area: Rect, buf: &mut Buffer, page: &[Line<'static>]) {
        Clear.render(area, buf);
        Paragraph::new(page.to_vec()).render_ref(area, buf);

        let visible = page.len();
        if visible < area.height as usize {
            for i in 0..(area.height as usize - visible) {
                let add = ((visible + i).min(u16::MAX as usize)) as u16;
                let y = area.y.saturating_add(add);
                Span::from("~")
                    .dim()
                    .render_ref(Rect::new(area.x, y, 1, 1), buf);
            }
        }
    }

    fn render_bottom_bar(
        &self,
        full_area: Rect,
        content_area: Rect,
        buf: &mut Buffer,
        wrapped: &[Line<'static>],
    ) {
        let sep_y = content_area.bottom();
        let sep_rect = Rect::new(full_area.x, sep_y, full_area.width, 1);

        Span::from("─".repeat(sep_rect.width as usize))
            .dim()
            .render_ref(sep_rect, buf);
        let percent = if wrapped.is_empty() {
            100
        } else {
            let max_scroll = wrapped.len().saturating_sub(content_area.height as usize);
            if max_scroll == 0 {
                100
            } else {
                (((self.scroll_offset.min(max_scroll)) as f32 / max_scroll as f32) * 100.0).round()
                    as u8
            }
        };
        let pct_text = format!(" {percent}% ");
        let pct_w = pct_text.chars().count() as u16;
        let pct_x = sep_rect.x + sep_rect.width - pct_w - 1;
        Span::from(pct_text)
            .dim()
            .render_ref(Rect::new(pct_x, sep_rect.y, pct_w, 1), buf);
    }

    fn handle_key_event(&mut self, tui: &mut tui::Tui, key_event: KeyEvent) -> Result<()> {
        match key_event {
            KeyEvent {
                code: KeyCode::Up,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
            KeyEvent {
                code: KeyCode::Down,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
            }
            KeyEvent {
                code: KeyCode::PageUp,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                let area = self.scroll_area(tui.terminal.viewport_area);
                self.scroll_offset = self.scroll_offset.saturating_sub(area.height as usize);
            }
            KeyEvent {
                code: KeyCode::PageDown | KeyCode::Char(' '),
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                let area = self.scroll_area(tui.terminal.viewport_area);
                self.scroll_offset = self.scroll_offset.saturating_add(area.height as usize);
            }
            KeyEvent {
                code: KeyCode::Home,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                self.scroll_offset = 0;
            }
            KeyEvent {
                code: KeyCode::End,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                self.scroll_offset = usize::MAX;
            }
            _ => {
                return Ok(());
            }
        }
        tui.frame_requester()
            .schedule_frame_in(Duration::from_millis(16));
        Ok(())
    }

    fn update_last_content_height(&mut self, height: u16) {
        self.last_content_height = Some(height as usize);
    }

    fn scroll_area(&self, area: Rect) -> Rect {
        let mut area = area;
        area.y = area.y.saturating_add(1);
        area.height = area.height.saturating_sub(2);
        area
    }
}

#[derive(Debug, Clone)]
struct WrapCache {
    width: u16,
    wrapped: Vec<Line<'static>>,
    /// For each input Text chunk, the inclusive-excluded range of wrapped lines produced.
    chunk_ranges: Vec<std::ops::Range<usize>>,
    base_len: usize,
}

impl PagerView {
    fn ensure_wrapped(&mut self, width: u16) {
        let width = width.max(1);
        let needs = match self.wrap_cache {
            Some(ref c) => c.width != width || c.base_len != self.texts.len(),
            None => true,
        };
        if !needs {
            return;
        }
        let mut wrapped: Vec<Line<'static>> = Vec::new();
        let mut chunk_ranges: Vec<std::ops::Range<usize>> = Vec::with_capacity(self.texts.len());
        for text in &self.texts {
            let start = wrapped.len();
            for line in &text.lines {
                let ws = crate::wrapping::word_wrap_line(line, width as usize);
                push_owned_lines(&ws, &mut wrapped);
            }
            let end = wrapped.len();
            chunk_ranges.push(start..end);
        }
        self.wrap_cache = Some(WrapCache {
            width,
            wrapped,
            chunk_ranges,
            base_len: self.texts.len(),
        });
    }

    fn cached(&self) -> &[Line<'static>] {
        if let Some(cache) = self.wrap_cache.as_ref() {
            &cache.wrapped
        } else {
            &[]
        }
    }

    fn is_scrolled_to_bottom(&self) -> bool {
        if self.scroll_offset == usize::MAX {
            return true;
        }
        let Some(cache) = &self.wrap_cache else {
            return false;
        };
        let Some(height) = self.last_content_height else {
            return false;
        };
        if cache.wrapped.is_empty() {
            return true;
        }
        let visible = height.min(cache.wrapped.len());
        let max_scroll = cache.wrapped.len().saturating_sub(visible);
        self.scroll_offset >= max_scroll
    }

    /// Request that the given text chunk index be scrolled into view on next render.
    fn scroll_chunk_into_view(&mut self, chunk_index: usize) {
        self.pending_scroll_chunk = Some(chunk_index);
    }

    fn ensure_range_visible(
        &mut self,
        range: std::ops::Range<usize>,
        viewport_height: usize,
        total_wrapped: usize,
    ) {
        if viewport_height == 0 || total_wrapped == 0 {
            return;
        }
        let first = range.start.min(total_wrapped.saturating_sub(1));
        let last = range
            .end
            .saturating_sub(1)
            .min(total_wrapped.saturating_sub(1));
        let current_top = self.scroll_offset.min(total_wrapped.saturating_sub(1));
        let current_bottom = current_top.saturating_add(viewport_height.saturating_sub(1));

        if first < current_top {
            self.scroll_offset = first;
        } else if last > current_bottom {
            // Scroll just enough so that 'last' is visible at the bottom
            self.scroll_offset = last.saturating_sub(viewport_height.saturating_sub(1));
        }
    }
}

pub(crate) struct TranscriptOverlay {
    view: PagerView,
    cells: Vec<Arc<dyn HistoryCell>>,
    highlight_cell: Option<usize>,
    is_done: bool,
}

impl TranscriptOverlay {
    pub(crate) fn new(transcript_cells: Vec<Arc<dyn HistoryCell>>) -> Self {
        Self {
            view: PagerView::new(
                Self::render_cells_to_texts(&transcript_cells, None),
                "T R A N S C R I P T".to_string(),
                usize::MAX,
            ),
            cells: transcript_cells,
            highlight_cell: None,
            is_done: false,
        }
    }

    fn render_cells_to_texts(
        cells: &[Arc<dyn HistoryCell>],
        highlight_cell: Option<usize>,
    ) -> Vec<Text<'static>> {
        let mut texts: Vec<Text<'static>> = Vec::new();
        let mut first = true;
        for (idx, cell) in cells.iter().enumerate() {
            let mut lines: Vec<Line<'static>> = Vec::new();
            if !cell.is_stream_continuation() && !first {
                lines.push(Line::from(""));
            }
            let cell_lines = if Some(idx) == highlight_cell {
                cell.transcript_lines()
                    .into_iter()
                    .map(Stylize::reversed)
                    .collect()
            } else {
                cell.transcript_lines()
            };
            lines.extend(cell_lines);
            texts.push(Text::from(lines));
            first = false;
        }
        texts
    }

    pub(crate) fn insert_cell(&mut self, cell: Arc<dyn HistoryCell>) {
        let follow_bottom = self.view.is_scrolled_to_bottom();
        // Append as a new Text chunk (with a separating blank if needed)
        let mut lines: Vec<Line<'static>> = Vec::new();
        if !cell.is_stream_continuation() && !self.cells.is_empty() {
            lines.push(Line::from(""));
        }
        lines.extend(cell.transcript_lines());
        self.view.texts.push(Text::from(lines));
        self.cells.push(cell);
        self.view.wrap_cache = None;
        if follow_bottom {
            self.view.scroll_offset = usize::MAX;
        }
    }

    pub(crate) fn set_highlight_cell(&mut self, cell: Option<usize>) {
        self.highlight_cell = cell;
        self.view.wrap_cache = None;
        self.view.texts = Self::render_cells_to_texts(&self.cells, self.highlight_cell);
        if let Some(idx) = self.highlight_cell {
            self.view.scroll_chunk_into_view(idx);
        }
    }

    fn render_hints(&self, area: Rect, buf: &mut Buffer) {
        let line1 = Rect::new(area.x, area.y, area.width, 1);
        let line2 = Rect::new(area.x, area.y.saturating_add(1), area.width, 1);
        render_key_hints(line1, buf, PAGER_KEY_HINTS);
        let mut pairs: Vec<(&str, &str)> = vec![("q", "quit"), ("Esc", "edit prev")];
        if self.highlight_cell.is_some() {
            pairs.push(("⏎", "edit message"));
        }
        render_key_hints(line2, buf, &pairs);
    }

    pub(crate) fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let top_h = area.height.saturating_sub(3);
        let top = Rect::new(area.x, area.y, area.width, top_h);
        let bottom = Rect::new(area.x, area.y + top_h, area.width, 3);
        self.view.render(top, buf);
        self.render_hints(bottom, buf);
    }
}

impl TranscriptOverlay {
    pub(crate) fn handle_event(&mut self, tui: &mut tui::Tui, event: TuiEvent) -> Result<()> {
        match event {
            TuiEvent::Key(key_event) => match key_event {
                KeyEvent {
                    code: KeyCode::Char('q'),
                    kind: KeyEventKind::Press,
                    ..
                }
                | KeyEvent {
                    code: KeyCode::Char('t'),
                    modifiers: crossterm::event::KeyModifiers::CONTROL,
                    kind: KeyEventKind::Press,
                    ..
                }
                | KeyEvent {
                    code: KeyCode::Char('c'),
                    modifiers: crossterm::event::KeyModifiers::CONTROL,
                    kind: KeyEventKind::Press,
                    ..
                } => {
                    self.is_done = true;
                    Ok(())
                }
                other => self.view.handle_key_event(tui, other),
            },
            TuiEvent::Draw => {
                tui.draw(u16::MAX, |frame| {
                    self.render(frame.area(), frame.buffer);
                })?;
                Ok(())
            }
            _ => Ok(()),
        }
    }
    pub(crate) fn is_done(&self) -> bool {
        self.is_done
    }
}

pub(crate) struct StaticOverlay {
    view: PagerView,
    is_done: bool,
}

impl StaticOverlay {
    pub(crate) fn with_title(lines: Vec<Line<'static>>, title: String) -> Self {
        Self {
            view: PagerView::new(vec![Text::from(lines)], title, 0),
            is_done: false,
        }
    }

    fn render_hints(&self, area: Rect, buf: &mut Buffer) {
        let line1 = Rect::new(area.x, area.y, area.width, 1);
        let line2 = Rect::new(area.x, area.y.saturating_add(1), area.width, 1);
        render_key_hints(line1, buf, PAGER_KEY_HINTS);
        let pairs = [("q", "quit")];
        render_key_hints(line2, buf, &pairs);
    }

    pub(crate) fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let top_h = area.height.saturating_sub(3);
        let top = Rect::new(area.x, area.y, area.width, top_h);
        let bottom = Rect::new(area.x, area.y + top_h, area.width, 3);
        self.view.render(top, buf);
        self.render_hints(bottom, buf);
    }
}

impl StaticOverlay {
    pub(crate) fn handle_event(&mut self, tui: &mut tui::Tui, event: TuiEvent) -> Result<()> {
        match event {
            TuiEvent::Key(key_event) => match key_event {
                KeyEvent {
                    code: KeyCode::Char('q'),
                    kind: KeyEventKind::Press,
                    ..
                }
                | KeyEvent {
                    code: KeyCode::Char('c'),
                    modifiers: crossterm::event::KeyModifiers::CONTROL,
                    kind: KeyEventKind::Press,
                    ..
                } => {
                    self.is_done = true;
                    Ok(())
                }
                other => self.view.handle_key_event(tui, other),
            },
            TuiEvent::Draw => {
                tui.draw(u16::MAX, |frame| {
                    self.render(frame.area(), frame.buffer);
                })?;
                Ok(())
            }
            _ => Ok(()),
        }
    }
    pub(crate) fn is_done(&self) -> bool {
        self.is_done
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    use crate::history_cell::CommandOutput;
    use crate::history_cell::HistoryCell;
    use crate::history_cell::PatchEventType;
    use crate::history_cell::new_patch_event;
    use codex_core::protocol::FileChange;
    use codex_protocol::parse_command::ParsedCommand;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[derive(Debug)]
    struct TestCell {
        lines: Vec<Line<'static>>,
    }

    impl crate::history_cell::HistoryCell for TestCell {
        fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
            self.lines.clone()
        }

        fn transcript_lines(&self) -> Vec<Line<'static>> {
            self.lines.clone()
        }
    }

    #[test]
    fn edit_prev_hint_is_visible() {
        let mut overlay = TranscriptOverlay::new(vec![Arc::new(TestCell {
            lines: vec![Line::from("hello")],
        })]);

        // Render into a small buffer and assert the backtrack hint is present
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        overlay.render(area, &mut buf);

        // Flatten buffer to a string and check for the hint text
        let mut s = String::new();
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                s.push(buf[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            s.push('\n');
        }
        assert!(
            s.contains("edit prev"),
            "expected 'edit prev' hint in overlay footer, got: {s:?}"
        );
    }

    #[test]
    fn transcript_overlay_snapshot_basic() {
        // Prepare a transcript overlay with a few lines
        let mut overlay = TranscriptOverlay::new(vec![
            Arc::new(TestCell {
                lines: vec![Line::from("alpha")],
            }),
            Arc::new(TestCell {
                lines: vec![Line::from("beta")],
            }),
            Arc::new(TestCell {
                lines: vec![Line::from("gamma")],
            }),
        ]);
        let mut term = Terminal::new(TestBackend::new(40, 10)).expect("term");
        term.draw(|f| overlay.render(f.area(), f.buffer_mut()))
            .expect("draw");
        assert_snapshot!(term.backend());
    }

    fn buffer_to_text(buf: &Buffer, area: Rect) -> String {
        let mut out = String::new();
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                let symbol = buf[(x, y)].symbol();
                if symbol.is_empty() {
                    out.push(' ');
                } else {
                    out.push(symbol.chars().next().unwrap_or(' '));
                }
            }
            // Trim trailing spaces for stability.
            while out.ends_with(' ') {
                out.pop();
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn transcript_overlay_apply_patch_scroll_vt100_clears_previous_page() {
        let cwd = PathBuf::from("/repo");
        let mut cells: Vec<Arc<dyn HistoryCell>> = Vec::new();

        let mut approval_changes = HashMap::new();
        approval_changes.insert(
            PathBuf::from("foo.txt"),
            FileChange::Add {
                content: "hello\nworld\n".to_string(),
            },
        );
        let approval_cell: Arc<dyn HistoryCell> = Arc::new(new_patch_event(
            PatchEventType::ApprovalRequest,
            approval_changes,
            &cwd,
        ));
        cells.push(approval_cell);

        let mut apply_changes = HashMap::new();
        apply_changes.insert(
            PathBuf::from("foo.txt"),
            FileChange::Add {
                content: "hello\nworld\n".to_string(),
            },
        );
        let apply_begin_cell: Arc<dyn HistoryCell> = Arc::new(new_patch_event(
            PatchEventType::ApplyBegin {
                auto_approved: false,
            },
            apply_changes,
            &cwd,
        ));
        cells.push(apply_begin_cell);

        let apply_end_cell: Arc<dyn HistoryCell> =
            Arc::new(crate::history_cell::new_user_approval_decision(vec![
                "✓ Patch applied".green().bold().into(),
                "src/foo.txt".dim().into(),
            ]));
        cells.push(apply_end_cell);

        let mut exec_cell = crate::history_cell::new_active_exec_command(
            "exec-1".into(),
            vec!["bash".into(), "-lc".into(), "ls".into()],
            vec![ParsedCommand::Unknown { cmd: "ls".into() }],
        );
        exec_cell.complete_call(
            "exec-1",
            CommandOutput {
                exit_code: 0,
                stdout: "src\nREADME.md\n".into(),
                stderr: String::new(),
                formatted_output: "src\nREADME.md\n".into(),
            },
            Duration::from_millis(420),
        );
        let exec_cell: Arc<dyn HistoryCell> = Arc::new(exec_cell);
        cells.push(exec_cell);

        let mut overlay = TranscriptOverlay::new(cells);
        let area = Rect::new(0, 0, 80, 12);
        let mut buf = Buffer::empty(area);

        overlay.render(area, &mut buf);
        overlay.view.scroll_offset = 0;
        overlay.view.wrap_cache = None;
        overlay.render(area, &mut buf);

        let snapshot = buffer_to_text(&buf, area);
        assert_snapshot!("transcript_overlay_apply_patch_scroll_vt100", snapshot);
    }

    #[test]
    fn transcript_overlay_keeps_scroll_pinned_at_bottom() {
        let mut overlay = TranscriptOverlay::new(
            (0..20)
                .map(|i| {
                    Arc::new(TestCell {
                        lines: vec![Line::from(format!("line{i}"))],
                    }) as Arc<dyn HistoryCell>
                })
                .collect(),
        );
        let mut term = Terminal::new(TestBackend::new(40, 12)).expect("term");
        term.draw(|f| overlay.render(f.area(), f.buffer_mut()))
            .expect("draw");

        assert!(
            overlay.view.is_scrolled_to_bottom(),
            "expected initial render to leave view at bottom"
        );

        overlay.insert_cell(Arc::new(TestCell {
            lines: vec!["tail".into()],
        }));

        assert_eq!(overlay.view.scroll_offset, usize::MAX);
    }

    #[test]
    fn transcript_overlay_preserves_manual_scroll_position() {
        let mut overlay = TranscriptOverlay::new(
            (0..20)
                .map(|i| {
                    Arc::new(TestCell {
                        lines: vec![Line::from(format!("line{i}"))],
                    }) as Arc<dyn HistoryCell>
                })
                .collect(),
        );
        let mut term = Terminal::new(TestBackend::new(40, 12)).expect("term");
        term.draw(|f| overlay.render(f.area(), f.buffer_mut()))
            .expect("draw");

        overlay.view.scroll_offset = 0;

        overlay.insert_cell(Arc::new(TestCell {
            lines: vec!["tail".into()],
        }));

        assert_eq!(overlay.view.scroll_offset, 0);
    }

    #[test]
    fn static_overlay_snapshot_basic() {
        // Prepare a static overlay with a few lines and a title
        let mut overlay = StaticOverlay::with_title(
            vec!["one".into(), "two".into(), "three".into()],
            "S T A T I C".to_string(),
        );
        let mut term = Terminal::new(TestBackend::new(40, 10)).expect("term");
        term.draw(|f| overlay.render(f.area(), f.buffer_mut()))
            .expect("draw");
        assert_snapshot!(term.backend());
    }

    #[test]
    fn pager_wrap_cache_reuses_for_same_width_and_rebuilds_on_change() {
        let long = "This is a long line that should wrap multiple times to ensure non-empty wrapped output.";
        let mut pv = PagerView::new(
            vec![Text::from(vec![long.into()]), Text::from(vec![long.into()])],
            "T".to_string(),
            0,
        );

        // Build cache at width 24
        pv.ensure_wrapped(24);
        let w1 = pv.cached();
        assert!(!w1.is_empty(), "expected wrapped output to be non-empty");
        let ptr1 = w1.as_ptr();

        // Re-run with same width: cache should be reused (pointer stability heuristic)
        pv.ensure_wrapped(24);
        let w2 = pv.cached();
        let ptr2 = w2.as_ptr();
        assert_eq!(ptr1, ptr2, "cache should not rebuild for unchanged width");

        // Change width: cache should rebuild and likely produce different length
        // Drop immutable borrow before mutating
        let prev_len = w2.len();
        pv.ensure_wrapped(36);
        let w3 = pv.cached();
        assert_ne!(
            prev_len,
            w3.len(),
            "wrapped length should change on width change"
        );
    }

    #[test]
    fn pager_wrap_cache_invalidates_on_append() {
        let long = "Another long line for wrapping behavior verification.";
        let mut pv = PagerView::new(vec![Text::from(vec![long.into()])], "T".to_string(), 0);
        pv.ensure_wrapped(28);
        let w1 = pv.cached();
        let len1 = w1.len();

        // Append new lines should cause ensure_wrapped to rebuild due to len change
        pv.texts.push(Text::from(vec![long.into()]));
        pv.texts.push(Text::from(vec![long.into()]));
        pv.ensure_wrapped(28);
        let w2 = pv.cached();
        assert!(
            w2.len() >= len1,
            "wrapped length should grow or stay same after append"
        );
    }
}
