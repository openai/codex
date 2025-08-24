use std::io::Result;
use std::io::Stdout;
use std::io::stdout;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
#[cfg(unix)]
use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use crossterm::SynchronizedUpdate;
use crossterm::cursor;
use crossterm::cursor::MoveTo;
use crossterm::event::DisableBracketedPaste;
use crossterm::event::EnableBracketedPaste;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use crossterm::event::KeyboardEnhancementFlags;
use crossterm::event::PopKeyboardEnhancementFlags;
use crossterm::event::PushKeyboardEnhancementFlags;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::ScrollUp;
use ratatui::backend::Backend;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::disable_raw_mode;
use ratatui::crossterm::terminal::enable_raw_mode;
use ratatui::layout::Offset;
use ratatui::text::Line;

use crate::clipboard_paste::paste_image_to_temp_png;
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
    AttachImage {
        path: PathBuf,
        width: u32,
        height: u32,
        format_label: &'static str,
    },
}

pub struct Tui {
    frame_schedule_tx: tokio::sync::mpsc::UnboundedSender<Instant>,
    draw_tx: tokio::sync::broadcast::Sender<()>,
    pub(crate) terminal: Terminal,
    pending_history_lines: Vec<Line<'static>>,
    alt_saved_viewport: Option<ratatui::layout::Rect>,
    #[cfg(unix)]
    resume_pending: Arc<AtomicU8>, // Stores a ResumeAction
    // True when overlay alt-screen UI is active
    alt_screen_active: Arc<AtomicBool>,
}

#[cfg(unix)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
enum ResumeAction {
    None = 0,
    RealignInline = 1,
    RestoreAlt = 2,
}

#[cfg(unix)]
enum PreparedResumeAction {
    RestoreAltScreen,
    RealignViewport(ratatui::layout::Rect),
}

