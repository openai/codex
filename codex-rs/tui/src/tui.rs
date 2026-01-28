use std::fmt;
use std::future::Future;
use std::io::IsTerminal;
use std::io::Result;
use std::io::stdin;
use std::io::stdout;
use std::panic;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use crate::render::model::RenderLine as Line;
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
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::disable_raw_mode;
use ratatui::crossterm::terminal::enable_raw_mode;
use ratatui::layout::Offset;
use ratatui::layout::Rect;
use tokio::sync::broadcast;
use tokio_stream::Stream;

pub use self::frame_requester::FrameRequester;
use self::tui_backend::TuiBackend;
use crate::custom_terminal;
use crate::custom_terminal::Terminal as CustomTerminal;
use crate::notifications::DesktopNotificationBackend;
use crate::notifications::NotificationBackendKind;
use crate::notifications::detect_backend;
use crate::tui::event_stream::EventBroker;
use crate::tui::event_stream::TuiEventStream;
#[cfg(unix)]
use crate::tui::job_control::SuspendContext;

mod curses_backend;
mod event_stream;
mod frame_rate_limiter;
mod frame_requester;
#[cfg(unix)]
mod job_control;
pub(crate) mod tui_backend;

/// A type alias for the terminal type used in this application.
pub type Terminal = CustomTerminal<TuiBackend>;

/// Configures terminal input modes for interactive use.
///
/// # Returns
/// - `Result<()>`: Result of the mode setup.
pub fn set_modes() -> Result<()> {
    execute!(stdout(), EnableBracketedPaste)?;

    enable_raw_mode()?;
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
    /// Writes the ANSI escape code for enabling alternate scroll.
    ///
    /// # Arguments
    /// - `f` (&mut impl fmt::Write): Target formatter.
    ///
    /// # Returns
    /// - `fmt::Result`: Result of the write operation.
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "\x1b[?1007h")
    }

    #[cfg(windows)]
    /// Returns an error indicating WinAPI is not supported for this command.
    ///
    /// # Returns
    /// - `Result<()>`: Error result for WinAPI execution.
    fn execute_winapi(&self) -> Result<()> {
        Err(std::io::Error::other(
            "tried to execute EnableAlternateScroll using WinAPI; use ANSI instead",
        ))
    }

    #[cfg(windows)]
    /// Reports whether ANSI codes are supported on Windows.
    ///
    /// # Returns
    /// - `bool`: True when ANSI is supported.
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DisableAlternateScroll;

impl Command for DisableAlternateScroll {
    /// Writes the ANSI escape code for disabling alternate scroll.
    ///
    /// # Arguments
    /// - `f` (&mut impl fmt::Write): Target formatter.
    ///
    /// # Returns
    /// - `fmt::Result`: Result of the write operation.
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "\x1b[?1007l")
    }

    #[cfg(windows)]
    /// Returns an error indicating WinAPI is not supported for this command.
    ///
    /// # Returns
    /// - `Result<()>`: Error result for WinAPI execution.
    fn execute_winapi(&self) -> Result<()> {
        Err(std::io::Error::other(
            "tried to execute DisableAlternateScroll using WinAPI; use ANSI instead",
        ))
    }

    #[cfg(windows)]
    /// Reports whether ANSI codes are supported on Windows.
    ///
    /// # Returns
    /// - `bool`: True when ANSI is supported.
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

/// Restores common terminal state after a session.
///
/// # Arguments
/// - `should_disable_raw_mode` (bool): Whether to disable raw mode.
///
/// # Returns
/// - `Result<()>`: Result of the restore operation.
fn restore_common(should_disable_raw_mode: bool) -> Result<()> {
    let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
    execute!(stdout(), DisableBracketedPaste)?;
    let _ = execute!(stdout(), DisableFocusChange);
    if should_disable_raw_mode {
        disable_raw_mode()?;
    }
    let _ = execute!(stdout(), crossterm::cursor::Show);
    Ok(())
}

/// Restores the terminal to its original state.
///
/// # Returns
/// - `Result<()>`: Result of the restore operation.
pub fn restore() -> Result<()> {
    let should_disable_raw_mode = true;
    restore_common(should_disable_raw_mode)
}

