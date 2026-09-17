//! Typed and spoken steering preserve ownership and private reasoning boundaries.

use super::*;
use pretty_assertions::assert_eq;

fn complete_stale_handoff(chat: &mut ChatWidget, thread_id: ThreadId, question: &str) {
    let turn_id = "stale-turn";
    start_item(
        chat,
        thread_id,
        turn_id,
        user_item(&format!(
            "<realtime_delegation><input>{question}</input></realtime_delegation>"
        )),
    );
    let answer = agent_item(
        "stale-answer",
        "stale answer",
        Some(MessagePhase::FinalAnswer),
    );
    start_item(chat, thread_id, turn_id, answer.clone());
    complete_item(chat, thread_id, turn_id, answer.clone());
    finish_turn(
        chat,
        thread_id,
        turn_id,
        vec![answer],
        TurnStatus::Completed,
    );
}

#[tokio::test]
async fn typed_turn_remains_a_normal_text_response_while_voice_is_active() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let turn_id = "typed-turn";
    chat.turn_lifecycle.last_turn_id = Some(turn_id.to_string());
    let typed = user_item("pick a random number");
    start_item(&mut chat, thread_id, turn_id, typed.clone());
    complete_item(&mut chat, thread_id, turn_id, typed);
    let answer = agent_item("typed-answer", "73", Some(MessagePhase::FinalAnswer));
    start_item(&mut chat, thread_id, turn_id, answer.clone());
    complete_item(&mut chat, thread_id, turn_id, answer.clone());
    finish_turn(
        &mut chat,
        thread_id,
        turn_id,
        vec![answer],
        TurnStatus::Completed,
    );

    assert!(matches!(
        chat.pending_notification.as_ref(),
        Some(crate::chatwidget::Notification::AgentTurnComplete { response }) if response == "73"
    ));
    let mut rendered = String::new();
    while let Ok(event) = events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            for line in cell.transcript_lines(/*width*/ 80) {
                rendered.push_str(&line.to_string());
            }
        }
    }
    assert!(rendered.contains("pick a random number"));
    assert!(rendered.contains("73"));
    assert!(ops.try_recv().is_err());
}

#[tokio::test]
async fn typed_steering_restores_normal_output_for_an_existing_voice_item() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let turn_id = "shared-turn";
    start_item(
        &mut chat,
        thread_id,
        turn_id,
        user_item("<realtime_delegation><input>earlier voice</input></realtime_delegation>"),
    );
    let answer = agent_item(
        "shared-answer",
        "typed answer",
        Some(MessagePhase::FinalAnswer),
    );
    start_item(&mut chat, thread_id, turn_id, answer.clone());
    let private = agent_item(
        "private-commentary",
        "private voice commentary",
        Some(MessagePhase::Commentary),
    );
    start_item(&mut chat, thread_id, turn_id, private.clone());
    start_item(
        &mut chat,
        thread_id,
        turn_id,
        user_item("new typed question"),
    );
    complete_item(&mut chat, thread_id, turn_id, private);
    complete_item(&mut chat, thread_id, turn_id, answer.clone());
    finish_turn(
        &mut chat,
        thread_id,
        turn_id,
        vec![answer],
        TurnStatus::Completed,
    );

    let mut rendered = String::new();
    while let Ok(event) = events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event
            && !cell.as_any().is::<FinalMessageSeparator>()
        {
            for line in cell.display_lines(/*width*/ 80) {
                rendered.push_str(&line.to_string());
            }
        }
    }
    assert!(rendered.contains("typed answer"), "{rendered}");
    assert!(!rendered.contains("private voice commentary"), "{rendered}");
    insta::assert_snapshot!("typed_steering_keeps_voice_commentary_private", rendered);
    assert!(ops.try_recv().is_err());
}

#[tokio::test]
async fn newer_voice_steering_an_existing_typed_turn_is_spoken_once() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let turn_id = "shared-typed-turn";
    start_item(&mut chat, thread_id, turn_id, user_item("typed task"));
    let earlier_typed = agent_item("typed-item", "existing typed work", /*phase*/ None);
    start_item(&mut chat, thread_id, turn_id, earlier_typed);

    chat.on_realtime_transcript_delta("user".to_string(), "spoken follow-up".to_string());
    chat.on_realtime_transcript_done("user".to_string(), "spoken follow-up".to_string());
    let delegation =
        user_item("<realtime_delegation><input>spoken follow-up</input></realtime_delegation>");
    start_item(&mut chat, thread_id, turn_id, delegation.clone());
    complete_item(&mut chat, thread_id, turn_id, delegation);
    let answer = agent_item(
        "voice-item",
        "answer to the spoken follow-up",
        /*phase*/ None,
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

    assert!(matches!(
        ops.try_recv(),
        Ok(AppCommand::RealtimeConversationSpeech { text, .. })
            if text.as_str() == "answer to the spoken follow-up"
    ));
    assert!(ops.try_recv().is_err());
    while let Ok(event) = events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            assert!(
                cell.display_lines(/*width*/ 80)
                    .iter()
                    .all(|line| !line.to_string().contains("realtime_delegation"))
            );
        }
    }
}

