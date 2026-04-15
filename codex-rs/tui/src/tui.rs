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
use std::time::Duration;

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
use ratatui::layout::Size;
use ratatui::text::Line;
use tokio::sync::broadcast;
use tokio_stream::Stream;

pub use self::frame_requester::FrameRequester;
use crate::custom_terminal;
use crate::custom_terminal::Terminal as CustomTerminal;
use crate::notifications::DesktopNotificationBackend;
use crate::notifications::detect_backend;
use crate::tui::event_stream::EventBroker;
use crate::tui::event_stream::TuiEventStream;
#[cfg(unix)]
use crate::tui::job_control::SuspendContext;
use codex_config::types::NotificationCondition;
use codex_config::types::NotificationMethod;

mod event_stream;
mod frame_rate_limiter;
mod frame_requester;
#[cfg(unix)]
mod job_control;

/// Target frame interval for UI redraw scheduling.
pub(crate) const TARGET_FRAME_INTERVAL: Duration = frame_rate_limiter::MIN_FRAME_INTERVAL;

/// A type alias for the terminal type used in this application
pub type Terminal = CustomTerminal<CrosstermBackend<Stdout>>;

fn should_emit_notification(condition: NotificationCondition, terminal_focused: bool) -> bool {
    match condition {
        NotificationCondition::Unfocused => !terminal_focused,
        NotificationCondition::Always => true,
    }
}

#[cfg(test)]
mod tests {
    use super::should_emit_notification;
    use codex_config::types::NotificationCondition;

    #[test]
    fn unfocused_notification_condition_is_suppressed_when_focused() {
        assert!(!should_emit_notification(
            NotificationCondition::Unfocused,
            /*terminal_focused*/ true
        ));
    }

    #[test]
    fn always_notification_condition_emits_when_focused() {
        assert!(should_emit_notification(
            NotificationCondition::Always,
            /*terminal_focused*/ true
        ));
    }

    #[test]
    fn unfocused_notification_condition_emits_when_unfocused() {
        assert!(should_emit_notification(
            NotificationCondition::Unfocused,
            /*terminal_focused*/ false
        ));
    }
}

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
/// Inverse of `set_modes`.
pub fn restore() -> Result<()> {
    let should_disable_raw_mode = true;
    restore_common(should_disable_raw_mode)
}

