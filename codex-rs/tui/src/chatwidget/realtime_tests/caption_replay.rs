//! Replay and captions reconcile once across thread and session changes.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn replay_preserves_typed_updates_before_voice_steers_the_turn() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.thread_id = Some(ThreadId::new());
    chat.replay_thread_turns(
        vec![Turn {
            id: "typed-then-voice".into(),
            items: vec![
                user_item("Typed question"),
                agent_item(
                    "typed-update",
                    "Checking the typed request",
                    Some(MessagePhase::Commentary),
                ),
                ThreadItem::Reasoning {
                    id: "typed-reasoning".into(),
                    summary: vec!["Typed reasoning summary".into()],
                    content: Vec::new(),
                },
                user_item(
                    "<realtime_delegation><input>spoken correction</input></realtime_delegation>",
                ),
                agent_item(
                    "private-update",
                    "Private voice commentary",
                    Some(MessagePhase::Commentary),
                ),
                ThreadItem::Reasoning {
                    id: "private-reasoning".into(),
                    summary: vec!["Private voice reasoning".into()],
                    content: Vec::new(),
                },
            ],
            items_view: TurnItemsView::Full,
            status: TurnStatus::Completed,
            error: None,
            started_at: None,
            completed_at: None,
            duration_ms: None,
        }],
        ReplayKind::ThreadSnapshot,
    );
    chat.flush_answer_stream_with_separator();
    commit_realtime_history_events(&mut chat, &mut events);
    let rendered = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(
                cell.transcript_lines(/*width*/ 80)
                    .into_iter()
                    .map(|line| line.to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("Checking the typed request"),
        "{rendered}"
    );
    assert!(rendered.contains("Typed reasoning summary"), "{rendered}");
    assert!(!rendered.contains("Private voice"), "{rendered}");
    insta::assert_snapshot!("typed_turn_replay_before_voice_steering", rendered);
}

#[tokio::test]
async fn in_progress_voice_replay_restores_the_late_reasoning_guard() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    let voice_request =
        user_item("<realtime_delegation><input>question</input></realtime_delegation>");
    chat.replay_thread_turns(
        vec![Turn {
            id: "saved-voice-turn".into(),
            items: vec![voice_request.clone()],
            items_view: TurnItemsView::Full,
            status: TurnStatus::InProgress,
            error: None,
            started_at: None,
            completed_at: None,
            duration_ms: None,
        }],
        ReplayKind::ThreadSnapshot,
    );
    chat.handle_server_notification(
        ServerNotification::ItemStarted(ItemStartedNotification {
            thread_id: thread_id.to_string(),
            turn_id: "buffered-voice-turn".into(),
            item: voice_request,
            started_at_ms: 0,
        }),
        Some(ReplayKind::ThreadSnapshot),
    );
    while events.try_recv().is_ok() {}
    for turn_id in ["saved-voice-turn", "buffered-voice-turn"] {
        chat.handle_server_notification(
            ServerNotification::ReasoningSummaryTextDelta(ReasoningSummaryTextDeltaNotification {
                thread_id: thread_id.to_string(),
                turn_id: turn_id.into(),
                item_id: "late-reasoning".into(),
                delta: "private after switch".into(),
                summary_index: 0,
            }),
            /*replay_kind*/ None,
        );
        complete_item(
            &mut chat,
            thread_id,
            turn_id,
            ThreadItem::Reasoning {
                id: "late-reasoning".into(),
                summary: vec!["private after switch".into()],
                content: Vec::new(),
            },
        );
        assert!(chat.is_realtime_delegated_reasoning_turn(turn_id));
    }
    commit_realtime_history_events(&mut chat, &mut events);
    while let Ok(event) = events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            let rendered = cell
                .transcript_lines(/*width*/ 80)
                .into_iter()
                .map(|line| line.to_string())
                .collect::<String>();
            assert!(!rendered.contains("private"), "{rendered}");
        }
    }
}

