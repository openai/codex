//! Voice delegation routes completed answers without exposing private protocol text.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn closing_voice_before_turn_completion_restores_the_delegated_answer_once() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let turn_id = "voice-turn-closing";
    start_item(
        &mut chat,
        thread_id,
        turn_id,
        user_item("<realtime_delegation><input>the answer?</input></realtime_delegation>"),
    );
    let answer = agent_item(
        "final-answer",
        "The answer is 42.",
        Some(MessagePhase::FinalAnswer),
    );
    start_item(&mut chat, thread_id, turn_id, answer.clone());
    complete_item(&mut chat, thread_id, turn_id, answer.clone());
    assert!(events.try_recv().is_err());

    chat.on_realtime_conversation_closed(/*reason*/ None);
    finish_turn(
        &mut chat,
        thread_id,
        turn_id,
        vec![answer],
        TurnStatus::Completed,
    );

    let mut visible_answers = 0;
    commit_realtime_history_events(&mut chat, &mut events);
    while let Ok(event) = events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            visible_answers += cell
                .display_lines(/*width*/ 80)
                .iter()
                .any(|line| line.to_string().contains("The answer is 42."))
                as usize;
        }
    }
    assert_eq!(visible_answers, 1);
    assert!(ops.try_recv().is_err());
}

#[tokio::test]
async fn delegated_voice_work_speaks_only_the_completed_final_answer_once() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let turn_id = "voice-turn";
    chat.turn_lifecycle.last_turn_id = Some(turn_id.to_string());
    let delegation =
        user_item("<realtime_delegation><input>best pizza?</input></realtime_delegation>");
    start_item(&mut chat, thread_id, turn_id, delegation.clone());
    complete_item(&mut chat, thread_id, turn_id, delegation);

    let commentary = agent_item(
        "commentary",
        "[COMMENTARY] checking current guides",
        Some(MessagePhase::Commentary),
    );
    start_item(&mut chat, thread_id, turn_id, commentary.clone());
    complete_item(&mut chat, thread_id, turn_id, commentary);
    let first_final = agent_item("first-final", "Earlier guess", /*phase*/ None);
    start_item(&mut chat, thread_id, turn_id, first_final.clone());
    complete_item(&mut chat, thread_id, turn_id, first_final);
    let final_answer = agent_item(
        "last-final",
        "[FINAL] Lucali in Brooklyn",
        /*phase*/ None,
    );
    start_item(&mut chat, thread_id, turn_id, final_answer.clone());
    chat.handle_server_notification(
        ServerNotification::AgentMessageDelta(AgentMessageDeltaNotification {
            thread_id: thread_id.to_string(),
            turn_id: turn_id.to_string(),
            item_id: "last-final".to_string(),
            delta: "Lucali".to_string(),
        }),
        /*replay_kind*/ None,
    );
    complete_item(&mut chat, thread_id, turn_id, final_answer.clone());
    assert!(ops.try_recv().is_err());
    assert!(events.try_recv().is_err());

    finish_turn(
        &mut chat,
        thread_id,
        turn_id,
        vec![final_answer],
        TurnStatus::Completed,
    );

    assert!(
        chat.pending_notification.is_none(),
        "voice-delegated turns should not trigger desktop completion notifications"
    );
    assert!(matches!(
        ops.try_recv(),
        Ok(AppCommand::RealtimeConversationSpeech { thread_id: spoken_thread_id, text, .. })
            if spoken_thread_id == thread_id && text.as_str() == "Lucali in Brooklyn"
    ));
    assert!(ops.try_recv().is_err());
    commit_realtime_history_events(&mut chat, &mut events);
    while let Ok(event) = events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            assert!(
                cell.display_lines(/*width*/ 80)
                    .iter()
                    .all(|line| !line.to_string().contains("Earlier guess"))
            );
        }
    }
}

