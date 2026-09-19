use std::fmt;
use std::future::Future;
use std::io;
use std::io::IsTerminal;
use std::io::Result;
use std::io::Stdout;
use std::io::Write;
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
use crossterm::cursor::SetCursorStyle;
use crossterm::event::DisableBracketedPaste;
use crossterm::event::DisableFocusChange;
use crossterm::event::EnableBracketedPaste;
#[cfg(not(windows))]
use crossterm::event::EnableFocusChange;
use crossterm::event::KeyEvent;
use crossterm::event::MouseEvent;
#[cfg(not(unix))]
use crossterm::terminal::supports_keyboard_enhancement;
use ratatui::backend::Backend;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::disable_raw_mode;
use ratatui::crossterm::terminal::enable_raw_mode;
use ratatui::layout::Offset;
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::layout::Size;
use ratatui::text::Line;
use tokio::sync::broadcast;
use tokio_stream::Stream;

pub use self::frame_requester::FrameRequester;
use self::input_boundary::TerminalInitializationGuard;
pub(crate) use self::input_boundary::discard_pending_terminal_input;
#[cfg(all(test, unix))]
use self::input_boundary::terminal_input_is_readable;
use crate::custom_terminal;
use crate::custom_terminal::Terminal as CustomTerminal;
use crate::insert_history::HistoryLineWrapPolicy;
use crate::notifications::DesktopNotificationBackend;
use crate::notifications::detect_backend;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::plain_hyperlink_lines;
use crate::tui::alternate_screen::ALTERNATE_SCREEN;
use crate::tui::event_stream::EventBroker;
use crate::tui::event_stream::TuiEventStream;
#[cfg(unix)]
use crate::tui::job_control::SuspendContext;
use crate::tui::screen_size::ScreenSizePolicy;
use crate::tui::scrollback::ScrollbackStrategy;
use codex_config::types::NotificationCondition;
use codex_config::types::NotificationMethod;

mod alternate_screen;
mod event_stream;
mod frame_rate_limiter;
mod frame_requester;
mod history_tail;
mod input_boundary;
#[cfg(unix)]
mod job_control;
mod keyboard_modes;
#[cfg(test)]
#[path = "tui/owned_screen_tests.rs"]
mod owned_screen_tests;
#[cfg(all(test, unix))]
#[path = "tui_panic_tests.rs"]
mod panic_tests;
mod screen_size;
mod scrollback;
mod selection_clipboard;
mod size_monitor;
#[cfg(all(test, unix))]
#[path = "tui_startup_tests.rs"]
mod startup_tests;
mod terminal_stderr;
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(any(windows, test))]
mod windows_console;

/// Target frame interval for UI redraw scheduling.
pub(crate) const TARGET_FRAME_INTERVAL: Duration = frame_rate_limiter::MIN_FRAME_INTERVAL;

/// A type alias for the terminal type used in this application
pub type Terminal = CustomTerminal<CrosstermBackend<Stdout>>;

pub(crate) struct InitializedTerminal {
    pub(crate) terminal: Terminal,
    pub(crate) enhanced_keys_supported: bool,
    pub(crate) stderr_guard: terminal_stderr::TerminalStderrGuard,
}

pub(crate) fn running_in_vscode_terminal() -> bool {
    keyboard_modes::running_in_vscode_terminal()
}