/// Restores the terminal to its original state, keeping raw mode enabled.
///
/// # Returns
/// - `Result<()>`: Result of the restore operation.
pub fn restore_keep_raw() -> Result<()> {
    let should_disable_raw_mode = false;
    restore_common(should_disable_raw_mode)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreMode {
    #[allow(dead_code)]
    Full,
    KeepRaw,
}

impl RestoreMode {
    /// Restores terminal state according to the selected mode.
    ///
    /// # Returns
    /// - `Result<()>`: Result of the restore operation.
    fn restore(self) -> Result<()> {
        match self {
            RestoreMode::Full => restore(),
            RestoreMode::KeepRaw => restore_keep_raw(),
        }
    }
}

/// Flushes the underlying stdin buffer to clear buffered input.
///
/// # Returns
/// - `()`: No return value.
#[cfg(unix)]
fn flush_terminal_input_buffer() {
    let result = unsafe { libc::tcflush(libc::STDIN_FILENO, libc::TCIFLUSH) };
    if result != 0 {
        let err = std::io::Error::last_os_error();
        tracing::warn!("failed to tcflush stdin: {err}");
    }
}

/// Flushes the underlying stdin buffer to clear buffered input.
///
/// # Returns
/// - `()`: No return value.
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

#[cfg(not(any(unix, windows)))]
/// Flushes the stdin buffer on unsupported platforms.
///
/// # Returns
/// - `()`: No-op for unsupported platforms.
pub(crate) fn flush_terminal_input_buffer() {}

/// Initialize the terminal.
///
/// # Returns
/// - `Result<Terminal>`: Initialized terminal or an error.
pub fn init() -> Result<Terminal> {
    if !stdin().is_terminal() {
        return Err(std::io::Error::other("stdin is not a terminal"));
    }
    if !stdout().is_terminal() {
        return Err(std::io::Error::other("stdout is not a terminal"));
    }
    set_modes()?;

    set_panic_hook();

    let backend = TuiBackend::new_default()?;
    CustomTerminal::with_options(backend)
}

/// Installs a panic hook that restores terminal state before delegating.
///
/// # Returns
/// - `()`: No return value.
fn set_panic_hook() {
    let hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = restore();
        hook(panic_info);
    }));
}

#[derive(Clone, Debug)]
/// Events emitted by the TUI event stream.
pub enum TuiEvent {
    Key(KeyEvent),
    Paste(String),
    Draw,
}

pub struct Tui {
    frame_requester: FrameRequester,
    draw_tx: broadcast::Sender<()>,
    event_broker: Arc<EventBroker>,
    pub(crate) terminal: Terminal,
    pending_history_lines: Vec<Line>,
    alt_saved_viewport: Option<ratatui::layout::Rect>,
    #[cfg(unix)]
    suspend_context: SuspendContext,
    /// True when overlay alt-screen UI is active.
    alt_screen_active: Arc<AtomicBool>,
    /// True when terminal/tab is focused; updated internally from crossterm events.
    terminal_focused: Arc<AtomicBool>,
    enhanced_keys_supported: bool,
    notification_backend: Option<DesktopNotificationBackend>,
    /// When false, enter_alt_screen becomes a no-op.
    alt_screen_enabled: bool,
}

