use std::io::Result;
use std::io::Stdout;
use std::io::stdout;
use std::pin::Pin;
use std::time::Duration;
use std::time::Instant;

use crossterm::SynchronizedUpdate;
use crossterm::cursor;
use crossterm::cursor::MoveTo;
use crossterm::event::DisableBracketedPaste;
use crossterm::event::EnableBracketedPaste;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyboardEnhancementFlags;
use crossterm::event::PopKeyboardEnhancementFlags;
use crossterm::event::PushKeyboardEnhancementFlags;
use crossterm::terminal::ScrollUp;
use ratatui::backend::Backend;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::disable_raw_mode;
use ratatui::crossterm::terminal::enable_raw_mode;
use ratatui::layout::Offset;
use ratatui::text::Line;

use crate::custom_terminal;
use crate::custom_terminal::Terminal as CustomTerminal;
use tokio::select;
use tokio_stream::Stream;

/// A type alias for the terminal type used in this application
pub type Terminal = CustomTerminal<CrosstermBackend<Stdout>>;

pub fn set_modes() -> Result<()> {
    execute!(stdout(), EnableBracketedPaste)?;

    enable_raw_mode()?;
    // Enable keyboard enhancement flags so modifiers for keys like Enter are disambiguated.
    // chat_composer.rs is using a keyboard event listener to enter for any modified keys
    // to create a new line that require this.
    // Some terminals (notably legacy Windows consoles) do not support
    // keyboard enhancement flags. Attempt to enable them, but continue
    // gracefully if unsupported.
    let _ = execute!(
        stdout(),
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
        )
    );
    Ok(())
}

/// Restore the terminal to its original state.
/// Inverse of `set_modes`.
pub fn restore() -> Result<()> {
    // Pop may fail on platforms that didn't support the push; ignore errors.
    let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
    execute!(stdout(), DisableBracketedPaste)?;
    disable_raw_mode()?;
    let _ = execute!(stdout(), crossterm::cursor::Show);
    Ok(())
}

/// Initialize the terminal (inline viewport; history stays in normal scrollback)
pub fn init() -> Result<Terminal> {
    set_modes()?;

    set_panic_hook();

    // Instead of clearing the screen (which can drop scrollback in some terminals),
    // scroll existing lines up until the cursor reaches the top, then start at (0, 0).
    if let Ok((_x, y)) = cursor::position()
        && y > 0
    {
        execute!(stdout(), ScrollUp(y))?;
    }
    execute!(stdout(), MoveTo(0, 0))?;

    let backend = CrosstermBackend::new(stdout());
    let tui = CustomTerminal::with_options(backend)?;
    Ok(tui)
}

fn set_panic_hook() {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = restore(); // ignore any errors as we are already failing
        hook(panic_info);
    }));
}

#[derive(Debug)]
pub enum TuiEvent {
    Key(KeyEvent),
    Paste(String),
    Draw,
    #[cfg(unix)]
    ResumeFromSuspend,
}

pub struct Tui {
    frame_schedule_tx: tokio::sync::mpsc::UnboundedSender<Instant>,
    draw_tx: tokio::sync::broadcast::Sender<()>,
    pub(crate) terminal: Terminal,
    pending_history_lines: Vec<Line<'static>>,
}

#[derive(Clone, Debug)]
pub struct FrameRequester {
    frame_schedule_tx: tokio::sync::mpsc::UnboundedSender<Instant>,
}
impl FrameRequester {
    pub fn schedule_frame(&self) {
        let _ = self.frame_schedule_tx.send(Instant::now());
    }
    pub fn schedule_frame_in(&self, dur: Duration) {
        let _ = self.frame_schedule_tx.send(Instant::now() + dur);
    }
}

#[cfg(test)]
impl FrameRequester {
    /// Create a no-op frame requester for tests.
    pub(crate) fn test_dummy() -> Self {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        FrameRequester {
            frame_schedule_tx: tx,
        }
    }
}