#[tokio::test]
async fn delegation_started_before_peer_connection_keeps_its_voice_origin() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    chat.realtime_conversation.phase = RealtimeConversationPhase::Starting;
    chat.realtime_conversation.backend_started = true;
    chat.realtime_conversation.webrtc_connected = false;
    let turn_id = "startup-delegation";
    chat.turn_lifecycle.agent_turn_running = true;
    chat.turn_lifecycle.last_turn_id = Some(turn_id.to_string());
    let delegation =
        user_item("<realtime_delegation><input>spoken question</input></realtime_delegation>");
    start_item(&mut chat, thread_id, turn_id, delegation.clone());
    complete_item(&mut chat, thread_id, turn_id, delegation);

    chat.on_realtime_webrtc_connected(chat.realtime_conversation.attempt_id, Ok(()));
    let final_answer = agent_item(
        "startup-final",
        "[FINAL] Spoken answer",
        /*phase*/ None,
    );
    start_item(&mut chat, thread_id, turn_id, final_answer.clone());
    complete_item(&mut chat, thread_id, turn_id, final_answer.clone());
    finish_turn(
        &mut chat,
        thread_id,
        turn_id,
        vec![final_answer],
        TurnStatus::Completed,
    );

    assert!(matches!(
        ops.try_recv(),
        Ok(AppCommand::RealtimeConversationSpeech { text, .. }) if text.as_str() == "Spoken answer"
    ));
    assert!(ops.try_recv().is_err());
    let mut rendered_history = Vec::new();
    commit_realtime_history_events(&mut chat, &mut events);
    while let Ok(event) = events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            let rendered = cell
                .transcript_lines(/*width*/ 80)
                .iter()
                .map(ToString::to_string)
                .collect::<String>();
            assert!(!rendered.contains("Spoken answer"));
            assert!(!rendered.contains("<realtime_delegation>"));
            rendered_history.push(rendered);
        }
    }
    insta::assert_snapshot!(
        "voice_delegation_during_connection",
        without_completion_metadata(&rendered_history.join("\n"))
    );
}

#[tokio::test]
async fn delegated_answer_with_async_question_opens_the_editor_instead_of_speech() {
    let (mut chat, _sender, _events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let turn_id = "question-turn";
    start_item(
        &mut chat,
        thread_id,
        turn_id,
        user_item("<realtime_delegation><input>choose</input></realtime_delegation>"),
    );
    let answer = ThreadItem::AgentMessage {
        id: "question-answer".into(),
        text: "Which option?".into(),
        phase: Some(MessagePhase::FinalAnswer),
        questions: Some(vec![AsyncUserInputQuestion {
            title: "Which option?".into(),
            options: None,
        }]),
        memory_citation: None,
        delivery: None,
    };
    start_item(&mut chat, thread_id, turn_id, answer.clone());
    complete_item(&mut chat, thread_id, turn_id, answer.clone());
    finish_turn(
        &mut chat,
        thread_id,
        turn_id,
        vec![answer],
        TurnStatus::Completed,
    );

    assert_eq!(
        chat.bottom_pane
            .questions
            .as_ref()
            .map(|editor| editor.unanswered_count()),
        Some(1)
    );
    assert!(
        ops.try_recv().is_err(),
        "question-bearing answer stays in the TUI"
    );
}

#[tokio::test]
async fn typed_turn_running_during_peer_connection_stays_typed() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    chat.realtime_conversation.phase = RealtimeConversationPhase::Starting;
    chat.realtime_conversation.backend_started = true;
    chat.realtime_conversation.webrtc_connected = false;
    chat.turn_lifecycle.agent_turn_running = true;
    chat.turn_lifecycle.last_turn_id = Some("typed-startup".to_string());

    chat.on_realtime_webrtc_connected(chat.realtime_conversation.attempt_id, Ok(()));

    assert!(!chat.realtime_conversation.latest_input_was_voice);
    assert!(matches!(
        chat.realtime_conversation.turn_origins.get("typed-startup"),
        Some(super::super::RealtimeTurnOrigin::Typed { .. })
    ));
    assert_eq!(chat.thread_id(), Some(thread_id));
}

#[tokio::test]
async fn delegated_item_that_becomes_final_at_turn_completion_is_recoverable() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let turn_id = "changed-final-turn";
    start_item(
        &mut chat,
        thread_id,
        turn_id,
        user_item("<realtime_delegation><input>question</input></realtime_delegation>"),
    );
    let analysis = agent_item(
        "same-id",
        "[ANALYSIS] secret",
        Some(MessagePhase::Commentary),
    );
    start_item(&mut chat, thread_id, turn_id, analysis.clone());
    complete_item(&mut chat, thread_id, turn_id, analysis);
    let final_answer = agent_item("same-id", "Public answer", Some(MessagePhase::FinalAnswer));
    finish_turn(
        &mut chat,
        thread_id,
        turn_id,
        vec![final_answer],
        TurnStatus::Completed,
    );
    let AppCommand::RealtimeConversationSpeech {
        delivery_id, text, ..
    } = ops.try_recv().unwrap()
    else {
        panic!("changed final should be delivered");
    };
    assert_eq!(text.as_str(), "Public answer");
    chat.restore_undelivered_realtime_speech(delivery_id);
    commit_realtime_history_events(&mut chat, &mut events);
    while let Ok(event) = events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            for line in cell.display_lines(/*width*/ 80) {
                assert!(!line.to_string().contains("secret"));
            }
        }
    }
}

