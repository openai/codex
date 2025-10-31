use std::io::IsTerminal;
use std::io::Result;
use std::io::Stdout;
use std::io::stdout;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
#[cfg(unix)]
use std::sync::atomic::AtomicU8;
#[cfg(unix)]
use std::sync::atomic::AtomicU16;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use crossterm::Command;
use crossterm::SynchronizedUpdate;
#[cfg(unix)]
use crossterm::cursor::MoveTo;
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

/// Enable the terminal capabilities the Codex TUI depends on.
///
/// - Enables bracketed paste so multi-line submissions reach [`ChatComposer`] as a single
///   payload.
/// - Switches to raw mode to expose low-level key events without line buffering.
/// - Attempts to push Crossterm keyboard enhancement flags so modifier-aware keys are visible.
///   [`ChatComposer`] listens for modifier-rich enter presses, so we best-effort enable the
///   extension even when consoles may ignore it.
/// - Enables focus reporting so
///   [`ChatWidget::maybe_post_pending_notification`][ChatWidgetNotif] can gate alerts.
///
/// Ratatui leaves these switches to callers; centralizing them here guarantees the inline viewport
/// starts from a consistent configuration.
///
/// [`ChatComposer`]: crate::bottom_pane::chat_composer::ChatComposer
/// [ChatWidgetNotif]: crate::chatwidget::ChatWidget::maybe_post_pending_notification
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

/// Crossterm command that enables "alternate scroll" (SGR/DECSET 1007) so mouse wheels translate
/// into arrow keys while the alt screen is active. See the xterm control sequence reference
/// (<https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h2-Mouse-Tracking>) for details.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EnableAlternateScroll;

impl Command for EnableAlternateScroll {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        write!(f, "\x1b[?1007h")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        Err(std::io::Error::other(
            "tried to execute EnableAlternateScroll using WinAPI; use ANSI instead",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

/// Crossterm command that disables SGR/DECSET 1007 so scroll wheels go back to native terminal
/// behavior; refer to the same xterm control sequence documentation for details.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DisableAlternateScroll;

impl Command for DisableAlternateScroll {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        write!(f, "\x1b[?1007l")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
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
///
/// Undo the side effects of [`set_modes`].
///
/// - Pops any keyboard enhancement flags that were pushed.
/// - Disables bracketed paste and focus tracking.
/// - Leaves raw mode and ensures the cursor is visible.
///
/// The disable calls are best-effort because some terminals refuse the matching sequences,
/// especially after a suspend/resume cycle.
pub fn restore() -> Result<()> {
    // Pop may fail on platforms that didn't support the push; ignore errors.
    let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
    execute!(stdout(), DisableBracketedPaste)?;
    let _ = execute!(stdout(), DisableFocusChange);
    disable_raw_mode()?;
    let _ = execute!(stdout(), crossterm::cursor::Show);
    Ok(())
}

/// Initialize the inline viewport while preserving scrollback.
///
/// - Rejects initialization if stdout is not a TTY.
/// - Enables the raw-mode feature set via [`set_modes`].
/// - Installs a panic hook that restores the terminal before unwinding.
///
/// Existing terminal contents are left untouched so scrollback stays intact, even on terminals that
/// treat `Clear` as destructive.
///
/// Unlike [`ratatui::Terminal::new`], this uses our `CustomTerminal` wrapper so the interactive
/// viewport stays inline and history remains accessible in the native scrollback buffer—the main
/// architectural difference from Ratatui's default alt-screen workflow.
pub fn init() -> Result<Terminal> {
    if !stdout().is_terminal() {
        return Err(std::io::Error::other("stdout is not a terminal"));
    }
    set_modes()?;

    set_panic_hook();

    let backend = CrosstermBackend::new(stdout());
    let tui = CustomTerminal::with_options(backend)?;
    Ok(tui)
}

/// Ensure panics drop the terminal back to cooked mode before surfacing.
///
/// Ratatui defers this to the consumer; we override the default panic hook so that even unexpected
/// crashes clear raw mode and cursor hiding. The hook delegates to the previous handler after
/// restoration so panic reporting (and test harness output) remains unchanged.
fn set_panic_hook() {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = restore(); // ignore any errors as we are already failing
        hook(panic_info);
    }));
}

#[derive(Debug)]
pub enum TuiEvent {
    /// Raw `crossterm` key event, including modifier-rich variants unlocked by [`set_modes`].
    Key(KeyEvent),
    /// Bracketed paste payload delivered as a single string.
    Paste(String),
    /// Request to redraw the UI, typically due to focus changes, resizes, or coalesced frame
    /// requests.
    Draw,
}

/// Drives the Codex UI on top of an inline [`CustomTerminal`].
///
/// Light fork of Ratatui's terminal that adapts to Codex requirements.
///
/// - Keeps transcript history in the native scrollback while reserving a configurable inline
///   viewport for interactive UI using [`crate::insert_history::insert_history_lines`].
/// - Owns the scheduler that coalesces draw requests to avoid redundant rendering work, exposing
///   handles via [`FrameRequester`].
/// - Manages alt-screen transitions so overlays can temporarily take over the full display through
///   [`enter_alt_screen`] and [`leave_alt_screen`].
/// - Caches capability probes such as keyboard enhancement support and default palette refreshes.
///
/// [`FrameRequester`]: crate::tui::FrameRequester
/// [`enter_alt_screen`]: Tui::enter_alt_screen
/// [`leave_alt_screen`]: Tui::leave_alt_screen
pub struct Tui {
    /// Channel used to schedule future frames; owned by the background coalescer task and cloned
    /// into every [`FrameRequester`] handed to widgets.
    frame_schedule_tx: tokio::sync::mpsc::UnboundedSender<Instant>,

