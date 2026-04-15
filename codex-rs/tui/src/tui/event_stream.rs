//! Event stream plumbing for the TUI.
//!
//! - [`EventBroker`] holds the shared crossterm stream so multiple callers reuse the same
//!   input source and can drop/recreate it on pause/resume without rebuilding consumers.
//! - [`TuiEventStream`] wraps a draw event subscription plus the shared [`EventBroker`] and maps crossterm
//!   events into [`TuiEvent`].
//! - [`EventSource`] abstracts the underlying event producer; the real implementation is
//!   [`CrosstermEventSource`] and tests can swap in [`FakeEventSource`].
//!
//! The motivation for dropping/recreating the crossterm event stream is to enable the TUI to fully relinquish stdin.
//! If the stream is not dropped, it will continue to read from stdin even if it is not actively being polled
//! (due to how crossterm's EventStream is implemented), potentially stealing input from other processes reading stdin,
//! like terminal text editors. This race can cause missed input or capturing terminal query responses (for example, OSC palette/size queries)
//! that the other process expects to read. Stopping polling, instead of dropping the stream, is only sufficient when the
//! pause happens before the stream enters a pending state; otherwise the crossterm reader thread may keep reading
//! from stdin, so the safer approach is to drop and recreate the event stream when we need to hand off the terminal.
//!
//! See https://ratatui.rs/recipes/apps/spawn-vim/ and https://www.reddit.com/r/rust/comments/1f3o33u/myterious_crossterm_input_after_running_vim for more details.

use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;

use codex_terminal_detection::TerminalInfo;
use codex_terminal_detection::TerminalName;
use crossterm::event::Event;
use crossterm::terminal;
use tokio::sync::broadcast;
use tokio::sync::watch;
use tokio::time::Interval;
use tokio::time::MissedTickBehavior;
use tokio_stream::Stream;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::WatchStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

use super::TuiEvent;

const SUPERSET_RESIZE_WATCHDOG_INTERVAL: Duration = Duration::from_millis(100);

/// Result type produced by an event source.
pub type EventResult = std::io::Result<Event>;

/// Abstraction over a source of terminal events. Allows swapping in a fake for tests.
/// Value in production is [`CrosstermEventSource`].
pub trait EventSource: Send + 'static {
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<EventResult>>;
}

type TerminalSizeReader = Arc<dyn Fn() -> std::io::Result<(u16, u16)> + Send + Sync>;

struct ResizeWatchdog {
    interval: Interval,
    last_size: Option<(u16, u16)>,
    read_size: TerminalSizeReader,
    logged_read_error: bool,
}

impl ResizeWatchdog {
    fn new(interval_duration: Duration, read_size: TerminalSizeReader) -> Self {
        let mut interval = tokio::time::interval(interval_duration);
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        Self {
            interval,
            last_size: None,
            read_size,
            logged_read_error: false,
        }
    }

    fn superset() -> Self {
        Self::new(SUPERSET_RESIZE_WATCHDOG_INTERVAL, Arc::new(terminal::size))
    }

    fn set_observed_size(&mut self, size: (u16, u16)) {
        self.last_size = Some(size);
    }

    fn observe_current_size(&mut self) -> bool {
        let size = match (self.read_size)() {
            Ok(size) => {
                if self.logged_read_error {
                    tracing::debug!(
                        event = "resize_watchdog_size_read_recovered",
                        "resize watchdog size reads recovered"
                    );
                    self.logged_read_error = false;
                }
                size
            }
            Err(err) => {
                if !self.logged_read_error {
                    tracing::warn!(
                        event = "resize_watchdog_size_read_failed",
                        error = %err,
                        "resize watchdog failed to read terminal size"
                    );
                    self.logged_read_error = true;
                }
                return false;
            }
        };
        let previous_size = self.last_size;
        let changed = previous_size.is_some_and(|last_size| last_size != size);
        self.last_size = Some(size);
        match previous_size {
            Some((old_cols, old_rows)) if changed => {
                tracing::info!(
                    event = "resize_watchdog_size_changed",
                    old_cols,
                    old_rows,
                    new_cols = size.0,
                    new_rows = size.1,
                    "resize watchdog detected terminal size change"
                );
            }
            None => {
                tracing::info!(
                    event = "resize_watchdog_size_seeded",
                    cols = size.0,
                    rows = size.1,
                    "resize watchdog seeded terminal size"
                );
            }
            Some(_) => {}
        }
        changed
    }
}