/// Restore the terminal to its original state, but keep raw mode enabled.
pub fn restore_keep_raw() -> Result<()> {
    let should_disable_raw_mode = false;
    restore_common(should_disable_raw_mode)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreMode {
    #[allow(dead_code)]
    Full, // Fully restore the terminal (disables raw mode).
    KeepRaw, // Restore the terminal but keep raw mode enabled.
}

impl RestoreMode {
    fn restore(self) -> Result<()> {
        match self {
            RestoreMode::Full => restore(),
            RestoreMode::KeepRaw => restore_keep_raw(),
        }
    }
}

/// Flush the underlying stdin buffer to clear any input that may be buffered at the terminal level.
/// For example, clears any user input that occurred while the crossterm EventStream was dropped.
#[cfg(unix)]
fn flush_terminal_input_buffer() {
    // Safety: flushing the stdin queue is safe and does not move ownership.
    let result = unsafe { libc::tcflush(libc::STDIN_FILENO, libc::TCIFLUSH) };
    if result != 0 {
        let err = std::io::Error::last_os_error();
        tracing::warn!("failed to tcflush stdin: {err}");
    }
}

/// Flush the underlying stdin buffer to clear any input that may be buffered at the terminal level.
/// For example, clears any user input that occurred while the crossterm EventStream was dropped.
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
pub(crate) fn flush_terminal_input_buffer() {}

/// Initialize the terminal (inline viewport; history stays in normal scrollback)
pub fn init() -> Result<Terminal> {
    if !stdin().is_terminal() {
        return Err(std::io::Error::other("stdin is not a terminal"));
    }
    if !stdout().is_terminal() {
        return Err(std::io::Error::other("stdout is not a terminal"));
    }
    set_modes()?;

    flush_terminal_input_buffer();

    set_panic_hook();

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

/// Event consumed by the top-level TUI event loop.
///
/// Resize is intentionally modeled separately from [`TuiEvent::Draw`] because resize handling has to reconcile terminal geometry and cached viewport state before rendering the next frame.
#[derive(Clone, Debug)]
pub enum TuiEvent {
    /// Keyboard input delivered by crossterm.
    Key(KeyEvent),
    /// Bracketed paste payload delivered by crossterm.
    Paste(String),
    /// Terminal geometry changed or was inferred to have changed.
    ///
    /// Resize events should be rendered immediately and should not be downgraded to a scheduled draw. Some terminal hosts can report stale cursor positions during resize, so handlers use this variant to request a conservative repaint path.
    Resize,
    /// Scheduled repaint request from application state changes.
    Draw,
}

pub struct Tui {
    frame_requester: FrameRequester,
    draw_tx: broadcast::Sender<()>,
    event_broker: Arc<EventBroker>,
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
    notification_backend: Option<DesktopNotificationBackend>,
    notification_condition: NotificationCondition,
    is_zellij: bool,
    force_full_repaint: bool,
    // When false, enter_alt_screen() becomes a no-op (for Zellij scrollback support)
    alt_screen_enabled: bool,
}

impl Tui {
    pub fn new(terminal: Terminal) -> Self {
        let (draw_tx, _) = broadcast::channel(1);
        let frame_requester = FrameRequester::new(draw_tx.clone());

        // Detect keyboard enhancement support before any EventStream is created so the
        // crossterm poller can acquire its lock without contention.
        let enhanced_keys_supported = supports_keyboard_enhancement().unwrap_or(false);
        // Cache this to avoid contention with the event reader.
        supports_color::on_cached(supports_color::Stream::Stdout);
        let _ = crate::terminal_palette::default_colors();
        let terminal_info = codex_terminal_detection::terminal_info();
        let is_zellij = matches!(
            terminal_info.multiplexer,
            Some(codex_terminal_detection::Multiplexer::Zellij {})
        );

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
            notification_backend: Some(detect_backend(NotificationMethod::default())),
            notification_condition: NotificationCondition::default(),
            is_zellij,
            force_full_repaint: false,
            alt_screen_enabled: true,
        }
    }

    /// Set whether alternate screen is enabled. When false, enter_alt_screen() becomes a no-op.
    pub fn set_alt_screen_enabled(&mut self, enabled: bool) {
        self.alt_screen_enabled = enabled;
    }

    pub fn set_notification_settings(
        &mut self,
        method: NotificationMethod,
        condition: NotificationCondition,
    ) {
        self.notification_backend = Some(detect_backend(method));
        self.notification_condition = condition;
    }

    pub fn frame_requester(&self) -> FrameRequester {
        self.frame_requester.clone()
    }

    /// Requests that the next draw clear and invalidate the terminal viewport.
    ///
    /// This is used for resize handling when the cursor-position viewport heuristic is less trustworthy than the size event itself. It is a one-shot flag consumed by [`Tui::draw`]; callers should set it immediately before the draw that needs the conservative repaint.
    pub(crate) fn force_full_repaint(&mut self) {
        self.force_full_repaint = true;
    }

    pub fn enhanced_keys_supported(&self) -> bool {
        self.enhanced_keys_supported
    }

    pub fn is_alt_screen_active(&self) -> bool {
        self.alt_screen_active.load(Ordering::Relaxed)
    }

    // Drop crossterm EventStream to avoid stdin conflicts with other processes.
    pub fn pause_events(&mut self) {
        self.event_broker.pause_events();
    }

    // Resume crossterm EventStream to resume stdin polling.
    // Inverse of `pause_events`.
    pub fn resume_events(&mut self) {
        self.event_broker.resume_events();
    }

    /// Temporarily restore terminal state to run an external interactive program `f`.
    ///
    /// This pauses crossterm's stdin polling by dropping the underlying event stream, restores
    /// terminal modes (optionally keeping raw mode enabled), then re-applies Codex TUI modes and
    /// flushes pending stdin input before resuming events.
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
    /// Returns true if a notification was posted.
    pub fn notify(&mut self, message: impl AsRef<str>) -> bool {
        let terminal_focused = self.terminal_focused.load(Ordering::Relaxed);
        if !should_emit_notification(self.notification_condition, terminal_focused) {
            return false;
        }

        let Some(backend) = self.notification_backend.as_mut() else {
            return false;
        };

        let message = message.as_ref().to_string();
        match backend.notify(&message) {
            Ok(()) => true,
            Err(err) => {
                let method = backend.method();
                tracing::warn!(
                    error = %err,
                    method = %method,
                    "Failed to emit terminal notification; disabling future notifications"
                );
                self.notification_backend = None;
                false
            }
        }
    }

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

    /// Enter alternate screen and expand the viewport to full terminal size, saving the current
    /// inline viewport for restoration when leaving.
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

    pub fn insert_history_lines(&mut self, lines: Vec<Line<'static>>) {
        self.pending_history_lines.extend(lines);
        self.frame_requester().schedule_frame();
    }

    pub fn clear_pending_history_lines(&mut self) {
        self.pending_history_lines.clear();
    }

    /// Resize the inline viewport to `height` rows, scrolling content above it if
    /// the viewport would extend past the bottom of the screen. Returns `true` when
    /// the caller must invalidate the diff buffer (Zellij mode), because the scroll
    /// was performed with raw newlines that ratatui cannot track.
    fn update_inline_viewport(
        terminal: &mut Terminal,
        height: u16,
        is_zellij: bool,
    ) -> Result<bool> {
        let size = terminal.size()?;
        let mut needs_full_repaint = false;

        let mut area = terminal.viewport_area;
        area.height = height.min(size.height);
        area.width = size.width;
        if area.bottom() > size.height {
            let scroll_by = area.bottom() - size.height;
            if is_zellij {
                Self::scroll_zellij_expanded_viewport(terminal, size, scroll_by)?;
                needs_full_repaint = true;
            } else {
                terminal
                    .backend_mut()
                    .scroll_region_up(0..area.top(), scroll_by)?;
            }
            area.y = size.height - area.height;
        }
        if area != terminal.viewport_area {
            // TODO(nornagon): probably this could be collapsed with the clear + set_viewport_area above.
            terminal.clear()?;
            terminal.set_viewport_area(area);
        }

        Ok(needs_full_repaint)
    }

    /// Push content above the viewport upward by `scroll_by` rows using raw
    /// newlines at the screen bottom. This is the Zellij-safe alternative to
    /// `scroll_region_up`, which relies on DECSTBM sequences Zellij does not
    /// support.
    fn scroll_zellij_expanded_viewport(
        terminal: &mut Terminal,
        size: Size,
        scroll_by: u16,
    ) -> Result<()> {
        crossterm::queue!(
            terminal.backend_mut(),
            crossterm::cursor::MoveTo(0, size.height.saturating_sub(1))
        )?;
        for _ in 0..scroll_by {
            crossterm::queue!(terminal.backend_mut(), crossterm::style::Print("\n"))?;
        }
        Ok(())
    }

    /// Write any buffered history lines above the viewport and clear the buffer.
    /// Returns `true` when Zellij mode was used, signaling that the caller must
    /// invalidate the diff buffer for a full repaint.
    fn flush_pending_history_lines(
        terminal: &mut Terminal,
        pending_history_lines: &mut Vec<Line<'static>>,
        is_zellij: bool,
    ) -> Result<bool> {
        if pending_history_lines.is_empty() {
            return Ok(false);
        }

        crate::insert_history::insert_history_lines_with_mode(
            terminal,
            pending_history_lines.clone(),
            crate::insert_history::InsertHistoryMode::new(is_zellij),
        )?;
        pending_history_lines.clear();
        Ok(is_zellij)
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

        let force_full_repaint = self.force_full_repaint;
        self.force_full_repaint = false;

        // Precompute any viewport updates that need a cursor-position query before entering
        // the synchronized update, to avoid racing with the event reader. Forced repaint
        // callers skip this heuristic because some terminal hosts can report stale cursor
        // positions while a blurred split pane is being resized rapidly.
        let mut pending_viewport_area = if force_full_repaint {
            None
        } else {
            self.pending_viewport_area()?
        };

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

            let mut needs_full_repaint =
                Self::update_inline_viewport(terminal, height, self.is_zellij)?;
            needs_full_repaint |= Self::flush_pending_history_lines(
                terminal,
                &mut self.pending_history_lines,
                self.is_zellij,
            )?;

            if force_full_repaint {
                terminal.clear()?;
                needs_full_repaint = true;
            }

            if needs_full_repaint {
                terminal.invalidate_viewport();
            }

            // Update the y position for suspending so Ctrl-Z can place the cursor correctly.
            #[cfg(unix)]
            {
                let area = terminal.viewport_area;
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