    /// Broadcast channel that delivers draw notifications to the event stream so the UI loop can
    /// wake without holding mutable access to the terminal.
    draw_tx: tokio::sync::broadcast::Sender<()>,

    /// Inline terminal wrapper that keeps the active viewport and history buffers in sync with
    /// Ratatui widgets.
    pub(crate) terminal: Terminal,

    /// History lines waiting to be spliced above the viewport on the next draw; populated by
    /// background tasks that stream transcript updates.
    pending_history_lines: Vec<Line<'static>>,

    /// Saved viewport rectangle from inline mode so alt-screen overlays can be restored to the
    /// exact scroll position users left.
    alt_saved_viewport: Option<ratatui::layout::Rect>,

    /// Pending resume action recorded by the event loop when `Ctrl+Z` is processed; applied during
    /// the next synchronized update to avoid cursor-query races.
    #[cfg(unix)]
    resume_pending: Arc<AtomicU8>,

    /// Cached cursor row where the inline viewport ends, ensuring the shell prompt lands beneath
    /// the UI after a suspend.
    #[cfg(unix)]
    suspend_cursor_y: Arc<AtomicU16>,

    /// Tracks whether an alt-screen overlay currently owns the terminal so mouse wheel handling and
    /// viewport restoration behave correctly.
    alt_screen_active: Arc<AtomicBool>,

    /// Reflects the window focus state based on Crossterm events; used to gate OSC 9 notifications
    /// and palette refreshes.
    terminal_focused: Arc<AtomicBool>,