fn should_enable_superset_resize_watchdog(
    terminal_info: &TerminalInfo,
    has_superset_env_markers: bool,
) -> bool {
    terminal_info.name == TerminalName::Superset || has_superset_env_markers
}

fn has_superset_env_markers() -> bool {
    std::env::var_os("SUPERSET_TERMINAL_ID").is_some()
        && std::env::var_os("SUPERSET_WORKSPACE_ID").is_some()
}

/// Shared crossterm input state for all [`TuiEventStream`] instances. A single crossterm EventStream
/// is reused so all streams still see the same input source.
///
/// This intermediate layer enables dropping/recreating the underlying EventStream (pause/resume) without rebuilding consumers.
pub struct EventBroker<S: EventSource = CrosstermEventSource> {
    state: Mutex<EventBrokerState<S>>,
    resume_events_tx: watch::Sender<()>,
}

/// Tracks state of underlying [`EventSource`].
enum EventBrokerState<S: EventSource> {
    Paused,     // Underlying event source (i.e., crossterm EventStream) dropped
    Start,      // A new event source will be created on next poll
    Running(S), // Event source is currently running
}

impl<S: EventSource + Default> EventBrokerState<S> {
    /// Return the running event source, starting it if needed; None when paused.
    fn active_event_source_mut(&mut self) -> Option<&mut S> {
        match self {
            EventBrokerState::Paused => None,
            EventBrokerState::Start => {
                *self = EventBrokerState::Running(S::default());
                match self {
                    EventBrokerState::Running(events) => Some(events),
                    EventBrokerState::Paused | EventBrokerState::Start => unreachable!(),
                }
            }
            EventBrokerState::Running(events) => Some(events),
        }
    }
}

impl<S: EventSource + Default> EventBroker<S> {
    pub fn new() -> Self {
        let (resume_events_tx, _resume_events_rx) = watch::channel(());
        Self {
            state: Mutex::new(EventBrokerState::Start),
            resume_events_tx,
        }
    }

    /// Drop the underlying event source
    pub fn pause_events(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *state = EventBrokerState::Paused;
    }

    /// Create a new instance of the underlying event source
    pub fn resume_events(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *state = EventBrokerState::Start;
        let _ = self.resume_events_tx.send(());
    }

    /// Subscribe to a notification that fires whenever [`Self::resume_events`] is called.
    ///
    /// This is used to wake `poll_crossterm_event` when it is paused and waiting for the
    /// underlying crossterm stream to be recreated.
    pub fn resume_events_rx(&self) -> watch::Receiver<()> {
        self.resume_events_tx.subscribe()
    }
}

/// Real crossterm-backed event source.
pub struct CrosstermEventSource(pub crossterm::event::EventStream);

impl Default for CrosstermEventSource {
    fn default() -> Self {
        Self(crossterm::event::EventStream::new())
    }
}

impl EventSource for CrosstermEventSource {
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<EventResult>> {
        Pin::new(&mut self.get_mut().0).poll_next(cx)
    }
}

