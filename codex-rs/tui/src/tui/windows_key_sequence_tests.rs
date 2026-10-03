use super::*;
use futures::FutureExt;
use pretty_assertions::assert_eq;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::UnboundedReceiverStream;

fn keys(text: &str) -> Vec<Event> {
    text.chars()
        .map(|ch| {
            Event::Key(KeyEvent::new(
                if ch == '\u{1b}' {
                    KeyCode::Esc
                } else {
                    KeyCode::Char(ch)
                },
                KeyModifiers::NONE,
            ))
        })
        .collect()
}

async fn decoded(events: Vec<Event>) -> Vec<Event> {
    WindowsKeySequence::new(tokio_stream::iter(events.into_iter().map(Ok)))
        .map(Result::unwrap)
        .collect()
        .await
}

#[tokio::test]
async fn repeated_mappings_decode_with_or_without_key_releases() {
    let expected = Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT));
    let presses = keys("\u{1b}[13;2u\u{1b}[13;2u");
    assert_eq!(decoded(presses.clone()).await, vec![expected.clone(); 2]);
    let records = presses
        .into_iter()
        .flat_map(|event| {
            let Event::Key(key) = event else {
                unreachable!()
            };
            [
                Event::Key(key),
                Event::Key(KeyEvent {
                    kind: KeyEventKind::Release,
                    ..key
                }),
            ]
        })
        .collect();
    assert_eq!(decoded(records).await, vec![expected; 2]);
}

#[tokio::test(start_paused = true)]
async fn fragmented_mapping_waits_without_leaking_escape() {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let mut stream = WindowsKeySequence::new(UnboundedReceiverStream::new(rx));
    let events = keys("\u{1b}[13;2u");
    for event in &events[..6] {
        tx.send(Ok(event.clone())).unwrap();
        assert!(stream.next().now_or_never().is_none());
        tokio::time::advance(Duration::from_millis(/*millis*/ 5)).await;
    }
    tx.send(Ok(events[6].clone())).unwrap();
    assert_eq!(
        stream.next().await.unwrap().unwrap(),
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT))
    );
}

#[tokio::test(start_paused = true)]
async fn delayed_poll_consumes_queued_suffix_without_waiting_for_more_input() {
    for suffix in ["[13;2u", "[13;"] {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let mut stream = WindowsKeySequence::new(UnboundedReceiverStream::new(rx));
        tx.send(Ok(keys("\u{1b}")[0].clone())).unwrap();
        assert!(stream.next().now_or_never().is_none());
        for event in keys(suffix) {
            tx.send(Ok(event)).unwrap();
        }
        tokio::time::advance(SEQUENCE_TIMEOUT).await;
        let expected = if suffix.ends_with('u') {
            vec![Event::Key(KeyEvent::new(
                KeyCode::Enter,
                KeyModifiers::SHIFT,
            ))]
        } else {
            keys("\u{1b}[13;")
        };
        for event in expected {
            assert_eq!(
                stream.next().now_or_never().unwrap().unwrap().unwrap(),
                event
            );
        }
        assert!(stream.next().now_or_never().is_none());
    }
}

#[tokio::test]
async fn continuously_ready_mappings_yield_one_newline_at_a_time() {
    let events = keys("\u{1b}[13;2u").into_iter().cycle();
    let mut stream = WindowsKeySequence::new(tokio_stream::iter(events.map(Ok)));
    for _ in 0..2 {
        assert_eq!(
            stream.next().now_or_never().unwrap().unwrap().unwrap(),
            Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT))
        );
    }
}

