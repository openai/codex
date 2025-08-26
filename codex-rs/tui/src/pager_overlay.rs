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

pub(crate) trait PagerOverlay {
    fn handle_event(&mut self, tui: &mut tui::Tui, event: TuiEvent) -> Result<()>;
    fn insert_lines(&mut self, _lines: Vec<Line<'static>>) {
        // Default: ignore history inserts (e.g., Static overlays)
    }
    fn is_done(&self) -> bool;
    fn set_highlight_range(&mut self, _range: Option<(usize, usize)>) {
        // Default: no-op (only Transcript uses this)
    }
    fn set_scroll_offset(&mut self, offset: usize);
}
struct PagerView {
    lines: Vec<Line<'static>>,
    scroll_offset: usize,
    is_done: bool,
    title: String,
    highlight_range: Option<(usize, usize)>,
    show_enter_edit_hint: bool,
}

impl PagerView {
    fn new(
        lines: Vec<Line<'static>>,
        title: String,
        scroll_offset: usize,
        show_enter_edit_hint: bool,
    ) -> Self {
        Self {
            lines,
            scroll_offset,
            is_done: false,
            title,
            highlight_range: None,
            show_enter_edit_hint,
        }
    }

    fn handle_event(&mut self, tui: &mut tui::Tui, event: TuiEvent) -> Result<()> {
        match event {
            TuiEvent::Key(key_event) => self.handle_key_event(tui, key_event),
            TuiEvent::Draw => Ok({
                tui.draw(u16::MAX, |frame| {
                    self.render(frame.area(), frame.buffer);
                })?;
            }),
            _ => Ok(()),
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.render_header(area, buf);
        let content_area = self.scroll_area(area);
        let mut lines = self.lines.clone();
        self.apply_highlight_to_lines(&mut lines);
        let wrapped = insert_history::word_wrap_lines(&lines, content_area.width);
        self.render_content_page(content_area, buf, &wrapped);
        self.render_bottom_section(area, content_area, buf, &wrapped);
    }

    fn render_header(&self, area: Rect, buf: &mut Buffer) {
        Span::from("/ ".repeat(area.width as usize / 2))
            .dim()
            .render_ref(area, buf);
        let header = format!("/ {}", self.title);
        Span::from(header).dim().render_ref(area, buf);
    }

    fn apply_highlight_to_lines(&self, lines: &mut [Line<'static>]) {
        if let Some((start, end)) = self.highlight_range {
            use ratatui::style::Modifier;
            let len = lines.len();
            let start = start.min(len);
            let end = end.min(len);
            for (idx, line) in lines.iter_mut().enumerate().take(end).skip(start) {
                let mut spans = Vec::with_capacity(line.spans.len());
                for (i, s) in line.spans.iter().enumerate() {
                    let mut style = s.style;
                    style.add_modifier |= Modifier::REVERSED;
                    if idx == start && i == 0 {
                        style.add_modifier |= Modifier::BOLD;
                    }
                    spans.push(Span {
                        style,
                        content: s.content.clone(),
                    });
                }
                line.spans = spans;
            }
        }
    }

    fn render_content_page(&mut self, area: Rect, buf: &mut Buffer, wrapped: &[Line<'static>]) {
        self.scroll_offset = self
            .scroll_offset
            .min(wrapped.len().saturating_sub(area.height as usize));
        let start = self.scroll_offset;
        let end = (start + area.height as usize).min(wrapped.len());
        let page = &wrapped[start..end];
        Paragraph::new(page.to_vec()).render_ref(area, buf);

        let visible = end.saturating_sub(start);
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
        if self.show_enter_edit_hint {
            hints2.extend([
                "   ".into(),
                "Esc".set_style(key_hint_style),
                " edit prev".into(),
            ]);
            self.maybe_append_enter_edit_hint(&mut hints2, key_hint_style);
        }
        Paragraph::new(vec![Line::from(hints1).dim(), Line::from(hints2).dim()])
            .render_ref(hints_rect, buf);
    }

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

    fn handle_key_event(&mut self, tui: &mut tui::Tui, key_event: KeyEvent) -> Result<()> {
        match key_event {
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
                return Ok(());
            }
        }
        tui.frame_requester().schedule_frame();
        Ok(())
    }

    fn scroll_area(&self, area: Rect) -> Rect {
        let mut area = area;
        area.y = area.y.saturating_add(1);
        area.height = area.height.saturating_sub(5);
        area
    }
}

pub(crate) struct TranscriptOverlay {
    view: PagerView,
}

impl TranscriptOverlay {
    pub(crate) fn new(transcript_lines: Vec<Line<'static>>) -> Self {
        Self {
            view: PagerView::new(
                transcript_lines,
                "T R A N S C R I P T".to_string(),
                usize::MAX,
                true,
            ),
        }
    }

    // Test helper parity with previous module
    #[cfg(test)]
    pub(crate) fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.view.render(area, buf);
    }
}

impl PagerOverlay for TranscriptOverlay {
    fn handle_event(&mut self, tui: &mut tui::Tui, event: TuiEvent) -> Result<()> {
        self.view.handle_event(tui, event)
    }
    fn insert_lines(&mut self, lines: Vec<Line<'static>>) {
        self.view.lines.extend(lines);
    }
    fn is_done(&self) -> bool {
        self.view.is_done
    }
    fn set_highlight_range(&mut self, range: Option<(usize, usize)>) {
        self.view.highlight_range = range;
    }
    fn set_scroll_offset(&mut self, offset: usize) {
        self.view.scroll_offset = offset;
    }
}

pub(crate) struct StaticOverlay {
    view: PagerView,
}

impl StaticOverlay {
    pub(crate) fn with_title(lines: Vec<Line<'static>>, title: String) -> Self {
        Self {
            view: PagerView::new(lines, title, 0, false),
        }
    }

    fn handle_event(&mut self, tui: &mut tui::Tui, event: TuiEvent) -> Result<()> {
        self.view.handle_event(tui, event)
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.view.render(area, buf);
    }

    fn handle_key_event(&mut self, tui: &mut tui::Tui, key_event: KeyEvent) -> Result<()> {
        self.view.handle_key_event(tui, key_event)
    }

    fn render_content_page(&mut self, area: Rect, buf: &mut Buffer, wrapped: &[Line<'static>]) {
        self.view.render_content_page(area, buf, wrapped);
    }

    fn render_bottom_section(
        &self,
        full_area: Rect,
        content_area: Rect,
        buf: &mut Buffer,
        wrapped: &[Line<'static>],
    ) {
        self.view
            .render_bottom_section(full_area, content_area, buf, wrapped);
    }

    fn scroll_area(&self, area: Rect) -> Rect {
        self.view.scroll_area(area)
    }
}

impl PagerOverlay for StaticOverlay {
    fn handle_event(&mut self, tui: &mut tui::Tui, event: TuiEvent) -> Result<()> {
        self.view.handle_event(tui, event)
    }
    // insert_lines: use default no-op
    fn is_done(&self) -> bool {
        self.view.is_done
    }
    // set_highlight_range: default no-op
    fn set_scroll_offset(&mut self, offset: usize) {
        self.view.scroll_offset = offset;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_prev_hint_is_visible() {
        let mut overlay = TranscriptOverlay::new(vec![Line::from("hello")]);

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
}
