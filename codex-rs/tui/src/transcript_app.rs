use std::io::Result;
use std::time::Duration;

use crate::insert_history;
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
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;

#[derive(Debug, Clone)]
struct WrapCache {
    width: u16,
    wrapped: Vec<Line<'static>>,
    src_idx: Vec<usize>,
    base_len: usize,
}

pub(crate) struct TranscriptApp {
    pub(crate) transcript_lines: Vec<Line<'static>>,
    pub(crate) scroll_offset: usize,
    pub(crate) is_done: bool,
    title: String,
    highlight_range: Option<(usize, usize)>,
    wrap_cache: Option<WrapCache>,
}

impl TranscriptApp {
    pub(crate) fn new(transcript_lines: Vec<Line<'static>>) -> Self {
        Self {
            transcript_lines,
            scroll_offset: usize::MAX,
            is_done: false,
            title: "T R A N S C R I P T".to_string(),
            highlight_range: None,
            wrap_cache: None,
        }
    }

    pub(crate) fn with_title(transcript_lines: Vec<Line<'static>>, title: String) -> Self {
        Self {
            transcript_lines,
            scroll_offset: 0,
            is_done: false,
            title,
            highlight_range: None,
            wrap_cache: None,
        }
    }
    pub(crate) fn insert_lines(&mut self, lines: Vec<Line<'static>>) {
        self.wrap_cache = None;
        self.transcript_lines.extend(lines);
    }

    /// Highlight the specified range [start, end) of transcript lines.
    pub(crate) fn set_highlight_range(&mut self, range: Option<(usize, usize)>) {
        self.highlight_range = range;
    }

    pub(crate) fn handle_event(&mut self, tui: &mut tui::Tui, event: TuiEvent) -> Result<()> {
        match event {
            TuiEvent::Key(key_event) => self.handle_key_event(tui, key_event),
            TuiEvent::Draw => {
                tui.draw(u16::MAX, |frame| {
                    self.render(frame.area(), frame.buffer);
                })?;
            }
            _ => {}
        }
        Ok(())
    }

    // set_backtrack_mode removed: overlay always shows backtrack guidance now.

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.render_header(area, buf);

        // Main content area (excludes header and bottom status section)
        let content_area = self.scroll_area(area);
        self.ensure_wrapped(content_area.width);
        // Clamp scroll based on cached wrapped length (avoid holding borrow while mutating self)
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

