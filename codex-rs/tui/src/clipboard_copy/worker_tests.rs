//! Blocked backends leave UI work and teardown independent of clipboard progress.

use super::*;
use crate::clipboard_copy::CopyOutcome;
use crate::history_cell::PlainHistoryCell;
use crate::keymap::RuntimeKeymap;
use crate::pager_overlay::Overlay;
use crate::tui::TuiEvent;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use std::time::Duration;

#[tokio::test]
async fn blocked_copy_allows_overlay_exit_rejects_backlog_and_wakes_completion() {
    let mut tui = crate::tui::test_support::make_test_tui().unwrap();
    let (draws, mut draw_rx) = tokio::sync::broadcast::channel(/*capacity*/ 1);
    let frames = FrameRequester::new(draws);
    let (started, start_rx) = mpsc::channel();
    let (release, released) = mpsc::channel();
    tui.clipboard
        .start(frames.clone(), move |text, format, setup| {
            setup.begin_delivery().unwrap();
            started.send((text.to_owned(), format)).unwrap();
            released.recv().unwrap();
            (Ok(CopyOutcome::Copied(Some(ClipboardLease::test()))), None)
        })
        .unwrap();
    let mut app = Box::new(crate::app::test_support::make_test_app().await);
    let (mut chat, tx, mut events, _ops) =
        crate::chatwidget::tests::make_chatwidget_manual_with_sender().await;
    chat.handle_server_notification(
        codex_app_server_protocol::ServerNotification::ItemCompleted(
            codex_app_server_protocol::ItemCompletedNotification {
                thread_id: String::new(),
                turn_id: "turn-1".into(),
                completed_at_ms: 0,
                item: codex_app_server_protocol::ThreadItem::AgentMessage {
                    id: "msg-1".into(),
                    text: "café\nsecond line".into(),
                    phase: None,
                    memory_citation: None,
                    delivery: None,
                    questions: None,
                },
            },
        ),
        /*replay_kind*/ None,
    );
    app.chat_widget = chat;
    app.app_event_tx = tx;
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config)
        .await
        .unwrap();
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL)),
    )
    .await
    .unwrap();
    // Copy must start without draining AppEvent, before a subsequent paste key can arrive.
    assert!(tui.clipboard.is_busy());
    while events.try_recv().is_ok() {}
    assert_eq!(
        start_rx
            .recv_timeout(Duration::from_secs(/*secs*/ 5))
            .unwrap(),
        ("café\nsecond line".into(), CopyFormat::Markdown)
    );
    for _ in 0..100 {
        assert_eq!(
            tui.copy_transcript_selection("newer", CopyFormat::PlainText),
            Ok(CopyStatus::Busy)
        );
    }
    assert!(tui.clipboard.poll().is_none());
    let mut overlay = Overlay::new_transcript(
        vec![Arc::new(PlainHistoryCell::new(vec!["selected".into()]))],
        RuntimeKeymap::defaults().pager,
        /*copy_on_select*/ false,
    );
    overlay
        .handle_event(
            &mut tui,
            TuiEvent::Key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL)),
        )
        .unwrap();
    assert!(overlay.is_done());
    drop(overlay);
    assert!(tui.clipboard.is_busy());

    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL)),
    )
    .await
    .unwrap();
    let mut feedback = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let crate::app_event::AppEvent::InsertHistoryCell(cell) = event {
            feedback.extend(
                cell.display_lines(/*width*/ 80)
                    .iter()
                    .map(ToString::to_string),
            );
        }
    }
    insta::assert_snapshot!("image_paste_while_copy_is_busy", feedback.join("\n"));
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(/*secs*/ 5), draw_rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(tui.clipboard.poll(), Some(&(1, Ok(CopyStatus::Confirmed))));
    assert!(!tui.clipboard.is_busy());
    assert!(start_rx.try_recv().is_err());
}

#[test]
fn dropping_worker_does_not_wait_for_blocked_backend() {
    let mut worker = ClipboardWorker::default();
    let (started, start_rx) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let (finished, finish_rx) = mpsc::channel();
    worker
        .start(FrameRequester::test_dummy(), move |_, _, _| {
            started.send(()).unwrap();
            released.recv().unwrap();
            finished.send(()).unwrap();
            (Err("unavailable".into()), None)
        })
        .unwrap();
    worker
        .copy(
            "text".into(),
            CopyFormat::PlainText,
            FrameRequester::test_dummy(),
        )
        .unwrap();
    start_rx
        .recv_timeout(Duration::from_secs(/*secs*/ 5))
        .unwrap();
    drop(worker);
    release.send(()).unwrap();
    finish_rx
        .recv_timeout(Duration::from_secs(/*secs*/ 5))
        .unwrap();
}

