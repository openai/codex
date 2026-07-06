use std::fmt;
use std::future::Future;
use std::io::Result;
use std::io::Stdout;
use std::io::Write;
use std::io::stdout;
use std::panic;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use crossterm::Command;
use crossterm::SynchronizedUpdate;
use crossterm::cursor::SetCursorStyle;
use crossterm::event::DisableBracketedPaste;
use crossterm::event::DisableFocusChange;
use crossterm::event::EnableBracketedPaste;
use crossterm::event::EnableFocusChange;
use crossterm::event::KeyEvent;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
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
use ratatui::text::Line;
use tokio::sync::broadcast;
use tokio_stream::Stream;

pub use self::frame_requester::FrameRequester;
use crate::custom_terminal;
use crate::custom_terminal::Terminal as CustomTerminal;
use crate::insert_history::HistoryLineWrapPolicy;
use crate::insert_history::InsertHistoryMode;
use crate::notifications::DesktopNotificationBackend;
use crate::notifications::detect_backend;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::plain_hyperlink_lines;
use crate::tui::event_stream::EventBroker;
use crate::tui::event_stream::InitialInputPolicy;
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
mod keyboard_modes;
mod startup;
mod terminal_stderr;
#[cfg(test)]
pub(crate) mod test_support;

/// Target frame interval for UI redraw scheduling.
pub(crate) const TARGET_FRAME_INTERVAL: Duration = frame_rate_limiter::MIN_FRAME_INTERVAL;
pub(crate) const STARTUP_INPUT_QUIET_PERIOD: Duration = Duration::from_secs(/*secs*/ 1);
static TERMINAL_LIFECYCLE_LOCK: Mutex<()> = Mutex::new(());
static ALT_SCREEN_OWNED: AtomicBool = AtomicBool::new(false);

pub(crate) fn terminal_lifecycle_guard() -> std::sync::MutexGuard<'static, ()> {
    TERMINAL_LIFECYCLE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub(crate) fn note_alt_screen_entered() {
    ALT_SCREEN_OWNED.store(true, Ordering::SeqCst);
}

pub(crate) fn note_alt_screen_left() {
    ALT_SCREEN_OWNED.store(false, Ordering::SeqCst);
}

/// A type alias for the terminal type used in this application
pub type Terminal = CustomTerminal<CrosstermBackend<Stdout>>;

pub(crate) struct InitializedTerminal {
    pub(crate) terminal: Terminal,
    pub(crate) enhanced_keys_supported: bool,
    pub(crate) stderr_guard: terminal_stderr::TerminalStderrGuard,
    pub(crate) startup_input: StartupInputBuffer,
    pub(crate) startup_capture_active: bool,
}

pub(crate) use startup::PreparedTerminal;
use startup::StartupActionLatch;
use startup::StartupInputBuffer;
use startup::StartupInputHandoff;
pub(crate) use startup::abandon_prepared_terminal;
use startup::capture_startup_input;

