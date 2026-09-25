//! Undelivered speech returns to visible history without leaks or clipping.

use super::*;
use pretty_assertions::assert_eq;

fn history_text(events: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>) -> String {
    std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) if !cell.as_any().is::<FinalMessageSeparator>() => {
                Some(cell.display_lines(/*width*/ 80))
            }
            _ => None,
        })
        .flatten()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn paraphrased_voice_answers_do_not_reappear_on_overflow_or_stop() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let mut last_exchange = String::new();
    for index in 0..=super::super::MAX_PENDING_SPEECH_DELIVERIES {
        let turn_id = format!("turn-{index}");
        let question = format!("Question {index}");
        chat.on_realtime_transcript_delta("user".into(), question.clone());
        chat.on_realtime_transcript_done("user".into(), question.clone());
        start_item(
            &mut chat,
            thread_id,
            &turn_id,
            user_item(&format!(
                "<realtime_delegation><input>{question}</input></realtime_delegation>"
            )),
        );
        let answer = agent_item(
            &format!("answer-{index}"),
            &format!("Backend answer {index}"),
            Some(MessagePhase::FinalAnswer),
        );
        start_item(&mut chat, thread_id, &turn_id, answer.clone());
        complete_item(&mut chat, thread_id, &turn_id, answer.clone());
        finish_turn(
            &mut chat,
            thread_id,
            &turn_id,
            vec![answer],
            TurnStatus::Completed,
        );
        let AppCommand::RealtimeConversationSpeech { delivery_id, .. } = ops.try_recv().unwrap()
        else {
            panic!("completed voice turn should queue speech");
        };
        chat.accept_realtime_speech(delivery_id);
        let spoken = format!("Spoken paraphrase {index}");
        chat.on_realtime_transcript_delta("assistant".into(), spoken.clone());
        chat.on_realtime_transcript_done("assistant".into(), spoken);
        commit_realtime_history_events(&mut chat, &mut events);
        last_exchange = history_text(&mut events);
        assert!(
            !last_exchange.contains("Backend answer"),
            "old backend answer appeared during turn {index}"
        );
    }
    chat.on_realtime_transcript_done("user".into(), "Next question".into());
    chat.stop_realtime_conversation();
    commit_realtime_history_events(&mut chat, &mut events);
    let rendered = format!("{last_exchange}\n{}", history_text(&mut events));
    insta::assert_snapshot!("paraphrased_voice_recovery", rendered);
}

#[tokio::test]
async fn failed_speech_recovers_only_for_the_current_input() {
    for superseded in [false, true] {
        let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
        let thread_id = activate_voice(&mut chat);
        let turn_id = "voice-turn";
        start_item(
            &mut chat,
            thread_id,
            turn_id,
            user_item("<realtime_delegation><input>question</input></realtime_delegation>"),
        );
        let answer = agent_item(
            "answer",
            "Undelivered answer",
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
        let AppCommand::RealtimeConversationSpeech { delivery_id, .. } = ops.try_recv().unwrap()
        else {
            panic!("completed voice turn should queue speech");
        };
        if superseded {
            chat.note_realtime_typed_input("New task");
        }
        while events.try_recv().is_ok() {}
        chat.restore_undelivered_realtime_speech(delivery_id);
        chat.restore_undelivered_realtime_speech(delivery_id);
        assert_eq!(
            history_text(&mut events),
            if superseded {
                ""
            } else {
                "• Undelivered answer"
            }
        );
    }
}

#[tokio::test]
async fn delayed_voice_transcript_preserves_unspoken_text_fallback() {
    for completed_before_transcript in [false, true] {
        let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
        let thread_id = activate_voice(&mut chat);
        let turn_id = "voice-turn";
        start_item(
            &mut chat,
            thread_id,
            turn_id,
            user_item("<realtime_delegation><input>question</input></realtime_delegation>"),
        );
        let answer = agent_item("answer", "Current answer", Some(MessagePhase::FinalAnswer));
        start_item(&mut chat, thread_id, turn_id, answer.clone());
        if completed_before_transcript {
            complete_item(&mut chat, thread_id, turn_id, answer.clone());
        }
        chat.on_realtime_transcript_done("user".into(), "question".into());
        commit_realtime_history_events(&mut chat, &mut events);
        while events.try_recv().is_ok() {}
        if !completed_before_transcript {
            complete_item(&mut chat, thread_id, turn_id, answer.clone());
        }
        finish_turn(
            &mut chat,
            thread_id,
            turn_id,
            vec![answer],
            TurnStatus::Completed,
        );
        insta::allow_duplicates! {
            insta::assert_snapshot!(history_text(&mut events), @"• Current answer");
        }
        assert!(ops.try_recv().is_err());
    }
}

#[tokio::test]
async fn queued_voice_answers_return_to_history_once_if_voice_closes_before_delivery() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let mut deliveries = Vec::new();
    for (turn_id, item_id, text) in [
        ("first-turn", "first-answer", "First answer"),
        ("second-turn", "second-answer", "Second answer"),
    ] {
        start_item(
            &mut chat,
            thread_id,
            turn_id,
            user_item("<realtime_delegation><input>question</input></realtime_delegation>"),
        );
        let answer = agent_item(item_id, text, Some(MessagePhase::FinalAnswer));
        start_item(&mut chat, thread_id, turn_id, answer.clone());
        complete_item(&mut chat, thread_id, turn_id, answer.clone());
        finish_turn(
            &mut chat,
            thread_id,
            turn_id,
            vec![answer],
            TurnStatus::Completed,
        );
        let AppCommand::RealtimeConversationSpeech { delivery_id, .. } = ops.try_recv().unwrap()
        else {
            panic!("completed voice turn should queue speech");
        };
        deliveries.push(delivery_id);
    }
    assert_eq!(chat.realtime_conversation.pending_speech.len(), 2);
    while events.try_recv().is_ok() {}

    chat.stop_realtime_conversation();
    chat.reset_realtime_conversation();
    for delivery_id in deliveries {
        chat.restore_undelivered_realtime_speech(delivery_id);
    }
    let mut rendered = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            rendered.push(
                cell.display_lines(/*width*/ 80)
                    .into_iter()
                    .map(|line| line.to_string())
                    .collect::<String>(),
            );
        }
    }
    assert_eq!(
        rendered
            .iter()
            .filter(|line| line.contains("First answer"))
            .count(),
        1
    );
    assert_eq!(
        rendered
            .iter()
            .filter(|line| line.contains("Second answer"))
            .count(),
        1
    );
}

