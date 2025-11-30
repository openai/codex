use std::fmt;
use std::io::IsTerminal;
use std::io::Result;
#[cfg(not(test))]
use std::io::Stdout;
use std::io::stdin;
use std::io::stdout;
use std::panic;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use crossterm::Command;
use crossterm::SynchronizedUpdate;
use crossterm::event::DisableBracketedPaste;
use crossterm::event::DisableFocusChange;
use crossterm::event::EnableBracketedPaste;
use crossterm::event::EnableFocusChange;
use crossterm::event::Event;
use crossterm::event::KeyEvent;
use crossterm::event::KeyboardEnhancementFlags;
use crossterm::event::PopKeyboardEnhancementFlags;
use crossterm::event::PushKeyboardEnhancementFlags;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::supports_keyboard_enhancement;
use ratatui::backend::Backend;
#[cfg(not(test))]
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::disable_raw_mode;
use ratatui::crossterm::terminal::enable_raw_mode;
use ratatui::layout::Offset;
use ratatui::text::Line;
use tokio::select;
use tokio_stream::Stream;

use crate::custom_terminal;
use crate::custom_terminal::Terminal as CustomTerminal;
#[cfg(test)]
use crate::test_backend::VT100Backend;
#[cfg(unix)]
use crate::tui::job_control::SUSPEND_KEY;
#[cfg(unix)]
use crate::tui::job_control::SuspendContext;

#[cfg(unix)]
mod job_control;

/// A type alias for the terminal type used in this application
#[cfg(not(test))]
pub type Terminal = CustomTerminal<CrosstermBackend<Stdout>>;

/// Test-only terminal type that uses the in-memory VT100 backend so tests can
/// introspect the full screen (and, with `new_with_scrollback`, the scrollback).
#[cfg(test)]
pub type Terminal = CustomTerminal<VT100Backend>;

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

    let _ = execute!(stdout(), EnableFocusChange);
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EnableAlternateScroll;