#[tokio::test]
async fn voice_handoff_preserves_started_typed_reasoning_but_hides_new_reasoning() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let turn_id = "mixed-reasoning-turn";
    start_item(&mut chat, thread_id, turn_id, user_item("typed task"));
    start_item(
        &mut chat,
        thread_id,
        turn_id,
        ThreadItem::Reasoning {
            id: "typed-reasoning".into(),
            summary: Vec::new(),
            content: Vec::new(),
        },
    );
    chat.handle_server_notification(
        ServerNotification::ReasoningSummaryTextDelta(ReasoningSummaryTextDeltaNotification {
            thread_id: thread_id.to_string(),
            turn_id: turn_id.into(),
            item_id: "typed-reasoning".into(),
            delta: "Typed beginning ".into(),
            summary_index: 0,
        }),
        /*replay_kind*/ None,
    );

    let delegation =
        user_item("<realtime_delegation><input>spoken follow-up</input></realtime_delegation>");
    start_item(&mut chat, thread_id, turn_id, delegation);
    chat.handle_server_notification(
        ServerNotification::ReasoningSummaryTextDelta(ReasoningSummaryTextDeltaNotification {
            thread_id: thread_id.to_string(),
            turn_id: turn_id.into(),
            item_id: "typed-reasoning".into(),
            delta: "and typed tail".into(),
            summary_index: 0,
        }),
        /*replay_kind*/ None,
    );
    complete_item(
        &mut chat,
        thread_id,
        turn_id,
        ThreadItem::Reasoning {
            id: "typed-reasoning".into(),
            summary: vec!["Typed beginning and typed tail".into()],
            content: Vec::new(),
        },
    );
    start_item(
        &mut chat,
        thread_id,
        turn_id,
        ThreadItem::Reasoning {
            id: "private-reasoning".into(),
            summary: Vec::new(),
            content: Vec::new(),
        },
    );
    chat.handle_server_notification(
        ServerNotification::ReasoningSummaryTextDelta(ReasoningSummaryTextDeltaNotification {
            thread_id: thread_id.to_string(),
            turn_id: turn_id.into(),
            item_id: "private-reasoning".into(),
            delta: "private after handoff".into(),
            summary_index: 0,
        }),
        /*replay_kind*/ None,
    );
    complete_item(
        &mut chat,
        thread_id,
        turn_id,
        ThreadItem::Reasoning {
            id: "private-reasoning".into(),
            summary: vec!["private after handoff".into()],
            content: Vec::new(),
        },
    );

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
        rendered.contains("Typed beginning and typed tail"),
        "{rendered}"
    );
    assert!(!rendered.contains("private after handoff"), "{rendered}");
    insta::assert_snapshot!("typed_reasoning_through_voice_handoff", rendered);
}

#[tokio::test]
async fn voice_delegation_without_transcript_can_steer_an_existing_typed_turn() {
    let (mut chat, _sender, _events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let turn_id = "shared-turn-without-transcript";
    start_item(
        &mut chat,
        thread_id,
        turn_id,
        user_item("earlier typed task"),
    );
    start_item(
        &mut chat,
        thread_id,
        turn_id,
        user_item("<realtime_delegation><input>new spoken request</input></realtime_delegation>"),
    );
    let answer = agent_item(
        "voice-answer",
        "new spoken answer",
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

    assert!(matches!(
        ops.try_recv(),
        Ok(AppCommand::RealtimeConversationSpeech { text, .. }) if text.as_str() == "new spoken answer"
    ));
    assert!(ops.try_recv().is_err());
}

#[tokio::test]
async fn typed_agent_item_completes_visibly_after_voice_handoff() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let turn_id = "typed-then-voice";
    start_item(&mut chat, thread_id, turn_id, user_item("typed request"));
    let typed = agent_item(
        "typed-commentary",
        "Visible typed commentary",
        Some(MessagePhase::Commentary),
    );
    start_item(&mut chat, thread_id, turn_id, typed.clone());
    start_item(
        &mut chat,
        thread_id,
        turn_id,
        user_item("<realtime_delegation><input>voice request</input></realtime_delegation>"),
    );
    complete_item(&mut chat, thread_id, turn_id, typed);
    let private = agent_item(
        "voice-commentary",
        "Hidden voice commentary",
        Some(MessagePhase::Commentary),
    );
    start_item(&mut chat, thread_id, turn_id, private.clone());
    complete_item(&mut chat, thread_id, turn_id, private);
    let rendered = std::iter::from_fn(|| events.try_recv().ok())
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
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered.contains("Visible typed commentary"), "{rendered}");
    assert!(!rendered.contains("Hidden voice commentary"), "{rendered}");
}