        let (wrapped, src_idx) = self.cached();
        let page = self.page_with_optional_highlight(wrapped, src_idx, start, end);
        self.render_content_page(content_area, buf, &page);
        self.render_bottom_section(area, content_area, buf, wrapped);
    }

    // Private helpers
    fn render_header(&self, area: Rect, buf: &mut Buffer) {
        Span::from("/ ".repeat(area.width as usize / 2))
            .dim()
            .render_ref(area, buf);
        let header = format!("/ {}", self.title);
        Span::from(header).dim().render_ref(area, buf);
    }

    fn render_content_page(&self, area: Rect, buf: &mut Buffer, page: &[Line<'static>]) {
        Paragraph::new(page.to_vec()).render_ref(area, buf);

        // Fill remaining visible lines (if any) with a leading '~' in the first column.
        let visible = page.len() as u16;
        if area.height > visible {
            let extra = area.height - visible;
            for i in 0..extra {
                let y = area.y.saturating_add(visible + i);
                Span::from("~")
                    .dim()
                    .render_ref(Rect::new(area.x, y, 1, 1), buf);
            }
        }
    }

    /// Render the bottom status section (separator, percent scrolled, key hints).
    fn render_bottom_section(
        &self,
        full_area: Rect,
        content_area: Rect,
        buf: &mut Buffer,
        wrapped: &[Line<'static>],
    ) {
        let sep_y = content_area.bottom();
        let sep_rect = Rect::new(full_area.x, sep_y, full_area.width, 1);
        let hints_rect = Rect::new(full_area.x, sep_y + 1, full_area.width, 2);

        self.render_separator(buf, sep_rect);
        let percent = self.compute_scroll_percent(wrapped.len(), content_area.height);
        self.render_scroll_percentage(buf, sep_rect, percent);
        self.render_hints(buf, hints_rect);
    }

    /// Draw a dim horizontal separator line across the provided rect.
    fn render_separator(&self, buf: &mut Buffer, sep_rect: Rect) {
        Span::from("─".repeat(sep_rect.width as usize))
            .dim()
            .render_ref(sep_rect, buf);
    }

    /// Compute percent scrolled (0–100) based on wrapped length and content height.
    fn compute_scroll_percent(&self, wrapped_len: usize, content_height: u16) -> u8 {
        let max_scroll = wrapped_len.saturating_sub(content_height as usize);
        if max_scroll == 0 {
            100
        } else {
            (((self.scroll_offset.min(max_scroll)) as f32 / max_scroll as f32) * 100.0).round()
                as u8
        }
    }

    /// Right-align and render the dim percent scrolled label on the separator line.
    fn render_scroll_percentage(&self, buf: &mut Buffer, sep_rect: Rect, percent: u8) {
        let pct_text = format!(" {percent}% ");
        let pct_w = pct_text.chars().count() as u16;
        let pct_x = sep_rect.x + sep_rect.width - pct_w - 1;
        Span::from(pct_text)
            .dim()
            .render_ref(Rect::new(pct_x, sep_rect.y, pct_w, 1), buf);
    }

    /// Render the dimmed key hints (scroll/page/jump and backtrack cue).
    fn render_hints(&self, buf: &mut Buffer, hints_rect: Rect) {
        let key_hint_style = Style::default().fg(Color::Cyan);
        let hints1 = vec![
            " ".into(),
            "↑".set_style(key_hint_style),
            "/".into(),
            "↓".set_style(key_hint_style),
            " scroll   ".into(),
            "PgUp".set_style(key_hint_style),
            "/".into(),
            "PgDn".set_style(key_hint_style),
            " page   ".into(),
            "Home".set_style(key_hint_style),
            "/".into(),
            "End".set_style(key_hint_style),
            " jump".into(),
        ];
        let mut hints2 = vec![" ".into(), "q".set_style(key_hint_style), " quit".into()];
        hints2.extend([
            "   ".into(),
            "Esc".set_style(key_hint_style),
            " edit prev".into(),
        ]);
        self.maybe_append_enter_edit_hint(&mut hints2, key_hint_style);
        Paragraph::new(vec![Line::from(hints1).dim(), Line::from(hints2).dim()])
            .render_ref(hints_rect, buf);
    }

    /// Conditionally append the "⏎ edit message" hint when a valid highlight is active.
    fn maybe_append_enter_edit_hint(&self, hints: &mut Vec<Span<'static>>, key_hint_style: Style) {
        if let Some((start, end)) = self.highlight_range
            && end > start
        {
            hints.extend([
                "   ".into(),
                "⏎".set_style(key_hint_style),
                " edit message".into(),
            ]);
        }
    }

    fn handle_key_event(&mut self, tui: &mut tui::Tui, key_event: KeyEvent) {
        let mut defer_draw_ms: Option<u64> = None;
        match key_event {
            // Ctrl+Z is handled at the App level when transcript overlay is active
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
            }
            KeyEvent {
                code: KeyCode::Up,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                defer_draw_ms = Some(16);
            }
            KeyEvent {
                code: KeyCode::Down,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
                defer_draw_ms = Some(16);
            }
            KeyEvent {
                code: KeyCode::PageUp,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                let area = self.scroll_area(tui.terminal.viewport_area);
                self.scroll_offset = self.scroll_offset.saturating_sub(area.height as usize);
                defer_draw_ms = Some(16);
            }
            KeyEvent {
                code: KeyCode::PageDown | KeyCode::Char(' '),
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                let area = self.scroll_area(tui.terminal.viewport_area);
                self.scroll_offset = self.scroll_offset.saturating_add(area.height as usize);
                defer_draw_ms = Some(16);
            }
            KeyEvent {
                code: KeyCode::Home,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                self.scroll_offset = 0;
                defer_draw_ms = Some(16);
            }
            KeyEvent {
                code: KeyCode::End,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                self.scroll_offset = usize::MAX;
                defer_draw_ms = Some(16);
            }
            _ => {
                return;
            }
        }
        if let Some(ms) = defer_draw_ms {
            tui.frame_requester()
                .schedule_frame_in(Duration::from_millis(ms));
        } else {
            tui.frame_requester().schedule_frame();
        }
    }

    fn scroll_area(&self, area: Rect) -> Rect {
        let mut area = area;
        // Reserve 1 line for the header and 4 lines for the bottom status section. This matches the chat composer.
        area.y = area.y.saturating_add(1);
        area.height = area.height.saturating_sub(5);
        area
    }
}