#[tokio::test]
async fn pending_voice_answers_stay_bounded_without_restoring_old_inputs() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let mut oldest_delivery_id = None;
    for index in 0..=2 * super::super::MAX_PENDING_SPEECH_DELIVERIES {
        if index == super::super::MAX_PENDING_SPEECH_DELIVERIES + 1 {
            chat.park_voice();
        }
        let turn_id = format!("turn-{index}");
        chat.on_realtime_transcript_done("user".into(), format!("Question {index}"));
        start_item(
            &mut chat,
            thread_id,
            &turn_id,
            user_item("<realtime_delegation><input>question</input></realtime_delegation>"),
        );
        let answer = agent_item(
            &format!("answer-{index}"),
            &format!("Answer number {index}"),
            Some(MessagePhase::FinalAnswer),
        );
        start_item(&mut chat, thread_id, &turn_id, answer.clone());
        complete_item(&mut chat, thread_id, &turn_id, answer.clone());
        finish_turn(
            &mut chat,
            thread_id,
            &turn_id,
            vec![answer],
            TurnStatus::Completed,
        );
        let AppCommand::RealtimeConversationSpeech { delivery_id, .. } = ops.try_recv().unwrap()
        else {
            panic!("completed voice turn should queue speech");
        };
        if index == 0 {
            oldest_delivery_id = Some(delivery_id);
        }
    }
    assert_eq!(
        chat.realtime_conversation.pending_speech.len(),
        super::super::MAX_PENDING_SPEECH_DELIVERIES
    );
    assert!(!chat.has_pending_realtime_speech(oldest_delivery_id.unwrap()));
    let mut restored = 0;
    while let Ok(event) = events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event
            && cell
                .display_lines(/*width*/ 80)
                .iter()
                .any(|line| line.to_string().contains("Answer number 0"))
        {
            restored += 1;
        }
    }
    assert_eq!(restored, 0);
    assert!(ops.try_recv().is_err());
    chat.stop_realtime_conversation();
    chat.reset_realtime_conversation();
    let last = 2 * super::super::MAX_PENDING_SPEECH_DELIVERIES;
    assert_eq!(
        chat.take_undelivered_realtime_speech_for_replay(),
        vec![(
            thread_id,
            format!("turn-{last}"),
            agent_item(
                &format!("answer-{last}"),
                &format!("Answer number {last}"),
                Some(MessagePhase::FinalAnswer),
            ),
        )]
    );
}