impl Tui {
    /// Creates a new TUI instance.
    ///
    /// # Arguments
    /// - `terminal` (Terminal): Terminal backend wrapper.
    ///
    /// # Returns
    /// - `Tui`: Initialized TUI instance.
    pub fn new(terminal: Terminal) -> Self {
        let (draw_tx, _) = broadcast::channel(1);
        let frame_requester = FrameRequester::new(draw_tx.clone());

        let enhanced_keys_supported = supports_keyboard_enhancement().unwrap_or(false);
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

    /// Sets whether alternate screen is enabled.
    ///
    /// # Arguments
    /// - `enabled` (bool): True to enable alternate screen.
    ///
    /// # Returns
    /// - `()`: No return value.
    pub fn set_alt_screen_enabled(&mut self, enabled: bool) {
        self.alt_screen_enabled = enabled;
    }

    /// Returns the frame requester for scheduling draws.
    ///
    /// # Returns
    /// - `FrameRequester`: Frame requester handle.
    pub fn frame_requester(&self) -> FrameRequester {
        self.frame_requester.clone()
    }

    /// Returns whether enhanced key reporting is supported.
    ///
    /// # Returns
    /// - `bool`: True if enhanced keys are supported.
    pub fn enhanced_keys_supported(&self) -> bool {
        self.enhanced_keys_supported
    }

    /// Returns whether the alternate screen is active.
    ///
    /// # Returns
    /// - `bool`: True if alternate screen is active.
    pub fn is_alt_screen_active(&self) -> bool {
        self.alt_screen_active.load(Ordering::Relaxed)
    }

    /// Pauses event processing.
    ///
    /// # Returns
    /// - `()`: No return value.
    pub fn pause_events(&mut self) {
        self.event_broker.pause_events();
    }

    /// Resumes event processing.
    ///
    /// # Returns
    /// - `()`: No return value.
    pub fn resume_events(&mut self) {
        self.event_broker.resume_events();
    }

    /// Temporarily restores terminal state to run an external program.
    ///
    /// # Arguments
    /// - `mode` (RestoreMode): Terminal restore mode to apply.
    /// - `f` (F): Async function to run while the terminal is restored.
    ///
    /// # Returns
    /// - `R`: Result returned by the provided function.
    pub async fn with_restored<R, F, Fut>(&mut self, mode: RestoreMode, f: F) -> R
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = R>,
    {
        self.pause_events();

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
        flush_terminal_input_buffer();

        if was_alt_screen {
            let _ = self.enter_alt_screen();
        }

        self.resume_events();
        output
    }

    /// Emits a desktop notification if the terminal is unfocused.
    ///
    /// # Arguments
    /// - `message` (impl AsRef<str>): Notification text.
    ///
    /// # Returns
    /// - `bool`: True if a notification was posted.
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

    /// Returns the event stream for UI updates.
    ///
    /// # Returns
    /// - `Pin<Box<dyn Stream<Item = TuiEvent> + Send + 'static>>`: Event stream.
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

    /// Enters alternate screen and expands the viewport.
    ///
    /// # Returns
    /// - `Result<()>`: Result of the enter operation.
    pub fn enter_alt_screen(&mut self) -> Result<()> {
        if !self.alt_screen_enabled {
            return Ok(());
        }
        if self.terminal.backend().is_crossterm() {
            let _ = execute!(stdout(), EnterAlternateScreen);
            let _ = execute!(stdout(), EnableAlternateScroll);
        }
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

    /// Leaves alternate screen and restores the saved inline viewport.
    ///
    /// # Returns
    /// - `Result<()>`: Result of the leave operation.
    pub fn leave_alt_screen(&mut self) -> Result<()> {
        if !self.alt_screen_enabled {
            return Ok(());
        }
        if self.terminal.backend().is_crossterm() {
            let _ = execute!(stdout(), DisableAlternateScroll);
            let _ = execute!(stdout(), LeaveAlternateScreen);
        }
        if let Some(saved) = self.alt_saved_viewport.take() {
            self.terminal.set_viewport_area(saved);
        }
        self.alt_screen_active.store(false, Ordering::Relaxed);
        Ok(())
    }

    /// Queues history lines for insertion above the viewport.
    ///
    /// # Arguments
    /// - `lines` (Vec<Line>): Lines to insert.
    ///
    /// # Returns
    /// - `()`: No return value.
    pub fn insert_history_lines(&mut self, lines: Vec<Line>) {
        self.pending_history_lines.extend(lines);
        self.frame_requester().schedule_frame();
    }

    /// Draws a frame with the specified viewport height.
    ///
    /// # Arguments
    /// - `height` (u16): Desired viewport height.
    /// - `draw_fn` (impl FnOnce(&mut custom_terminal::Frame)): Render callback.
    ///
    /// # Returns
    /// - `Result<()>`: Result of the draw operation.
    pub fn draw(
        &mut self,
        height: u16,
        draw_fn: impl FnOnce(&mut custom_terminal::Frame),
    ) -> Result<()> {
        #[cfg(unix)]
        let mut prepared_resume = self
            .suspend_context
            .prepare_resume_action(&mut self.terminal, &mut self.alt_saved_viewport);

        let mut pending_viewport_area = self.pending_viewport_area()?;
        let mut draw_fn = Some(draw_fn);

        let mut run_draw = |terminal: &mut Terminal| -> Result<()> {
            #[cfg(unix)]
            if let Some(prepared) = prepared_resume.take() {
                prepared.apply(terminal)?;
            }

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
                )?;
                self.pending_history_lines.clear();
            }

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

            let draw_fn = draw_fn
                .take()
                .ok_or_else(|| std::io::Error::other("draw function already used"))?;
            terminal.draw(draw_fn)
        };

        if self.terminal.backend().is_crossterm() {
            stdout().sync_update(|_| run_draw(&mut self.terminal))??;
        } else {
            run_draw(&mut self.terminal)?;
        }
        Ok(())
    }

    /// Computes a new viewport area when the terminal size changes.
    ///
    /// # Returns
    /// - `Result<Option<Rect>>`: New viewport area, if any.
    fn pending_viewport_area(&mut self) -> Result<Option<Rect>> {
        let terminal = &mut self.terminal;
        let screen_size = terminal.size()?;
        let last_known_screen_size = terminal.last_known_screen_size;
        if screen_size != last_known_screen_size
            && let Ok(cursor_pos) = terminal.get_cursor_position()
        {
            let last_known_cursor_pos = terminal.last_known_cursor_pos;
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
