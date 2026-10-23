//! Resume snapshots must accept an unfinished reasoning stream without an earlier item/started.

use super::*;
use crate::chatwidget::tests::helpers::render_bottom_popup;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ReasoningSummaryTextDeltaNotification;

#[tokio::test]
async fn resumed_reasoning_without_start_accepts_deltas_and_completion() {
    for initial_replay in [false, true] {
        for snapshot_item_id in [None, Some("reasoning"), Some("previous")] {
            for with_delta in [false, true] {
                let (mut app, mut events, _ops) = make_test_app_with_channels().await;
                let thread_id = ThreadId::new();
                let session = test_thread_session(thread_id, app.config.cwd.to_path_buf());
                let items = snapshot_item_id
                    .into_iter()
                    .map(|id| ThreadItemPayload::Reasoning {
                        id: id.into(),
                        summary: vec![
                            if id == "previous" {
                                "**Earlier work**\nEarlier paragraph."
                            } else {
                                "**Inspecting**\nSnapshot paragraph."
                            }
                            .into(),
                        ],
                        content: Vec::new(),
                    })
                    .collect();
                let turns = vec![test_turn("turn", TurnStatus::InProgress, items)];
                if initial_replay {
                    app.chat_widget.handle_thread_session(session);
                    app.chat_widget
                        .replay_thread_turns(turns, ReplayKind::ResumeInitialMessages);
                } else {
                    // Reconnect constructs a fresh store; no start notification was retained.
                    let store =
                        ThreadEventStore::new_with_session(/*capacity*/ 1, session, turns);
                    assert!(store.active_reasoning_item.is_none());
                    app.replay_thread_snapshot(
                        store.snapshot(),
                        /*resume_restored_queue*/ false,
                    );
                }
                let delta = |turn_id: &str, text: &str| {
                    ServerNotification::ReasoningSummaryTextDelta(
                        ReasoningSummaryTextDeltaNotification {
                            thread_id: thread_id.to_string(),
                            turn_id: turn_id.into(),
                            item_id: "reasoning".into(),
                            delta: text.into(),
                            summary_index: 0,
                        },
                    )
                };
                app.chat_widget.handle_server_notification(
                    delta("old-turn", "Wrong turn"),
                    /*replay_kind*/ None,
                );
                assert!(
                    !render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("Wrong turn")
                );
                if with_delta {
                    app.chat_widget.handle_server_notification(
                        delta("turn", "\n**Running checks**"),
                        /*replay_kind*/ None,
                    );
                    assert!(
                        render_bottom_popup(&app.chat_widget, /*width*/ 80)
                            .contains("Running checks")
                    );
                }
                app.chat_widget.handle_server_notification(
                    ServerNotification::ItemCompleted(ItemCompletedNotification::new(
                        thread_id.to_string(),
                        "turn".into(),
                        ThreadItemPayload::Reasoning {
                            id: "reasoning".into(),
                            summary: vec![
                                "**Inspecting**\nSnapshot paragraph.\nCompleted paragraph.".into(),
                            ],
                            content: Vec::new(),
                        },
                        /*started_at_ms*/ None,
                        /*completed_at_ms*/ 0,
                    )),
                    /*replay_kind*/ None,
                );
                app.chat_widget.handle_server_notification(
                    delta("turn", "Stale update"),
                    /*replay_kind*/ None,
                );
                assert!(
                    !render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("Stale update")
                );
                let transcript = reasoning_transcript(&mut events);
                insta::allow_duplicates! {
                    if snapshot_item_id == Some("previous") {
                        insta::assert_snapshot!(transcript, @"
                        • Earlier paragraph.

                        • Snapshot paragraph.
                          Completed paragraph.
                        ");
                    } else {
                        insta::assert_snapshot!(transcript, @"
                        • Snapshot paragraph.
                          Completed paragraph.
                        ");
                    }
                }
            }
        }
    }
}

#[tokio::test]
async fn resumed_trailing_reasoning_is_kept_when_no_more_reasoning_arrives() {
    for next_item in [false, true] {
        let (mut app, mut events, _ops) = make_test_app_with_channels().await;
        let thread_id = ThreadId::new();
        let session = test_thread_session(thread_id, app.config.cwd.to_path_buf());
        let store = ThreadEventStore::new_with_session(
            /*capacity*/ 1,
            session,
            vec![test_turn(
                "turn",
                TurnStatus::InProgress,
                vec![ThreadItemPayload::Reasoning {
                    id: "finished".into(),
                    summary: vec!["**Inspecting**\nSaved paragraph.".into()],
                    content: Vec::new(),
                }],
            )],
        );
        app.replay_thread_snapshot(store.snapshot(), /*resume_restored_queue*/ false);
        if next_item {
            app.chat_widget.handle_server_notification(
                ServerNotification::ItemStarted(
                    codex_app_server_protocol::ItemStartedNotification::new(
                        thread_id.to_string(),
                        "turn".into(),
                        ThreadItemPayload::Reasoning {
                            id: "next".into(),
                            summary: Vec::new(),
                            content: Vec::new(),
                        },
                        /*started_at_ms*/ 0,
                    ),
                ),
                /*replay_kind*/ None,
            );
        }
        app.chat_widget.handle_server_notification(
            turn_completed_notification(thread_id, "turn", TurnStatus::Completed),
            /*replay_kind*/ None,
        );
        insta::allow_duplicates! {
            insta::assert_snapshot!(reasoning_transcript(&mut events), @"
            • Saved paragraph.
            ");
        }
    }
}

fn reasoning_transcript(events: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>) -> String {
    std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell)
                if cell
                    .as_any()
                    .is::<crate::history_cell::ReasoningSummaryCell>() =>
            {
                Some(lines_to_single_string(&cell.transcript_lines(/*width*/ 80)))
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}