#[tokio::test]
async fn expired_setup_discards_delivery_and_keeps_worker_busy_until_it_returns() {
    // Expiry must also be enforced by the worker when the UI has not polled yet.
    for poll_before_release in [true, false] {
        let mut worker = ClipboardWorker::default();
        let (frames, mut scheduled) = FrameRequester::test_channel();
        let (started, start_rx) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let (writes, write_rx) = mpsc::channel();
        worker
            .start(frames.clone(), move |text, _, setup| {
                started.send(()).unwrap();
                released.recv().unwrap();
                let outcome = setup.begin_delivery().map(|()| {
                    writes.send(text.to_owned()).unwrap();
                    CopyOutcome::Copied(None)
                });
                (outcome, None)
            })
            .unwrap();
        worker
            .copy("abandoned".into(), CopyFormat::PlainText, frames.clone())
            .unwrap();
        start_rx.recv_timeout(SETUP_TIMEOUT).unwrap();
        scheduled.try_recv().unwrap();
        assert!(worker.poll().is_none());
        // Every poll re-arms the setup deadline after earlier UI draws consume it.
        assert!(scheduled.try_recv().is_ok());
        let setup = &worker.pending.as_ref().unwrap().1;
        *setup.phase.lock().unwrap() = Setup::Pending(Instant::now());
        let failure = (1, Err(SETUP_TIMEOUT_MESSAGE.into()));
        if poll_before_release {
            assert_eq!(worker.poll(), Some(&failure));
        }
        assert!(worker.is_busy());
        assert_eq!(
            worker.copy("retry".into(), CopyFormat::PlainText, frames.clone()),
            Ok(CopyStatus::Busy)
        );
        release.send(()).unwrap();
        tokio::time::timeout(SETUP_TIMEOUT, async {
            while worker.is_busy() {
                worker.poll();
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(worker.poll(), Some(&failure));
        assert!(write_rx.try_recv().is_err());
        assert!(start_rx.try_recv().is_err());
        worker
            .copy("fresh".into(), CopyFormat::PlainText, frames)
            .unwrap();
        start_rx.recv_timeout(SETUP_TIMEOUT).unwrap();
        assert_eq!(worker.poll(), Some(&failure));
        release.send(()).unwrap();
        assert_eq!(write_rx.recv_timeout(SETUP_TIMEOUT).unwrap(), "fresh");
        tokio::time::timeout(SETUP_TIMEOUT, async {
            while worker.is_busy() {
                worker.poll();
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(worker.poll(), Some(&(2, Ok(CopyStatus::Confirmed))));
    }
}

#[test]
fn drop_waits_for_response_channel_disconnect() {
    let (requests, _incoming) = mpsc::channel();
    let (outgoing, responses) = mpsc::sync_channel(/*bound*/ 0);
    let mut worker = ClipboardWorker {
        requests: Some(requests),
        responses: Some(responses),
        pending: None,
        next_id: 0,
        completed: None,
    };
    worker
        .copy(
            "text".into(),
            CopyFormat::PlainText,
            FrameRequester::test_dummy(),
        )
        .unwrap();
    let cleanup = std::thread::spawn(move || {
        // The second rendezvous probes cleanup after receiving a result: shutdown
        // must wait for disconnection, not mistake the first result for handoff.
        for _ in 0..2 {
            outgoing
                .send(Response {
                    result: Ok(CopyStatus::Confirmed),
                    terminal_text: None,
                })
                .unwrap();
        }
    });
    drop(worker);
    cleanup.join().unwrap();
}

#[test]
fn delivery_claim_cannot_be_abandoned() {
    let setup = CopySetup {
        phase: Mutex::new(Setup::Pending(Instant::now() + SETUP_TIMEOUT)),
        frames: FrameRequester::test_dummy(),
    };
    assert_eq!(setup.begin_delivery(), Ok(()));
    assert!(
        !setup
            .phase
            .lock()
            .unwrap()
            .timed_out(Instant::now() + SETUP_TIMEOUT)
    );
    // Backend fallback checks cannot revoke a delivery that already began.
    assert_eq!(setup.begin_delivery(), Ok(()));
}
