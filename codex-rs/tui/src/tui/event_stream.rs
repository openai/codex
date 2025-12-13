//! Event stream plumbing for the TUI.
//!
//! - [`EventBroker`] holds the shared crossterm stream so multiple callers reuse the same
//!   input source and can drop/recreate it on pause/resume without rebuilding consumers.
//! - [`TuiEventStream`] wraps a per-call draw subscription plus the shared broker and maps crossterm
//!   events into [`TuiEvent`].
//! - [`EventSource`] abstracts the underlying event producer; the real implementation is
//!   [`CrosstermEventSource`] and tests can swap in [`FakeEventSource`].
//!
//! The motivation for dropping/recreating the crossterm event stream is to enable fully relinquishing stdin.
//! If the stream is not dropped, it will continue to read from stdin even if it is not actively being polled,
//! potentially stealing input from other processes.
//!
//! See https://ratatui.rs/recipes/apps/spawn-vim/ and https://openai.slack.com/archives/C095U48JNL9/p1765401070172969?thread_ts=1765398553.890439&cid=C095U48JNL9

use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;

use crossterm::event::Event;
use tokio::sync::broadcast;
use tokio_stream::Stream;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

use super::TuiEvent;

/// Result type produced by an event source.
pub type EventResult = Result<Event, ()>;

/// Abstraction over a source of terminal events. Allows swapping in a fake for tests.
pub trait EventSource: Send + 'static {
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<EventResult>>;
}

/// Shared crossterm input state for all `event_stream()` handles. A single EventStream
/// is reused so all streams still see the same input source.
///
/// This intermediate layer enables dropping/recreating the underlying EventStream (pause/resume) without rebuilding consumers.
pub struct EventBroker<S: EventSource = CrosstermEventSource> {
    pub paused: bool,
    pub crossterm_events: Option<S>,
}