#[tokio::test]
async fn stale_voice_handoff_after_newer_typed_input_is_never_spoken() {
    let (mut chat, _sender, _events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    chat.on_realtime_transcript_done("user".to_string(), "How are you doing".to_string());
    chat.note_realtime_typed_input("what's the best pizza in new york?");
    complete_stale_handoff(&mut chat, thread_id, "How are you doing");

    assert!(ops.try_recv().is_err());
}

#[tokio::test]
async fn typed_submission_suppresses_old_speaker_audio() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    chat.note_realtime_typed_input("typed correction");
    let generation = chat.realtime_conversation.input_generation;
    assert_eq!(
        chat.realtime_conversation.speaker_suppression_generation,
        Some(generation)
    );
    start_item(
        &mut chat,
        thread_id,
        "typed-turn",
        user_item("typed correction"),
    );
    assert_eq!(
        chat.realtime_conversation.speaker_suppression_generation,
        Some(chat.realtime_conversation.input_generation)
    );
}

#[tokio::test]
async fn late_voice_transcript_after_typed_input_cannot_revive_a_stale_handoff() {
    let (mut chat, _sender, _events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    chat.on_realtime_transcript_delta("user".to_string(), "How are ".to_string());
    chat.note_realtime_typed_input("what's the best pizza in new york?");
    chat.on_realtime_transcript_delta("user".to_string(), "you doing".to_string());
    chat.on_realtime_transcript_done("user".to_string(), "How are you doing".to_string());

    complete_stale_handoff(&mut chat, thread_id, "How are you doing");

    assert!(!chat.realtime_conversation.latest_input_was_voice);
    assert!(ops.try_recv().is_err());
}

#[tokio::test]
async fn duplicate_delta_less_transcript_after_typed_input_cannot_revive_old_voice() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    chat.on_realtime_transcript_done("user".to_string(), "old question".to_string());
    while events.try_recv().is_ok() {}
    chat.note_realtime_typed_input("newer typed question");
    let typed_generation = chat.realtime_conversation.input_generation;

    chat.on_realtime_transcript_done("user".to_string(), "old question".to_string());

    assert_eq!(
        chat.realtime_conversation.input_generation,
        typed_generation
    );
    assert!(!chat.realtime_conversation.latest_input_was_voice);
    assert!(events.try_recv().is_err());
    complete_stale_handoff(&mut chat, thread_id, "old question");

    assert!(ops.try_recv().is_err());
}

#[tokio::test]
async fn stale_voice_handoff_longer_than_the_display_limit_cannot_be_revived() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let old_question = format!("{} final words", "é".repeat(600));
    chat.on_realtime_transcript_done("user".to_string(), old_question.clone());
    while events.try_recv().is_ok() {}
    chat.note_realtime_typed_input("a newer typed question");
    let typed_generation = chat.realtime_conversation.input_generation;

    chat.on_realtime_transcript_done("user".to_string(), old_question.clone());

    assert_eq!(
        chat.realtime_conversation.input_generation,
        typed_generation
    );
    assert!(!chat.realtime_conversation.latest_input_was_voice);
    assert!(events.try_recv().is_err());
    complete_stale_handoff(&mut chat, thread_id, &old_question);

    assert!(ops.try_recv().is_err());
}

#[tokio::test]
async fn new_voice_handoff_without_transcript_still_speaks_after_a_typed_turn() {
    let (mut chat, _sender, _events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    chat.note_realtime_typed_input("a previous typed question");
    let turn_id = "new-voice-turn";
    start_item(
        &mut chat,
        thread_id,
        turn_id,
        user_item("<realtime_delegation><input>fresh spoken request</input></realtime_delegation>"),
    );
    let answer = agent_item(
        "answer",
        "The voice answer",
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

    assert!(matches!(
        ops.try_recv(),
        Ok(AppCommand::RealtimeConversationSpeech { text, .. }) if text.as_str() == "The voice answer"
    ));
    assert!(ops.try_recv().is_err());
}