#[tokio::test]
async fn accepted_voice_answer_with_only_an_old_caption_returns_to_history_on_close() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let turn_id = "accepted-turn";
    start_item(
        &mut chat,
        thread_id,
        turn_id,
        user_item("<realtime_delegation><input>question</input></realtime_delegation>"),
    );
    let answer = agent_item("answer", "Accepted answer", Some(MessagePhase::FinalAnswer));
    start_item(&mut chat, thread_id, turn_id, answer.clone());
    complete_item(&mut chat, thread_id, turn_id, answer.clone());
    finish_turn(
        &mut chat,
        thread_id,
        turn_id,
        vec![answer],
        TurnStatus::Completed,
    );
    let AppCommand::RealtimeConversationSpeech { delivery_id, .. } = ops.try_recv().unwrap() else {
        panic!("completed voice turn should queue speech");
    };
    while events.try_recv().is_ok() {}
    chat.accept_realtime_speech(delivery_id);
    assert!(!chat.has_pending_realtime_speech(delivery_id));
    chat.realtime_conversation.assistant_transcript_generation =
        Some(chat.realtime_conversation.input_generation.wrapping_sub(1));
    chat.on_realtime_transcript_done("assistant".into(), "Earlier speech".into());
    assert_eq!(chat.realtime_conversation.pending_speech.len(), 1);
    chat.on_realtime_conversation_closed(Some("transport_closed".into()));
    chat.restore_undelivered_realtime_speech(delivery_id);
    let mut answers = 0;
    commit_realtime_history_events(&mut chat, &mut events);
    while let Ok(event) = events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            let rendered = cell
                .display_lines(/*width*/ 80)
                .into_iter()
                .map(|line| line.to_string())
                .collect::<String>();
            answers += usize::from(rendered.contains("Accepted answer"));
        }
    }
    assert_eq!(answers, 1);
}

#[tokio::test]
async fn unrelated_caption_started_before_speech_queue_keeps_answer_fallback() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let turn_id = "answer-after-old-caption";
    start_item(
        &mut chat,
        thread_id,
        turn_id,
        user_item("<realtime_delegation><input>question</input></realtime_delegation>"),
    );
    chat.on_realtime_transcript_delta("assistant".into(), "Unrelated ".into());
    let answer = agent_item("answer", "New answer", Some(MessagePhase::FinalAnswer));
    start_item(&mut chat, thread_id, turn_id, answer.clone());
    complete_item(&mut chat, thread_id, turn_id, answer.clone());
    finish_turn(
        &mut chat,
        thread_id,
        turn_id,
        vec![answer],
        TurnStatus::Completed,
    );
    let AppCommand::RealtimeConversationSpeech { delivery_id, .. } = ops.try_recv().unwrap() else {
        panic!("completed voice turn should queue speech");
    };
    while events.try_recv().is_ok() {}
    chat.accept_realtime_speech(delivery_id);
    chat.on_realtime_transcript_done("assistant".into(), "Unrelated caption".into());
    assert_eq!(chat.realtime_conversation.pending_speech.len(), 1);
    chat.on_realtime_conversation_closed(Some("transport_closed".into()));
    let restored = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(
                cell.display_lines(/*width*/ 80)
                    .into_iter()
                    .map(|line| line.to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            _ => None,
        })
        .filter(|text| text.contains("New answer"))
        .count();
    assert_eq!(restored, 1);
}

#[tokio::test]
async fn done_only_caption_cannot_retire_a_newer_voice_answer() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let turn_id = "done-only-turn";
    start_item(
        &mut chat,
        thread_id,
        turn_id,
        user_item("<realtime_delegation><input>question</input></realtime_delegation>"),
    );
    let answer = agent_item("answer", "New answer", Some(MessagePhase::FinalAnswer));
    start_item(&mut chat, thread_id, turn_id, answer.clone());
    complete_item(&mut chat, thread_id, turn_id, answer.clone());
    finish_turn(
        &mut chat,
        thread_id,
        turn_id,
        vec![answer],
        TurnStatus::Completed,
    );
    let AppCommand::RealtimeConversationSpeech { delivery_id, .. } = ops.try_recv().unwrap() else {
        panic!("completed voice turn should queue speech");
    };
    while events.try_recv().is_ok() {}
    chat.accept_realtime_speech(delivery_id);
    assert!(
        chat.realtime_conversation
            .assistant_transcript_generation
            .is_none()
    );
    chat.on_realtime_transcript_done("assistant".into(), "Old done-only caption".into());
    assert_eq!(chat.realtime_conversation.pending_speech.len(), 1);
    chat.on_realtime_conversation_closed(Some("transport_closed".into()));
    let restored = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(
                cell.display_lines(/*width*/ 80)
                    .into_iter()
                    .map(|line| line.to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            _ => None,
        })
        .filter(|text| text.contains("New answer"))
        .count();
    assert_eq!(restored, 1);
}

