use super::*;
use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use std::task::Context;
use std::task::Poll;

use codex_terminal_detection::TerminalName;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::Wrap;
use tokio::time::timeout;
use tokio_stream::StreamExt;

use crate::custom_terminal::Terminal;
use crate::test_backend::VT100Backend;
use crate::tui::TuiEvent;
use crate::tui::event_stream::EventBroker;
use crate::tui::event_stream::EventResult;
use crate::tui::event_stream::EventSource;
use crate::tui::event_stream::TuiEventStream;

#[derive(Default)]
struct KeySource {
    sent: bool,
}

impl EventSource for KeySource {
    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<EventResult>> {
        if self.sent {
            return Poll::Pending;
        }
        self.sent = true;
        Poll::Ready(Some(Ok(Event::Key(KeyEvent::new(
            KeyCode::Char('x'),
            KeyModifiers::NONE,
        )))))
    }
}

struct Harness {
    broker: Arc<EventBroker<KeySource>>,
    queries: tokio::sync::mpsc::UnboundedReceiver<()>,
    replies: std::sync::mpsc::Sender<io::Result<Size>>,
    draws: broadcast::Receiver<()>,
}

impl Harness {
    fn new(size: Size) -> Self {
        let (draw_tx, draws) = broadcast::channel(/*capacity*/ 1);
        let (started, queries) = tokio::sync::mpsc::unbounded_channel();
        let (replies, responses) = std::sync::mpsc::channel();
        let monitor = SizeMonitor::spawn(size, draw_tx, move || {
            let _ = started.send(());
            responses
                .recv_timeout(Duration::from_secs(5))
                .unwrap_or_else(|_| Err(io::Error::other("test query was not released")))
        })
        .expect("monitor");
        let mut broker = EventBroker::new();
        broker.size_monitor = Some(monitor);
        Self {
            broker: Arc::new(broker),
            queries,
            replies,
            draws,
        }
    }

    fn monitor(&self) -> &SizeMonitor {
        self.broker.size_monitor.as_ref().unwrap()
    }

    async fn next_query(&mut self) {
        self.monitor().worker.thread().unpark();
        timeout(Duration::from_secs(2), self.queries.recv())
            .await
            .unwrap()
            .unwrap();
    }

    fn stream(&self) -> TuiEventStream<KeySource> {
        TuiEventStream::new(
            self.broker.clone(),
            self.draws.resubscribe(),
            Arc::new(AtomicBool::new(/*v*/ false)),
            #[cfg(unix)]
            crate::tui::job_control::SuspendContext::new(),
            #[cfg(unix)]
            Arc::new(AtomicBool::new(/*v*/ false)),
        )
    }
}

#[tokio::test]
async fn missed_resize_recovers_and_slow_query_does_not_block_input() {
    let initial = Size::new(/*width*/ 12, /*height*/ 4);
    let resized = Size::new(/*width*/ 8, /*height*/ 4);
    let mut harness = Harness::new(initial);
    let mut stream = harness.stream();
    harness.next_query().await;
    // The worker is blocked waiting for a reply. Input still traverses the real event stream.
    assert!(matches!(
        timeout(Duration::from_millis(100), stream.next())
            .await
            .unwrap(),
        Some(TuiEvent::Key(KeyEvent {
            code: KeyCode::Char('x'),
            ..
        }))
    ));
    assert!(
        timeout(
            CHECK_INTERVAL + Duration::from_millis(50),
            harness.queries.recv()
        )
        .await
        .is_err()
    );
    harness.replies.send(Ok(resized)).unwrap();
    let event = timeout(Duration::from_secs(2), stream.next())
        .await
        .unwrap()
        .unwrap();
    let TuiEvent::Resize(size) = event else {
        panic!("expected recovered resize: {event:?}")
    };
    assert_eq!(size, resized);

    let mut terminal = Terminal::with_screen_size_and_cursor_position_for_test(
        VT100Backend::new(resized.width, resized.height),
        initial,
        Position { x: 0, y: 0 },
    );
    let area = Rect::new(/*x*/ 0, /*y*/ 0, size.width, size.height);
    terminal.set_viewport_area(area);
    terminal
        .draw_with_size(size, &mut Vec::new(), |frame| {
            Paragraph::new("alpha beta")
                .wrap(Wrap { trim: false })
                .render(area, frame.buffer_mut());
        })
        .unwrap();
    assert_eq!(terminal.last_known_screen_size, resized);
    insta::assert_snapshot!(terminal.backend().vt100().screen().contents(), @"alpha\nbeta");
}