#[tokio::test(start_paused = true)]
async fn standalone_escape_and_partial_prefix_wake_at_fixed_deadline() {
    for text in ["\u{1b}", "\u{1b}[13;"] {
        let events = keys(text);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let mut stream = WindowsKeySequence::new(UnboundedReceiverStream::new(rx));
        tx.send(Ok(events[0].clone())).unwrap();
        assert!(stream.next().now_or_never().is_none());
        tokio::time::advance(Duration::from_millis(/*millis*/ 40)).await;
        for event in &events[1..] {
            tx.send(Ok(event.clone())).unwrap();
        }
        assert!(stream.next().now_or_never().is_none());
        // next() must be woken by the original deadline, without another input event.
        let start = tokio::time::Instant::now();
        assert_eq!(stream.next().await.unwrap().unwrap(), events[0]);
        assert_eq!(start.elapsed(), Duration::from_millis(/*millis*/ 10));
        for event in &events[1..] {
            assert_eq!(stream.next().await.unwrap().unwrap(), *event);
        }
        drop(tx);
        assert!(stream.next().await.is_none());
    }
}

#[tokio::test]
async fn literal_text_paste_native_keys_and_failed_prefixes_are_unchanged() {
    let mut events = keys("literal [13;2u\u{1b}[13;3u\u{1b}[x");
    events.extend([
        Event::Paste("literal \u{1b}[13;2u".to_string()),
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT)),
        Event::Key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL)),
        Event::Key(KeyEvent::new_with_kind(
            KeyCode::Char('x'),
            KeyModifiers::NONE,
            KeyEventKind::Repeat,
        )),
        Event::Key(KeyEvent::new_with_kind(
            KeyCode::Esc,
            KeyModifiers::NONE,
            KeyEventKind::Release,
        )),
    ]);
    assert_eq!(decoded(events.clone()).await, events);
    for interrupt in [
        Event::Resize(80, 24),
        Event::FocusLost,
        Event::Paste("[13;2u".to_string()),
    ] {
        let mut events = keys("\u{1b}[");
        events.push(interrupt);
        events.extend(keys("13;2u"));
        assert_eq!(decoded(events.clone()).await, events);
    }
}

#[tokio::test]
async fn mismatch_can_start_another_mapping_and_eof_replays_partial_input() {
    let mut expected = keys("\u{1b}[1");
    expected.push(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::SHIFT,
    )));
    expected.extend(keys("\u{1b}["));
    assert_eq!(decoded(keys("\u{1b}[1\u{1b}[13;2u\u{1b}[")).await, expected);
}

#[tokio::test]
async fn errors_follow_pending_input_and_do_not_keep_partial_state() {
    let mut events: Vec<_> = keys("\u{1b}[").into_iter().map(Ok).collect();
    events.push(Err(std::io::Error::other("input failed")));
    events.extend(keys("13;2u").into_iter().map(Ok));
    let mut stream = WindowsKeySequence::new(tokio_stream::iter(events));
    for event in keys("\u{1b}[") {
        assert_eq!(stream.next().await.unwrap().unwrap(), event);
    }
    assert_eq!(
        stream.next().await.unwrap().unwrap_err().to_string(),
        "input failed"
    );
    assert_eq!(
        stream.map(Result::unwrap).collect::<Vec<_>>().await,
        keys("13;2u")
    );
}

#[tokio::test]
async fn mapped_shift_enter_renders_newlines_in_the_composer() {
    use crate::app_event::AppEvent;
    use crate::app_event_sender::AppEventSender;
    use crate::bottom_pane::ChatComposer;
    use crate::bottom_pane::InputResult;
    use crate::render::renderable::Renderable;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let mut composer = ChatComposer::new(
        /*has_input_focus*/ true,
        AppEventSender::new(tx),
        /*enhanced_keys_supported*/ false,
        "Ask Codex to do anything".to_string(),
        /*disable_paste_burst*/ true,
    );
    for event in decoded(keys("first line\u{1b}[13;2u\u{1b}[13;2usecond line [13;2u")).await {
        let Event::Key(key) = event else {
            unreachable!()
        };
        assert!(matches!(
            composer.handle_key_event(key).0,
            InputResult::None
        ));
    }
    assert_eq!(composer.current_text(), "first line\n\nsecond line [13;2u");
    let mut terminal = Terminal::new(TestBackend::new(/*width*/ 60, /*height*/ 9)).unwrap();
    terminal
        .draw(|frame| composer.render(frame.area(), frame.buffer_mut()))
        .unwrap();
    insta::assert_snapshot!("windows_mapped_shift_enter", terminal.backend());
}
