//! Terminal lifecycle, input plumbing, and viewport control for the TUI.
//!
//! This module owns the interaction with the terminal itself: enabling and restoring raw
//! input modes, entering and leaving the alternate screen, and translating low-level
//! crossterm events into higher-level [`TuiEvent`]s. It also manages inline-viewport
//! rendering, including the scrollback-aware viewport adjustments that keep the cursor in a
//! stable position during resize events.
//!
//! The [`Tui`] type is the state owner for terminal-backed concerns (the ratatui backend,
//! input event stream, draw scheduling, and focus tracking). It does not own application
//! state or rendering logic; callers supply the draw closure and handle all application
//! updates outside this module.
//!
//! Correctness depends on pairing [`set_modes`] with [`restore`] or [`restore_keep_raw`],
//! and on using [`Tui::with_restored`] when running external interactive programs so the
//! event stream and terminal modes are paused and re-established in a consistent order.

use std::fmt;
use std::future::Future;
use std::io::IsTerminal;
use std::io::Result;
use std::io::Stdout;
use std::io::stdin;
use std::io::stdout;
use std::panic;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use crossterm::Command;
use crossterm::SynchronizedUpdate;
use crossterm::event::DisableBracketedPaste;
use crossterm::event::DisableFocusChange;
use crossterm::event::EnableBracketedPaste;
use crossterm::event::EnableFocusChange;
use crossterm::event::KeyEvent;
use crossterm::event::KeyboardEnhancementFlags;
use crossterm::event::PopKeyboardEnhancementFlags;
use crossterm::event::PushKeyboardEnhancementFlags;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::supports_keyboard_enhancement;
use ratatui::backend::Backend;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::disable_raw_mode;
use ratatui::crossterm::terminal::enable_raw_mode;
use ratatui::layout::Offset;
use ratatui::layout::Rect;
use ratatui::text::Line;
use tokio::sync::broadcast;
use tokio_stream::Stream;

pub use self::frame_requester::FrameRequester;
use crate::custom_terminal;
use crate::custom_terminal::Terminal as CustomTerminal;
use crate::notifications::DesktopNotificationBackend;
use crate::notifications::NotificationBackendKind;
use crate::notifications::detect_backend;
use crate::tui::event_stream::EventBroker;
use crate::tui::event_stream::TuiEventStream;
#[cfg(unix)]
use crate::tui::job_control::SuspendContext;

mod event_stream;
mod frame_rate_limiter;
mod frame_requester;
#[cfg(unix)]
mod job_control;

/// A type alias for the terminal type used by the TUI layer.
pub type Terminal = CustomTerminal<CrosstermBackend<Stdout>>;

/// Enable terminal modes required for the interactive UI.
///
/// This turns on raw mode, bracketed paste, focus tracking, and best-effort keyboard
/// enhancement flags. Callers must pair this with [`restore`] or
/// [`restore_keep_raw`] on exit.
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

/// ANSI command that enables alternate-scroll mode inside the alternate screen.
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

/// ANSI command that disables alternate-scroll mode on exit.
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

/// Restore terminal modes with optional raw-mode disabling.
///
/// This is shared by [`restore`] and [`restore_keep_raw`] to ensure we unwind the same
/// keyboard enhancement and focus settings regardless of raw-mode policy.
fn restore_common(should_disable_raw_mode: bool) -> Result<()> {
    // Pop may fail on platforms that didn't support the push; ignore errors.
    let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
    execute!(stdout(), DisableBracketedPaste)?;
    let _ = execute!(stdout(), DisableFocusChange);
    if should_disable_raw_mode {
        disable_raw_mode()?;
    }
    let _ = execute!(stdout(), crossterm::cursor::Show);
    Ok(())
}

/// Restore the terminal to its original state.
///
/// This is the inverse of [`set_modes`] and disables raw mode.
pub fn restore() -> Result<()> {
    let should_disable_raw_mode = true;
    restore_common(should_disable_raw_mode)
}

/// Restore the terminal to its original state, but keep raw mode enabled.
pub fn restore_keep_raw() -> Result<()> {
    let should_disable_raw_mode = false;
    restore_common(should_disable_raw_mode)
}

/// Controls how much terminal state should be restored after pausing the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreMode {
    /// Fully restore the terminal (disables raw mode).
    #[allow(dead_code)]
    Full,

    /// Restore the terminal but keep raw mode enabled.
    KeepRaw,
}