impl<S: EventSource + Default> EventBroker<S> {
    pub fn new() -> Self {
        Self {
            paused: false,
            crossterm_events: None,
        }
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
        let inner = Pin::new(&mut self.get_mut().0);
        match inner.poll_next(cx) {
            Poll::Ready(Some(Ok(event))) => Poll::Ready(Some(Ok(event))),
            Poll::Ready(Some(Err(_))) => Poll::Ready(Some(Err(()))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Per-call event stream wrapper. Each handle has its own draw subscription but
/// pulls crossterm events from the shared broker so nested/sequential streams
/// can coexist (only one should be polled at a time to avoid stealing).
pub struct TuiEventStream<S: EventSource + Default + Unpin = CrosstermEventSource> {
    broker: Arc<Mutex<EventBroker<S>>>,
    draw_stream: BroadcastStream<()>,
    terminal_focused: Arc<AtomicBool>,
    poll_draw_first: bool,
    #[cfg(unix)]
    suspend_context: crate::tui::job_control::SuspendContext,
    #[cfg(unix)]
    alt_screen_active: Arc<AtomicBool>,
}

impl<S: EventSource + Default + Unpin> TuiEventStream<S> {
    #[cfg(unix)]
    pub fn new(
        broker: Arc<Mutex<EventBroker<S>>>,
        draw_rx: broadcast::Receiver<()>,
        terminal_focused: Arc<AtomicBool>,
        suspend_context: crate::tui::job_control::SuspendContext,
        alt_screen_active: Arc<AtomicBool>,
    ) -> Self {
        Self {
            broker,
            draw_stream: BroadcastStream::new(draw_rx),
            terminal_focused,
            poll_draw_first: false,
            suspend_context,
            alt_screen_active,
        }
    }

    #[cfg(not(unix))]
    pub fn new(
        broker: Arc<Mutex<EventBroker<S>>>,
        draw_rx: broadcast::Receiver<()>,
        terminal_focused: Arc<AtomicBool>,
    ) -> Self {
        Self {
            broker,
            draw_stream: BroadcastStream::new(draw_rx),
            terminal_focused,
            poll_draw_first: false,
        }
    }

    pub fn poll_crossterm_event(&mut self, cx: &mut Context<'_>) -> Poll<Option<TuiEvent>> {
        // Some crossterm events map to None (e.g. FocusLost, mouse); loop so we keep polling
        // until we return a mapped event, hit Pending, or see EOF/error.
        loop {
            let poll_result = {
                let mut broker = self
                    .broker
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if broker.paused {
                    broker.crossterm_events = None;
                    return Poll::Pending;
                }
                let events = broker.crossterm_events.get_or_insert_with(S::default);
                match Pin::new(events).poll_next(cx) {
                    Poll::Ready(Some(Ok(event))) => Some(event),
                    Poll::Ready(Some(Err(_))) | Poll::Ready(None) => {
                        broker.crossterm_events = None;
                        return Poll::Ready(None);
                    }
                    Poll::Pending => return Poll::Pending,
                }
            };

            if let Some(mapped) = poll_result.and_then(|event| self.map_event(event)) {
                return Poll::Ready(Some(mapped));
            }
        }
    }

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

    fn map_event(&mut self, event: Event) -> Option<TuiEvent> {
        match event {
            Event::Key(key_event) => {
                #[cfg(unix)]
                if crate::tui::job_control::SUSPEND_KEY.is_press(key_event) {
                    let _ = self.suspend_context.suspend(&self.alt_screen_active);
                    return Some(TuiEvent::Draw);
                }
                Some(TuiEvent::Key(key_event))
            }
            Event::Resize(_, _) => Some(TuiEvent::Draw),
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
    use tokio::sync::broadcast;
    use tokio::sync::mpsc;
    use tokio_stream::StreamExt;

    /// Simple fake event source for tests; feed events via the handle.
    struct FakeEventSource {
        rx: mpsc::UnboundedReceiver<EventResult>,
        tx: mpsc::UnboundedSender<EventResult>,
    }

    struct FakeEventSourceHandle {
        broker: Arc<Mutex<EventBroker<FakeEventSource>>>,
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
        fn new(broker: Arc<Mutex<EventBroker<FakeEventSource>>>) -> Self {
            Self { broker }
        }

        fn send(&self, event: EventResult) {
            let mut broker = self
                .broker
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if broker.paused {
                return;
            }
            let source = broker
                .crossterm_events
                .get_or_insert_with(FakeEventSource::default);
            let _ = source.tx.send(event);
        }
    }

    impl EventSource for FakeEventSource {
        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<EventResult>> {
            let mut inner = Pin::new(&mut self.get_mut().rx);
            match inner.poll_recv(cx) {
                Poll::Ready(Some(event)) => Poll::Ready(Some(event)),
                Poll::Ready(None) => Poll::Ready(None),
                Poll::Pending => Poll::Pending,
            }
        }
    }

    #[cfg(unix)]
    fn make_stream(
        broker: Arc<Mutex<EventBroker<FakeEventSource>>>,
        draw_rx: broadcast::Receiver<()>,
        terminal_focused: Arc<AtomicBool>,
    ) -> TuiEventStream<FakeEventSource> {
        TuiEventStream::new(
            broker,
            draw_rx,
            terminal_focused,
            crate::tui::job_control::SuspendContext::new(),
            Arc::new(AtomicBool::new(false)),
        )
    }

    #[cfg(not(unix))]
    fn make_stream(
        broker: Arc<Mutex<EventBroker<FakeEventSource>>>,
        draw_rx: broadcast::Receiver<()>,
        terminal_focused: Arc<AtomicBool>,
    ) -> TuiEventStream<FakeEventSource> {
        TuiEventStream::new(broker, draw_rx, terminal_focused)
    }

    type SetupState = (
        Arc<Mutex<EventBroker<FakeEventSource>>>,
        FakeEventSourceHandle,
        broadcast::Sender<()>,
        broadcast::Receiver<()>,
        Arc<AtomicBool>,
    );

    fn setup() -> SetupState {
        let source = FakeEventSource::new();
        let broker = Arc::new(Mutex::new(EventBroker::new()));
        broker.lock().unwrap().crossterm_events = Some(source);
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
    async fn error_or_eof_ends_stream() {
        let (broker, handle, _draw_tx, draw_rx, terminal_focused) = setup();
        let mut stream = make_stream(broker, draw_rx, terminal_focused);

        handle.send(Err(()));

        let next = stream.next().await;
        assert!(next.is_none());
    }
}