#[cfg(unix)]
fn take_resume_action(pending: &AtomicU8) -> ResumeAction {
    match pending.swap(ResumeAction::None as u8, Ordering::Relaxed) {
        1 => ResumeAction::RealignInline,
        2 => ResumeAction::RestoreAlt,
        _ => ResumeAction::None,
    }
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
            alt_saved_viewport: None,
            #[cfg(unix)]
            resume_pending: Arc::new(AtomicU8::new(0)),
            alt_screen_active: Arc::new(AtomicBool::new(false)),
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
        #[cfg(unix)]
        let resume_pending = self.resume_pending.clone();
        #[cfg(unix)]
        let alt_screen_active = self.alt_screen_active.clone();
        let event_stream = async_stream::stream! {
            loop {
                select! {
                    Some(Ok(event)) = crossterm_events.next() => {
                        match event {
                            // Detect Ctrl+V to attach an image from the clipboard.
                            Event::Key(key_event @ KeyEvent {
                                code: KeyCode::Char('v'),
                                modifiers: KeyModifiers::CONTROL,
                                kind: KeyEventKind::Press,
                                ..
                            }) => {
                                match paste_image_to_temp_png() {
                                    Ok((path, info)) => {
                                        yield TuiEvent::AttachImage {
                                            path,
                                            width: info.width,
                                            height: info.height,
                                            format_label: info.encoded_format.label(),
                                        };
                                    }
                                    Err(_) => {
                                        // Fall back to normal key handling if no image is available.
                                        yield TuiEvent::Key(key_event);
                                    }
                                }
                            }

                            crossterm::event::Event::Key(key_event) => {
                                #[cfg(unix)]
                                if matches!(
                                    key_event,
                                    crossterm::event::KeyEvent {
                                        code: crossterm::event::KeyCode::Char('z'),
                                        modifiers: crossterm::event::KeyModifiers::CONTROL,
                                        kind: crossterm::event::KeyEventKind::Press,
                                        ..
                                    }
                                )
                                {
                                    if alt_screen_active.load(Ordering::Relaxed) {
                                        let _ = execute!(stdout(), LeaveAlternateScreen);
                                        resume_pending.store(ResumeAction::RestoreAlt as u8, Ordering::Relaxed);
                                    } else {
                                        resume_pending.store(ResumeAction::RealignInline as u8, Ordering::Relaxed);
                                    }
                                    let _ = execute!(stdout(), crossterm::cursor::Show);
                                    let _ = Tui::suspend();
                                    yield TuiEvent::Draw;
                                    continue;
                                }
                                yield TuiEvent::Key(key_event);
                            }
                            Event::Resize(_, _) => {
                                yield TuiEvent::Draw;
                            }
                            Event::Paste(pasted) => {
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

    #[cfg(unix)]
    fn prepare_resume_action(
        &mut self,
        action: ResumeAction,
    ) -> Result<Option<PreparedResumeAction>> {
        match action {
            ResumeAction::RealignInline => {
                let cursor_pos = self.terminal.get_cursor_position()?;
                Ok(Some(PreparedResumeAction::RealignViewport(
                    ratatui::layout::Rect::new(0, cursor_pos.y, 0, 0),
                )))
            }
            ResumeAction::RestoreAlt => {
                if let Ok((_x, y)) = crossterm::cursor::position()
                    && let Some(saved) = self.alt_saved_viewport.as_mut()
                {
                    saved.y = y;
                }
                Ok(Some(PreparedResumeAction::RestoreAltScreen))
            }
            ResumeAction::None => Ok(None),
        }
    }

    #[cfg(unix)]
    fn apply_prepared_resume_action(&mut self, prepared: PreparedResumeAction) -> Result<()> {
        match prepared {
            PreparedResumeAction::RealignViewport(area) => {
                self.terminal.set_viewport_area(area);
            }
            PreparedResumeAction::RestoreAltScreen => {
                execute!(self.terminal.backend_mut(), EnterAlternateScreen)?;
                if let Ok(size) = self.terminal.size() {
                    self.terminal.set_viewport_area(ratatui::layout::Rect::new(
                        0,
                        0,
                        size.width,
                        size.height,
                    ));
                    self.terminal.clear()?;
                }
            }
        }
        Ok(())
    }

    /// Enter alternate screen and expand the viewport to full terminal size, saving the current
    /// inline viewport for restoration when leaving.
    pub fn enter_alt_screen(&mut self) -> Result<()> {
        let _ = execute!(self.terminal.backend_mut(), EnterAlternateScreen);
        if let Ok(size) = self.terminal.size() {
            self.alt_saved_viewport = Some(self.terminal.viewport_area);
            self.terminal.set_viewport_area(ratatui::layout::Rect::new(
                0,
                0,
                size.width,
                size.height,
            ));
            let _ = self.terminal.clear();
        }
        self.alt_screen_active.store(true, Ordering::Relaxed);
        Ok(())
    }

    /// Leave alternate screen and restore the previously saved inline viewport, if any.
    pub fn leave_alt_screen(&mut self) -> Result<()> {
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        if let Some(saved) = self.alt_saved_viewport.take() {
            self.terminal.set_viewport_area(saved);
        }
        self.alt_screen_active.store(false, Ordering::Relaxed);
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
        // Precompute any viewport updates that need a cursor-position query before entering
        // the synchronized update, to avoid racing with the event reader.
        let mut pending_viewport_area: Option<ratatui::layout::Rect> = None;
        #[cfg(unix)]
        let mut prepared_resume =
            self.prepare_resume_action(take_resume_action(&self.resume_pending))?;
        {
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
                    pending_viewport_area = Some(new_viewport_area);
                }
            }
        }

        std::io::stdout().sync_update(|_| {
            #[cfg(unix)]
            {
                if let Some(prepared) = prepared_resume.take() {
                    self.apply_prepared_resume_action(prepared)?;
                }
            }
            let terminal = &mut self.terminal;
            if let Some(new_area) = pending_viewport_area.take() {
                terminal.set_viewport_area(new_area);
                terminal.clear()?;
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