impl Tui {
    pub fn new(terminal: Terminal) -> Self {
        let (frame_schedule_tx, frame_schedule_rx) = tokio::sync::mpsc::unbounded_channel();
        let (draw_tx, _) = tokio::sync::broadcast::channel(1);

        // Spawn background scheduler to coalesce frame requests and emit draws at deadlines.
        let draw_tx_clone = draw_tx.clone();
        tokio::spawn(async move {
            use tokio::select;
            use tokio::time::Instant as TokioInstant;
            use tokio::time::sleep_until;

            let mut rx = frame_schedule_rx;
            let mut next_deadline: Option<Instant> = None;

            loop {
                let target = next_deadline
                    .unwrap_or_else(|| Instant::now() + Duration::from_secs(60 * 60 * 24 * 365));
                let sleep_fut = sleep_until(TokioInstant::from_std(target));
                tokio::pin!(sleep_fut);

                select! {
                    recv = rx.recv() => {
                        match recv {
                            Some(at) => {
                                if next_deadline.is_none_or(|cur| at < cur) {
                                    next_deadline = Some(at);
                                }
                                if at <= Instant::now() {
                                    next_deadline = None;
                                    let _ = draw_tx_clone.send(());
                                }
                            }
                            None => break,
                        }
                    }
                    _ = &mut sleep_fut => {
                        if next_deadline.is_some() {
                            next_deadline = None;
                            let _ = draw_tx_clone.send(());
                        }
                    }
                }
            }
        });

        Self {
            frame_schedule_tx,
            draw_tx,
            terminal,
            pending_history_lines: vec![],
        }
    }

    pub fn frame_requester(&self) -> FrameRequester {
        FrameRequester {
            frame_schedule_tx: self.frame_schedule_tx.clone(),
        }
    }

    pub fn event_stream(&self) -> Pin<Box<dyn Stream<Item = TuiEvent> + Send + 'static>> {
        use tokio_stream::StreamExt;
        let mut crossterm_events = crossterm::event::EventStream::new();
        let mut draw_rx = self.draw_tx.subscribe();
        let event_stream = async_stream::stream! {
            loop {
                select! {
                    Some(Ok(event)) = crossterm_events.next() => {
                        match event {
                            crossterm::event::Event::Key(KeyEvent {
                                code: KeyCode::Char('z'),
                                modifiers: crossterm::event::KeyModifiers::CONTROL,
                                kind: KeyEventKind::Press,
                                ..
                            }) => {
                                #[cfg(unix)]
                                {
                                    let _ = Tui::suspend();
                                    yield TuiEvent::ResumeFromSuspend;
                                    yield TuiEvent::Draw;
                                }
                            }
                            crossterm::event::Event::Key(key_event) => {
                                yield TuiEvent::Key(key_event);
                            }
                            crossterm::event::Event::Resize(_, _) => {
                                yield TuiEvent::Draw;
                            }
                            crossterm::event::Event::Paste(pasted) => {
                                yield TuiEvent::Paste(pasted);
                            }
                            _ => {}
                        }
                    }
                    result = draw_rx.recv() => {
                        match result {
                            Ok(_) => {
                                yield TuiEvent::Draw;
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                                // We dropped one or more draw notifications; coalesce to a single draw.
                                yield TuiEvent::Draw;
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                // Sender dropped; stop emitting draws from this source.
                            }
                        }
                    }
                }
            }
        };
        Box::pin(event_stream)
    }

    #[cfg(unix)]
    fn suspend() -> Result<()> {
        restore()?;
        unsafe { libc::kill(0, libc::SIGTSTP) };
        set_modes()?;
        Ok(())
    }

    pub fn insert_history_lines(&mut self, lines: Vec<Line<'static>>) {
        self.pending_history_lines.extend(lines);
        self.frame_requester().schedule_frame();
    }

    pub fn draw(
        &mut self,
        height: u16,
        draw_fn: impl FnOnce(&mut custom_terminal::Frame),
    ) -> Result<()> {
        std::io::stdout().sync_update(|_| {
            let terminal = &mut self.terminal;
            let screen_size = terminal.size()?;
            let last_known_screen_size = terminal.last_known_screen_size;
            if screen_size != last_known_screen_size {
                let cursor_pos = terminal.get_cursor_position()?;
                let last_known_cursor_pos = terminal.last_known_cursor_pos;
                if cursor_pos.y != last_known_cursor_pos.y {
                    let cursor_delta = cursor_pos.y as i32 - last_known_cursor_pos.y as i32;

                    let new_viewport_area = terminal.viewport_area.offset(Offset {
                        x: 0,
                        y: cursor_delta,
                    });
                    terminal.set_viewport_area(new_viewport_area);
                    terminal.clear()?;
                }
            }

            let size = terminal.size()?;

            let mut area = terminal.viewport_area;
            area.height = height.min(size.height);
            area.width = size.width;
            if area.bottom() > size.height {
                terminal
                    .backend_mut()
                    .scroll_region_up(0..area.top(), area.bottom() - size.height)?;
                area.y = size.height - area.height;
            }
            if area != terminal.viewport_area {
                terminal.clear()?;
                terminal.set_viewport_area(area);
            }
            if !self.pending_history_lines.is_empty() {
                crate::insert_history::insert_history_lines(
                    terminal,
                    self.pending_history_lines.clone(),
                );
                self.pending_history_lines.clear();
            }
            terminal.draw(|frame| {
                draw_fn(frame);
            })?;
            Ok(())
        })?
    }
}