#[tokio::test]
async fn explicit_final_answer_can_explain_private_channel_markers() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let turn_id = "marker-final-turn";
    start_item(
        &mut chat,
        thread_id,
        turn_id,
        user_item("<realtime_delegation><input>explain markers</input></realtime_delegation>"),
    );
    let answer = agent_item(
        "marker-final",
        "[COMMENTARY] is a label in the documentation.",
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
    let AppCommand::RealtimeConversationSpeech {
        delivery_id, text, ..
    } = ops.try_recv().expect("final answer should be delivered")
    else {
        panic!("expected speech delivery");
    };
    assert_eq!(
        text.as_str(),
        "[COMMENTARY] is a label in the documentation."
    );
    assert!(ops.try_recv().is_err());
    chat.restore_undelivered_realtime_speech(delivery_id);
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
    insta::assert_snapshot!(
        "explicit_final_answer_with_channel_marker",
        without_completion_metadata(&rendered)
    );
}

#[tokio::test]
async fn newer_spoken_input_prevents_an_older_voice_turn_from_speaking() {
    let (mut chat, _sender, _events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    chat.on_realtime_transcript_delta("user".to_string(), "first question".to_string());
    chat.on_realtime_transcript_done("user".to_string(), "first question".to_string());

    let old_turn_id = "old-voice-turn";
    start_item(
        &mut chat,
        thread_id,
        old_turn_id,
        user_item("<realtime_delegation><input>first question</input></realtime_delegation>"),
    );
    let old_answer = agent_item(
        "old-answer",
        "answer to the earlier question",
        Some(MessagePhase::FinalAnswer),
    );
    start_item(&mut chat, thread_id, old_turn_id, old_answer.clone());
    complete_item(&mut chat, thread_id, old_turn_id, old_answer.clone());

    chat.on_realtime_transcript_delta("user".to_string(), "newer question".to_string());
    chat.on_realtime_transcript_done("user".to_string(), "newer question".to_string());
    finish_turn(
        &mut chat,
        thread_id,
        old_turn_id,
        vec![old_answer],
        TurnStatus::Completed,
    );

    assert!(ops.try_recv().is_err());
}

#[tokio::test]
async fn newer_voice_delegation_on_the_same_turn_speaks_only_its_own_answer() {
    let (mut chat, _sender, _events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    chat.on_realtime_transcript_done("user".to_string(), "first".to_string());
    let turn_id = "shared-voice-turn";
    start_item(
        &mut chat,
        thread_id,
        turn_id,
        user_item("<realtime_delegation><input>first question</input></realtime_delegation>"),
    );
    let old_answer = agent_item(
        "old-answer",
        "answer to the earlier question",
        Some(MessagePhase::FinalAnswer),
    );
    start_item(&mut chat, thread_id, turn_id, old_answer.clone());
    complete_item(&mut chat, thread_id, turn_id, old_answer);

    start_item(
        &mut chat,
        thread_id,
        turn_id,
        user_item("<realtime_delegation><input>newer question</input></realtime_delegation>"),
    );
    let latest_answer = agent_item(
        "latest-answer",
        "answer to the newer question",
        Some(MessagePhase::FinalAnswer),
    );
    start_item(&mut chat, thread_id, turn_id, latest_answer.clone());
    complete_item(&mut chat, thread_id, turn_id, latest_answer.clone());
    finish_turn(
        &mut chat,
        thread_id,
        turn_id,
        vec![latest_answer],
        TurnStatus::Completed,
    );

    assert!(matches!(
        ops.try_recv(),
        Ok(AppCommand::RealtimeConversationSpeech { text, .. })
            if text.as_str() == "answer to the newer question"
    ));
    let owner = chat.realtime_conversation.speaker_suppression_generation;
    assert_eq!(owner, None);
    chat.on_realtime_transcript_delta("assistant".to_string(), "answer".to_string());
    let owner = chat.realtime_conversation.speaker_suppression_generation;
    assert!(owner.is_none());
    assert!(ops.try_recv().is_err());
}