pub(super) fn flush_terminal_input_buffer() {
    startup::flush_terminal_input_buffer();
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
        self.pause_events();
        #[cfg(unix)]
        self.deactivate_signal_suspend_context();
        if let Err(err) = self.clear_ambient_pet_image() {
            tracing::debug!(error = %err, "failed to clear ambient pet image on TUI drop");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;
    use tokio_stream::StreamExt;

    use super::StartupInputBuffer;
    use super::StartupInputTarget;
    use super::clear_for_viewport_change;
    use super::should_emit_notification;
    use crate::custom_terminal::Terminal as CustomTerminal;
    use crate::test_backend::VT100Backend;
    use crate::tui::test_support::make_test_tui;
    use codex_config::types::NotificationCondition;
    use ratatui::layout::Position;
    use ratatui::layout::Rect;

    #[test]
    fn unfocused_notification_condition_is_suppressed_when_focused() {
        assert!(!should_emit_notification(
            NotificationCondition::Unfocused,
            /*terminal_focused*/ true
        ));
    }

    #[test]
    fn panic_restore_does_not_recursively_lock_a_terminal_transition() {
        let lifecycle = super::terminal_lifecycle_guard();
        super::restore_after_panic_best_effort();
        drop(lifecycle);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn startup_input_remains_owned_until_event_stream_creation() -> std::io::Result<()> {
        let mut tui = make_test_tui()?;

        let _ = tui.take_startup_text_with_capture_and_bindings(&[], |_| Ok(()))?;
        assert!(tui.startup_input_active);

        let _events = tui.event_stream()?;
        assert!(!tui.startup_input_active);
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn startup_text_captures_input_between_startup_screens() -> std::io::Result<()> {
        let mut tui = make_test_tui()?;
        tui.startup_capture_active = true;
        tui.startup_input_active = false;
        let mut capture_called = false;

        let startup_text = tui.take_startup_text_with_capture_and_bindings(&[], |input| {
            capture_called = true;
            input.push_text("later");
            Ok(())
        })?;

        assert_eq!(startup_text.as_deref(), Some("later"));
        assert!(capture_called);
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn every_pre_composer_screen_claims_a_protected_handoff() -> std::io::Result<()> {
        let mut tui = make_test_tui()?;

        assert!(
            tui.claim_startup_screen_input(/*startup_screen_active*/ true)
                .claimed
        );
        assert!(
            tui.claim_startup_screen_input(/*startup_screen_active*/ true)
                .claimed
        );
        assert!(
            !tui.claim_startup_screen_input(/*startup_screen_active*/ false)
                .claimed
        );
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn startup_input_keeps_pending_whitespace_until_final_capture() -> std::io::Result<()> {
        let mut tui = make_test_tui()?;
        tui.startup_capture_active = true;
        let mut startup_input = StartupInputBuffer::default();
        startup_input.handle_probe_input(b"a\n");
        tui.startup_input = Some(startup_input);

        let startup_text = tui.take_startup_text_with_capture_and_bindings(&[], |input| {
            input.handle_probe_input(b"b");
            Ok(())
        })?;

        assert_eq!(startup_text.as_deref(), Some("a\nb"));
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn startup_text_uses_final_submission_bindings() -> std::io::Result<()> {
        let mut tui = make_test_tui()?;
        let submit = crate::key_hint::plain(KeyCode::Char('x'));
        let mut startup_input = StartupInputBuffer::default();
        startup_input.handle_probe_input(b"axb");
        tui.startup_input = Some(startup_input);

        let startup_text =
            tui.take_startup_text_with_capture_and_bindings(&[submit], |_| Ok(()))?;

        assert_eq!(startup_text.as_deref(), Some("ab"));
        assert!(
            tui.claim_startup_input()
                .quarantined_actions
                .iter()
                .any(|action| action.binding == submit)
        );
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn startup_interrupt_survives_text_extraction() -> std::io::Result<()> {
        let mut tui = make_test_tui()?;
        let mut startup_input = StartupInputBuffer::default();
        startup_input.handle_probe_input(b"draft\x03");
        tui.startup_input = Some(startup_input);

        assert_eq!(
            tui.take_startup_text_with_capture_and_bindings(&[], |_| Ok(()))?
                .as_deref(),
            Some("draft")
        );
        let mut events = tui.startup_event_stream(
            &[],
            StartupInputTarget::Composer,
            super::StartupTextPolicy::Preserve,
        )?;
        assert!(matches!(
            events.next().await,
            Some(super::TuiEvent::Key(key)) if key == KeyEvent::new(
                KeyCode::Char('c'),
                KeyModifiers::CONTROL,
            )
        ));
        Ok(())
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
}

pub fn set_modes() -> Result<()> {
    let _lifecycle = terminal_lifecycle_guard();
    set_modes_unlocked()
}

fn set_modes_unlocked() -> Result<()> {
    startup::pause_startup_input_capture_for_full_modes();
    set_base_modes()?;
    set_event_modes();
    startup::note_full_terminal_modes();
    Ok(())
}

fn set_base_modes() -> Result<()> {
    ensure_virtual_terminal_processing()?;

    execute!(stdout(), EnableBracketedPaste)?;

    enable_raw_mode()?;
    Ok(())
}

fn set_event_modes() {
    // Enable keyboard enhancement flags so modifiers for keys like Enter are disambiguated.
    // chat_composer.rs is using a keyboard event listener to enter for any modified keys
    // to create a new line that require this.
    // Some terminals (notably legacy Windows consoles) do not support
    // keyboard enhancement flags. Attempt to enable them, but continue
    // gracefully if unsupported.
    keyboard_modes::enable_keyboard_enhancement();

    let _ = execute!(stdout(), EnableFocusChange);
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
pub(crate) struct DisableAlternateScroll;

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

    match keyboard_restore {
        KeyboardRestore::PopStack => keyboard_modes::restore_keyboard_enhancement_stack(),
        KeyboardRestore::ResetAfterExit => keyboard_modes::reset_keyboard_reporting_after_exit(),
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
    if matches!(raw_mode_restore, RawModeRestore::Disable) {
        startup::note_capture_terminal_mode();
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
pub fn restore() -> Result<()> {
    let _lifecycle = terminal_lifecycle_guard();
    restore_common(RawModeRestore::Disable, KeyboardRestore::PopStack)
}

/// Leave a temporary startup screen while keeping the early capture phase active.
pub(super) fn restore_startup_screen() -> Result<()> {
    let _lifecycle = terminal_lifecycle_guard();
    let mut first_error = restore_common(RawModeRestore::Disable, KeyboardRestore::PopStack).err();
    if let Err(err) = execute!(stdout(), EnableBracketedPaste) {
        first_error.get_or_insert(err);
    }
    #[cfg(windows)]
    if let Err(err) = startup::reapply_startup_capture_mode_locked() {
        first_error.get_or_insert(err);
    }
    match first_error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

#[cfg(unix)]
fn reapply_raw_mode_after_continue_unlocked() -> Result<()> {
    disable_raw_mode()?;
    enable_raw_mode()?;
    execute!(stdout(), EnableBracketedPaste)
}

/// Restore the terminal after Codex is exiting.
///
/// Uses a stronger keyboard reset than [`restore`] so the parent shell recovers even if a
/// terminal missed the stack pop that normally pairs with [`set_modes`]. Queued input is flushed
/// only after restoration so late key repeats cannot escape to the parent shell.
pub(crate) fn restore_after_exit_best_effort() -> Result<()> {
    let _lifecycle = terminal_lifecycle_guard();
    restore_after_exit_best_effort_unlocked()
}

fn restore_after_panic_best_effort() {
    match TERMINAL_LIFECYCLE_LOCK.try_lock() {
        Ok(_lifecycle) => {
            let _ = restore_after_exit_best_effort_unlocked();
        }
        Err(std::sync::TryLockError::Poisoned(err)) => {
            let _lifecycle = err.into_inner();
            let _ = restore_after_exit_best_effort_unlocked();
        }
        Err(std::sync::TryLockError::WouldBlock) => {
            // The panic interrupted another terminal transition. Avoid recursively waiting on
            // this non-reentrant lock; normal stack unwinding releases it before the outer
            // terminal restore guard runs.
        }
    }
}

fn restore_after_exit_best_effort_unlocked() -> Result<()> {
    match restore_after_exit_unlocked() {
        Ok(()) => Ok(()),
        Err(_) => restore_after_exit_unlocked(),
    }
}

fn restore_after_exit_unlocked() -> Result<()> {
    if !startup::has_startup_capture_mode() {
        flush_terminal_input_buffer();
        return Ok(());
    }
    let mut first_error =
        restore_common(RawModeRestore::Disable, KeyboardRestore::ResetAfterExit).err();
    if ALT_SCREEN_OWNED.load(Ordering::SeqCst) {
        let alternate_scroll_disabled = match execute!(stdout(), DisableAlternateScroll) {
            Ok(()) => true,
            Err(err) => {
                first_error.get_or_insert(err);
                false
            }
        };
        match execute!(stdout(), LeaveAlternateScreen) {
            Ok(()) if alternate_scroll_disabled => note_alt_screen_left(),
            Ok(()) => {}
            Err(err) => {
                first_error.get_or_insert(err);
            }
        }
    }
    if let Err(err) = terminal_stderr::finish() {
        first_error.get_or_insert(err);
    }
    if let Err(err) = startup::restore_startup_capture_mode() {
        first_error.get_or_insert(err);
    }
    flush_terminal_input_buffer();

    match first_error {
        Some(err) => Err(err),
        None => {
            startup::finish_startup_capture_restore();
            Ok(())
        }
    }
}

pub(super) fn exit_after_terminal_signal(code: i32) -> ! {
    let _lifecycle = terminal_lifecycle_guard();
    let _ = restore_after_exit_best_effort_unlocked();
    std::process::exit(code);
}

/// Restore the terminal to its original state, but keep raw mode enabled.
pub fn restore_keep_raw() -> Result<()> {
    let _lifecycle = terminal_lifecycle_guard();
    restore_common(RawModeRestore::Keep, KeyboardRestore::PopStack)
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
    match crate::terminal_probe::console_default_colors() {
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
        restore_after_panic_best_effort();
        hook(panic_info);
    }));
}

#[derive(Clone, Debug)]
pub enum TuiEvent {
    /// A terminal key event after focus, paste, and protocol bookkeeping has been handled.
    Key(KeyEvent),
    /// A bracketed paste payload normalized by the app layer before it reaches the composer.
    Paste(String),
    /// Text-like key input captured by the startup handoff for the composer specifically.
    StartupComposerKey(KeyEvent),
    /// A non-submitting composer action captured by the startup handoff.
    StartupComposerAction(KeyEvent),
    /// A paste captured by the startup handoff for the composer specifically.
    StartupComposerPaste(String),
    /// A terminal size notification that should be handled as resize-sensitive draw work.
    ///
    /// Resize is separate from `Draw` so the app can run feature-gated pre-render logic without
    /// changing the default draw path for scheduled frames.
    Resize,
    /// A scheduled repaint that does not necessarily correspond to a terminal size change.
    Draw,
    /// The startup event stream has finished protecting the terminal ownership handoff.
    StartupInputSettled,
}

#[derive(Clone, Copy)]
pub(crate) enum StartupInputTarget {
    Composer,
    ActiveView,
}

#[derive(Clone, Copy)]
pub(crate) enum StartupTextPolicy {
    Preserve,
    Discard,
}

pub struct Tui {
    frame_requester: FrameRequester,
    draw_tx: broadcast::Sender<()>,
    event_broker: Arc<EventBroker>,
    pub(crate) terminal: Terminal,
    pending_history_lines: Vec<PendingHistoryLines>,
    ambient_pet_image_state: crate::pets::PetImageRenderState,
    pet_picker_preview_image_state: crate::pets::PetImageRenderState,
    alt_saved_viewport: Option<ratatui::layout::Rect>,
    #[cfg(unix)]
    suspend_context: SuspendContext,
    #[cfg(unix)]
    signal_suspend_registration: Option<startup::SignalSuspendRegistration>,
    // True when overlay alt-screen UI is active
    alt_screen_active: Arc<AtomicBool>,
    // True when terminal/tab is focused; updated internally from crossterm events
    terminal_focused: Arc<AtomicBool>,
    enhanced_keys_supported: bool,
    notification_backend: Option<DesktopNotificationBackend>,
    notification_condition: NotificationCondition,
    startup_input: Option<StartupInputBuffer>,
    startup_input_active: bool,
    startup_capture_active: bool,
    startup_crossterm_input_active: bool,
    startup_action_latch: Arc<Mutex<StartupActionLatch>>,
    // Raw terminal-wrapped history needs a non-scroll-region insertion path in Zellij.
    is_zellij: bool,
    // When false, enter_alt_screen() becomes a no-op.
    alt_screen_enabled: bool,
    // Keeps unmanaged process stderr writes out of the inline viewport.
    _stderr_guard: terminal_stderr::TerminalStderrGuard,
}

struct PendingHistoryLines {
    lines: Vec<HyperlinkLine>,
    wrap_policy: HistoryLineWrapPolicy,
}

fn clear_for_viewport_change<B>(terminal: &mut CustomTerminal<B>, new_area: Rect) -> Result<()>
where
    B: Backend + Write,
{
    let clear_position = if terminal.viewport_area.is_empty() {
        new_area.as_position()
    } else {
        terminal.viewport_area.as_position()
    };
    terminal.clear_after_position(clear_position)
}

impl Tui {
    pub(crate) fn new(
        terminal: Terminal,
        enhanced_keys_supported: bool,
        stderr_guard: terminal_stderr::TerminalStderrGuard,
        startup_input: Option<StartupInputBuffer>,
        startup_capture_active: bool,
    ) -> Self {
        let (draw_tx, _) = broadcast::channel(1);
        let frame_requester = FrameRequester::new(draw_tx.clone());
        let event_broker = Arc::new(EventBroker::new());
        #[cfg(unix)]
        let suspend_context = SuspendContext::new();
        let alt_screen_active = Arc::new(AtomicBool::new(false));
        #[cfg(unix)]
        let signal_suspend_registration = startup::register_signal_suspend_context(
            event_broker.clone(),
            suspend_context.clone(),
            alt_screen_active.clone(),
            frame_requester.clone(),
        );

        // Cache this to avoid contention with the event reader.
        supports_color::on_cached(supports_color::Stream::Stdout);
        let _ = crate::terminal_palette::default_colors();
        let is_zellij = codex_terminal_detection::terminal_info().is_zellij();

        Self {
            frame_requester,
            draw_tx,
            event_broker,
            terminal,
            pending_history_lines: vec![],
            ambient_pet_image_state: crate::pets::PetImageRenderState::default(),
            pet_picker_preview_image_state: crate::pets::PetImageRenderState::default(),
            alt_saved_viewport: None,
            #[cfg(unix)]
            suspend_context,
            #[cfg(unix)]
            signal_suspend_registration: Some(signal_suspend_registration),
            alt_screen_active,
            terminal_focused: Arc::new(AtomicBool::new(true)),
            enhanced_keys_supported,
            notification_backend: Some(detect_backend(NotificationMethod::default())),
            notification_condition: NotificationCondition::default(),
            startup_input,
            startup_input_active: true,
            startup_capture_active,
            startup_crossterm_input_active: false,
            startup_action_latch: Arc::new(Mutex::new(StartupActionLatch::default())),
            is_zellij,
            alt_screen_enabled: true,
            _stderr_guard: stderr_guard,
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

    pub fn enhanced_keys_supported(&self) -> bool {
        self.enhanced_keys_supported
    }

    pub fn is_alt_screen_active(&self) -> bool {
        self.alt_screen_active.load(Ordering::Relaxed)
    }

    #[cfg(unix)]
    pub(crate) fn deactivate_signal_suspend_context(&mut self) {
        if let Some(registration) = self.signal_suspend_registration.take() {
            startup::unregister_signal_suspend_context(registration);
        }
    }

    pub(crate) fn prepare_for_terminal_restore(&mut self) {
        self.pause_events();
        #[cfg(unix)]
        self.deactivate_signal_suspend_context();
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
    /// terminal modes and stderr (optionally keeping raw mode enabled), then re-applies Codex TUI
    /// modes and stderr suppression before resuming events.
    pub async fn with_restored<R, F, Fut>(&mut self, mode: RestoreMode, f: F) -> R
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = R>,
    {
        #[cfg(unix)]
        let signal_synchronization = self
            .signal_suspend_registration
            .as_ref()
            .map(startup::SignalSuspendRegistration::synchronization);
        #[cfg(unix)]
        let signal_transition = signal_synchronization.as_ref().map(|(operation, _)| {
            operation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        });

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
        let restore_startup_capture = matches!(mode, RestoreMode::Full);
        if restore_startup_capture
            && let Err(err) = startup::temporarily_restore_startup_capture_mode()
        {
            tracing::warn!("failed to restore startup capture before external program: {err}");
        }
        if let Err(err) = terminal_stderr::pause() {
            tracing::warn!("failed to restore terminal stderr before external program: {err}");
        }
        #[cfg(unix)]
        if let Some((_, external_owner)) = &signal_synchronization {
            external_owner.store(true, Ordering::SeqCst);
        }
        #[cfg(unix)]
        drop(signal_transition);

        let output = f().await;

        #[cfg(unix)]
        let signal_transition = signal_synchronization.as_ref().map(|(operation, _)| {
            operation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        });
        if let Err(err) = terminal_stderr::resume() {
            tracing::warn!("failed to suppress terminal stderr after external program: {err}");
        }
        if restore_startup_capture && let Err(err) = startup::reapply_startup_capture_mode() {
            tracing::warn!("failed to re-enable startup capture after external program: {err}");
        }
        if let Err(err) = set_modes() {
            tracing::warn!("failed to re-enable terminal modes after external program: {err}");
        }
        // After the external program `f` finishes, reset terminal state and flush any buffered keypresses.
        flush_terminal_input_buffer();

        if was_alt_screen {
            let _ = self.enter_alt_screen();
        }

        self.resume_events();
        #[cfg(unix)]
        if let Some((_, external_owner)) = &signal_synchronization {
            external_owner.store(false, Ordering::SeqCst);
        }
        #[cfg(unix)]
        drop(signal_transition);
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

    /// Begin reading interactive events for a screen.
    ///
    /// A screen that requests events becomes the new input owner, so text captured before the TUI
    /// was ready must not survive through it and later appear in the composer.
    pub fn event_stream(
        &mut self,
    ) -> Result<Pin<Box<dyn Stream<Item = TuiEvent> + Send + 'static>>> {
        let startup_screen_active = self.startup_capture_active;
        if startup_screen_active {
            if self.startup_crossterm_input_active {
                set_modes()?;
            } else {
                self.capture_startup_input_for_full_modes()?;
                self.startup_crossterm_input_active = true;
            }
        }
        let startup_input = self.claim_startup_screen_input(startup_screen_active);
        self.event_broker.resume_events();
        let mut stream = self.build_event_stream(InitialInputPolicy::DiscardAll, startup_input);
        if startup_screen_active {
            stream = stream
                .recording_startup_actions(self.startup_action_latch.clone())
                .restoring_startup_capture_on_drop();
        }
        Ok(Box::pin(stream))
    }

    pub(crate) fn startup_event_stream(
        &mut self,
        submission_bindings: &[crate::key_hint::KeyBinding],
        target: StartupInputTarget,
        text_policy: StartupTextPolicy,
    ) -> Result<Pin<Box<dyn Stream<Item = TuiEvent> + Send + 'static>>> {
        if self.startup_capture_active && !self.startup_crossterm_input_active {
            self.capture_startup_input_for_full_modes()?;
        } else if self.startup_capture_active {
            set_modes()?;
        }
        let startup_text = match (target, text_policy) {
            (StartupInputTarget::Composer, StartupTextPolicy::Preserve) => {
                self.startup_input.as_mut().and_then(|input| {
                    input.take_text_excluding_submission_bindings(submission_bindings)
                })
            }
            (StartupInputTarget::Composer, StartupTextPolicy::Discard)
            | (StartupInputTarget::ActiveView, _) => None,
        };
        let mut startup_input = self.claim_startup_input();
        if self.startup_capture_active {
            startup::finish_startup_input_capture()?;
            self.startup_capture_active = false;
        }
        self.event_broker.resume_events();
        // A startup screen may already have discarded the original buffer. The composer still
        // needs one protected handoff for anything crossterm accumulated during later startup.
        startup_input.claimed = true;
        startup_input.submission_bindings = submission_bindings.to_vec();
        Ok(Box::pin(
            self.build_event_stream(
                match (target, text_policy) {
                    (StartupInputTarget::Composer, StartupTextPolicy::Preserve) => {
                        InitialInputPolicy::PreserveText
                    }
                    (StartupInputTarget::Composer, StartupTextPolicy::Discard)
                    | (StartupInputTarget::ActiveView, _) => InitialInputPolicy::DiscardAll,
                },
                startup_input,
            )
            .restoring_startup_composer_text(startup_text),
        ))
    }

    fn capture_startup_input_for_full_modes(&mut self) -> Result<()> {
        let input = self.startup_input.get_or_insert_default();
        startup::capture_startup_input_for_full_modes(input)?;
        self.startup_input_active = true;
        Ok(())
    }

    fn claim_startup_input(&mut self) -> StartupInputHandoff {
        let claimed = self.startup_input_active;
        let mut input = self.startup_input.take().unwrap_or_default();
        let latched_input = self
            .startup_action_latch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .drain_into(&mut input);
        let mut startup_input = if claimed || latched_input {
            input.into_handoff()
        } else {
            StartupInputHandoff::default()
        };
        self.startup_input_active = false;
        #[cfg(unix)]
        if startup_input.suspend_requested {
            self.event_broker.pause_events();
            let suspend_result = self.suspend_context.suspend(&self.alt_screen_active);
            self.event_broker.resume_events();
            if let Err(err) = suspend_result {
                tracing::warn!(
                    event = "tui_startup_suspend_failed",
                    error = %err,
                    "failed to suspend TUI process during startup"
                );
            }
            startup_input.resume_draw_requested = true;
        }
        startup_input
    }

    fn claim_startup_screen_input(&mut self, startup_screen_active: bool) -> StartupInputHandoff {
        let mut startup_input = self.claim_startup_input();
        if startup_screen_active {
            // Each pre-composer screen owns a distinct handoff. Input typed during a slow gap
            // must not bypass protection merely because an earlier screen claimed the original
            // startup buffer.
            startup_input.claimed = true;
            // The filter drains stale input before this synthetic draw, then accepts keys only
            // after the screen has rendered.
            startup_input.resume_draw_requested = true;
        }
        startup_input
    }

    fn build_event_stream(
        &self,
        initial_input_policy: InitialInputPolicy,
        startup_input: StartupInputHandoff,
    ) -> TuiEventStream {
        #[cfg(unix)]
        let stream = TuiEventStream::new(
            self.event_broker.clone(),
            self.draw_tx.subscribe(),
            self.terminal_focused.clone(),
            self.suspend_context.clone(),
            self.alt_screen_active.clone(),
        )
        .with_enhanced_key_events(self.enhanced_keys_supported);
        #[cfg(not(unix))]
        let stream = TuiEventStream::new(
            self.event_broker.clone(),
            self.draw_tx.subscribe(),
            self.terminal_focused.clone(),
        )
        .with_enhanced_key_events(self.enhanced_keys_supported);
        configure_initial_input(stream, initial_input_policy, startup_input)
    }

    pub(crate) fn take_startup_text(
        &mut self,
        submission_bindings: &[crate::key_hint::KeyBinding],
    ) -> Result<Option<String>> {
        self.take_startup_text_with_capture_and_bindings(submission_bindings, capture_startup_input)
    }

    fn take_startup_text_with_capture_and_bindings(
        &mut self,
        submission_bindings: &[crate::key_hint::KeyBinding],
        capture: impl FnOnce(&mut StartupInputBuffer) -> Result<()>,
    ) -> Result<Option<String>> {
        let mut input = self.startup_input.take().unwrap_or_default();
        let latched_input = self
            .startup_action_latch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .drain_into(&mut input);
        if self.startup_capture_active && !self.startup_crossterm_input_active {
            capture(&mut input)?;
            // The bounded reader may have captured new input after an earlier startup screen
            // claimed the previous buffer. Keep its action provenance alive for the final
            // protected handoff even when there is no printable text to restore.
            self.startup_input_active = true;
        }
        if !self.startup_input_active && !latched_input {
            self.startup_input = Some(input);
            return Ok(None);
        }
        // Keep action/control state until `event_stream()` hands input to crossterm.
        let text = input.take_text_excluding_submission_bindings(submission_bindings);
        self.startup_input = Some(input);
        self.startup_input_active = true;
        Ok(text)
    }

    /// Enter alternate screen and expand the viewport to full terminal size, saving the current
    /// inline viewport for restoration when leaving.
    pub fn enter_alt_screen(&mut self) -> Result<()> {
        if !self.alt_screen_enabled {
            return Ok(());
        }
        let _lifecycle = terminal_lifecycle_guard();
        execute!(self.terminal.backend_mut(), EnterAlternateScreen)?;
        note_alt_screen_entered();
        // Enable "alternate scroll" so terminals may translate wheel to arrows
        execute!(self.terminal.backend_mut(), EnableAlternateScroll)?;
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
        let _lifecycle = terminal_lifecycle_guard();
        // Disable alternate scroll when leaving alt-screen
        execute!(self.terminal.backend_mut(), DisableAlternateScroll)?;
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen)?;
        note_alt_screen_left();
        if let Some(saved) = self.alt_saved_viewport.take() {
            self.terminal.set_viewport_area(saved);
        }
        self.alt_screen_active.store(false, Ordering::Relaxed);
        Ok(())
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
    ) -> Result<bool> {
        let size = terminal.size()?;
        let terminal_height_shrank = size.height < terminal.last_known_screen_size.height;
        let terminal_height_grew = size.height > terminal.last_known_screen_size.height;
        let viewport_was_bottom_aligned =
            terminal.viewport_area.bottom() == terminal.last_known_screen_size.height;
        let previous_area = terminal.viewport_area;

        let mut area = terminal.viewport_area;
        area.height = height.min(size.height);
        area.width = size.width;
        let mut needs_full_repaint = false;

        if area.bottom() > size.height {
            let scroll_by = area.bottom() - size.height;
            if !terminal_height_shrank {
                terminal
                    .backend_mut()
                    .scroll_region_up(0..area.top(), scroll_by)?;
            }
            area.y = size.height - area.height;
        } else if terminal_height_grew && viewport_was_bottom_aligned {
            area.y = size.height - area.height;
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
        is_zellij: bool,
    ) -> Result<()> {
        if pending_history_lines.is_empty() {
            return Ok(());
        }

        for batch in pending_history_lines.iter() {
            let mode = if is_zellij && batch.wrap_policy == HistoryLineWrapPolicy::Terminal {
                InsertHistoryMode::ZellijRaw
            } else {
                InsertHistoryMode::Standard
            };
            crate::insert_history::insert_history_hyperlink_lines_with_mode_and_wrap_policy(
                terminal,
                batch.lines.clone(),
                mode,
                batch.wrap_policy,
            )?;
        }
        pending_history_lines.clear();
        Ok(())
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
            .prepare_resume_action(&mut self.alt_saved_viewport);

        // Precompute any viewport updates that need a cursor-position query before entering
        // the synchronized update, to avoid racing with the event reader.
        let mut pending_viewport_area = self.pending_viewport_area()?;

        ensure_virtual_terminal_processing()?;

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
                // On startup, the old viewport can still be empty. Clear from the
                // new viewport top so stale shell cells do not show through spaces.
                clear_for_viewport_change(terminal, area)?;
                terminal.set_viewport_area(area);
            }

            Self::flush_pending_history_lines(
                terminal,
                &mut self.pending_history_lines,
                self.is_zellij,
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

            terminal.draw(|frame| {
                draw_fn(frame);
            })
        })?
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
                prepared.apply(&mut self.terminal)?;
            }

            let terminal = &mut self.terminal;
            let needs_full_repaint =
                Self::update_inline_viewport_for_resize_reflow(terminal, height)?;
            Self::flush_pending_history_lines(
                terminal,
                &mut self.pending_history_lines,
                self.is_zellij,
            )?;

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

fn configure_initial_input<S>(
    stream: TuiEventStream<S>,
    policy: InitialInputPolicy,
    mut startup_input: StartupInputHandoff,
) -> TuiEventStream<S>
where
    S: event_stream::EventSource + Default + Unpin,
{
    if !startup_input.claimed {
        return stream;
    }
    let mut startup_repeat_actions = std::mem::take(&mut startup_input.repeat_actions);
    let pending_plain_whitespace = std::mem::take(&mut startup_input.pending_plain_whitespace);
    let pending_plain_whitespace_actions =
        std::mem::take(&mut startup_input.pending_plain_whitespace_actions);
    debug_assert_eq!(
        pending_plain_whitespace.chars().count(),
        pending_plain_whitespace_actions.len()
    );
    for (ch, action) in pending_plain_whitespace
        .chars()
        .zip(pending_plain_whitespace_actions)
    {
        if !action.release_observed
            && !startup_repeat_actions.iter().any(|existing| {
                existing.binding == action.binding
                    && existing.from_raw_probe == action.from_raw_probe
            })
        {
            startup_repeat_actions.push(action);
        }
        // Keep ambiguous trailing whitespace in the handoff filter: it is discarded if startup
        // settles here, but becomes ordinary draft data if later text makes it internal.
        startup_input.pending_plain_whitespace.push(ch);
    }
    for action in &startup_input.quarantined_actions {
        if !startup_repeat_actions.iter().any(|existing| {
            existing.binding == action.binding && existing.from_raw_probe == action.from_raw_probe
        }) {
            startup_repeat_actions.push(*action);
        }
    }
    if let Some((binding, from_raw_probe)) = startup_input.trailing_printable_action
        && !startup_repeat_actions.iter().any(|action| {
            startup::startup_action_matches(action.binding, action.from_raw_probe, binding)
        })
    {
        startup_repeat_actions.push(startup::StartupBlockedAction::captured(
            binding,
            from_raw_probe,
        ));
    }
    let trailing_action = match policy {
        InitialInputPolicy::DiscardAll => startup_input.trailing_printable_action,
        InitialInputPolicy::PreserveText => {
            startup_input
                .trailing_printable_action
                .filter(|(action, from_raw_probe)| {
                    startup_input
                        .submission_bindings
                        .iter()
                        .copied()
                        .any(|binding| {
                            startup::startup_action_matches(*action, *from_raw_probe, binding)
                        })
                })
        }
    };
    if matches!(policy, InitialInputPolicy::PreserveText) {
        let submission_bindings = &startup_input.submission_bindings;
        startup_input.quarantined_actions.retain(|action| {
            !action.quiet_elapsed
                || !action.preserve_after_quiet
                || submission_bindings.iter().copied().any(|binding| {
                    startup::startup_action_matches(action.binding, action.from_raw_probe, binding)
                })
        });
    }
    let unquiet_action = startup_input
        .quarantined_actions
        .iter()
        .any(|action| !action.quiet_elapsed);
    let start_quiet = match policy {
        // Confirmation screens are already visible when they claim input. Always establish a
        // quiet boundary so a key arriving just after an initially empty poll cannot confirm one.
        InitialInputPolicy::DiscardAll => {
            startup_input.unknown_action_seen || trailing_action.is_some() || unquiet_action
        }
        InitialInputPolicy::PreserveText => {
            startup_input.restored_text
                || startup_input.unknown_action_seen
                || trailing_action.is_some()
                || unquiet_action
        }
    };
    stream
        .filtering_initial_input(event_stream::InitialInputConfig {
            start_quiet,
            pending_interrupt: startup_input.interrupt_requested,
            pending_draw: startup_input.resume_draw_requested,
            pending_plain_whitespace: startup_input.pending_plain_whitespace,
            trailing_action: trailing_action.map(|(binding, _)| binding),
            trailing_action_from_raw_probe: trailing_action
                .is_some_and(|(_, from_raw_probe)| from_raw_probe),
            ..event_stream::InitialInputConfig::new(policy)
        })
        .protecting_initial_submission_bindings(startup_input.submission_bindings)
        .blocking_initial_actions(startup_input.quarantined_actions)
        .blocking_startup_repeats(startup_repeat_actions)
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