impl TranscriptApp {
    fn ensure_wrapped(&mut self, width: u16) {
        let width = width.max(1);
        let needs = match self.wrap_cache {
            Some(ref c) => c.width != width || c.base_len != self.transcript_lines.len(),
            None => true,
        };
        if !needs {
            return;
        }
        let mut wrapped: Vec<Line<'static>> = Vec::new();
        let mut src_idx: Vec<usize> = Vec::new();
        for (i, line) in self.transcript_lines.iter().enumerate() {
            let ws = insert_history::word_wrap_lines(std::slice::from_ref(line), width);
            src_idx.extend(std::iter::repeat_n(i, ws.len()));
            wrapped.extend(ws);
        }
        self.wrap_cache = Some(WrapCache {
            width,
            wrapped,
            src_idx,
            base_len: self.transcript_lines.len(),
        });
    }

    fn cached(&self) -> (&[Line<'static>], &[usize]) {
        if let Some(cache) = self.wrap_cache.as_ref() {
            (&cache.wrapped, &cache.src_idx)
        } else {
            (&[], &[])
        }
    }

    fn page_with_optional_highlight<'a>(
        &self,
        wrapped: &'a [Line<'static>],
        src_idx: &[usize],
        start: usize,
        end: usize,
    ) -> std::borrow::Cow<'a, [Line<'static>]> {
        use ratatui::style::Modifier;
        let (hi_start, hi_end) = match self.highlight_range {
            Some(r) => r,
            None => return std::borrow::Cow::Borrowed(&wrapped[start..end]),
        };
        let mut out: Vec<Line<'static>> = Vec::with_capacity(end - start);
        let mut bold_done = false;
        for (row, src_line) in wrapped
            .iter()
            .enumerate()
            .skip(start)
            .take(end.saturating_sub(start))
        {
            let mut line = src_line.clone();
            if let Some(src) = src_idx.get(row).copied()
                && src >= hi_start
                && src < hi_end
            {
                for (i, s) in line.spans.iter_mut().enumerate() {
                    s.style.add_modifier |= Modifier::REVERSED;
                    if !bold_done && i == 0 {
                        s.style.add_modifier |= Modifier::BOLD;
                        bold_done = true;
                    }
                }
            }
            out.push(line);
        }
        std::borrow::Cow::Owned(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_prev_hint_is_visible() {
        let mut app = TranscriptApp::new(vec![Line::from("hello")]);

        // Render into a small buffer and assert the backtrack hint is present
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        app.render(area, &mut buf);

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
    fn wrap_cache_reuses_for_same_width_and_rebuilds_on_change() {
        let long = "This is a long line that should wrap multiple times to ensure non-empty wrapped output.";
        let mut app = TranscriptApp::new(vec![Line::from(long), Line::from(long)]);

        // Build cache at width 24
        app.ensure_wrapped(24);
        let (wrapped1, _) = app.cached();
        assert!(!wrapped1.is_empty(), "expected wrapped output to be non-empty");
        let ptr1 = wrapped1.as_ptr();

        // Re-run with same width: cache should be reused (pointer stability heuristic)
        app.ensure_wrapped(24);
        let (wrapped2, _) = app.cached();
        let ptr2 = wrapped2.as_ptr();
        assert_eq!(ptr1, ptr2, "cache should not rebuild for unchanged width");

        // Change width: cache should rebuild
        app.ensure_wrapped(36);
        let (wrapped3, _) = app.cached();
        let ptr3 = wrapped3.as_ptr();
        assert_ne!(ptr2, ptr3, "cache should rebuild on width change");
        // And width field updated
        assert_eq!(app.wrap_cache.as_ref().expect("cache").width, 36);
    }

    #[test]
    fn wrap_cache_invalidates_on_insert_lines() {
        let long = "Another long line for wrapping behavior verification.";
        let mut app = TranscriptApp::new(vec![Line::from(long)]);
        app.ensure_wrapped(28);
        let (wrapped1, _) = app.cached();
        let ptr1 = wrapped1.as_ptr();
        let len1 = wrapped1.len();

        // Insert new lines should drop the cache
        app.insert_lines(vec![Line::from(long), Line::from(long)]);
        assert!(app.wrap_cache.is_none(), "wrap cache should be None after insert_lines");

        // Rebuild and confirm content grew
        app.ensure_wrapped(28);
        let (wrapped2, _) = app.cached();
        let ptr2 = wrapped2.as_ptr();
        assert!(wrapped2.len() >= len1, "wrapped length should grow or stay same");
        assert_ne!(ptr1, ptr2, "cache should rebuild after insert_lines");
    }
}
