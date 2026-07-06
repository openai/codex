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

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;

use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use tokio::sync::broadcast;
use tokio::sync::watch;
use tokio::time::Instant;
use tokio::time::Sleep;
use tokio_stream::Stream;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::WatchStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

use super::STARTUP_INPUT_QUIET_PERIOD;
use super::TuiEvent;

/// Result type produced by an event source.
pub type EventResult = std::io::Result<Event>;

#[derive(Clone, Copy)]
pub(super) enum InitialInputPolicy {
    DiscardAll,
    PreserveText,
}

struct InitialInputFilter {
    policy: InitialInputPolicy,
    drain_ready: bool,
    quiet_timer: Option<Pin<Box<Sleep>>>,
    interrupt_forwarded: bool,
    suspend_forwarded: bool,
}

impl InitialInputFilter {
    fn new(
        policy: InitialInputPolicy,
        start_quiet: bool,
        interrupt_forwarded: bool,
        suspend_forwarded: bool,
    ) -> Self {
        let quiet_timer = start_quiet.then(|| {
            Box::pin(tokio::time::sleep_until(
                Instant::now() + STARTUP_INPUT_QUIET_PERIOD,
            ))
        });
        Self {
            policy,
            drain_ready: !start_quiet,
            quiet_timer,
            interrupt_forwarded,
            suspend_forwarded,
        }
    }

    fn reset_quiet_timer(&mut self) {
        let deadline = Instant::now() + STARTUP_INPUT_QUIET_PERIOD;
        if let Some(timer) = &mut self.quiet_timer {
            timer.as_mut().reset(deadline);
        } else {
            self.quiet_timer = Some(Box::pin(tokio::time::sleep_until(deadline)));
        }
        self.drain_ready = false;
    }
}