/// TuiEventStream is a struct for reading TUI events (draws and user input).
/// Each instance has its own draw subscription (the draw channel is broadcast, so
/// multiple receivers are fine), while crossterm input is funneled through a
/// single shared [`EventBroker`] because crossterm uses a global stdin reader and
/// does not support fan-out. Multiple TuiEventStream instances can exist during the app lifetime
/// (for nested or sequential screens), but only one should be polled at a time,
/// otherwise one instance can consume ("steal") input events and the other will miss them.
pub struct TuiEventStream<S: EventSource + Default + Unpin = CrosstermEventSource> {
    broker: Arc<EventBroker<S>>,
    draw_stream: BroadcastStream<()>,
    resume_stream: WatchStream<()>,
    resize_watchdog: Option<ResizeWatchdog>,
    terminal_name: TerminalName,
    terminal_focused: Arc<AtomicBool>,
    poll_draw_first: bool,
    #[cfg(unix)]
    suspend_context: crate::tui::job_control::SuspendContext,
    #[cfg(unix)]
    alt_screen_active: Arc<AtomicBool>,
}

impl<S: EventSource + Default + Unpin> TuiEventStream<S> {
    pub fn new(
        broker: Arc<EventBroker<S>>,
        draw_rx: broadcast::Receiver<()>,
        terminal_focused: Arc<AtomicBool>,
        #[cfg(unix)] suspend_context: crate::tui::job_control::SuspendContext,
        #[cfg(unix)] alt_screen_active: Arc<AtomicBool>,
    ) -> Self {
        let resume_stream = WatchStream::from_changes(broker.resume_events_rx());
        let terminal_info = codex_terminal_detection::terminal_info();
        let has_superset_env_markers = has_superset_env_markers();
        let enable_resize_watchdog =
            should_enable_superset_resize_watchdog(&terminal_info, has_superset_env_markers);
        tracing::info!(
            event = "terminal_resize_event_stream_configured",
            terminal_name = ?terminal_info.name,
            term_program = ?terminal_info.term_program,
            term_program_version = ?terminal_info.version,
            term = ?terminal_info.term,
            colorterm = ?std::env::var("COLORTERM").ok(),
            target_os = std::env::consts::OS,
            target_arch = std::env::consts::ARCH,
            is_windows = cfg!(windows),
            is_macos = cfg!(target_os = "macos"),
            vscode_ipc_hook_cli_present = std::env::var_os("VSCODE_IPC_HOOK_CLI").is_some(),
            vscode_injection_present = std::env::var_os("VSCODE_INJECTION").is_some(),
            vscode_shell_login_present = std::env::var_os("VSCODE_SHELL_LOGIN").is_some(),
            vscode_cwd_present = std::env::var_os("VSCODE_CWD").is_some(),
            term_session_id_present = std::env::var_os("TERM_SESSION_ID").is_some(),
            lc_terminal_present = std::env::var_os("LC_TERMINAL").is_some(),
            resize_watchdog_enabled = enable_resize_watchdog,
            "configured terminal resize event stream"
        );
        tracing::info!(
            event = "superset_resize_watchdog_configured",
            enabled = enable_resize_watchdog,
            terminal_name = ?terminal_info.name,
            term_program = ?terminal_info.term_program,
            term_program_version = ?terminal_info.version,
            term = ?terminal_info.term,
            has_superset_env_markers,
            "configured Superset resize watchdog"
        );
        let resize_watchdog = enable_resize_watchdog.then(ResizeWatchdog::superset);
        Self {
            broker,
            draw_stream: BroadcastStream::new(draw_rx),
            resume_stream,
            resize_watchdog,
            terminal_name: terminal_info.name,
            terminal_focused,
            poll_draw_first: false,
            #[cfg(unix)]
            suspend_context,
            #[cfg(unix)]
            alt_screen_active,
        }
    }