#[tokio::test]
async fn newer_resize_invalidates_in_flight_and_queued_samples() {
    let initial = Size::new(/*width*/ 80, /*height*/ 24);
    let newer = Size::new(/*width*/ 100, /*height*/ 30);
    let mut harness = Harness::new(initial);
    harness.next_query().await;
    harness.monitor().observe(newer);
    harness.replies.send(Ok(initial)).unwrap();
    harness.next_query().await; // The previous result has been processed.
    assert_eq!(harness.monitor().take_resize(), None);
    assert_eq!(
        harness.draws.try_recv(),
        Err(broadcast::error::TryRecvError::Empty)
    );

    harness.replies.send(Ok(initial)).unwrap();
    timeout(Duration::from_secs(2), harness.draws.recv())
        .await
        .unwrap()
        .unwrap();
    harness.monitor().observe(newer); // Also discard a result already queued for the UI.
    assert_eq!(harness.monitor().take_resize(), None);
}

#[tokio::test]
async fn unchanged_invalid_and_failed_samples_do_not_redraw() {
    let initial = Size::new(/*width*/ 80, /*height*/ 24);
    let mut harness = Harness::new(initial);
    harness.next_query().await;
    for sample in [
        Ok(initial),
        Ok(Size::new(/*width*/ 0, /*height*/ 0)),
        Err(io::Error::other("query failed")),
    ] {
        harness.replies.send(sample).unwrap();
        harness.next_query().await;
        assert_eq!(harness.monitor().take_resize(), None);
        assert_eq!(
            harness.draws.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        );
    }
}

#[tokio::test]
async fn terminal_handoff_pauses_queries_and_discards_old_results() {
    let initial = Size::new(/*width*/ 80, /*height*/ 24);
    let resized = Size::new(/*width*/ 40, /*height*/ 12);
    let mut harness = Harness::new(initial);
    harness.next_query().await;
    harness.broker.pause_events();
    harness.replies.send(Ok(resized)).unwrap();
    harness.monitor().worker.thread().unpark();
    assert!(
        timeout(Duration::from_millis(100), harness.queries.recv())
            .await
            .is_err()
    );
    assert_eq!(harness.monitor().take_resize(), None);
    harness.broker.resume_events();
    harness.next_query().await;
    harness.replies.send(Ok(resized)).unwrap();
    timeout(Duration::from_secs(2), harness.draws.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(harness.monitor().take_resize(), Some(resized));
}

#[tokio::test]
async fn drop_does_not_wait_for_a_blocked_query_or_start_another() {
    let initial = Size::new(/*width*/ 80, /*height*/ 24);
    let mut harness = Harness::new(initial);
    harness.next_query().await;
    let state = harness.monitor().state.clone();
    drop(harness.broker);
    assert!(state.lock().unwrap().stopped);
    harness
        .replies
        .send(Ok(Size::new(/*width*/ 40, /*height*/ 12)))
        .unwrap();
    assert_eq!(
        timeout(Duration::from_secs(2), harness.queries.recv())
            .await
            .unwrap(),
        None
    );
    assert_eq!(state.lock().unwrap().pending, None);
}

#[test]
fn monitor_requires_tmux_detection() {
    let (draw_tx, _) = broadcast::channel(/*capacity*/ 1);
    let mut info = TerminalInfo {
        backend: None,
        name: TerminalName::Ghostty,
        term_program: None,
        version: None,
        term: None,
        multiplexer: None,
    };
    let size = Size::new(/*width*/ 80, /*height*/ 24);
    assert!(SizeMonitor::start(&info, size, draw_tx.clone()).is_none());
    info.multiplexer = Some(Multiplexer::Zellij { version: None });
    assert!(SizeMonitor::start(&info, size, draw_tx.clone()).is_none());
    info.multiplexer = Some(Multiplexer::Tmux { version: None });
    assert_eq!(
        SizeMonitor::start(&info, size, draw_tx).is_some(),
        cfg!(unix)
    );
}
