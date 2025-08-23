use std::io::Result;

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

pub(crate) struct TranscriptApp {
    // Base (unmodified) transcript lines
    base_transcript_lines: Vec<Line<'static>>,
    // Renderable transcript lines (may include highlight styling)
    pub(crate) transcript_lines: Vec<Line<'static>>,
    pub(crate) scroll_offset: usize,
    pub(crate) is_done: bool,
    title: String,
    // Optional highlight range [start, end) in terms of base_transcript_lines indices
    highlight_range: Option<(usize, usize)>,
    // When true, show backtracking key hints (Esc to step, etc.)
    backtrack_mode: bool,
}

impl TranscriptApp {
    pub(crate) fn new(transcript_lines: Vec<Line<'static>>) -> Self {
        Self {
            base_transcript_lines: transcript_lines.clone(),
            transcript_lines,
            scroll_offset: usize::MAX,
            is_done: false,
            title: "T R A N S C R I P T".to_string(),
            highlight_range: None,
            backtrack_mode: false,
        }
    }

    pub(crate) fn with_title(transcript_lines: Vec<Line<'static>>, title: String) -> Self {
        Self {
            base_transcript_lines: transcript_lines.clone(),
            transcript_lines,
            scroll_offset: 0,
            is_done: false,
            title,
            highlight_range: None,
            backtrack_mode: false,
        }
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

    pub(crate) fn insert_lines(&mut self, lines: Vec<Line<'static>>) {
        self.base_transcript_lines.extend(lines.clone());
        // If a highlight is active, rebuild with highlight; else append directly.
        if self.highlight_range.is_some() {
            self.rebuild_highlighted_lines();
        } else {
            self.transcript_lines.extend(lines);
        }
    }

    /// Highlight the specified range [start, end) of base transcript lines.
    pub(crate) fn set_highlight_range(&mut self, range: Option<(usize, usize)>) {
        self.highlight_range = range;
        self.rebuild_highlighted_lines();
    }

    /// Enable or disable backtrack mode to show appropriate key hints.
    pub(crate) fn set_backtrack_mode(&mut self, enabled: bool) {
        self.backtrack_mode = enabled;
    }

    fn rebuild_highlighted_lines(&mut self) {
        // Start from base and optionally apply highlight styles to the target range.
        let mut out = self.base_transcript_lines.clone();
        if let Some((start, end)) = self.highlight_range {
            use ratatui::style::Modifier;
            let len = out.len();
            let start = start.min(len);
            let end = end.min(len);
            for (idx, line) in out.iter_mut().enumerate().take(end).skip(start) {
                // Apply REVERSED to all spans; add BOLD on the first line (header)
                let mut spans = Vec::with_capacity(line.spans.len());
                for (i, s) in line.spans.iter().enumerate() {
                    let mut style = s.style;
                    style.add_modifier |= Modifier::REVERSED;
                    if idx == start && i == 0 {
                        style.add_modifier |= Modifier::BOLD;
                    }
                    spans.push(ratatui::text::Span {
                        style,
                        content: s.content.clone(),
                    });
                }
                line.spans = spans;
            }
        }
        self.transcript_lines = out;
    }

    fn handle_key_event(&mut self, tui: &mut tui::Tui, key_event: KeyEvent) {
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
                return;
            }
        }
        tui.frame_requester().schedule_frame();
    }

    fn scroll_area(&self, area: Rect) -> Rect {
        let mut area = area;
        // Reserve 1 line for the header and 4 lines for the bottom status section. This matches the chat composer.
        area.y = area.y.saturating_add(1);
        area.height = area.height.saturating_sub(5);
        area
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        Span::from("/ ".repeat(area.width as usize / 2))
            .dim()
            .render_ref(area, buf);
        let header = format!("/ {}", self.title);
        Span::from(header).dim().render_ref(area, buf);

        // Main content area (excludes header and bottom status section)
        let content_area = self.scroll_area(area);
        let wrapped = insert_history::word_wrap_lines(&self.transcript_lines, content_area.width);

        // Clamp scroll offset to valid range
        self.scroll_offset = self
            .scroll_offset
            .min(wrapped.len().saturating_sub(content_area.height as usize));
        let start = self.scroll_offset;
        let end = (start + content_area.height as usize).min(wrapped.len());
        let page = &wrapped[start..end];
        Paragraph::new(page.to_vec()).render_ref(content_area, buf);

        // Fill remaining visible lines (if any) with a leading '~' in the first column.
        let visible = (end - start) as u16;
        if content_area.height > visible {
            let extra = content_area.height - visible;
            for i in 0..extra {
                let y = content_area.y.saturating_add(visible + i);
                Span::from("~")
                    .dim()
                    .render_ref(Rect::new(content_area.x, y, 1, 1), buf);
            }
        }

        // Bottom status section (4 lines): separator with % scrolled, then key hints (styled like chat composer)
        let sep_y = content_area.bottom();
        let sep_rect = Rect::new(area.x, sep_y, area.width, 1);
        let hints_rect = Rect::new(area.x, sep_y + 1, area.width, 2);

        // Separator line (dim)
        Span::from("─".repeat(sep_rect.width as usize))
            .dim()
            .render_ref(sep_rect, buf);

        // Scroll percentage (0-100%) aligned near the right edge
        let max_scroll = wrapped.len().saturating_sub(content_area.height as usize);
        let percent: u8 = if max_scroll == 0 {
            100
        } else {
            (((self.scroll_offset.min(max_scroll)) as f32 / max_scroll as f32) * 100.0).round()
                as u8
        };
        let pct_text = format!(" {percent}% ");
        let pct_w = pct_text.chars().count() as u16;
        let pct_x = sep_rect.x + sep_rect.width - pct_w - 1;
        Span::from(pct_text)
            .dim()
            .render_ref(Rect::new(pct_x, sep_rect.y, pct_w, 1), buf);

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
        if self.backtrack_mode {
            // Include Esc hint when previewing backtrack in the transcript overlay
            hints2.extend([
                "   ".into(),
                "Esc".set_style(key_hint_style),
                " backtrack".into(),
            ]);
        }
        Paragraph::new(vec![Line::from(hints1).dim(), Line::from(hints2).dim()])
            .render_ref(hints_rect, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backtrack_hint_is_visible_when_enabled() {
        let mut app = TranscriptApp::new(vec![Line::from("hello")]);
        app.set_backtrack_mode(true);

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
            s.contains("backtrack"),
            "expected backtrack hint in overlay footer, got: {s:?}"
        );
    }
}