#[tokio::test]
async fn delegated_analysis_never_enters_history_on_completion_overflow_or_stop() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let turn_id = "private-turn";
    start_item(
        &mut chat,
        thread_id,
        turn_id,
        user_item("<realtime_delegation><input>question</input></realtime_delegation>"),
    );
    let private_items = [
        agent_item(
            "analysis",
            "[ANALYSIS] private thought",
            /*phase*/ None,
        ),
        agent_item(
            "commentary",
            "private commentary",
            Some(MessagePhase::Commentary),
        ),
        agent_item(
            "oversized-analysis",
            &format!(
                "[ANALYSIS] {}",
                "secret".repeat(super::super::MAX_PENDING_SPEECH_ITEM_BYTES)
            ),
            /*phase*/ None,
        ),
    ];
    for item in &private_items {
        start_item(&mut chat, thread_id, turn_id, item.clone());
        complete_item(&mut chat, thread_id, turn_id, item.clone());
    }
    assert!(chat.realtime_conversation.pending_speech.is_empty());
    finish_turn(
        &mut chat,
        thread_id,
        turn_id,
        private_items.to_vec(),
        TurnStatus::Completed,
    );
    for index in 0..=super::super::MAX_PENDING_SPEECH_DELIVERIES {
        let turn_id = format!("answer-turn-{index}");
        start_item(
            &mut chat,
            thread_id,
            &turn_id,
            user_item("<realtime_delegation><input>question</input></realtime_delegation>"),
        );
        let answer = agent_item(
            &format!("answer-{index}"),
            &format!("Public answer {index}"),
            Some(MessagePhase::FinalAnswer),
        );
        start_item(&mut chat, thread_id, &turn_id, answer.clone());
        complete_item(&mut chat, thread_id, &turn_id, answer);
    }
    chat.stop_realtime_conversation();
    chat.reset_realtime_conversation();
    while let Ok(event) = events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            let lines = cell.display_lines(/*width*/ 80);
            assert!(lines.iter().all(|line| {
                !line.to_string().contains("private") && !line.to_string().contains("secret")
            }));
        }
    }
    assert!(matches!(
        ops.try_recv(),
        Ok(AppCommand::RealtimeConversationStop { .. })
    ));
    assert!(ops.try_recv().is_err(), "no private speech may be queued");
}

#[tokio::test]
async fn delegated_reasoning_never_enters_live_history() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let turn_id = "private-reasoning-turn";
    start_item(
        &mut chat,
        thread_id,
        turn_id,
        user_item("<realtime_delegation><input>question</input></realtime_delegation>"),
    );
    chat.config.show_raw_agent_reasoning = true;
    for notification in [
        ServerNotification::ReasoningSummaryPartAdded(ReasoningSummaryPartAddedNotification {
            thread_id: thread_id.to_string(),
            turn_id: turn_id.into(),
            item_id: "reasoning".into(),
            summary_index: 0,
        }),
        ServerNotification::ReasoningSummaryTextDelta(ReasoningSummaryTextDeltaNotification {
            thread_id: thread_id.to_string(),
            turn_id: turn_id.into(),
            item_id: "reasoning".into(),
            delta: "private summary".into(),
            summary_index: 0,
        }),
        ServerNotification::ReasoningTextDelta(ReasoningTextDeltaNotification {
            thread_id: thread_id.to_string(),
            turn_id: turn_id.into(),
            item_id: "reasoning".into(),
            delta: "private raw reasoning".into(),
            content_index: 0,
        }),
    ] {
        chat.handle_server_notification(notification, /*replay_kind*/ None);
    }
    complete_item(
        &mut chat,
        thread_id,
        turn_id,
        ThreadItem::Reasoning {
            id: "reasoning".into(),
            summary: vec!["private summary".into()],
            content: vec!["private raw reasoning".into()],
        },
    );
    chat.reset_realtime_conversation();
    start_item(
        &mut chat,
        thread_id,
        "late-start-turn",
        user_item("<realtime_delegation><input>delayed input</input></realtime_delegation>"),
    );
    assert!(chat.is_realtime_delegated_reasoning_turn("late-start-turn"));
    chat.handle_server_notification(
        ServerNotification::ReasoningSummaryTextDelta(ReasoningSummaryTextDeltaNotification {
            thread_id: thread_id.to_string(),
            turn_id: turn_id.into(),
            item_id: "late-reasoning".into(),
            delta: "private late summary".into(),
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
            summary: vec!["private late summary".into()],
            content: Vec::new(),
        },
    );
    chat.handle_server_notification(
        ServerNotification::ReasoningSummaryTextDelta(ReasoningSummaryTextDeltaNotification {
            thread_id: thread_id.to_string(),
            turn_id: "late-start-turn".into(),
            item_id: "late-start-reasoning".into(),
            delta: "private delayed start".into(),
            summary_index: 0,
        }),
        /*replay_kind*/ None,
    );
    complete_item(
        &mut chat,
        thread_id,
        "late-start-turn",
        ThreadItem::Reasoning {
            id: "late-start-reasoning".into(),
            summary: vec!["private delayed start".into()],
            content: Vec::new(),
        },
    );
    chat.handle_server_notification(
        ServerNotification::AgentMessageDelta(AgentMessageDeltaNotification {
            thread_id: thread_id.to_string(),
            turn_id: turn_id.into(),
            item_id: "late-commentary".into(),
            delta: "private commentary stream".into(),
        }),
        /*replay_kind*/ None,
    );
    complete_item(
        &mut chat,
        thread_id,
        turn_id,
        agent_item(
            "late-commentary",
            "private commentary item",
            Some(MessagePhase::Commentary),
        ),
    );
    finish_turn(
        &mut chat,
        thread_id,
        turn_id,
        vec![agent_item(
            "private-none",
            "[ANALYSIS] private fallback",
            /*phase*/ None,
        )],
        TurnStatus::Completed,
    );
    chat.flush_answer_stream_with_separator();
    while let Ok(event) = events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            let rendered = cell
                .transcript_lines(/*width*/ 80)
                .into_iter()
                .map(|line| line.to_string())
                .collect::<Vec<_>>()
                .join("\n");
            assert!(!rendered.contains("private"), "{rendered}");
        }
    }
    assert!(!chat.is_realtime_delegated_reasoning_turn(turn_id));
    let typed_reasoning = ThreadItem::Reasoning {
        id: "typed-reasoning".into(),
        summary: vec!["ordinary summary".into()],
        content: Vec::new(),
    };
    start_item(&mut chat, thread_id, "typed-turn", typed_reasoning.clone());
    chat.handle_server_notification(
        ServerNotification::ReasoningSummaryTextDelta(ReasoningSummaryTextDeltaNotification {
            thread_id: thread_id.to_string(),
            turn_id: "typed-turn".into(),
            item_id: "typed-reasoning".into(),
            delta: "ordinary summary".into(),
            summary_index: 0,
        }),
        /*replay_kind*/ None,
    );
    complete_item(&mut chat, thread_id, "typed-turn", typed_reasoning);
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
        .collect::<String>();
    assert!(rendered.contains("ordinary summary"), "{rendered}");
}

