use std::fmt;
use std::future::Future;
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
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::layout::Size;
use ratatui::text::Line;
use tokio::sync::broadcast;
use tokio_stream::Stream;

pub use self::frame_requester::FrameRequester;
use crate::custom_terminal;
use crate::custom_terminal::Terminal as CustomTerminal;
use crate::insert_history::InsertHistoryMode;
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

struct PendingHistoryInsert {
    lines: Vec<Line<'static>>,
    mode: InsertHistoryMode,
}

fn should_emit_notification(condition: NotificationCondition, terminal_focused: bool) -> bool {
    match condition {
        NotificationCondition::Unfocused => !terminal_focused,
        NotificationCondition::Always => true,
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::io::Write;
    use std::ops::Range;

    use super::Tui;
    use super::should_emit_notification;
    use crate::custom_terminal::Terminal as CustomTerminal;
    use codex_config::types::NotificationCondition;
    use pretty_assertions::assert_eq;
    use ratatui::backend::Backend;
    use ratatui::backend::ClearType;
    use ratatui::backend::WindowSize;
    use ratatui::buffer::Cell;
    use ratatui::layout::Position;
    use ratatui::layout::Rect;
    use ratatui::layout::Size;
    use ratatui::text::Line;

    #[derive(Debug)]
    struct RecordingBackend {
        size: Size,
        cursor_position: Position,
        clear_positions: Vec<Position>,
        scrolls_up: Vec<(Range<u16>, u16)>,
    }

    impl RecordingBackend {
        fn new(width: u16, height: u16) -> Self {
            Self {
                size: Size::new(width, height),
                cursor_position: Position { x: 0, y: 0 },
                clear_positions: Vec::new(),
                scrolls_up: Vec::new(),
            }
        }

        fn set_size(&mut self, width: u16, height: u16) {
            self.size = Size::new(width, height);
        }

        fn set_cursor_position(&mut self, x: u16, y: u16) {
            self.cursor_position = Position { x, y };
        }
    }

    impl Write for RecordingBackend {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Backend for RecordingBackend {
        fn draw<'a, I>(&mut self, _content: I) -> io::Result<()>
        where
            I: Iterator<Item = (u16, u16, &'a Cell)>,
        {
            Ok(())
        }

        fn hide_cursor(&mut self) -> io::Result<()> {
            Ok(())
        }

        fn show_cursor(&mut self) -> io::Result<()> {
            Ok(())
        }

        fn get_cursor_position(&mut self) -> io::Result<Position> {
            Ok(self.cursor_position)
        }

        fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
            self.cursor_position = position.into();
            Ok(())
        }

        fn clear(&mut self) -> io::Result<()> {
            Ok(())
        }

        fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
            assert_eq!(clear_type, ClearType::AfterCursor);
            self.clear_positions.push(self.cursor_position);
            Ok(())
        }

        fn size(&self) -> io::Result<Size> {
            Ok(self.size)
        }

        fn window_size(&mut self) -> io::Result<WindowSize> {
            Ok(WindowSize {
                columns_rows: self.size,
                pixels: Size::new(/*width*/ 640, /*height*/ 480),
            })
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }

        fn scroll_region_up(&mut self, region: Range<u16>, scroll_by: u16) -> io::Result<()> {
            self.scrolls_up.push((region, scroll_by));
            Ok(())
        }

        fn scroll_region_down(&mut self, _region: Range<u16>, _scroll_by: u16) -> io::Result<()> {
            Ok(())
        }
    }

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

    #[test]
    fn height_shrink_reanchors_inline_viewport_without_scrolling_visible_rows() {
        let mut terminal = CustomTerminal::with_options(RecordingBackend::new(
            /*width*/ 80, /*height*/ 10,
        ))
        .expect("terminal");
        terminal.set_viewport_area(Rect::new(
            /*x*/ 0, /*y*/ 7, /*width*/ 80, /*height*/ 3,
        ));
        terminal.backend_mut().set_size(/*width*/ 80, /*height*/ 6);

        let needs_full_repaint = Tui::update_inline_viewport(
            &mut terminal,
            /*height*/ 3,
            /*is_zellij*/ false,
            /*terminal_resize_reflow_enabled*/ true,
        )
        .expect("update viewport");

        assert!(needs_full_repaint);
        assert_eq!(terminal.viewport_area, Rect::new(0, 3, 80, 3));
        assert!(terminal.backend().scrolls_up.is_empty());
        assert_eq!(
            terminal.backend().clear_positions,
            vec![Position { x: 0, y: 3 }]
        );
    }

    #[test]
    fn legacy_height_shrink_scrolls_visible_rows() {
        let mut terminal = CustomTerminal::with_options(RecordingBackend::new(
            /*width*/ 80, /*height*/ 10,
        ))
        .expect("terminal");
        terminal.set_viewport_area(Rect::new(
            /*x*/ 0, /*y*/ 7, /*width*/ 80, /*height*/ 3,
        ));
        terminal.backend_mut().set_size(/*width*/ 80, /*height*/ 6);

        let needs_full_repaint = Tui::update_inline_viewport(
            &mut terminal,
            /*height*/ 3,
            /*is_zellij*/ false,
            /*terminal_resize_reflow_enabled*/ false,
        )
        .expect("update viewport");

        assert!(!needs_full_repaint);
        assert_eq!(
            terminal.viewport_area,
            Rect::new(
                /*x*/ 0, /*y*/ 3, /*width*/ 80, /*height*/ 3
            )
        );
        assert_eq!(terminal.backend().scrolls_up, vec![(0..7, 4)]);
        assert_eq!(
            terminal.backend().clear_positions,
            vec![Position { x: 0, y: 7 }]
        );
    }

    #[test]
    fn height_shrink_ignores_cursor_offset_heuristic_before_resize_event() {
        let mut terminal = CustomTerminal::with_options(RecordingBackend::new(
            /*width*/ 80, /*height*/ 10,
        ))
        .expect("terminal");
        terminal.set_viewport_area(Rect::new(
            /*x*/ 0, /*y*/ 7, /*width*/ 80, /*height*/ 3,
        ));
        terminal.last_known_cursor_pos = Position { x: 10, y: 9 };
        terminal
            .backend_mut()
            .set_cursor_position(/*x*/ 10, /*y*/ 1);
        terminal.backend_mut().set_size(/*width*/ 80, /*height*/ 6);

        assert_eq!(
            Tui::pending_viewport_area_for_terminal(
                &mut terminal,
                /*terminal_resize_reflow_enabled*/ true
            )
            .expect("pending viewport"),
            None
        );

        let needs_full_repaint = Tui::update_inline_viewport(
            &mut terminal,
            /*height*/ 3,
            /*is_zellij*/ false,
            /*terminal_resize_reflow_enabled*/ true,
        )
        .expect("update viewport");

        assert!(needs_full_repaint);
        assert_eq!(
            terminal.viewport_area,
            Rect::new(
                /*x*/ 0, /*y*/ 3, /*width*/ 80, /*height*/ 3
            )
        );

        crate::insert_history::insert_history_lines(&mut terminal, vec![Line::from("history")])
            .expect("insert history");

        assert_eq!(
            terminal.viewport_area,
            Rect::new(
                /*x*/ 0, /*y*/ 3, /*width*/ 80, /*height*/ 3
            )
        );
    }

    #[test]
    fn legacy_height_resize_preserves_cursor_offset_heuristic() {
        let mut terminal = CustomTerminal::with_options(RecordingBackend::new(
            /*width*/ 80, /*height*/ 10,
        ))
        .expect("terminal");
        terminal.set_viewport_area(Rect::new(
            /*x*/ 0, /*y*/ 7, /*width*/ 80, /*height*/ 3,
        ));
        terminal.last_known_cursor_pos = Position { x: 10, y: 9 };
        terminal
            .backend_mut()
            .set_cursor_position(/*x*/ 10, /*y*/ 1);
        terminal.backend_mut().set_size(/*width*/ 80, /*height*/ 6);

        assert_eq!(
            Tui::pending_viewport_area_for_terminal(
                &mut terminal,
                /*terminal_resize_reflow_enabled*/ false
            )
            .expect("pending viewport"),
            Some(Rect::new(
                /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 3,
            ))
        );
    }

    #[test]
    fn height_shrink_preserves_floating_inline_viewport_when_it_still_fits() {
        let mut terminal = CustomTerminal::with_options(RecordingBackend::new(
            /*width*/ 80, /*height*/ 10,
        ))
        .expect("terminal");
        terminal.set_viewport_area(Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 3,
        ));
        terminal.backend_mut().set_size(/*width*/ 80, /*height*/ 6);

        let needs_full_repaint = Tui::update_inline_viewport(
            &mut terminal,
            /*height*/ 3,
            /*is_zellij*/ false,
            /*terminal_resize_reflow_enabled*/ true,
        )
        .expect("update viewport");

        assert!(!needs_full_repaint);
        assert_eq!(
            terminal.viewport_area,
            Rect::new(
                /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 3
            )
        );
        assert!(terminal.backend().scrolls_up.is_empty());
        assert!(terminal.backend().clear_positions.is_empty());
    }

    #[test]
    fn resize_reflow_reinserts_history_without_moving_viewport() {
        let mut terminal = CustomTerminal::with_options(RecordingBackend::new(
            /*width*/ 80, /*height*/ 10,
        ))
        .expect("terminal");
        terminal.set_viewport_area(Rect::new(
            /*x*/ 0, /*y*/ 2, /*width*/ 80, /*height*/ 3,
        ));
        terminal.backend_mut().set_size(/*width*/ 80, /*height*/ 6);

        crate::insert_history::insert_history_lines_with_mode(
            &mut terminal,
            vec![Line::from("reflowed history")],
            crate::insert_history::InsertHistoryMode::StandardPreserveViewport,
        )
        .expect("insert history");

        assert_eq!(
            terminal.viewport_area,
            Rect::new(
                /*x*/ 0, /*y*/ 2, /*width*/ 80, /*height*/ 3
            )
        );
    }

    #[test]
    fn resize_reflow_width_only_resize_ignores_cursor_offset_heuristic() {
        let mut terminal = CustomTerminal::with_options(RecordingBackend::new(
            /*width*/ 80, /*height*/ 10,
        ))
        .expect("terminal");
        terminal.set_viewport_area(Rect::new(
            /*x*/ 0, /*y*/ 7, /*width*/ 80, /*height*/ 3,
        ));
        terminal.last_known_cursor_pos = Position { x: 10, y: 9 };
        terminal
            .backend_mut()
            .set_cursor_position(/*x*/ 10, /*y*/ 8);
        terminal.backend_mut().set_size(/*width*/ 60, /*height*/ 10);

        assert_eq!(
            Tui::pending_viewport_area_for_terminal(
                &mut terminal,
                /*terminal_resize_reflow_enabled*/ true
            )
            .expect("pending viewport"),
            None
        );
    }

    #[test]
    fn legacy_width_only_resize_preserves_cursor_offset_heuristic() {
        let mut terminal = CustomTerminal::with_options(RecordingBackend::new(
            /*width*/ 80, /*height*/ 10,
        ))
        .expect("terminal");
        terminal.set_viewport_area(Rect::new(
            /*x*/ 0, /*y*/ 7, /*width*/ 80, /*height*/ 3,
        ));
        terminal.last_known_cursor_pos = Position { x: 10, y: 9 };
        terminal
            .backend_mut()
            .set_cursor_position(/*x*/ 10, /*y*/ 8);
        terminal.backend_mut().set_size(/*width*/ 60, /*height*/ 10);

        assert_eq!(
            Tui::pending_viewport_area_for_terminal(
                &mut terminal,
                /*terminal_resize_reflow_enabled*/ false
            )
            .expect("pending viewport"),
            Some(Rect::new(
                /*x*/ 0, /*y*/ 6, /*width*/ 80, /*height*/ 3,
            ))
        );
    }

    #[test]
    fn stable_height_viewport_growth_still_scrolls_history_above_viewport() {
        let mut terminal = CustomTerminal::with_options(RecordingBackend::new(
            /*width*/ 80, /*height*/ 10,
        ))
        .expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 7, 80, 3));

        let needs_full_repaint = Tui::update_inline_viewport(
            &mut terminal,
            /*height*/ 5,
            /*is_zellij*/ false,
            /*terminal_resize_reflow_enabled*/ true,
        )
        .expect("update viewport");

        assert!(needs_full_repaint);
        assert_eq!(terminal.viewport_area, Rect::new(0, 5, 80, 5));
        assert_eq!(terminal.backend().scrolls_up, vec![(0..7, 2)]);
        assert_eq!(
            terminal.backend().clear_positions,
            vec![Position { x: 0, y: 5 }]
        );
    }

    #[test]
    fn height_growth_reanchors_bottom_aligned_inline_viewport() {
        let mut terminal = CustomTerminal::with_options(RecordingBackend::new(
            /*width*/ 80, /*height*/ 6,
        ))
        .expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 3, 80, 3));
        terminal.backend_mut().set_size(/*width*/ 80, /*height*/ 10);

        let needs_full_repaint = Tui::update_inline_viewport(
            &mut terminal,
            /*height*/ 3,
            /*is_zellij*/ false,
            /*terminal_resize_reflow_enabled*/ true,
        )
        .expect("update viewport");

        assert!(needs_full_repaint);
        assert_eq!(terminal.viewport_area, Rect::new(0, 7, 80, 3));
        assert!(terminal.backend().scrolls_up.is_empty());
        assert_eq!(
            terminal.backend().clear_positions,
            vec![Position { x: 0, y: 3 }]
        );
    }

    #[test]
    fn legacy_height_growth_keeps_existing_inline_viewport_position() {
        let mut terminal = CustomTerminal::with_options(RecordingBackend::new(
            /*width*/ 80, /*height*/ 6,
        ))
        .expect("terminal");
        terminal.set_viewport_area(Rect::new(
            /*x*/ 0, /*y*/ 3, /*width*/ 80, /*height*/ 3,
        ));
        terminal.backend_mut().set_size(/*width*/ 80, /*height*/ 10);

        let needs_full_repaint = Tui::update_inline_viewport(
            &mut terminal,
            /*height*/ 3,
            /*is_zellij*/ false,
            /*terminal_resize_reflow_enabled*/ false,
        )
        .expect("update viewport");

        assert!(!needs_full_repaint);
        assert_eq!(
            terminal.viewport_area,
            Rect::new(
                /*x*/ 0, /*y*/ 3, /*width*/ 80, /*height*/ 3
            )
        );
        assert!(terminal.backend().scrolls_up.is_empty());
        assert!(terminal.backend().clear_positions.is_empty());
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

#[derive(Clone, Debug)]
pub enum TuiEvent {
    Key(KeyEvent),
    Paste(String),
    Resize,
    Draw,
}

pub struct Tui {
    frame_requester: FrameRequester,
    draw_tx: broadcast::Sender<()>,
    event_broker: Arc<EventBroker>,
    pub(crate) terminal: Terminal,
    pending_history_inserts: Vec<PendingHistoryInsert>,
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
    terminal_resize_reflow_enabled: bool,
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
        let is_zellij = matches!(
            codex_terminal_detection::terminal_info().multiplexer,
            Some(codex_terminal_detection::Multiplexer::Zellij {})
        );

        Self {
            frame_requester,
            draw_tx,
            event_broker: Arc::new(EventBroker::new()),
            terminal,
            pending_history_inserts: vec![],
            alt_saved_viewport: None,
            #[cfg(unix)]
            suspend_context: SuspendContext::new(),
            alt_screen_active: Arc::new(AtomicBool::new(false)),
            terminal_focused: Arc::new(AtomicBool::new(true)),
            enhanced_keys_supported,
            notification_backend: Some(detect_backend(NotificationMethod::default())),
            notification_condition: NotificationCondition::default(),
            is_zellij,
            terminal_resize_reflow_enabled: false,
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

    pub(crate) fn set_terminal_resize_reflow_enabled(&mut self, enabled: bool) {
        self.terminal_resize_reflow_enabled = enabled;
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
        self.queue_history_lines(lines, InsertHistoryMode::new(self.is_zellij));
    }

    pub(crate) fn insert_reflowed_history_lines(&mut self, lines: Vec<Line<'static>>) {
        self.queue_history_lines(
            lines,
            InsertHistoryMode::new_preserving_viewport(self.is_zellij),
        );
    }

    fn queue_history_lines(&mut self, lines: Vec<Line<'static>>, mode: InsertHistoryMode) {
        if lines.is_empty() {
            return;
        }
        if let Some(last) = self.pending_history_inserts.last_mut()
            && last.mode == mode
        {
            last.lines.extend(lines);
            self.frame_requester().schedule_frame();
            return;
        }
        self.pending_history_inserts
            .push(PendingHistoryInsert { lines, mode });
        self.frame_requester().schedule_frame();
    }

    /// Drop any queued history lines that have not yet been flushed to the terminal.
    pub fn clear_pending_history_lines(&mut self) {
        self.pending_history_inserts.clear();
    }

    /// Resize the inline viewport to `height` rows, scrolling content above it if
    /// the viewport would extend past the bottom of the screen. Returns `true` when
    /// the caller must invalidate the diff buffer (Zellij mode), because the scroll
    /// was performed with raw newlines that ratatui cannot track.
    fn update_inline_viewport<B>(
        terminal: &mut CustomTerminal<B>,
        height: u16,
        is_zellij: bool,
        terminal_resize_reflow_enabled: bool,
    ) -> Result<bool>
    where
        B: Backend + Write,
    {
        let size = terminal.size()?;
        let mut needs_full_repaint = false;
        let terminal_height_shrank = size.height < terminal.last_known_screen_size.height;
        let terminal_height_grew = size.height > terminal.last_known_screen_size.height;
        let viewport_was_bottom_aligned =
            terminal.viewport_area.bottom() == terminal.last_known_screen_size.height;
        let previous_area = terminal.viewport_area;

        let mut area = terminal.viewport_area;
        area.height = height.min(size.height);
        area.width = size.width;
        if area.bottom() > size.height {
            let scroll_by = area.bottom() - size.height;
            if !terminal_resize_reflow_enabled || !terminal_height_shrank {
                if is_zellij {
                    Self::scroll_zellij_expanded_viewport(terminal, size, scroll_by)?;
                    needs_full_repaint = true;
                } else {
                    terminal
                        .backend_mut()
                        .scroll_region_up(0..area.top(), scroll_by)?;
                }
            }
            area.y = size.height - area.height;
        } else if terminal_resize_reflow_enabled
            && terminal_height_grew
            && viewport_was_bottom_aligned
        {
            area.y = size.height - area.height;
        }
        if area != terminal.viewport_area {
            if terminal_resize_reflow_enabled {
                let clear_position = Position::new(/*x*/ 0, previous_area.y.min(area.y));
                terminal.set_viewport_area(area);
                terminal.clear_after_position(clear_position)?;
                needs_full_repaint = true;
            } else {
                terminal.clear()?;
                terminal.set_viewport_area(area);
            }
        }

        Ok(needs_full_repaint)
    }

    /// Push content above the viewport upward by `scroll_by` rows using raw
    /// newlines at the screen bottom. This is the Zellij-safe alternative to
    /// `scroll_region_up`, which relies on DECSTBM sequences Zellij does not
    /// support.
    fn scroll_zellij_expanded_viewport<B>(
        terminal: &mut CustomTerminal<B>,
        size: Size,
        scroll_by: u16,
    ) -> Result<()>
    where
        B: Backend + Write,
    {
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
        pending_history_inserts: &mut Vec<PendingHistoryInsert>,
    ) -> Result<bool> {
        if pending_history_inserts.is_empty() {
            return Ok(false);
        }

        let mut needs_full_repaint = false;
        for insert in pending_history_inserts.drain(..) {
            crate::insert_history::insert_history_lines_with_mode(
                terminal,
                insert.lines,
                insert.mode,
            )?;
            // Preserve-mode replays intentionally mutate terminal rows outside ratatui's normal
            // diff path. Repaint the viewport afterward so composer/status rows cannot stay stale
            // if the terminal scrolled or cleared adjacent rows during the replay.
            needs_full_repaint |= insert.mode.uses_zellij() || insert.mode.preserves_viewport();
        }
        Ok(needs_full_repaint)
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
        // the synchronized update, to avoid racing with the event reader. Explicit resize
        // events skip this heuristic because xterm.js can report stale cursor positions while
        // a blurred split pane is being resized rapidly.
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

            let mut needs_full_repaint = Self::update_inline_viewport(
                terminal,
                height,
                self.is_zellij,
                self.terminal_resize_reflow_enabled,
            )?;
            needs_full_repaint |=
                Self::flush_pending_history_lines(terminal, &mut self.pending_history_inserts)?;

            if needs_full_repaint {
                terminal.clear()?;
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
        Self::pending_viewport_area_for_terminal(
            &mut self.terminal,
            self.terminal_resize_reflow_enabled,
        )
    }

    fn pending_viewport_area_for_terminal<B>(
        terminal: &mut CustomTerminal<B>,
        terminal_resize_reflow_enabled: bool,
    ) -> Result<Option<Rect>>
    where
        B: Backend + Write,
    {
        let screen_size = terminal.size()?;
        let last_known_screen_size = terminal.last_known_screen_size;
        let width_changed = screen_size.width != last_known_screen_size.width;
        let height_changed = screen_size.height != last_known_screen_size.height;
        let should_apply_cursor_heuristic =
            !terminal_resize_reflow_enabled && (width_changed || height_changed);
        if should_apply_cursor_heuristic && let Ok(cursor_pos) = terminal.get_cursor_position() {
            let last_known_cursor_pos = terminal.last_known_cursor_pos;
            // The legacy path uses terminal cursor drift as a viewport hint. Resize reflow owns the
            // transcript anchor instead, because native terminal rewrap can move the cursor without
            // meaning Codex's inline viewport should move.
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
