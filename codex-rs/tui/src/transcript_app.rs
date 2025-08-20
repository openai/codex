use crate::insert_history;
use crate::tui;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::execute;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use tokio::select;

pub async fn run_transcript_app(tui: &mut tui::Tui, transcript_lines: Vec<Line<'static>>) {
    use tokio_stream::StreamExt;
    let _ = execute!(tui.terminal.backend_mut(), EnterAlternateScreen);
    #[allow(clippy::unwrap_used)]
    let size = tui.terminal.size().unwrap();
    let old_viewport_area = tui.terminal.viewport_area;
    tui.terminal
        .set_viewport_area(Rect::new(0, 0, size.width, size.height));
    let _ = tui.terminal.clear();

    let tui_events = tui.event_stream();
    tokio::pin!(tui_events);

    tui.frame_requester().schedule_frame();

    let mut app = TranscriptApp {
        transcript_lines,
        scroll_offset: usize::MAX,
        is_done: false,
    };

    while !app.is_done {
        select! {
            Some(event) = tui_events.next() => {
                match event {
                    crate::tui::TuiEvent::Key(key_event) => {
                        app.handle_key_event(tui, key_event);
                        tui.frame_requester().schedule_frame();
                    }
                    crate::tui::TuiEvent::Draw => {
                        let _ = tui.draw(u16::MAX, |frame| {
                            app.render(frame.area(), frame.buffer);
                        });
                    }
                    _ => {}
                }
            }
        }
    }

    let _ = execute!(tui.terminal.backend_mut(), LeaveAlternateScreen);

    tui.terminal.set_viewport_area(old_viewport_area);
}

pub(crate) struct TranscriptApp {
    pub(crate) transcript_lines: Vec<Line<'static>>,
    pub(crate) scroll_offset: usize,
    pub(crate) is_done: bool,
}

impl TranscriptApp {
    pub(crate) fn handle_key_event(&mut self, tui: &mut tui::Tui, key_event: KeyEvent) {
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
                code: KeyCode::PageDown,
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
            _ => {}
        }
    }

    fn scroll_area(&self, area: Rect) -> Rect {
        let mut area = area;
        area.y += 1;
        area.height -= 1;
        area
    }

    pub(crate) fn render(&mut self, area: Rect, buf: &mut Buffer) {
        Line::from("/ ".repeat(area.width as usize / 2)).render_ref(area, buf);
        Span::from("T R A N S C R I P T").render_ref(area, buf);

        let area = self.scroll_area(area);
        let wrapped = insert_history::word_wrap_lines(&self.transcript_lines, area.width);
        self.scroll_offset = self
            .scroll_offset
            .min(wrapped.len().saturating_sub(area.height as usize));
        let start = self.scroll_offset;
        let end = (start + area.height as usize).min(wrapped.len());
        let page = &wrapped[start..end];
        Paragraph::new(page.to_vec()).render_ref(area, buf);
    }
}