#[tokio::test]
async fn captioned_voice_answer_does_not_duplicate_on_close() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let turn_id = "captioned-turn";
    start_item(
        &mut chat,
        thread_id,
        turn_id,
        user_item("<realtime_delegation><input>question</input></realtime_delegation>"),
    );
    let answer = agent_item(
        "answer",
        "Captioned answer",
        Some(MessagePhase::FinalAnswer),
    );
    start_item(&mut chat, thread_id, turn_id, answer.clone());
    complete_item(&mut chat, thread_id, turn_id, answer.clone());
    finish_turn(
        &mut chat,
        thread_id,
        turn_id,
        vec![answer],
        TurnStatus::Completed,
    );
    let AppCommand::RealtimeConversationSpeech { delivery_id, .. } = ops.try_recv().unwrap() else {
        panic!("completed voice turn should queue speech");
    };
    while events.try_recv().is_ok() {}
    // A delta establishes which input generation owns the completed caption.
    chat.on_realtime_transcript_delta("assistant".into(), "Captioned ".into());
    chat.on_realtime_transcript_done("assistant".into(), "Captioned answer".into());
    chat.accept_realtime_speech(delivery_id);
    chat.reset_realtime_conversation();
    chat.restore_undelivered_realtime_speech(delivery_id);
    let mut rendered = Vec::new();
    commit_realtime_history_events(&mut chat, &mut events);
    while let Ok(event) = events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            rendered.extend(
                cell.display_lines(/*width*/ 80)
                    .into_iter()
                    .map(|line| line.to_string()),
            );
        }
    }
    insta::assert_snapshot!("captioned_voice_answer_on_close", rendered.join("\n"));
}

#[tokio::test]
async fn restored_partial_caption_accepts_late_completion_without_duplicate_history() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.local_settings.tui.animations = false;
    chat.restore_realtime_transcript_cells(VecDeque::from([
        super::super::RealtimeTranscriptRecord {
            role: "user".into(),
            text: "last ".into(),
            complete: false,
            before_turn_id: None,
        },
    ]));
    let live = chat
        .realtime_conversation
        .live_transcript_cell
        .as_ref()
        .unwrap();
    insta::assert_snapshot!(
        "restored_voice_partial_live",
        live.display_lines(/*width*/ 80)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
    chat.on_realtime_transcript_delta("user".into(), "words".into());
    let retained = chat.take_realtime_transcript_cells_for_replay();
    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0].text, "last words");
    chat.restore_realtime_transcript_cells(retained);
    chat.on_realtime_transcript_done("user".into(), "last words".into());
    assert!(chat.realtime_conversation.live_transcript_cell.is_none());
    assert_eq!(chat.realtime_conversation.accepted_transcripts.len(), 1);
    assert_eq!(
        chat.realtime_conversation.accepted_transcripts[0].text,
        "last words"
    );
    assert!(chat.realtime_conversation.accepted_transcripts[0].complete);
    commit_realtime_history_events(&mut chat, &mut events);
    let rendered = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(
                cell.display_lines(/*width*/ 80)
                    .into_iter()
                    .map(|line| line.to_string())
                    .collect::<String>(),
            ),
            _ => None,
        })
        .collect::<String>();
    assert_eq!(rendered.matches("last words").count(), 1);
}

#[tokio::test]
async fn empty_late_completion_discards_the_restored_partial() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.restore_realtime_transcript_cells(VecDeque::from([
        super::super::RealtimeTranscriptRecord {
            role: "user".into(),
            text: "unfinished".into(),
            complete: false,
            before_turn_id: None,
        },
    ]));
    chat.on_realtime_transcript_done("user".into(), String::new());

    assert!(chat.realtime_conversation.live_transcript_cell.is_none());
    assert!(chat.realtime_conversation.accepted_transcripts.is_empty());
    assert!(events.try_recv().is_err());
}