    /// Whether the terminal acknowledged Crossterm's keyboard enhancement flags; controls key hint
    /// rendering and modifier handling in the composer.
    enhanced_keys_supported: bool,
}

#[cfg(unix)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
enum ResumeAction {
    /// No post-resume work is required.
    None = 0,
    /// Recenter the inline viewport around the cursor the shell left behind.
    RealignInline = 1,
    /// Re-enter the alternate screen for overlays active when suspend happened.
    RestoreAlt = 2,
}

#[cfg(unix)]
enum PreparedResumeAction {
    /// Restore the alternate screen overlay and refresh its viewport.
    RestoreAltScreen,
    /// Realign the inline viewport to the provided area.
    RealignViewport(ratatui::layout::Rect),
}

/// Swap the pending resume action out of the atomic flag.
///
/// The event loop records the desired action (realign inline viewport versus restore the alt
/// screen) before invoking [`suspend`]. We clear the flag with relaxed ordering because only the
/// main thread reads it during draw. The integer representation keeps the atomic size small when
/// shared across threads.
#[cfg(unix)]
fn take_resume_action(pending: &AtomicU8) -> ResumeAction {
    match pending.swap(ResumeAction::None as u8, Ordering::Relaxed) {
        1 => ResumeAction::RealignInline,
        2 => ResumeAction::RestoreAlt,
        _ => ResumeAction::None,
    }
}

/// Handle that lets subsystems ask for a redraw without locking the terminal.
///
/// Requests are queued onto an unbounded channel and coalesced by the background task spawned in
/// [`Tui::new`]. `schedule_frame_in` exists so call sites can defer work—for example to debounce
/// status updates in [`BottomPane`].
///
/// [`BottomPane`]: crate::bottom_pane::BottomPane
#[derive(Clone, Debug)]
pub struct FrameRequester {
    /// Handle to the shared frame scheduler; sending instants through this channel triggers draws
    /// once the coalescer decides the deadline has arrived.
    frame_schedule_tx: tokio::sync::mpsc::UnboundedSender<Instant>,
}

impl FrameRequester {
    /// Request an immediate redraw.
    ///
    /// The scheduler collapses concurrent requests into a single `Draw`, so callers can
    /// fire-and-forget without coordinating.
    pub fn schedule_frame(&self) {
        let _ = self.frame_schedule_tx.send(Instant::now());
    }
    /// Request a redraw no earlier than `dur` in the future.
    ///
    /// Callers use this to debounce follow-up frames (for example, to animate a spinner while
    /// waiting on the network). The scheduler still collapses multiple pending deadlines to the
    /// earliest instant.
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
    /// Emit an OSC 9 desktop notification if the terminal pane lacks focus.
    ///
    /// Returns `true` when a notification escape sequence was written. We only notify when the pane
    /// is unfocused so OSC 9 terminals (iTerm2, Kitty, WezTerm) avoid redundant alerts. Terminals
    /// that ignore OSC 9 fail silently.
    pub fn notify(&mut self, message: impl AsRef<str>) -> bool {
        if !self.terminal_focused.load(Ordering::Relaxed) {
            let _ = execute!(stdout(), PostNotification(message.as_ref().to_string()));
            true
        } else {
            false
        }
    }