impl RestoreMode {
    /// Apply the selected restore policy.
    fn restore(self) -> Result<()> {
        match self {
            RestoreMode::Full => restore(),
            RestoreMode::KeepRaw => restore_keep_raw(),
        }
    }
}

/// Flush the underlying stdin buffer to clear any input queued by the terminal.
///
/// This clears user input that arrived while the crossterm `EventStream` was
/// temporarily dropped, preventing a backlog of stale keystrokes.
#[cfg(unix)]
fn flush_terminal_input_buffer() {
    // Safety: flushing the stdin queue is safe and does not move ownership.
    let result = unsafe { libc::tcflush(libc::STDIN_FILENO, libc::TCIFLUSH) };
    if result != 0 {
        let err = std::io::Error::last_os_error();
        tracing::warn!("failed to tcflush stdin: {err}");
    }
}

/// Flush the underlying stdin buffer to clear any input queued by the terminal.
///
/// This clears user input that arrived while the crossterm `EventStream` was
/// temporarily dropped, preventing a backlog of stale keystrokes.
#[cfg(windows)]
fn flush_terminal_input_buffer() {
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::FlushConsoleInputBuffer;
    use windows_sys::Win32::System::Console::GetStdHandle;
    use windows_sys::Win32::System::Console::STD_INPUT_HANDLE;

    let handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
    if handle == INVALID_HANDLE_VALUE || handle == 0 {
        let err = unsafe { GetLastError() };
        tracing::warn!("failed to get stdin handle for flush: error {err}");
        return;
    }

    let result = unsafe { FlushConsoleInputBuffer(handle) };
    if result == 0 {
        let err = unsafe { GetLastError() };
        tracing::warn!("failed to flush stdin buffer: error {err}");
    }
}

/// Flush the underlying stdin buffer when the platform exposes a native queue.
#[cfg(not(any(unix, windows)))]
pub(crate) fn flush_terminal_input_buffer() {}

/// Initialize the terminal for inline rendering (history stays in normal scrollback).
///
/// This validates that stdin/stdout are terminals, enables TUI input modes, installs a
/// panic hook that restores terminal state, and returns a configured [`Terminal`] wrapper.
pub fn init() -> Result<Terminal> {
    if !stdin().is_terminal() {
        return Err(std::io::Error::other("stdin is not a terminal"));
    }
    if !stdout().is_terminal() {
        return Err(std::io::Error::other("stdout is not a terminal"));
    }
    set_modes()?;

    set_panic_hook();

    let backend = CrosstermBackend::new(stdout());
    let tui = CustomTerminal::with_options(backend)?;
    Ok(tui)
}

/// Install a panic hook that attempts to restore terminal modes before unwinding.
fn set_panic_hook() {
    let hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = restore(); // ignore any errors as we are already failing
        hook(panic_info);
    }));
}

/// Input and draw events emitted by the terminal event stream.
#[derive(Clone, Debug)]
pub enum TuiEvent {
    /// A keyboard event from crossterm.
    Key(KeyEvent),

    /// A paste event containing the pasted text.
    Paste(String),

    /// A draw tick (frame requested).
    Draw,
}

/// Owns the terminal instance, event stream, and draw scheduling.
///
/// The `Tui` struct is the single owner of terminal state and the bridge between
/// crossterm's event stream and the application's render loop. It tracks viewport
/// bookkeeping, focus state, and alternate-screen lifetime so that rendering can
/// be driven externally without having to manage terminal invariants elsewhere.
pub struct Tui {
    /// Shared frame scheduler for debounced redraws.
    frame_requester: FrameRequester,

    /// Draw trigger broadcast channel used by the event stream.
    draw_tx: broadcast::Sender<()>,

    /// Broker for pausing/resuming crossterm events.
    event_broker: Arc<EventBroker>,

    /// Underlying terminal wrapper used for rendering and viewport control.
    pub(crate) terminal: Terminal,

    /// History lines buffered until the next draw cycle.
    pending_history_lines: Vec<Line<'static>>,

    /// Saved inline viewport while in the alternate screen.
    alt_saved_viewport: Option<ratatui::layout::Rect>,

    /// Suspend/resume state for Unix job control.
    #[cfg(unix)]
    suspend_context: SuspendContext,

    /// Tracks whether an alternate-screen UI overlay is currently active.
    alt_screen_active: Arc<AtomicBool>,