/// Abstraction over a source of terminal events. Allows swapping in a fake for tests.
/// Value in production is [`CrosstermEventSource`].
pub trait EventSource: Send + 'static {
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<EventResult>>;
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
    terminal_focused: Arc<AtomicBool>,
    poll_draw_first: bool,
    initial_input_filter: Option<InitialInputFilter>,
    pending_interrupt: bool,
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
        Self {
            broker,
            draw_stream: BroadcastStream::new(draw_rx),
            resume_stream,
            terminal_focused,
            poll_draw_first: false,
            initial_input_filter: None,
            pending_interrupt: false,
            #[cfg(unix)]
            suspend_context,
            #[cfg(unix)]
            alt_screen_active,
        }
    }

    pub(super) fn filtering_initial_input(
        mut self,
        policy: InitialInputPolicy,
        start_quiet: bool,
        pending_interrupt: bool,
        suspend_forwarded: bool,
    ) -> Self {
        self.initial_input_filter = Some(InitialInputFilter::new(
            policy,
            start_quiet,
            pending_interrupt,
            suspend_forwarded,
        ));
        self.pending_interrupt = pending_interrupt;
        self
    }

    fn should_forward_initial_event(&mut self, event: &Event) -> bool {
        let Some(filter) = &mut self.initial_input_filter else {
            return true;
        };
        match event {
            Event::Key(key_event) => {
                filter.reset_quiet_timer();
                if is_interrupt(*key_event) {
                    return if filter.interrupt_forwarded {
                        false
                    } else {
                        filter.interrupt_forwarded = true;
                        true
                    };
                }
                #[cfg(unix)]
                if crate::tui::job_control::SUSPEND_KEY.is_press(*key_event) {
                    return if filter.suspend_forwarded {
                        false
                    } else {
                        filter.suspend_forwarded = true;
                        true
                    };
                }
                matches!(filter.policy, InitialInputPolicy::PreserveText)
                    && is_text_input(*key_event)
            }
            Event::Paste(_) => {
                filter.reset_quiet_timer();
                matches!(filter.policy, InitialInputPolicy::PreserveText)
            }
            Event::Resize(_, _) | Event::FocusGained | Event::FocusLost => true,
            _ => false,
        }
    }

    fn poll_initial_input_filter(&mut self, cx: &mut Context<'_>) {
        let Some(filter) = &mut self.initial_input_filter else {
            return;
        };
        if filter.drain_ready {
            self.initial_input_filter = None;
            return;
        }
        let Some(timer) = &mut filter.quiet_timer else {
            self.initial_input_filter = None;
            return;
        };
        if timer.as_mut().poll(cx).is_ready() {
            self.initial_input_filter = None;
        }
    }

    /// Poll the shared crossterm stream for the next mapped `TuiEvent`.
    ///
    /// This skips events we don't use (mouse events, etc.) and keeps polling until it yields
    /// a mapped event, hits `Pending`, or sees EOF/error. When the broker is paused, it drops
    /// the underlying stream and returns `Pending` to fully release stdin.
    pub fn poll_crossterm_event(&mut self, cx: &mut Context<'_>) -> Poll<Option<TuiEvent>> {
        if std::mem::take(&mut self.pending_interrupt) {
            return Poll::Ready(Some(TuiEvent::Key(KeyEvent::new(
                KeyCode::Char('c'),
                KeyModifiers::CONTROL,
            ))));
        }
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
                            Poll::Pending => {
                                self.poll_initial_input_filter(cx);
                                return Poll::Pending;
                            }
                        }
                    }
                }
            };

            let Some(event) = poll_result else {
                continue;
            };
            if !self.should_forward_initial_event(&event) {
                continue;
            }
            if let Some(mapped) = self.map_crossterm_event(event) {
                return Poll::Ready(Some(mapped));
            }
        }
    }

    /// Poll the draw broadcast stream for the next draw event. Draw events are used to trigger a redraw of the TUI.
    pub fn poll_draw_event(&mut self, cx: &mut Context<'_>) -> Poll<Option<TuiEvent>> {
        match Pin::new(&mut self.draw_stream).poll_next(cx) {
            Poll::Ready(Some(Ok(()))) => Poll::Ready(Some(TuiEvent::Draw)),
            Poll::Ready(Some(Err(BroadcastStreamRecvError::Lagged(_)))) => {
                Poll::Ready(Some(TuiEvent::Draw))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }

    /// Map a crossterm event to a [`TuiEvent`], skipping events we don't use (mouse events, etc.).
    fn map_crossterm_event(&mut self, event: Event) -> Option<TuiEvent> {
        match event {
            Event::Key(key_event) => {
                #[cfg(unix)]
                if crate::tui::job_control::SUSPEND_KEY.is_press(key_event) {
                    self.broker.pause_events();
                    let suspend_result = self.suspend_context.suspend(&self.alt_screen_active);
                    self.broker.resume_events();
                    if let Err(err) = suspend_result {
                        tracing::warn!(
                            event = "tui_suspend_failed",
                            error = %err,
                            "failed to suspend TUI process"
                        );
                    }
                    return Some(TuiEvent::Draw);
                }
                Some(TuiEvent::Key(key_event))
            }
            Event::Resize(_, _) => Some(TuiEvent::Resize),
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

fn is_interrupt(key_event: KeyEvent) -> bool {
    matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
        && key_event.code == KeyCode::Char('c')
        && key_event.modifiers.contains(KeyModifiers::CONTROL)
}

fn is_text_input(key_event: KeyEvent) -> bool {
    matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
        && key_event
            .modifiers
            .difference(KeyModifiers::SHIFT)
            .is_empty()
        && match key_event.code {
            KeyCode::Char(ch) => !ch.is_control(),
            KeyCode::Backspace => true,
            _ => false,
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

    fn make_stream_filtering_initial_input(
        broker: Arc<EventBroker<FakeEventSource>>,
        draw_rx: broadcast::Receiver<()>,
        terminal_focused: Arc<AtomicBool>,
        policy: InitialInputPolicy,
        start_quiet: bool,
        pending_interrupt: bool,
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
        .filtering_initial_input(
            policy,
            start_quiet,
            pending_interrupt,
            /*suspend_forwarded*/ false,
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

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn initial_input_handoff_waits_for_legacy_repeats_to_settle() {
        let (broker, handle, _draw_tx, draw_rx, terminal_focused) = setup();
        let mut stream = make_stream_filtering_initial_input(
            broker,
            draw_rx,
            terminal_focused,
            InitialInputPolicy::DiscardAll,
            /*start_quiet*/ false,
            /*pending_interrupt*/ false,
        );
        handle.send(Ok(Event::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        ))));

        assert!(
            timeout(Duration::from_nanos(1), stream.next())
                .await
                .is_err()
        );
        tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD / 2).await;
        handle.send(Ok(Event::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        ))));
        assert!(
            timeout(Duration::from_nanos(1), stream.next())
                .await
                .is_err()
        );
        tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
        assert!(
            timeout(Duration::from_nanos(1), stream.next())
                .await
                .is_err()
        );

        let expected = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        handle.send(Ok(Event::Key(expected)));
        match timeout(Duration::from_nanos(1), stream.next()).await {
            Ok(Some(TuiEvent::Key(actual))) => assert_eq!(actual, expected),
            other => panic!("expected post-handoff key event, saw {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn startup_handoff_preserves_text_while_suppressing_actions() {
        let (broker, handle, _draw_tx, draw_rx, terminal_focused) = setup();
        let mut stream = make_stream_filtering_initial_input(
            broker,
            draw_rx,
            terminal_focused,
            InitialInputPolicy::PreserveText,
            /*start_quiet*/ true,
            /*pending_interrupt*/ false,
        );

        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        handle.send(Ok(Event::Key(key)));
        assert!(matches!(stream.next().await, Some(TuiEvent::Key(actual)) if actual == key));

        handle.send(Ok(Event::Paste("b\nc".to_string())));
        assert!(matches!(stream.next().await, Some(TuiEvent::Paste(text)) if text == "b\nc"));

        let backspace = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
        handle.send(Ok(Event::Key(backspace)));
        assert!(matches!(stream.next().await, Some(TuiEvent::Key(actual)) if actual == backspace));

        handle.send(Ok(Event::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        ))));
        assert!(
            timeout(Duration::from_nanos(1), stream.next())
                .await
                .is_err()
        );
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn startup_handoff_forwards_interrupt_only_once() {
        let (broker, handle, _draw_tx, draw_rx, terminal_focused) = setup();
        let mut stream = make_stream_filtering_initial_input(
            broker,
            draw_rx,
            terminal_focused,
            InitialInputPolicy::DiscardAll,
            /*start_quiet*/ true,
            /*pending_interrupt*/ true,
        );

        let interrupt = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(matches!(stream.next().await, Some(TuiEvent::Key(actual)) if actual == interrupt));
        handle.send(Ok(Event::Key(interrupt)));
        assert!(
            timeout(Duration::from_nanos(1), stream.next())
                .await
                .is_err()
        );
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