    /// Construct the controller around an already-configured terminal backend.
    ///
    /// - Spawns the background task that coalesces frame requests into `Draw` events, avoiding
    ///   flicker when history lines arrive in bursts.
    /// - Primes capability checks that require talking to the terminal driver so they do not race
    ///   the async event reader.
    /// - Differs from Ratatui's `Terminal::new`, which performs these probes lazily and assumes
    ///   exclusive control of the event loop.
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
                            let _ = draw_tx_clone.send(());
                        }
                    }
                }
            }
        });

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
            resume_pending: Arc::new(AtomicU8::new(0)),
            #[cfg(unix)]
            suspend_cursor_y: Arc::new(AtomicU16::new(0)),
            alt_screen_active: Arc::new(AtomicBool::new(false)),
            terminal_focused: Arc::new(AtomicBool::new(true)),
            enhanced_keys_supported,
        }
    }

    /// Return a cloneable handle for requesting future draws.
    ///
    /// Widgets hold onto this so they can redraw from async callbacks without needing mutable
    /// access to the `Tui`. The returned handle is cheap to clone because it only contains the
    /// underlying channel sender.
    pub fn frame_requester(&self) -> FrameRequester {
        FrameRequester {
            frame_schedule_tx: self.frame_schedule_tx.clone(),
        }
    }

    /// Returns whether the current terminal reported support for Crossterm's keyboard enhancement
    /// flags.
    ///
    /// Consumers use this to decide whether to show modifier-specific key hints. We cache the probe
    /// result because the underlying API talks to the terminal driver, and repeated checks would
    /// contend with the event reader.
    pub fn enhanced_keys_supported(&self) -> bool {
        self.enhanced_keys_supported
    }

    /// Build the async stream that drives the UI loop.
    ///
    /// - Merges raw `crossterm` events with draw notifications from the frame scheduler.
    /// - Handles suspend/resume (`Ctrl+Z`) without exposing implementation details to callers.
    /// - Translates focus events into redraws so palette refreshes take effect immediately.
    /// - Surfaces bracketed paste as a first-class variant instead of leaving it to widgets.
    /// - Disables the alternate scroll escape on exit from alt-screen mode so the shell prompt
    ///   behaves normally.
    pub fn event_stream(&self) -> Pin<Box<dyn Stream<Item = TuiEvent> + Send + 'static>> {
        use tokio_stream::StreamExt;
        let mut crossterm_events = crossterm::event::EventStream::new();
        let mut draw_rx = self.draw_tx.subscribe();
        #[cfg(unix)]
        let resume_pending = self.resume_pending.clone();
        #[cfg(unix)]
        let alt_screen_active = self.alt_screen_active.clone();
        #[cfg(unix)]
        let suspend_cursor_y = self.suspend_cursor_y.clone();
        let terminal_focused = self.terminal_focused.clone();
        let event_stream = async_stream::stream! {
            loop {
                select! {
                    Some(Ok(event)) = crossterm_events.next() => {
                        match event {
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
                                        // Disable alternate scroll when suspending from alt-screen
                                        let _ = execute!(stdout(), DisableAlternateScroll);
                                        let _ = execute!(stdout(), LeaveAlternateScreen);
                                        resume_pending.store(
                                            ResumeAction::RestoreAlt as u8,
                                            Ordering::Relaxed,
                                        );
                                    } else {
                                        resume_pending.store(
                                            ResumeAction::RealignInline as u8,
                                            Ordering::Relaxed,
                                        );
                                    }
                                    #[cfg(unix)]
                                    {
                                        let y = suspend_cursor_y.load(Ordering::Relaxed);
                                        let _ = execute!(stdout(), MoveTo(0, y));
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
                                // We dropped draw notifications; merge the backlog into one draw.
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

    /// Suspend the process after restoring terminal modes.
    ///
    /// Triggered internally when the user presses `Ctrl+Z`. The cursor is moved below the inline
    /// viewport before the signal so the shell prompt appears beneath the UI once the process
    /// stops. On resume [`Tui::draw`] applies any queued viewport adjustments.
    #[cfg(unix)]
    fn suspend() -> Result<()> {
        restore()?;
        unsafe { libc::kill(0, libc::SIGTSTP) };
        set_modes()?;
        Ok(())
    }

    /// Figure out what needs to happen after a suspend before we enter the synchronized update.
    ///
    /// We determine whether to realign the inline viewport or re-enter the alt-screen outside of
    /// the synchronized update lock because querying the cursor while holding the lock can hang in
    /// some terminals (WezTerm in particular). Errors from `get_cursor_position` are tolerated so
    /// that resume never panics; we fall back to the last known coordinates instead.
    #[cfg(unix)]
    fn prepare_resume_action(
        &mut self,
        action: ResumeAction,
    ) -> Result<Option<PreparedResumeAction>> {
        match action {
            ResumeAction::RealignInline => {
                let cursor_pos = self
                    .terminal
                    .get_cursor_position()
                    .unwrap_or(self.terminal.last_known_cursor_pos);
                Ok(Some(PreparedResumeAction::RealignViewport(
                    ratatui::layout::Rect::new(0, cursor_pos.y, 0, 0),
                )))
            }
            ResumeAction::RestoreAlt => {
                if let Ok(ratatui::layout::Position { y, .. }) = self.terminal.get_cursor_position()
                    && let Some(saved) = self.alt_saved_viewport.as_mut()
                {
                    saved.y = y;
                }
                Ok(Some(PreparedResumeAction::RestoreAltScreen))
            }
            ResumeAction::None => Ok(None),
        }
    }

    /// Apply the previously prepared post-resume action inside the synchronized update.
    ///
    /// Replaying the action here ensures the viewport changes happen atomically with the frame
    /// render. When resuming an alt-screen overlay we re-enable the alternate scroll escape so
    /// mouse wheels keep mapping to arrow keys.
    #[cfg(unix)]
    fn apply_prepared_resume_action(&mut self, prepared: PreparedResumeAction) -> Result<()> {
        match prepared {
            PreparedResumeAction::RealignViewport(area) => {
                self.terminal.set_viewport_area(area);
            }
            PreparedResumeAction::RestoreAltScreen => {
                execute!(self.terminal.backend_mut(), EnterAlternateScreen)?;
                // Enable "alternate scroll" so terminals may translate wheel to arrows
                execute!(self.terminal.backend_mut(), EnableAlternateScroll)?;
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

    /// Enter the alternate screen, expanding the viewport to the full terminal.
    ///
    /// We snapshot the inline viewport bounds so that leaving the alt screen can restore the inline
    /// history view exactly where it left off. Alternate scroll support is enabled here so mouse
    /// wheels map to arrow presses while the overlay is active—a deliberate deviation from
    /// Ratatui, where alt screen mode is the default rather than an opt-in overlay.
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

    /// Leave the alternate screen and restore the inline viewport, if present.
    ///
    /// Alternate scroll is disabled before dropping back to the inline viewport so that shells do
    /// not inherit the mapping. If we resumed from a suspend while in the alt screen, the viewport
    /// coordinates were already updated in [`prepare_resume_action`].
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

    /// Queue history lines to be spliced above the inline viewport.
    ///
    /// The lines are copied into a pending buffer and applied immediately before the next draw (see
    /// [`draw`]). This matches our approach of letting the terminal own the transcript so selection
    /// and scrollback behave like a regular terminal log. Callers such as
    /// [`App::handle_event`] use it whenever new transcript cells render.
    ///
    /// [`App::handle_event`]: crate::app::App::handle_event
    pub fn insert_history_lines(&mut self, lines: Vec<Line<'static>>) {
        self.pending_history_lines.extend(lines);
        self.frame_requester().schedule_frame();
    }

    /// Render a frame inside the managed viewport.
    ///
    /// - `height` caps how tall the inline viewport may grow for this frame; the draw closure
    ///   receives the same `Frame` type that Ratatui exposes.
    /// - Gathers cursor-dependent state (notably viewport alignment) before the synchronized update
    ///   lock so terminals such as WezTerm avoid cursor-query deadlocks.
    /// - Applies suspend/resume bookkeeping so `Ctrl+Z` returns to the same viewport layout.
    /// - Updates the viewport, splices pending history lines, refreshes the suspend cursor marker,
    ///   and finally delegates to the caller's draw logic.
    ///
    /// Compared to Ratatui's stock `Terminal::draw`, the key differences are inline viewport
    /// management and history injection; the rest of the rendering pipeline remains unchanged.
    /// Primary callers include [`App::handle_tui_event`] for the main chat loop and overlay flows
    /// such as [`run_update_prompt_if_needed`] and [`run_resume_picker`].
    ///
    /// [`App::handle_tui_event`]: crate::app::App::handle_tui_event
    /// [`run_update_prompt_if_needed`]: crate::update_prompt::run_update_prompt_if_needed
    /// [`run_resume_picker`]: crate::resume_picker::run_resume_picker
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
            if screen_size != last_known_screen_size
                && let Ok(cursor_pos) = terminal.get_cursor_position()
            {
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

        // Use synchronized update via backend instead of stdout()
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
                self.suspend_cursor_y
                    .store(inline_area_bottom, Ordering::Relaxed);
            }
            terminal.draw(|frame| {
                draw_fn(frame);
            })
        })?
    }
}

/// Command that emits an OSC 9 desktop notification with a message.
///
/// Only a subset of terminals (iTerm2, Kitty, WezTerm) honor OSC 9; others ignore the escape
/// sequence, which is acceptable because [`Tui::notify`] treats write errors as non-fatal.
#[derive(Debug, Clone)]
pub struct PostNotification(
    /// Message to surface via the OSC 9 escape sequence.
    pub String,
);

impl Command for PostNotification {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        write!(f, "\x1b]9;{}\x07", self.0)
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        Err(std::io::Error::other(
            "tried to execute PostNotification using WinAPI; use ANSI instead",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}