    /// Tracks whether the terminal/tab is focused based on crossterm events.
    terminal_focused: Arc<AtomicBool>,

    /// Whether the terminal supports keyboard enhancement flags.
    enhanced_keys_supported: bool,

    /// Optional desktop notification backend for unfocused updates.
    notification_backend: Option<DesktopNotificationBackend>,

    /// When false, `enter_alt_screen` becomes a no-op (for Zellij scrollback support).
    alt_screen_enabled: bool,
}

impl Tui {
    /// Construct a new `Tui` wrapper around an initialized terminal.
    pub fn new(terminal: Terminal) -> Self {
        let (draw_tx, _) = broadcast::channel(1);
        let frame_requester = FrameRequester::new(draw_tx.clone());

        // Detect keyboard enhancement support before any EventStream is created so the
        // crossterm poller can acquire its lock without contention.
        let enhanced_keys_supported = supports_keyboard_enhancement().unwrap_or(false);

        // Cache this to avoid contention with the event reader.
        supports_color::on_cached(supports_color::Stream::Stdout);
        let _ = crate::terminal_palette::default_colors();

        Self {
            frame_requester,
            draw_tx,
            event_broker: Arc::new(EventBroker::new()),
            terminal,
            pending_history_lines: vec![],
            alt_saved_viewport: None,
            #[cfg(unix)]
            suspend_context: SuspendContext::new(),
            alt_screen_active: Arc::new(AtomicBool::new(false)),
            terminal_focused: Arc::new(AtomicBool::new(true)),
            enhanced_keys_supported,
            notification_backend: Some(detect_backend()),
            alt_screen_enabled: true,
        }
    }

    /// Set whether alternate screen is enabled.
    ///
    /// When disabled, [`Tui::enter_alt_screen`] becomes a no-op so that inline scrollback
    /// remains available (for example, under Zellij).
    pub fn set_alt_screen_enabled(&mut self, enabled: bool) {
        self.alt_screen_enabled = enabled;
    }

    /// Return a handle to the frame requester used by this TUI.
    pub fn frame_requester(&self) -> FrameRequester {
        self.frame_requester.clone()
    }

    /// Report whether keyboard enhancement flags are supported.
    pub fn enhanced_keys_supported(&self) -> bool {
        self.enhanced_keys_supported
    }

    /// Report whether the alternate screen is currently active.
    pub fn is_alt_screen_active(&self) -> bool {
        self.alt_screen_active.load(Ordering::Relaxed)
    }

    /// Drop the crossterm `EventStream` to avoid stdin conflicts with other processes.
    pub fn pause_events(&mut self) {
        self.event_broker.pause_events();
    }

    /// Resume crossterm event polling after a [`Tui::pause_events`] call.
    pub fn resume_events(&mut self) {
        self.event_broker.resume_events();
    }

    /// Temporarily restore terminal state to run an external interactive program `f`.
    ///
    /// The flow is: pause event polling, exit the alternate screen if needed, restore terminal
    /// modes according to `mode`, run the external program, re-enable Codex TUI modes, flush
    /// buffered input, re-enter the alternate screen if necessary, then resume events.
    pub async fn with_restored<R, F, Fut>(&mut self, mode: RestoreMode, f: F) -> R
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = R>,
    {
        // Pause crossterm events to avoid stdin conflicts with external program `f`.
        self.pause_events();

        // Leave alt screen if active to avoid conflicts with external program `f`.
        let was_alt_screen = self.is_alt_screen_active();
        if was_alt_screen {
            let _ = self.leave_alt_screen();
        }

        if let Err(err) = mode.restore() {
            tracing::warn!("failed to restore terminal modes before external program: {err}");
        }

        let output = f().await;

        if let Err(err) = set_modes() {
            tracing::warn!("failed to re-enable terminal modes after external program: {err}");
        }

        // After the external program `f` finishes, reset terminal state and flush any buffered keypresses.
        flush_terminal_input_buffer();

        if was_alt_screen {
            let _ = self.enter_alt_screen();
        }

        self.resume_events();
        output
    }