fn should_emit_notification(condition: NotificationCondition, terminal_focused: bool) -> bool {
    match condition {
        NotificationCondition::Unfocused => !terminal_focused,
        NotificationCondition::Always => true,
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        if let Err(err) = self.clear_ambient_pet_image() {
            tracing::debug!(error = %err, "failed to clear ambient pet image on TUI drop");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use super::clear_for_viewport_change;
    use super::should_emit_notification;
    use crate::custom_terminal::Terminal as CustomTerminal;
    use crate::test_backend::VT100Backend;
    use codex_config::types::NotificationCondition;
    use ratatui::layout::Position;
    use ratatui::layout::Rect;
    use ratatui::text::Line;

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
    fn windows_console_input_modes_preserve_original_vt_input_state() {
        let input_record_mode = super::windows_console::input_record_mode(/*mode*/ 0x398);
        assert_eq!(input_record_mode, 0x198);
        assert_eq!(
            super::windows_console::restored_input_mode(
                input_record_mode,
                super::windows_console::VirtualTerminalInput::Enabled,
            ),
            0x398
        );
        assert_eq!(
            super::windows_console::restored_input_mode(
                /*mode*/ 0x198,
                super::windows_console::VirtualTerminalInput::Disabled,
            ),
            0x198
        );
    }

    #[test]
    fn unfocused_notification_condition_emits_when_unfocused() {
        assert!(should_emit_notification(
            NotificationCondition::Unfocused,
            /*terminal_focused*/ false
        ));
    }

    #[test]
    fn first_viewport_change_clears_from_new_viewport_when_old_viewport_is_empty() {
        let width = 12;
        let height = 4;
        let backend = VT100Backend::new(width, height);
        let mut terminal =
            CustomTerminal::with_options_and_cursor_position(backend, Position { x: 0, y: 1 })
                .expect("terminal");
        write!(
            terminal.backend_mut(),
            "shell line\r\nstale cells\r\nmore stale"
        )
        .expect("prefill terminal");

        clear_for_viewport_change(
            &mut terminal,
            Rect::new(
                /*x*/ 0,
                /*y*/ 1,
                /*width*/ width,
                /*height*/ height - 1,
            ),
        )
        .expect("clear transition");

        let rows: Vec<String> = terminal
            .backend()
            .vt100()
            .screen()
            .rows(/*start*/ 0, width)
            .collect();
        assert!(
            rows[0].contains("shell line"),
            "expected content before the viewport to remain visible, rows: {rows:?}"
        );
        assert!(
            !rows.iter().skip(1).any(|row| row.contains("stale")),
            "expected stale cells inside the new viewport to be cleared, rows: {rows:?}"
        );
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn inserting_history_lines_schedules_a_draw() {
        let mut tui = crate::tui::test_support::make_test_tui().expect("test tui");
        let mut draw_rx = tui.draw_tx.subscribe();

        tui.insert_history_lines(vec![Line::from("committed stream line")]);
        tokio::time::advance(std::time::Duration::from_millis(20)).await;

        assert!(matches!(
            tokio::time::timeout(std::time::Duration::from_millis(50), draw_rx.recv()).await,
            Ok(Ok(()))
        ));
    }
}

pub fn set_modes() -> Result<()> {
    ensure_virtual_terminal_processing()?;

    execute!(stdout(), EnableBracketedPaste)?;

    enable_raw_mode()?;
    #[cfg(windows)]
    windows_console::set_input_record_mode()?;
    // Enable keyboard enhancement flags so modifiers for keys like Enter are disambiguated.
    // chat_composer.rs is using a keyboard event listener to enter for any modified keys
    // to create a new line that require this.
    // Some terminals (notably legacy Windows consoles) do not support
    // keyboard enhancement flags. Attempt to enable them, but continue
    // gracefully if unsupported.
    keyboard_modes::enable_keyboard_enhancement(&mut stdout());

    #[cfg(not(windows))]
    let _ = execute!(stdout(), EnableFocusChange);
    #[cfg(windows)]
    let _ = execute!(stdout(), DisableFocusChange);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RawModeRestore {
    Disable,
    Keep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyboardRestore {
    PopStack,
    ResetAfterExit,
}

fn restore_common(
    raw_mode_restore: RawModeRestore,
    keyboard_restore: KeyboardRestore,
) -> Result<()> {
    let mut first_error = ensure_virtual_terminal_processing().err();

    if let Err(err) = ALTERNATE_SCREEN.restore(&mut stdout(), keyboard_restore) {
        first_error.get_or_insert(err);
    }

    if let Err(err) = execute!(stdout(), DisableBracketedPaste) {
        first_error.get_or_insert(err);
    }
    let _ = execute!(stdout(), DisableFocusChange);
    if matches!(raw_mode_restore, RawModeRestore::Disable)
        && let Err(err) = disable_raw_mode()
    {
        first_error.get_or_insert(err);
    }
    #[cfg(windows)]
    if let Err(err) = windows_console::restore_input_mode() {
        first_error.get_or_insert(err);
    }
    if let Err(err) = execute!(
        stdout(),
        SetCursorStyle::DefaultUserShape,
        crossterm::cursor::Show
    ) {
        first_error.get_or_insert(err);
    }
    match first_error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

/// Restore the terminal to its original state.
/// Inverse of `set_modes`.
#[cfg(unix)]
pub fn restore() -> Result<()> {
    restore_common(RawModeRestore::Disable, KeyboardRestore::PopStack)
}

/// Force crossterm's cached raw-mode state back in sync with the terminal after `fg`.
///
/// A shell may restore the job's saved termios after the process receives `SIGCONT`. When that
/// races with [`set_modes`], crossterm still believes raw mode is enabled even though the terminal
/// has returned to canonical, echoing mode. Clearing crossterm's saved state before enabling raw
/// mode again makes the kernel state authoritative once the shell has completed its handoff.
#[cfg(unix)]
pub(super) fn reapply_raw_mode_after_resume() -> Result<()> {
    disable_raw_mode()?;
    enable_raw_mode()
}

/// Restore the terminal after Codex is exiting.
///
/// Uses a stronger keyboard reset than `restore` so the parent shell recovers even if a
/// terminal missed the stack pop that normally pairs with [`set_modes`].
pub fn restore_after_exit() -> Result<()> {
    let mut first_error =
        restore_common(RawModeRestore::Disable, KeyboardRestore::ResetAfterExit).err();
    if let Err(err) = terminal_stderr::finish() {
        first_error.get_or_insert(err);
    }

    match first_error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

/// Restore the terminal to its original state, but keep raw mode enabled.
pub fn restore_keep_raw() -> Result<()> {
    restore_common(RawModeRestore::Keep, KeyboardRestore::PopStack)
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
pub(crate) fn init() -> Result<InitializedTerminal> {
    if !stdin().is_terminal() {
        return Err(std::io::Error::other("stdin is not a terminal"));
    }
    if !stdout().is_terminal() {
        return Err(std::io::Error::other("stdout is not a terminal"));
    }
    let mut restore_guard = TerminalInitializationGuard { active: true };
    set_modes()?;

    set_panic_hook();

    #[cfg(unix)]
    let backend = CrosstermBackend::new(stdout());

    #[cfg(unix)]
    let startup_probe = {
        use crate::terminal_probe::StartupKeyboardEnhancementProbe;

        let started_at = std::time::Instant::now();
        let keyboard_probe = if keyboard_modes::keyboard_enhancement_disabled() {
            StartupKeyboardEnhancementProbe::Skip
        } else {
            StartupKeyboardEnhancementProbe::Query
        };
        match crate::terminal_probe::startup(crate::terminal_probe::DEFAULT_TIMEOUT, keyboard_probe)
        {
            Ok(probe) => {
                tracing::info!(
                    duration_ms = %started_at.elapsed().as_millis(),
                    cursor_position = probe.cursor_position.is_some(),
                    default_colors = probe.default_colors.is_some(),
                    keyboard_enhancement_supported = ?probe.keyboard_enhancement_supported,
                    "terminal startup probes completed"
                );
                probe
            }
            Err(err) => {
                tracing::warn!(
                    duration_ms = %started_at.elapsed().as_millis(),
                    "terminal startup probes failed: {err}"
                );
                crate::terminal_probe::StartupProbe {
                    cursor_position: None,
                    default_colors: None,
                    keyboard_enhancement_supported: None,
                }
            }
        }
    };

    #[cfg(unix)]
    crate::terminal_palette::set_default_colors_from_startup_probe(startup_probe.default_colors);

    #[cfg(unix)]
    let cursor_pos = match startup_probe.cursor_position {
        Some(pos) => pos,
        None => {
            tracing::warn!("initial cursor position probe timed out; defaulting to origin");
            Position { x: 0, y: 0 }
        }
    };

    #[cfg(unix)]
    let enhanced_keys_supported = startup_probe
        .keyboard_enhancement_supported
        .unwrap_or(/*default*/ false);

    #[cfg(not(unix))]
    let mut backend = CrosstermBackend::new(stdout());

    #[cfg(not(unix))]
    let cursor_pos = cursor_position_with_crossterm(&mut backend);

    #[cfg(not(unix))]
    let enhanced_keys_supported =
        !keyboard_modes::keyboard_enhancement_disabled() && detect_keyboard_enhancement_supported();

    #[cfg(windows)]
    // OSC replies can arrive after their deadline. Do not issue terminal queries before directory
    // trust and other protected startup screens have finished accepting their security decisions.
    crate::terminal_palette::set_default_colors_from_startup_probe(/*colors*/ None);

    let tui = CustomTerminal::with_options_and_cursor_position(backend, cursor_pos)?;
    let stderr_guard = terminal_stderr::TerminalStderrGuard::install()?;
    let initialized_terminal = InitializedTerminal {
        terminal: tui,
        enhanced_keys_supported,
        stderr_guard,
    };
    restore_guard.active = false;
    Ok(initialized_terminal)
}

#[cfg(not(unix))]
fn cursor_position_with_crossterm(backend: &mut CrosstermBackend<Stdout>) -> Position {
    backend.get_cursor_position().unwrap_or_else(|err| {
        tracing::warn!("failed to read initial cursor position; defaulting to origin: {err}");
        Position { x: 0, y: 0 }
    })
}

#[cfg(not(unix))]
fn detect_keyboard_enhancement_supported() -> bool {
    // Non-Unix startup keeps the existing crossterm keyboard probe path because it already knows
    // how to interpret platform-specific event sources.
    supports_keyboard_enhancement().unwrap_or(/*default*/ false)
}

#[cfg(windows)]
fn probe_windows_default_colors() {
    let started_at = std::time::Instant::now();
    match crate::terminal_probe::default_colors(crate::terminal_probe::DEFAULT_TIMEOUT) {
        Ok(colors) => {
            tracing::info!(
                duration_ms = %started_at.elapsed().as_millis(),
                default_colors = colors.is_some(),
                "terminal default color probe completed"
            );
            crate::terminal_palette::set_default_colors_from_startup_probe(colors);
        }
        Err(err) => {
            tracing::warn!(
                duration_ms = %started_at.elapsed().as_millis(),
                "terminal default color probe failed: {err}"
            );
            crate::terminal_palette::set_default_colors_from_startup_probe(/*colors*/ None);
        }
    }
}

fn set_panic_hook() {
    let hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = restore_after_exit(); // ignore any errors as we are already failing
        hook(panic_info);
    }));
}

#[derive(Clone, Debug)]
pub enum TuiEvent {
    /// A terminal key event after focus, paste, and protocol bookkeeping has been handled.
    Key(KeyEvent),
    /// A bracketed paste payload normalized by the app layer before it reaches the composer.
    Paste(String),
    /// A terminal mouse event for pointer interactions.
    Mouse(MouseEvent),
    /// A terminal size notification and its reported dimensions.
    ///
    /// Resize is separate from `Draw` so the app can run feature-gated pre-render logic without
    /// changing the default draw path for scheduled frames.
    Resize(Size),
    /// A scheduled repaint that does not necessarily correspond to a terminal size change.
    Draw,
    /// The first repaint after returning from process suspension.
    ///
    /// The app refreshes terminal geometry for this draw because resize events are not delivered
    /// while the process is suspended.
    Resume,
    /// A terminal focus notification indicating that the terminal or tab became active.
    FocusGained,
    /// A terminal focus notification indicating that the terminal or tab became inactive.
    FocusLost,
}

/// The overlay requesting pointer reports; ordinary pickers retain alternate-scroll input.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum OverlayInput {
    #[default]
    Default,
    Transcript,
    StaticPager,
    #[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
    Usage,
}

impl OverlayInput {
    fn captures_mouse(self, owned_screen: bool) -> bool {
        match self {
            Self::Default => owned_screen,
            Self::StaticPager => false,
            Self::Transcript | Self::Usage => true,
        }
    }
}

pub struct Tui {
    frame_requester: FrameRequester,
    draw_tx: broadcast::Sender<()>,
    event_broker: Arc<EventBroker>,
    pub(crate) terminal: Terminal,
    pending_history_lines: Vec<PendingHistoryLines>,
    screen_size: ScreenSizePolicy,
    ambient_pet_image_state: crate::pets::PetImageRenderState,
    pet_picker_preview_image_state: crate::pets::PetImageRenderState,
    alt_saved_viewport: Option<ratatui::layout::Rect>,
    #[cfg(unix)]
    suspend_context: SuspendContext,
    // True when an overlay or the owned transcript uses the alternate screen.
    alt_screen_active: Arc<AtomicBool>,
    // True when terminal/tab is focused; updated internally from crossterm events
    terminal_focused: Arc<AtomicBool>,
    enhanced_keys_supported: bool,
    notification_backend: Option<DesktopNotificationBackend>,
    notification_condition: NotificationCondition,
    scrollback: ScrollbackStrategy,
    // When false, overlays stay on the inline screen.
    alt_screen_enabled: bool,
    // Keep the alternate screen alive when an overlay closes.
    owned_screen: bool,
    overlay_input: OverlayInput,
    // Selection copies survive closing a transcript overlay or startup session picker.
    selection_clipboard_lease: Option<crate::clipboard_copy::ClipboardLease>,
    // Keeps unmanaged process stderr writes out of the inline viewport.
    _stderr_guard: terminal_stderr::TerminalStderrGuard,
}

struct PendingHistoryLines {
    lines: Vec<HyperlinkLine>,
    wrap_policy: HistoryLineWrapPolicy,
}

fn clear_for_viewport_change<B>(terminal: &mut CustomTerminal<B>, new_area: Rect) -> Result<()>
where
    B: Backend<Error = io::Error> + Write,
{
    let clear_position = if terminal.viewport_area.is_empty() {
        new_area.as_position()
    } else {
        terminal.viewport_area.as_position()
    };
    terminal.clear_after_position(clear_position)
}

impl Tui {
    pub(crate) fn is_alt_screen_enabled(&self) -> bool {
        self.alt_screen_enabled
    }

    pub(crate) fn new(
        terminal: Terminal,
        enhanced_keys_supported: bool,
        stderr_guard: terminal_stderr::TerminalStderrGuard,
    ) -> Self {
        let (draw_tx, _) = broadcast::channel(1);
        let frame_requester = FrameRequester::new(draw_tx.clone());

        // Cache this to avoid contention with the event reader.
        supports_color::on_cached(supports_color::Stream::Stdout);
        let _ = crate::terminal_palette::default_colors();
        let terminal_info = codex_terminal_detection::terminal_info();
        let scrollback = ScrollbackStrategy::detect(&terminal_info);
        let mut event_broker = EventBroker::new();
        event_broker.size_monitor = size_monitor::SizeMonitor::start(
            &terminal_info,
            terminal.last_known_screen_size,
            draw_tx.clone(),
        );

        Self {
            frame_requester,
            draw_tx,
            event_broker: Arc::new(event_broker),
            terminal,
            pending_history_lines: vec![],
            screen_size: ScreenSizePolicy::default(),
            ambient_pet_image_state: crate::pets::PetImageRenderState::default(),
            pet_picker_preview_image_state: crate::pets::PetImageRenderState::default(),
            alt_saved_viewport: None,
            #[cfg(unix)]
            suspend_context: SuspendContext::new(),
            alt_screen_active: Arc::new(AtomicBool::new(false)),
            terminal_focused: Arc::new(AtomicBool::new(true)),
            enhanced_keys_supported,
            notification_backend: Some(detect_backend(NotificationMethod::default())),
            notification_condition: NotificationCondition::default(),
            scrollback,
            alt_screen_enabled: true,
            owned_screen: false,
            overlay_input: OverlayInput::Default,
            selection_clipboard_lease: None,
            _stderr_guard: stderr_guard,
        }
    }

    /// Set whether overlays switch to the alternate screen or stay inline.
    pub fn set_alt_screen_enabled(&mut self, enabled: bool) {
        if !enabled && self.owned_screen {
            let _ = self.set_owned_screen(/*owned*/ false);
        }
        self.alt_screen_enabled = enabled;
    }

    /// Defer a fresh fullscreen launch until its first synchronized frame is ready.
    pub(crate) fn prepare_owned_screen(&mut self, owned: bool) -> Result<()> {
        if owned && self.alt_screen_enabled && !self.is_alt_screen_active() {
            self.owned_screen = true;
            self.frame_requester.schedule_frame();
            Ok(())
        } else {
            self.set_owned_screen(owned)
        }
    }

    /// Own the terminal for the session, unless alternate-screen rendering is disabled.
    ///
    /// Closing an overlay retains the screen; external programs and suspension temporarily
    /// restore the shell. Startup resolves ownership before the conversation begins.
    pub fn set_owned_screen(&mut self, owned: bool) -> Result<()> {
        let owned = owned && self.alt_screen_enabled;
        if self.owned_screen == owned {
            return Ok(());
        }
        let transition = if owned {
            self.owned_screen = true;
            if self.is_alt_screen_active() {
                ALTERNATE_SCREEN
                    .configure_input(self.terminal.backend_mut(), /*capture_mouse*/ true)
            } else {
                self.enter_alt_screen()
            }
        } else {
            self.leave_alt_screen_for_handoff()
        };
        if let Err(error) = transition {
            self.owned_screen = self.is_alt_screen_active();
            self.terminal.invalidate_viewport();
            return Err(error);
        }
        self.owned_screen = owned;
        self.frame_requester.schedule_frame();
        Ok(())
    }

    /// Whether the session owns the screen, including while temporarily yielding to an editor.
    pub fn is_owned_screen(&self) -> bool {
        self.transcript_mode().is_owned()
    }

    pub(crate) fn transcript_mode(&self) -> crate::transcript_mode::TranscriptMode {
        crate::transcript_mode::TranscriptMode::resolve(self.owned_screen, self.alt_screen_enabled)
    }

    /// Retain the overlay input request across temporary editor, suspend and panic handoffs.
    /// Actual terminal capture is tracked by AlternateScreen so partial writes remain cleanable.
    pub(crate) fn set_overlay_input(&mut self, input: OverlayInput) -> Result<()> {
        if self.alt_screen_enabled && self.is_alt_screen_active() {
            self.overlay_input.apply(
                &ALTERNATE_SCREEN,
                self.terminal.backend_mut(),
                input,
                self.owned_screen,
            )
        } else {
            self.overlay_input = input;
            Ok(())
        }
    }

    pub fn set_notification_settings(
        &mut self,
        method: NotificationMethod,
        condition: NotificationCondition,
    ) {
        self.notification_backend = Some(detect_backend(method));
        self.notification_condition = condition;
    }

    pub(crate) fn is_terminal_focused(&self) -> bool {
        self.terminal_focused.load(Ordering::Relaxed)
    }

    pub fn frame_requester(&self) -> FrameRequester {
        self.frame_requester.clone()
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

    /// Discover the visible Windows theme only after protected startup decisions have completed.
    #[cfg(windows)]
    pub(crate) fn probe_default_colors_after_protected_startup(&mut self) {
        self.pause_events();
        probe_windows_default_colors();
        self.resume_events();
        self.frame_requester.schedule_frame();
    }

    /// Reclaim terminal modes and stderr after a panic hook ran inside a recovery boundary.
    pub(crate) fn recover_after_caught_panic(&mut self) -> Result<()> {
        set_modes()?;
        self._stderr_guard.recover_after_caught_panic()?;
        self.terminal.invalidate_cursor_state();
        if self.is_owned_screen() || self.overlay_input != OverlayInput::Default {
            let saved = self.alt_saved_viewport;
            self.alt_screen_active
                .store(/*val*/ false, Ordering::Relaxed);
            if let Some(saved) = saved {
                // The panic hook returned to the main screen; clear only its inline viewport.
                self.terminal.set_viewport_area(saved);
            }
            self.terminal.hide_cursor()?;
            self.enter_alt_screen()?;
            self.alt_saved_viewport = saved.or(self.alt_saved_viewport);
        }
        self.terminal.invalidate_viewport();
        self.frame_requester().schedule_frame();
        Ok(())
    }

    /// Discard buffered typeahead before a startup screen that can confirm an action.
    ///
    /// Startup probes can leave parsed key events in crossterm's queue, while later bootstrap
    /// work can leave additional bytes in the terminal input buffer. Neither should activate an
    /// update, trust, or migration prompt before the user has seen it. Pause the event stream,
    /// drain all input through crossterm so incomplete bracketed paste remains safely framed.
    pub(crate) fn discard_pending_input_before_interactive_screen(&mut self) -> Result<()> {
        self.pause_events();
        let drain_result = discard_pending_terminal_input();
        self.resume_events();
        drain_result
    }

    /// Temporarily restore terminal state to run an external interactive program `f`.
    ///
    /// This pauses crossterm's stdin polling by dropping the underlying event stream, restores
    /// terminal modes and stderr while keeping raw mode enabled, then re-applies Codex TUI modes
    /// and stderr suppression before resuming events.
    pub async fn with_restored<R, F, Fut>(&mut self, f: F) -> R
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = R>,
    {
        // Pause crossterm events to avoid stdin conflicts with external program `f`.
        self.pause_events();

        // Leave alt screen if active to avoid conflicts with external program `f`.
        let was_alt_screen = self.is_alt_screen_active();
        if was_alt_screen {
            let _ = self.leave_alt_screen_for_handoff();
        }

        if let Err(err) = restore_keep_raw() {
            tracing::warn!("failed to restore terminal modes before external program: {err}");
        }
        if let Err(err) = terminal_stderr::pause() {
            tracing::warn!("failed to restore terminal stderr before external program: {err}");
        }

        let output = f().await;

        if let Err(err) = terminal_stderr::resume() {
            tracing::warn!("failed to suppress terminal stderr after external program: {err}");
        }
        if let Err(err) = set_modes() {
            tracing::warn!("failed to re-enable terminal modes after external program: {err}");
        }
        self.terminal.invalidate_cursor_state();
        // After the external program `f` finishes, reset terminal state and flush any buffered keypresses.
        flush_terminal_input_buffer();

        if was_alt_screen {
            let _ = self.enter_alt_screen();
        }

        self.resume_events();
        self.schedule_screen_size_recheck(Duration::ZERO);
        output
    }

    /// Emit a desktop notification now if the terminal is unfocused.
    /// Returns true if a notification was posted.
    pub fn notify(&mut self, message: impl AsRef<str>) -> bool {
        let terminal_focused = self.is_terminal_focused();
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
        if self.is_alt_screen_active() {
            return Ok(());
        }
        // History queued before opening an overlay belongs to the inline transcript.
        // Flush before switching screens or expanding an inline overlay's viewport.
        let screen_size = self.terminal.last_known_screen_size;
        Self::flush_pending_history_lines(
            &mut self.terminal,
            &mut self.pending_history_lines,
            self.scrollback,
            screen_size,
        )?;
        if !self.alt_screen_enabled {
            return Ok(());
        }
        // Entry can change buffers before later input-mode configuration fails.
        self.terminal.invalidate_cursor_state();
        if self.owned_screen {
            // Returning to the shell must not resurrect an editable inline composer.
            self.terminal.hide_cursor()?;
            self.terminal.clear()?;
        }
        let result = ALTERNATE_SCREEN.enter(
            self.terminal.backend_mut(),
            self.overlay_input.captures_mouse(self.owned_screen),
        );
        self.alt_screen_active
            .store(ALTERNATE_SCREEN.is_active(), Ordering::Relaxed);
        if !self.is_alt_screen_active() {
            return result;
        }
        if self.owned_screen {
            // A newly entered screen may have its own visible cursor state.
            self.terminal.hide_cursor()?;
        }
        if let Ok(size) = self.terminal.size() {
            self.alt_saved_viewport = Some(self.terminal.viewport_area);
            self.terminal.resize(size)?;
            self.terminal.set_viewport_area(ratatui::layout::Rect::new(
                /*x*/ 0,
                /*y*/ 0,
                size.width,
                size.height,
            ));
            let _ = self.terminal.clear();
        }
        result
    }

    /// Close an overlay, preserving the alternate screen when the session owns it.
    pub fn leave_alt_screen(&mut self) -> Result<()> {
        let input_result = self.set_overlay_input(OverlayInput::Default);
        if self.is_owned_screen() {
            self.terminal.invalidate_viewport();
            return input_result;
        }
        let leave_result = self.leave_alt_screen_for_handoff();
        input_result.and(leave_result)
    }

    /// Yield to the shell even when the session owns the alternate screen.
    fn leave_alt_screen_for_handoff(&mut self) -> Result<()> {
        if !self.alt_screen_enabled || !self.is_alt_screen_active() {
            return Ok(());
        }
        let result = ALTERNATE_SCREEN.leave(self.terminal.backend_mut());
        if ALTERNATE_SCREEN.is_active() {
            return result;
        }
        self.terminal.invalidate_cursor_state();
        if let Some(saved) = self.alt_saved_viewport.take() {
            self.terminal.set_viewport_area(saved);
        }
        // The restored main screen does not contain the alternate screen's diff baseline.
        self.terminal.invalidate_viewport();
        self.alt_screen_active.store(false, Ordering::Relaxed);
        result
    }

    pub fn insert_history_lines(&mut self, lines: Vec<Line<'static>>) {
        self.insert_history_lines_with_wrap_policy(lines, HistoryLineWrapPolicy::PreWrap);
    }

    pub fn insert_history_lines_with_wrap_policy(
        &mut self,
        lines: Vec<Line<'static>>,
        wrap_policy: HistoryLineWrapPolicy,
    ) {
        self.insert_history_hyperlink_lines_with_wrap_policy(
            plain_hyperlink_lines(lines),
            wrap_policy,
        );
    }

    pub(crate) fn insert_history_hyperlink_lines_with_wrap_policy(
        &mut self,
        lines: Vec<HyperlinkLine>,
        wrap_policy: HistoryLineWrapPolicy,
    ) {
        if lines.is_empty() {
            return;
        }
        if let Some(last) = self.pending_history_lines.last_mut()
            && last.wrap_policy == wrap_policy
        {
            last.lines.extend(lines);
        } else {
            self.pending_history_lines
                .push(PendingHistoryLines { lines, wrap_policy });
        }
        self.frame_requester().schedule_frame();
    }

    pub fn clear_pending_history_lines(&mut self) {
        self.pending_history_lines.clear();
    }

    /// Resize the inline viewport for the resize-reflow path.
    ///
    /// Unlike the legacy draw path, this path does not scroll rows above the viewport when the
    /// terminal shrinks. Resize reflow owns rebuilding those rows from transcript source, so
    /// scrolling here would move the viewport once and then replay history into the wrong row.
    fn update_inline_viewport_for_resize_reflow(
        terminal: &mut Terminal,
        height: u16,
        screen_size: Size,
        scrollback: ScrollbackStrategy,
    ) -> Result<bool> {
        let terminal_height_shrank = screen_size.height < terminal.last_known_screen_size.height;
        let terminal_height_grew = screen_size.height > terminal.last_known_screen_size.height;
        let viewport_was_bottom_aligned =
            terminal.viewport_area.bottom() == terminal.last_known_screen_size.height;
        let previous_area = terminal.viewport_area;

        let mut area = terminal.viewport_area;
        area.height = height.min(screen_size.height);
        area.width = screen_size.width;
        let mut needs_full_repaint = false;

        if area.bottom() > screen_size.height {
            let scroll_by = area.bottom() - screen_size.height;
            if !terminal_height_shrank {
                scrollback.grow_viewport(terminal, area.top(), screen_size, scroll_by)?;
            }
            area.y = screen_size.height - area.height;
        } else if terminal_height_grew && viewport_was_bottom_aligned {
            area.y = screen_size.height - area.height;
        }

        if area != terminal.viewport_area {
            let clear_position = Position::new(/*x*/ 0, previous_area.y.min(area.y));
            terminal.set_viewport_area(area);
            terminal.clear_after_position(clear_position)?;
            needs_full_repaint = true;
        }

        Ok(needs_full_repaint)
    }

    /// Write any buffered history lines above the viewport and clear the buffer.
    fn flush_pending_history_lines(
        terminal: &mut Terminal,
        pending_history_lines: &mut Vec<PendingHistoryLines>,
        scrollback: ScrollbackStrategy,
        screen_size: Size,
    ) -> Result<()> {
        if pending_history_lines.is_empty() {
            return Ok(());
        }

        // A failed write can already have emitted part of this batch. Never retry that batch
        // automatically: replaying it would duplicate the successful prefix. Remaining batches
        // stay queued, and the caller reports the I/O error.
        while !pending_history_lines.is_empty() {
            let batch = pending_history_lines.remove(/*index*/ 0);
            let mode = scrollback.history_insertion_mode(batch.wrap_policy);
            crate::insert_history::insert_history_hyperlink_lines_with_mode_and_wrap_policy(
                terminal,
                &batch.lines,
                mode,
                batch.wrap_policy,
                screen_size,
            )?;
        }
        Ok(())
    }

    pub fn draw(
        &mut self,
        height: u16,
        draw_fn: impl FnOnce(&mut custom_terminal::Frame),
    ) -> Result<()> {
        let screen_size = self.take_event_screen_size()?;
        // If we are resuming from ^Z, we need to prepare the resume action now so we can apply it
        // in the synchronized update.
        #[cfg(unix)]
        let mut prepared_resume = self
            .suspend_context
            .prepare_resume_action(&mut self.alt_saved_viewport);

        // Precompute any viewport updates that need a cursor-position query before entering
        // the synchronized update, to avoid racing with the event reader.
        let mut pending_viewport_area = self.pending_viewport_area(screen_size)?;

        ensure_virtual_terminal_processing()?;

        stdout().sync_update(|_| {
            #[cfg(unix)]
            if let Some(prepared) = prepared_resume.take() {
                self.terminal.invalidate_cursor_state();
                prepared.apply(
                    &mut self.terminal,
                    screen_size,
                    self.owned_screen,
                    self.overlay_input.captures_mouse(self.owned_screen),
                )?;
            }

            if self.owned_screen && !self.is_alt_screen_active() {
                self.enter_alt_screen()?;
                pending_viewport_area = None;
            }

            let terminal = &mut self.terminal;
            if let Some(new_area) = pending_viewport_area.take() {
                terminal.set_viewport_area(new_area);
                terminal.clear()?;
            }

            let mut area = terminal.viewport_area;
            area.height = height.min(screen_size.height);
            area.width = screen_size.width;
            // If the viewport has expanded, scroll everything else up to make room.
            if area.bottom() > screen_size.height {
                self.scrollback.grow_viewport(
                    terminal,
                    area.top(),
                    screen_size,
                    area.bottom() - screen_size.height,
                )?;
                area.y = screen_size.height - area.height;
            }
            if area != terminal.viewport_area {
                // On startup, the old viewport can still be empty. Clear from the
                // new viewport top so stale shell cells do not show through spaces.
                clear_for_viewport_change(terminal, area)?;
                terminal.set_viewport_area(area);
            }

            Self::flush_pending_history_lines(
                terminal,
                &mut self.pending_history_lines,
                self.scrollback,
                screen_size,
            )?;

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

            terminal.draw_with_size(screen_size, |frame| {
                draw_fn(frame);
            })
        })??;
        Ok(())
    }

    pub fn draw_ambient_pet_image(
        &mut self,
        request: Option<crate::pets::AmbientPetDraw>,
    ) -> std::result::Result<(), crate::pets::PetImageRenderError> {
        if let Err(err) = ensure_virtual_terminal_processing() {
            return Err(crate::pets::PetImageRenderError::Terminal(err));
        }

        let terminal = &mut self.terminal;
        let state = &mut self.ambient_pet_image_state;
        stdout().sync_update(|_| {
            match crate::pets::render_ambient_pet_image(terminal.backend_mut(), state, request) {
                Ok(()) => Ok(Ok(())),
                Err(crate::pets::PetImageRenderError::Terminal(err)) => Err(err),
                Err(err @ crate::pets::PetImageRenderError::Asset(_)) => Ok(Err(err)),
            }
        })??
    }

    pub fn draw_pet_picker_preview_image(
        &mut self,
        request: Option<crate::pets::AmbientPetDraw>,
    ) -> std::result::Result<(), crate::pets::PetImageRenderError> {
        if let Err(err) = ensure_virtual_terminal_processing() {
            return Err(crate::pets::PetImageRenderError::Terminal(err));
        }

        let terminal = &mut self.terminal;
        let state = &mut self.pet_picker_preview_image_state;
        stdout().sync_update(|_| {
            match crate::pets::render_pet_picker_preview_image(
                terminal.backend_mut(),
                state,
                request,
            ) {
                Ok(()) => Ok(Ok(())),
                Err(crate::pets::PetImageRenderError::Terminal(err)) => Err(err),
                Err(err @ crate::pets::PetImageRenderError::Asset(_)) => Ok(Err(err)),
            }
        })??
    }

    pub fn clear_ambient_pet_image(
        &mut self,
    ) -> std::result::Result<(), crate::pets::PetImageRenderError> {
        if let Err(err) = ensure_virtual_terminal_processing() {
            return Err(crate::pets::PetImageRenderError::Terminal(err));
        }

        crate::pets::render_ambient_pet_image(
            self.terminal.backend_mut(),
            &mut self.ambient_pet_image_state,
            /*request*/ None,
        )
    }

    /// Draw a frame using the resize-reflow viewport and history insertion rules.
    ///
    /// This is the feature-gated counterpart to `draw`. It intentionally skips
    /// `pending_viewport_area`, whose cursor-position heuristic is part of the legacy path, and
    /// instead lets transcript reflow rebuild scrollback before the frame is rendered.
    pub fn draw_with_resize_reflow(
        &mut self,
        height: u16,
        screen_size: Size,
        draw_fn: impl FnOnce(&mut custom_terminal::Frame),
    ) -> Result<()> {
        // If we are resuming from ^Z, we need to prepare the resume action now so we can apply it
        // in the synchronized update.
        #[cfg(unix)]
        let mut prepared_resume = self
            .suspend_context
            .prepare_resume_action(&mut self.alt_saved_viewport);

        ensure_virtual_terminal_processing()?;

        stdout().sync_update(|_| {
            #[cfg(unix)]
            if let Some(prepared) = prepared_resume.take() {
                self.terminal.invalidate_cursor_state();
                prepared.apply(
                    &mut self.terminal,
                    screen_size,
                    self.owned_screen,
                    self.overlay_input.captures_mouse(self.owned_screen),
                )?;
            }

            if self.owned_screen && !self.is_alt_screen_active() {
                self.enter_alt_screen()?;
            }

            let terminal = &mut self.terminal;
            let needs_full_repaint = Self::update_inline_viewport_for_resize_reflow(
                terminal,
                height,
                screen_size,
                self.scrollback,
            )?;
            // A zero- or one-row history region cannot isolate raw history writes from the
            // viewport, so replayed rows can leave stale cells inside the composer.
            let history_can_overlap_viewport =
                !self.pending_history_lines.is_empty() && terminal.viewport_area.top() <= 1;
            Self::flush_pending_history_lines(
                terminal,
                &mut self.pending_history_lines,
                self.scrollback,
                screen_size,
            )?;

            if needs_full_repaint || history_can_overlap_viewport {
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

            terminal.draw_with_size(screen_size, |frame| {
                draw_fn(frame);
            })
        })??;
        Ok(())
    }

    fn pending_viewport_area(&mut self, screen_size: Size) -> Result<Option<Rect>> {
        let terminal = &mut self.terminal;
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

#[cfg(windows)]
fn ensure_virtual_terminal_processing() -> Result<()> {
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::ENABLE_PROCESSED_OUTPUT;
    use windows_sys::Win32::System::Console::ENABLE_VIRTUAL_TERMINAL_PROCESSING;
    use windows_sys::Win32::System::Console::GetConsoleMode;
    use windows_sys::Win32::System::Console::GetStdHandle;
    use windows_sys::Win32::System::Console::STD_ERROR_HANDLE;
    use windows_sys::Win32::System::Console::STD_OUTPUT_HANDLE;
    use windows_sys::Win32::System::Console::SetConsoleMode;

    fn enable_for_handle(handle: HANDLE) -> Result<()> {
        if handle == INVALID_HANDLE_VALUE || handle == 0 {
            return Ok(());
        }

        let mut mode = 0;
        if unsafe { GetConsoleMode(handle, &mut mode) } == 0 {
            return Ok(());
        }

        let requested = ENABLE_PROCESSED_OUTPUT | ENABLE_VIRTUAL_TERMINAL_PROCESSING;
        if mode & requested == requested {
            return Ok(());
        }

        if unsafe { SetConsoleMode(handle, mode | requested) } == 0 {
            return Err(std::io::Error::last_os_error());
        }

        Ok(())
    }

    let stdout_handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
    enable_for_handle(stdout_handle)?;

    let stderr_handle = unsafe { GetStdHandle(STD_ERROR_HANDLE) };
    enable_for_handle(stderr_handle)?;

    Ok(())
}

#[cfg(not(windows))]
fn ensure_virtual_terminal_processing() -> Result<()> {
    Ok(())
}