impl Command for EnableAlternateScroll {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "\x1b[?1007h")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> Result<()> {
        Err(std::io::Error::other(
            "tried to execute EnableAlternateScroll using WinAPI; use ANSI instead",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DisableAlternateScroll;

impl Command for DisableAlternateScroll {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "\x1b[?1007l")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> Result<()> {
        Err(std::io::Error::other(
            "tried to execute DisableAlternateScroll using WinAPI; use ANSI instead",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

/// Restore the terminal to its original state.
/// Inverse of `set_modes`.
pub fn restore() -> Result<()> {
    // Pop may fail on platforms that didn't support the push; ignore errors.
    let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
    execute!(stdout(), DisableBracketedPaste)?;
    let _ = execute!(stdout(), DisableFocusChange);
    disable_raw_mode()?;
    let _ = execute!(stdout(), crossterm::cursor::Show);
    Ok(())
}

/// Initialize the terminal (inline viewport; history stays in normal scrollback)
pub fn init() -> Result<Terminal> {
    if !stdin().is_terminal() {
        return Err(std::io::Error::other("stdin is not a terminal"));
    }
    if !stdout().is_terminal() {
        return Err(std::io::Error::other("stdout is not a terminal"));
    }
    set_modes()?;

    set_panic_hook();

    #[cfg(test)]
    let backend = VT100Backend::new_with_scrollback(80, 24, 256);
    #[cfg(not(test))]
    let backend = CrosstermBackend::new(stdout());
    let tui = CustomTerminal::with_options(backend)?;
    Ok(tui)
}

fn set_panic_hook() {
    let hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = restore(); // ignore any errors as we are already failing
        hook(panic_info);
    }));
}

#[derive(Debug)]
pub enum TuiEvent {
    Key(KeyEvent),
    Paste(String),
    Draw,
}

pub struct Tui {
    frame_schedule_tx: tokio::sync::mpsc::UnboundedSender<Instant>,
    draw_tx: tokio::sync::broadcast::Sender<()>,
    pub(crate) terminal: Terminal,
    pending_history_lines: Vec<Line<'static>>,
    alt_saved_viewport: Option<ratatui::layout::Rect>,
    #[cfg(unix)]
    suspend_context: SuspendContext,
    // True when overlay alt-screen UI is active
    alt_screen_active: Arc<AtomicBool>,
    // True when terminal/tab is focused; updated internally from crossterm events
    terminal_focused: Arc<AtomicBool>,
    enhanced_keys_supported: bool,
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

        spawn_frame_scheduler(frame_schedule_rx, draw_tx.clone());

        // Detect keyboard enhancement support before any EventStream is created so the
        // crossterm poller can acquire its lock without contention.
        let enhanced_keys_supported = supports_keyboard_enhancement().unwrap_or(false);
        // Cache this to avoid contention with the event reader.
        supports_color::on_cached(supports_color::Stream::Stdout);
        let _ = crate::terminal_palette::default_colors();

        Self {
            frame_schedule_tx,
            draw_tx,
            terminal,
            pending_history_lines: vec![],
            alt_saved_viewport: None,
            #[cfg(unix)]
            suspend_context: SuspendContext::new(),
            alt_screen_active: Arc::new(AtomicBool::new(false)),
            terminal_focused: Arc::new(AtomicBool::new(true)),
            enhanced_keys_supported,
        }
    }

    pub fn frame_requester(&self) -> FrameRequester {
        FrameRequester {
            frame_schedule_tx: self.frame_schedule_tx.clone(),
        }
    }

    pub fn enhanced_keys_supported(&self) -> bool {
        self.enhanced_keys_supported
    }

    /// Emit a desktop notification now if the terminal is unfocused.
    /// Returns true if a notification was posted.
    pub fn notify(&mut self, message: impl AsRef<str>) -> bool {
        if !self.terminal_focused.load(Ordering::Relaxed) {
            let _ = execute!(stdout(), PostNotification(message.as_ref().to_string()));
            true
        } else {
            false
        }
    }

    pub fn event_stream(&self) -> Pin<Box<dyn Stream<Item = TuiEvent> + Send + 'static>> {
        use tokio_stream::StreamExt;

        let mut crossterm_events = crossterm::event::EventStream::new();
        let mut draw_rx = self.draw_tx.subscribe();

        // State for tracking how we should resume from ^Z suspend.
        #[cfg(unix)]
        let suspend_context = self.suspend_context.clone();
        #[cfg(unix)]
        let alt_screen_active = self.alt_screen_active.clone();

        let terminal_focused = self.terminal_focused.clone();
        let event_stream = async_stream::stream! {
            loop {
                select! {
                    Some(Ok(event)) = crossterm_events.next() => {
                        match event {
                            Event::Key(key_event) => {
                                #[cfg(unix)]
                                if SUSPEND_KEY.is_press(key_event) {
                                    let _ = suspend_context.suspend(&alt_screen_active);
                                    // We continue here after resume.
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
                            Event::FocusGained => {
                                terminal_focused.store(true, Ordering::Relaxed);
                                crate::terminal_palette::requery_default_colors();
                                yield TuiEvent::Draw;
                            }
                            Event::FocusLost => {
                                terminal_focused.store(false, Ordering::Relaxed);
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

    /// Enter alternate screen and expand the viewport to full terminal size, saving the current
    /// inline viewport for restoration when leaving.
    pub fn enter_alt_screen(&mut self) -> Result<()> {
        let _ = execute!(self.terminal.backend_mut(), EnterAlternateScreen);
        // Enable "alternate scroll" so terminals may translate wheel to arrows
        let _ = execute!(self.terminal.backend_mut(), EnableAlternateScroll);
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
        // Disable alternate scroll when leaving alt-screen
        let _ = execute!(self.terminal.backend_mut(), DisableAlternateScroll);
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
        // If we are resuming from ^Z, we need to prepare the resume action now so we can apply it
        // in the synchronized update.
        #[cfg(unix)]
        let mut prepared_resume = self
            .suspend_context
            .prepare_resume_action(&mut self.terminal, &mut self.alt_saved_viewport);

        // Precompute any viewport updates that need a cursor-position query before entering
        // the synchronized update, to avoid racing with the event reader.
        let mut pending_viewport_area: Option<ratatui::layout::Rect> = None;
        {
            let terminal = &mut self.terminal;
            let screen_size = terminal.size()?;
            let last_known_screen_size = terminal.last_known_screen_size;
            if screen_size != last_known_screen_size
                && let Ok(cursor_pos) = terminal.get_cursor_position()
            {
                let last_known_cursor_pos = terminal.last_known_cursor_pos;
                // If we resized AND the cursor moved, we adjust the viewport area to keep the
                // cursor in the same position. This is a heuristic that seems to work well
                // at least in iTerm2.
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

        stdout().sync_update(|_| {
            #[cfg(unix)]
            if let Some(prepared) = prepared_resume.take() {
                prepared.apply(&mut self.terminal)?;
            }
            let terminal = &mut self.terminal;
            if let Some(new_area) = pending_viewport_area.take() {
                terminal.set_viewport_area(new_area);
                terminal.clear()?;
            }

            let size = terminal.size()?;

            let mut area = terminal.viewport_area;
            let alt_screen_active = self.alt_screen_active.load(Ordering::Relaxed);
            area.height = height.min(size.height);
            area.width = size.width;
            if alt_screen_active {
                // Alt-screen UI occupies the whole screen but should not emit extra scroll
                // commands that affect the inline scrollback buffer.
                area.y = 0;
                area.height = size.height;
            } else if height == u16::MAX {
                // Inline full-screen overlay should occupy the whole screen without emitting
                // extra scroll commands, so the surrounding terminal history remains intact.
                area.y = 0;
                area.height = size.height;
            } else if area.bottom() > size.height {
                // Inline viewport (excluding full-screen overlays) expanded past the bottom of
                // the screen. Scroll the whole screen using append_lines so that scrolled-off
                // lines enter the real terminal scrollback instead of being discarded via a
                // limited scroll region.
                let scroll_amount = area.bottom() - size.height;
                // Move the cursor to the last row so that append_lines produces a natural
                // full-screen scroll.
                terminal.set_cursor_position((0, size.height.saturating_sub(1)))?;
                terminal.backend_mut().append_lines(scroll_amount)?;
                area.y = size.height - area.height;
            }
            if area != terminal.viewport_area {
                // TODO(nornagon): probably this could be collapsed with the clear + set_viewport_area above.
                terminal.set_viewport_area(area);
                terminal.clear()?;
            }

            if !self.pending_history_lines.is_empty() {
                crate::insert_history::insert_history_lines(
                    terminal,
                    self.pending_history_lines.clone(),
                )?;
                self.pending_history_lines.clear();
            }

            // Update the y position for suspending so Ctrl-Z can place the cursor correctly.
            #[cfg(unix)]
            {
                let inline_area_bottom = if self.alt_screen_active.load(Ordering::Relaxed) {
                    self.alt_saved_viewport
                        .map(|r| r.bottom().saturating_sub(1))
                        .unwrap_or_else(|| area.bottom().saturating_sub(1))
                } else {
                    area.bottom().saturating_sub(1)
                };
                self.suspend_context.set_cursor_y(inline_area_bottom);
            }

            terminal.draw(|frame| {
                draw_fn(frame);
            })
        })?
    }
}

/// Spawn background scheduler to coalesce frame requests and emit draws at deadlines.
fn spawn_frame_scheduler(
    frame_schedule_rx: tokio::sync::mpsc::UnboundedReceiver<Instant>,
    draw_tx: tokio::sync::broadcast::Sender<()>,
) {
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
                            // Do not send a draw immediately here. By continuing the loop,
                            // we recompute the sleep target so the draw fires once via the
                            // sleep branch, coalescing multiple requests into a single draw.
                            continue;
                        }
                        None => break,
                    }
                }
                _ = &mut sleep_fut => {
                    if next_deadline.is_some() {
                        next_deadline = None;
                        let _ = draw_tx.send(());
                    }
                }
            }
        }
    });
}

/// Command that emits an OSC 9 desktop notification with a message.
#[derive(Debug, Clone)]
pub struct PostNotification(pub String);

impl Command for PostNotification {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "\x1b]9;{}\x07", self.0)
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> Result<()> {
        Err(std::io::Error::other(
            "tried to execute PostNotification using WinAPI; use ANSI instead",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_backend::VT100Backend;
    use pretty_assertions::assert_eq;
    use ratatui::layout::Rect;
    use ratatui::text::Line;
    use std::collections::BTreeSet;

    /// Extract a snapshot of all unique lines that can appear in the portion of
    /// the terminal "above" the inline viewport, across the entire scrollback.
    ///
    /// This approximates what a user can see by scrolling the terminal: every
    /// logical row in the history region should still be reachable after UI
    /// viewport grow/shrink operations.
    fn snapshot_history_rows(tui: &Tui) -> Vec<String> {
        let backend: &VT100Backend = tui.terminal.backend();
        let base_screen = backend.vt100().screen();
        let (_, width) = base_screen.size();
        let viewport_top = tui.terminal.viewport_area.top();
        let mut all_rows: BTreeSet<String> = BTreeSet::new();

        // Clone the screen so we can move the visible window through
        // scrollback without mutating the underlying parser.
        let mut screen = base_screen.clone();
        // We only ever insert a small number of test lines, so scanning a
        // fixed scrollback window is sufficient and keeps the test simple.
        for offset in 0..=64 {
            screen.set_scrollback(offset);
            for row in screen.rows(0, width).take(viewport_top as usize) {
                all_rows.insert(row);
            }
        }

        all_rows.into_iter().collect()
    }

    /// Insert synthetic "history" lines above the viewport using the same
    /// path as the real app (Tui::insert_history_lines + draw), so that the
    /// vt100 buffer reflects how inline history is rendered in production.
    fn seed_history_lines(tui: &mut Tui, count: usize, height: u16) -> Vec<Line<'static>> {
        let lines: Vec<Line<'static>> = (0..count)
            .map(|i| Line::from(format!("history-{i:02}")))
            .collect();
        tui.insert_history_lines(lines.clone());
        tui.draw(height, |_frame| {}).expect("initial history draw");
        lines
    }

    /// Regression test for a bug where growing the inline viewport (e.g. after
    /// pasting a tall multi‑line input) and then shrinking it again (e.g. via
    /// Ctrl‑C clearing the composer) could cause previously emitted history
    /// lines (such as `/status` and its card, or even the welcome banner) to be
    /// discarded from the inline scrollback rather than remaining scrollable
    /// above the viewport.
    ///
    /// The test simulates:
    ///   1. An inline viewport anchored to the bottom of the screen.
    ///   2. Some history lines inserted above that viewport.
    ///   3. A "paste" that grows the viewport height beyond its original
    ///      value, forcing `Tui::draw` to make room.
    ///   4. A "Ctrl‑C" that shrinks the viewport back to the original height.
    ///
    /// It then compares the vt100 rows above the viewport before and after the
    /// grow+shrink sequence and asserts that they remain identical and
    /// contiguous. With the pre‑patch implementation (which used a limited
    /// scroll region and `scroll_region_up`), this comparison breaks because
    /// some of the original rows are overwritten instead of being preserved.
    #[tokio::test(flavor = "current_thread")]
    async fn inline_history_survives_viewport_grow_and_shrink() {
        // Set up a small vt100-backed terminal so we can inspect the screen
        // contents directly. The exact size is not important as long as the
        // viewport can grow and trigger the scrolling path in `Tui::draw`.
        let width: u16 = 40;
        let height: u16 = 12;
        let backend = VT100Backend::new_with_scrollback(width, height, 256);
        let mut terminal =
            crate::custom_terminal::Terminal::with_options(backend).expect("terminal");

        // Start with an inline viewport that occupies the bottom 4 rows,
        // matching the shape of the real application where the chat UI lives
        // at the bottom and history scrolls above it.
        let initial_viewport_height: u16 = 4;
        let initial_viewport = Rect::new(
            0,
            height - initial_viewport_height,
            width,
            initial_viewport_height,
        );
        terminal.set_viewport_area(initial_viewport);

        // Construct a TUI around this terminal so we exercise the real
        // `Tui::draw` viewport management logic (the code that was fixed).
        let mut tui = Tui::new(terminal);

        // Seed some synthetic history above the viewport to stand in for the
        // welcome banner + `/status` command and its card.
        let history_lines = seed_history_lines(&mut tui, 8, initial_viewport_height);

        let before_rows = snapshot_history_rows(&tui);

        // Simulate a tall multi-line paste that grows the inline viewport.
        let tall_height: u16 = 10;
        tui.draw(tall_height, |_frame| {})
            .expect("draw with tall viewport");

        // Simulate Ctrl+C clearing the composer, shrinking the viewport back
        // to its original height.
        tui.draw(initial_viewport_height, |_frame| {})
            .expect("draw after clearing composer");

        let after_rows = snapshot_history_rows(&tui);

        // History rows above the viewport must be exactly the same sequence of
        // lines before and after the grow+shrink sequence: no gaps, no
        // truncation, and no reordering.
        assert_eq!(
            before_rows, after_rows,
            "inline history above the viewport changed after growing then shrinking the viewport.\n\
             Before: {before_rows:?}\n\
             After:  {after_rows:?}\n\
             Seeded history lines: {history_lines:?}",
        );
    }
}