    /// Poll the shared crossterm stream for the next mapped `TuiEvent`.
    ///
    /// This skips events we don't use (mouse events, etc.) and keeps polling until it yields
    /// a mapped event, hits `Pending`, or sees EOF/error. When the broker is paused, it drops
    /// the underlying stream and returns `Pending` to fully release stdin.
    pub fn poll_crossterm_event(&mut self, cx: &mut Context<'_>) -> Poll<Option<TuiEvent>> {
        // Some crossterm events map to None (e.g. FocusLost, mouse); loop so we keep polling
        // until we return a mapped event, hit Pending, or see EOF/error.
        loop {
            let poll_result = {
                let mut state = self
                    .broker
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let events = match state.active_event_source_mut() {
                    Some(events) => events,
                    None => {
                        drop(state);
                        // Poll resume_stream so resume_events wakes a stream paused here
                        match Pin::new(&mut self.resume_stream).poll_next(cx) {
                            Poll::Ready(Some(())) => continue,
                            Poll::Ready(None) => return Poll::Ready(None),
                            Poll::Pending => return Poll::Pending,
                        }
                    }
                };
                match Pin::new(events).poll_next(cx) {
                    Poll::Ready(Some(Ok(event))) => Some(event),
                    Poll::Ready(Some(Err(_))) | Poll::Ready(None) => {
                        *state = EventBrokerState::Start;
                        return Poll::Ready(None);
                    }
                    Poll::Pending => {
                        drop(state);
                        // Poll resume_stream so resume_events can wake us even while waiting on stdin
                        match Pin::new(&mut self.resume_stream).poll_next(cx) {
                            Poll::Ready(Some(())) => continue,
                            Poll::Ready(None) => return Poll::Ready(None),
                            Poll::Pending => return Poll::Pending,
                        }
                    }
                }
            };

            if let Some(mapped) = poll_result.and_then(|event| self.map_crossterm_event(event)) {
                return Poll::Ready(Some(mapped));
            }
        }
    }