#[tokio::test]
async fn answer_exceeding_speech_budget_is_shown_in_full_instead() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let turn_id = "text-only-final-turn";
    start_item(
        &mut chat,
        thread_id,
        turn_id,
        user_item("<realtime_delegation><input>question</input></realtime_delegation>"),
    );
    while events.try_recv().is_ok() {}
    let text = format!("Start. {} End.", "word ".repeat(/*n*/ 800));
    let answer = agent_item("text-only-answer", &text, Some(MessagePhase::FinalAnswer));
    start_item(&mut chat, thread_id, turn_id, answer.clone());
    complete_item(&mut chat, thread_id, turn_id, answer.clone());
    finish_turn(
        &mut chat,
        thread_id,
        turn_id,
        vec![answer],
        TurnStatus::Completed,
    );

    assert!(
        ops.try_recv().is_err(),
        "an over-budget answer must not be clipped for speech"
    );
    assert!(chat.realtime_conversation.pending_speech.is_empty());
    let rendered = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) if !cell.as_any().is::<FinalMessageSeparator>() => {
                Some(
                    cell.transcript_lines(/*width*/ 80)
                        .into_iter()
                        .map(|line| line.to_string())
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        rendered.split_whitespace().collect::<Vec<_>>().join(" "),
        format!("• {text}")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    );
}

#[tokio::test]
async fn oversized_delegated_answer_remains_in_normal_history() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let turn_id = "large-final-turn";
    start_item(
        &mut chat,
        thread_id,
        turn_id,
        user_item("<realtime_delegation><input>question</input></realtime_delegation>"),
    );
    let answer = agent_item(
        "large-answer",
        &"word ".repeat(super::super::MAX_PENDING_SPEECH_ITEM_BYTES / 5),
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

    assert!(chat.realtime_conversation.pending_speech.is_empty());
    assert!(ops.try_recv().is_err());
    assert!(events.try_recv().is_ok());
    assert_eq!(
        chat.transcript.last_completed_agent_message,
        Some((turn_id.to_string(), "large-answer".to_string()))
    );
}