    /// Emit a desktop notification now if the terminal is unfocused.
    ///
    /// Returns `true` if a notification was posted. When the configured backend fails, the
    /// backend is downgraded or disabled to avoid repeated failures.
    pub fn notify(&mut self, message: impl AsRef<str>) -> bool {
        if self.terminal_focused.load(Ordering::Relaxed) {
            return false;
        }

        let Some(backend) = self.notification_backend.as_mut() else {
            return false;
        };

        let message = message.as_ref().to_string();
        match backend.notify(&message) {
            Ok(()) => true,
            Err(err) => match backend.kind() {
                NotificationBackendKind::WindowsToast => {
                    tracing::error!(
                        error = %err,
                        "Failed to send Windows toast notification; falling back to OSC 9"
                    );
                    self.notification_backend = Some(DesktopNotificationBackend::osc9());
                    if let Some(backend) = self.notification_backend.as_mut() {
                        if let Err(osc_err) = backend.notify(&message) {
                            tracing::warn!(
                                error = %osc_err,
                                "Failed to emit OSC 9 notification after toast fallback; \
                                 disabling future notifications"
                            );
                            self.notification_backend = None;
                            return false;
                        }
                        return true;
                    }
                    false
                }
                NotificationBackendKind::Osc9 => {
                    tracing::warn!(
                        error = %err,
                        "Failed to emit OSC 9 notification; disabling future notifications"
                    );
                    self.notification_backend = None;
                    false
                }
            },
        }
    }

    /// Return a stream of TUI events (draw ticks, key events, paste events).
    ///
    /// The stream is backed by a paused/resumable crossterm poller and a broadcast channel for
    /// scheduled draw ticks, so consumers only need to poll a single source.
    pub fn event_stream(&self) -> Pin<Box<dyn Stream<Item = TuiEvent> + Send + 'static>> {
        #[cfg(unix)]
        let stream = TuiEventStream::new(
            self.event_broker.clone(),
            self.draw_tx.subscribe(),
            self.terminal_focused.clone(),
            self.suspend_context.clone(),
            self.alt_screen_active.clone(),
        );
        #[cfg(not(unix))]
        let stream = TuiEventStream::new(
            self.event_broker.clone(),
            self.draw_tx.subscribe(),
            self.terminal_focused.clone(),
        );
        Box::pin(stream)
    }

    /// Enter alternate screen and expand the viewport to full terminal size.
    ///
    /// This saves the current inline viewport for restoration when leaving.
    pub fn enter_alt_screen(&mut self) -> Result<()> {
        if !self.alt_screen_enabled {
            return Ok(());
        }
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
        if !self.alt_screen_enabled {
            return Ok(());
        }

        // Disable alternate scroll when leaving alt-screen
        let _ = execute!(self.terminal.backend_mut(), DisableAlternateScroll);
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        if let Some(saved) = self.alt_saved_viewport.take() {
            self.terminal.set_viewport_area(saved);
        }
        self.alt_screen_active.store(false, Ordering::Relaxed);
        Ok(())
    }

    /// Queue transcript history lines for insertion on the next draw.
    pub fn insert_history_lines(&mut self, lines: Vec<Line<'static>>) {
        self.pending_history_lines.extend(lines);
        self.frame_requester().schedule_frame();
    }

    /// Draw the UI into the terminal with a synchronized update.
    ///
    /// This prepares any suspend/resume state, adjusts the viewport for inline rendering,
    /// inserts pending history lines, and then delegates to the caller-provided draw function.
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
        let mut pending_viewport_area = self.pending_viewport_area()?;

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
            area.height = height.min(size.height);
            area.width = size.width;

            // If the viewport has expanded, scroll everything else up to make room.
            if area.bottom() > size.height {
                terminal
                    .backend_mut()
                    .scroll_region_up(0..area.top(), area.bottom() - size.height)?;
                area.y = size.height - area.height;
            }
            if area != terminal.viewport_area {
                // TODO(nornagon): probably this could be collapsed with the clear + set_viewport_area above.
                terminal.clear()?;
                terminal.set_viewport_area(area);
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

    /// Compute a new viewport area when terminal resize and cursor motion diverge.
    ///
    /// This keeps the cursor's relative position stable when the terminal size changes and
    /// the cursor has moved since the last known size.
    fn pending_viewport_area(&mut self) -> Result<Option<Rect>> {
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
                let offset = Offset {
                    x: 0,
                    y: cursor_pos.y as i32 - last_known_cursor_pos.y as i32,
                };
                return Ok(Some(terminal.viewport_area.offset(offset)));
            }
        }
        Ok(None)
    }
}