    /// Poll the draw broadcast stream for the next draw event. Draw events are used to trigger a redraw of the TUI.
    pub fn poll_draw_event(&mut self, cx: &mut Context<'_>) -> Poll<Option<TuiEvent>> {
        match Pin::new(&mut self.draw_stream).poll_next(cx) {
            Poll::Ready(Some(Ok(()))) => {
                tracing::info!(
                    event = "tui_draw_event_received",
                    terminal_name = ?self.terminal_name,
                    target_os = std::env::consts::OS,
                    "received scheduled TUI draw event"
                );
                Poll::Ready(Some(TuiEvent::Draw))
            }
            Poll::Ready(Some(Err(BroadcastStreamRecvError::Lagged(_)))) => {
                tracing::warn!(
                    event = "tui_draw_event_lagged",
                    terminal_name = ?self.terminal_name,
                    target_os = std::env::consts::OS,
                    "scheduled TUI draw event lagged"
                );
                Poll::Ready(Some(TuiEvent::Draw))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_resize_watchdog(&mut self, cx: &mut Context<'_>) -> Poll<Option<TuiEvent>> {
        let Some(watchdog) = self.resize_watchdog.as_mut() else {
            return Poll::Pending;
        };

        loop {
            match Pin::new(&mut watchdog.interval).poll_tick(cx) {
                Poll::Ready(_) => {
                    if watchdog.observe_current_size() {
                        tracing::info!(
                            event = "resize_watchdog_resize_emitted",
                            "resize watchdog emitted synthetic resize event"
                        );
                        return Poll::Ready(Some(TuiEvent::Resize));
                    }
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }

    /// Map a crossterm event to a [`TuiEvent`], skipping events we don't use (mouse events, etc.).
    fn map_crossterm_event(&mut self, event: Event) -> Option<TuiEvent> {
        match event {
            Event::Key(key_event) => {
                #[cfg(unix)]
                if crate::tui::job_control::SUSPEND_KEY.is_press(key_event) {
                    let _ = self.suspend_context.suspend(&self.alt_screen_active);
                    return Some(TuiEvent::Draw);
                }
                Some(TuiEvent::Key(key_event))
            }
            Event::Resize(cols, rows) => {
                tracing::info!(
                    event = "crossterm_resize_event_received",
                    cols,
                    rows,
                    terminal_name = ?self.terminal_name,
                    target_os = std::env::consts::OS,
                    is_windows = cfg!(windows),
                    is_macos = cfg!(target_os = "macos"),
                    watchdog_enabled = self.resize_watchdog.is_some(),
                    "received crossterm terminal resize event"
                );
                if let Some(watchdog) = self.resize_watchdog.as_mut() {
                    watchdog.set_observed_size((cols, rows));
                }
                Some(TuiEvent::Resize)
            }
            Event::Paste(pasted) => Some(TuiEvent::Paste(pasted)),
            Event::FocusGained => {
                self.terminal_focused.store(true, Ordering::Relaxed);
                crate::terminal_palette::requery_default_colors();
                Some(TuiEvent::Draw)
            }
            Event::FocusLost => {
                self.terminal_focused.store(false, Ordering::Relaxed);
                None
            }
            _ => None,
        }
    }
}

impl<S: EventSource + Default + Unpin> Unpin for TuiEventStream<S> {}

impl<S: EventSource + Default + Unpin> Stream for TuiEventStream<S> {
    type Item = TuiEvent;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // approximate fairness + no starvation via round-robin.
        let draw_first = self.poll_draw_first;
        self.poll_draw_first = !self.poll_draw_first;

        if draw_first {
            if let Poll::Ready(event) = self.poll_draw_event(cx) {
                return Poll::Ready(event);
            }
            if let Poll::Ready(event) = self.poll_crossterm_event(cx) {
                return Poll::Ready(event);
            }
        } else {
            if let Poll::Ready(event) = self.poll_crossterm_event(cx) {
                return Poll::Ready(event);
            }
            if let Poll::Ready(event) = self.poll_draw_event(cx) {
                return Poll::Ready(event);
            }
        }

        if let Poll::Ready(event) = self.poll_resize_watchdog(cx) {
            return Poll::Ready(event);
        }

        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::Event;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;
    use pretty_assertions::assert_eq;
    use std::task::Context;
    use std::task::Poll;
    use std::time::Duration;
    use tokio::sync::broadcast;
    use tokio::sync::mpsc;
    use tokio::time::timeout;
    use tokio_stream::StreamExt;

    /// Simple fake event source for tests; feed events via the handle.
    struct FakeEventSource {
        rx: mpsc::UnboundedReceiver<EventResult>,
        tx: mpsc::UnboundedSender<EventResult>,
    }

    struct FakeEventSourceHandle {
        broker: Arc<EventBroker<FakeEventSource>>,
    }

    impl FakeEventSource {
        fn new() -> Self {
            let (tx, rx) = mpsc::unbounded_channel();
            Self { rx, tx }
        }
    }

    impl Default for FakeEventSource {
        fn default() -> Self {
            Self::new()
        }
    }

    impl FakeEventSourceHandle {
        fn new(broker: Arc<EventBroker<FakeEventSource>>) -> Self {
            Self { broker }
        }

        fn send(&self, event: EventResult) {
            let mut state = self
                .broker
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(source) = state.active_event_source_mut() else {
                return;
            };
            let _ = source.tx.send(event);
        }
    }

    impl EventSource for FakeEventSource {
        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<EventResult>> {
            Pin::new(&mut self.get_mut().rx).poll_recv(cx)
        }
    }

    fn make_stream(
        broker: Arc<EventBroker<FakeEventSource>>,
        draw_rx: broadcast::Receiver<()>,
        terminal_focused: Arc<AtomicBool>,
    ) -> TuiEventStream<FakeEventSource> {
        TuiEventStream::new(
            broker,
            draw_rx,
            terminal_focused,
            #[cfg(unix)]
            crate::tui::job_control::SuspendContext::new(),
            #[cfg(unix)]
            Arc::new(AtomicBool::new(false)),
        )
    }

    fn terminal_info(name: TerminalName) -> TerminalInfo {
        TerminalInfo {
            name,
            term_program: None,
            version: None,
            term: None,
            multiplexer: None,
        }
    }

    fn make_resize_watchdog(
        size: Arc<std::sync::Mutex<(u16, u16)>>,
        interval_duration: Duration,
    ) -> ResizeWatchdog {
        ResizeWatchdog::new(
            interval_duration,
            Arc::new(move || {
                let size = size
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                Ok(*size)
            }),
        )
    }

    type SetupState = (
        Arc<EventBroker<FakeEventSource>>,
        FakeEventSourceHandle,
        broadcast::Sender<()>,
        broadcast::Receiver<()>,
        Arc<AtomicBool>,
    );

    fn setup() -> SetupState {
        let source = FakeEventSource::new();
        let broker = Arc::new(EventBroker::new());
        *broker.state.lock().unwrap() = EventBrokerState::Running(source);
        let handle = FakeEventSourceHandle::new(broker.clone());

        let (draw_tx, draw_rx) = broadcast::channel(1);
        let terminal_focused = Arc::new(AtomicBool::new(true));
        (broker, handle, draw_tx, draw_rx, terminal_focused)
    }

    #[tokio::test(flavor = "current_thread")]
    async fn key_event_skips_unmapped() {
        let (broker, handle, _draw_tx, draw_rx, terminal_focused) = setup();
        let mut stream = make_stream(broker, draw_rx, terminal_focused);

        handle.send(Ok(Event::FocusLost));
        handle.send(Ok(Event::Key(KeyEvent::new(
            KeyCode::Char('a'),
            KeyModifiers::NONE,
        ))));

        let next = stream.next().await.unwrap();
        match next {
            TuiEvent::Key(key) => {
                assert_eq!(key, KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
            }
            other => panic!("expected key event, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn draw_and_key_events_yield_both() {
        let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
        let mut stream = make_stream(broker, draw_rx, terminal_focused);

        let expected_key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        let _ = draw_tx.send(());
        handle.send(Ok(Event::Key(expected_key)));

        let first = stream.next().await.unwrap();
        let second = stream.next().await.unwrap();

        let mut saw_draw = false;
        let mut saw_key = false;
        for event in [first, second] {
            match event {
                TuiEvent::Draw => {
                    saw_draw = true;
                }
                TuiEvent::Key(key) => {
                    assert_eq!(key, expected_key);
                    saw_key = true;
                }
                other => panic!("expected draw or key event, got {other:?}"),
            }
        }

        assert!(saw_draw && saw_key, "expected both draw and key events");
    }

    #[test]
    fn superset_resize_watchdog_gate_is_narrow() {
        assert!(should_enable_superset_resize_watchdog(
            &terminal_info(TerminalName::Superset),
            /*has_superset_env_markers*/ false,
        ));
        assert!(should_enable_superset_resize_watchdog(
            &terminal_info(TerminalName::Unknown),
            /*has_superset_env_markers*/ true,
        ));
        assert!(!should_enable_superset_resize_watchdog(
            &terminal_info(TerminalName::Unknown),
            /*has_superset_env_markers*/ false,
        ));
        assert!(!should_enable_superset_resize_watchdog(
            &terminal_info(TerminalName::VsCode),
            /*has_superset_env_markers*/ false,
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn resize_watchdog_reports_changes_after_seed() {
        let size = Arc::new(std::sync::Mutex::new((80, 24)));
        let mut watchdog = make_resize_watchdog(size.clone(), Duration::from_millis(10));

        assert!(!watchdog.observe_current_size(), "first sample seeds size");
        assert!(
            !watchdog.observe_current_size(),
            "same size does not request resize"
        );

        *size
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = (100, 30);
        assert!(
            watchdog.observe_current_size(),
            "changed size requests resize"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn resize_watchdog_emits_resize_when_size_changes() {
        let (broker, _handle, _draw_tx, draw_rx, terminal_focused) = setup();
        let size = Arc::new(std::sync::Mutex::new((80, 24)));
        let mut stream = make_stream(broker, draw_rx, terminal_focused);
        stream.resize_watchdog = Some(make_resize_watchdog(size.clone(), Duration::from_millis(5)));

        let no_event = timeout(Duration::from_millis(20), stream.next()).await;
        assert!(no_event.is_err(), "unchanged terminal size should not emit");

        *size
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = (100, 30);
        let next = timeout(Duration::from_millis(50), stream.next())
            .await
            .expect("timed out waiting for watchdog resize");
        assert!(matches!(next, Some(TuiEvent::Resize)));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn resize_event_updates_watchdog_size() {
        let (broker, handle, _draw_tx, draw_rx, terminal_focused) = setup();
        let size = Arc::new(std::sync::Mutex::new((100, 30)));
        let mut stream = make_stream(broker, draw_rx, terminal_focused);
        stream.resize_watchdog = Some(make_resize_watchdog(size, Duration::from_millis(10)));

        handle.send(Ok(Event::Resize(100, 30)));

        let next = stream.next().await;
        assert!(matches!(next, Some(TuiEvent::Resize)));
        assert_eq!(
            stream
                .resize_watchdog
                .as_ref()
                .and_then(|watchdog| watchdog.last_size),
            Some((100, 30))
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lagged_draw_maps_to_draw() {
        let (broker, _handle, draw_tx, draw_rx, terminal_focused) = setup();
        let mut stream = make_stream(broker, draw_rx.resubscribe(), terminal_focused);

        // Fill channel to force Lagged on the receiver.
        let _ = draw_tx.send(());
        let _ = draw_tx.send(());

        let first = stream.next().await;
        assert!(matches!(first, Some(TuiEvent::Draw)));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn resize_event_maps_to_resize() {
        let (broker, handle, _draw_tx, draw_rx, terminal_focused) = setup();
        let mut stream = make_stream(broker, draw_rx, terminal_focused);

        handle.send(Ok(Event::Resize(80, 24)));

        let next = stream.next().await;
        assert!(matches!(next, Some(TuiEvent::Resize)));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn error_or_eof_ends_stream() {
        let (broker, handle, _draw_tx, draw_rx, terminal_focused) = setup();
        let mut stream = make_stream(broker, draw_rx, terminal_focused);

        handle.send(Err(std::io::Error::other("boom")));

        let next = stream.next().await;
        assert!(next.is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn resume_wakes_paused_stream() {
        let (broker, handle, _draw_tx, draw_rx, terminal_focused) = setup();
        let mut stream = make_stream(broker.clone(), draw_rx, terminal_focused);

        broker.pause_events();

        let task = tokio::spawn(async move { stream.next().await });
        tokio::task::yield_now().await;

        broker.resume_events();
        let expected_key = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE);
        handle.send(Ok(Event::Key(expected_key)));

        let event = timeout(Duration::from_millis(100), task)
            .await
            .expect("timed out waiting for resumed event")
            .expect("join failed");
        match event {
            Some(TuiEvent::Key(key)) => assert_eq!(key, expected_key),
            other => panic!("expected key event, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn resume_wakes_pending_stream() {
        let (broker, handle, _draw_tx, draw_rx, terminal_focused) = setup();
        let mut stream = make_stream(broker.clone(), draw_rx, terminal_focused);

        let task = tokio::spawn(async move { stream.next().await });
        tokio::task::yield_now().await;

        broker.pause_events();
        broker.resume_events();
        let expected_key = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE);
        handle.send(Ok(Event::Key(expected_key)));

        let event = timeout(Duration::from_millis(100), task)
            .await
            .expect("timed out waiting for resumed event")
            .expect("join failed");
        match event {
            Some(TuiEvent::Key(key)) => assert_eq!(key, expected_key),
            other => panic!("expected key event, got {other:?}"),
        }
    }
}