#[tokio::test]
async fn final_item_that_grows_after_completion_is_not_queued_without_recovery() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let turn_id = "expanded-final-turn";
    start_item(
        &mut chat,
        thread_id,
        turn_id,
        user_item("<realtime_delegation><input>question</input></realtime_delegation>"),
    );
    let short = agent_item("expanded-answer", "short", Some(MessagePhase::FinalAnswer));
    start_item(&mut chat, thread_id, turn_id, short.clone());
    complete_item(&mut chat, thread_id, turn_id, short);
    let expanded_text = "word ".repeat(super::super::MAX_PENDING_SPEECH_ITEM_BYTES / 5);
    let expanded = agent_item(
        "expanded-answer",
        &expanded_text,
        Some(MessagePhase::FinalAnswer),
    );
    finish_turn(
        &mut chat,
        thread_id,
        turn_id,
        vec![expanded],
        TurnStatus::Completed,
    );

    assert!(chat.realtime_conversation.pending_speech.is_empty());
    assert!(ops.try_recv().is_err());
    let recovered = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(cell.raw_lines()),
            _ => None,
        })
        .flatten()
        .map(|line| line.to_string())
        .filter(|text| text == "short" || text.starts_with("word "))
        .collect::<Vec<_>>();
    assert_eq!(recovered, vec![expanded_text.trim_end().to_string()]);
}

#[tokio::test]
async fn interrupted_voice_delegation_never_speaks_a_completed_item() {
    let (mut chat, _sender, _events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let turn_id = "interrupted-turn";
    start_item(
        &mut chat,
        thread_id,
        turn_id,
        user_item("<realtime_delegation><input>stop me</input></realtime_delegation>"),
    );
    let answer = agent_item("answer", "Never say this", Some(MessagePhase::FinalAnswer));
    start_item(&mut chat, thread_id, turn_id, answer.clone());
    complete_item(&mut chat, thread_id, turn_id, answer.clone());
    finish_turn(
        &mut chat,
        thread_id,
        turn_id,
        vec![answer],
        TurnStatus::Interrupted,
    );

    assert!(ops.try_recv().is_err());
}

#[tokio::test]
async fn replay_projects_the_delegated_request_without_internal_xml() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    chat.replay_thread_item(
        user_item(
            "<realtime_delegation><input>show &lt;code&gt; &amp; tests</input></realtime_delegation>",
        ),
        "old-turn".to_string(),
        ReplayKind::ResumeInitialMessages,
    );

    let Ok(AppEvent::InsertHistoryCell(cell)) = events.try_recv() else {
        panic!("a resumed voice delegation should preserve the original user request");
    };
    let rendered = cell
        .display_lines(/*width*/ 80)
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered.contains("show <code> & tests"));
    assert!(!rendered.contains("realtime_delegation"));
    assert!(ops.try_recv().is_err());
}

#[tokio::test]
async fn locally_typed_delegation_shaped_text_keeps_its_normal_answer() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let typed_text = "<realtime_delegation><input>explain this xml</input></realtime_delegation>";
    chat.note_realtime_typed_input(typed_text);
    let turn_id = "typed-xml-turn";
    let prompt = user_item(typed_text);
    start_item(&mut chat, thread_id, turn_id, prompt.clone());
    complete_item(&mut chat, thread_id, turn_id, prompt);
    let answer = agent_item("xml-answer", "It is an XML element.", /*phase*/ None);
    start_item(&mut chat, thread_id, turn_id, answer.clone());
    complete_item(&mut chat, thread_id, turn_id, answer.clone());
    finish_turn(
        &mut chat,
        thread_id,
        turn_id,
        vec![answer],
        TurnStatus::Completed,
    );

    let mut saw_answer = false;
    while let Ok(event) = events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            saw_answer |= cell
                .display_lines(/*width*/ 80)
                .iter()
                .any(|line| line.to_string().contains("It is an XML element."));
        }
    }
    assert!(saw_answer);
    assert!(ops.try_recv().is_err());
}

#[tokio::test]
async fn new_user_input_invalidates_already_queued_voice_speech() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    chat.realtime_conversation.attempt_id = 4;
    let generation = chat.realtime_conversation.input_generation;

    assert!(chat.is_current_realtime_attempt(thread_id, /*attempt_id*/ 4, generation));
    chat.note_realtime_typed_input("newer typed message");
    assert!(!chat.is_current_realtime_attempt(thread_id, /*attempt_id*/ 4, generation));

    chat.on_realtime_transcript_done("user".to_string(), "newer spoken message".to_string());
    assert!(!chat.is_current_realtime_attempt(thread_id, /*attempt_id*/ 4, generation));
}
