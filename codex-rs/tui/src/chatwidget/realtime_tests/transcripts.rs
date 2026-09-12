//! Partial and completed captions stay ordered and bounded through voice shutdown.

use super::*;
use pretty_assertions::assert_eq;

fn seed_interleaved_partials(chat: &mut ChatWidget) {
    chat.on_realtime_transcript_delta("assistant".into(), "First ".into());
    chat.on_realtime_transcript_delta("user".into(), "Correction".into());
    chat.on_realtime_transcript_delta("assistant".into(), "answer".into());
}

#[tokio::test]
async fn interleaved_transcripts_keep_interrupted_speech_suppressed() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    activate_voice(&mut chat);
    chat.on_realtime_transcript_delta("assistant".into(), "The long answer".into());
    chat.on_realtime_transcript_delta("user".into(), "Actually, ".into());
    let generation = chat.realtime_conversation.input_generation;

    chat.on_realtime_transcript_delta("assistant".into(), " keeps going".into());
    assert_eq!(
        chat.realtime_conversation.speaker_suppression_generation,
        Some(generation)
    );
    chat.on_realtime_transcript_done("user".into(), "Actually, make it short".into());
    chat.on_realtime_transcript_delta("assistant".into(), " and going".into());
    chat.on_realtime_transcript_done("assistant".into(), "The long answer keeps going".into());
    assert_eq!(
        chat.realtime_conversation.speaker_suppression_generation,
        Some(generation)
    );

    chat.on_realtime_transcript_delta("assistant".into(), "Short answer".into());
    assert!(
        chat.realtime_conversation
            .speaker_suppression_generation
            .is_none()
    );
}

#[tokio::test]
async fn delayed_user_transcript_suppresses_old_audio_after_microphone_mute() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    activate_voice(&mut chat);
    chat.on_realtime_transcript_delta("assistant".into(), "Old answer".into());
    chat.realtime_conversation.microphone_muted = true;

    chat.on_realtime_transcript_delta("user".into(), "Actually".into());

    assert_eq!(
        (
            chat.realtime_conversation.microphone_muted,
            chat.realtime_conversation.speaker_suppression_generation,
        ),
        (true, Some(chat.realtime_conversation.input_generation))
    );
}

#[tokio::test]
async fn interleaved_transcripts_do_not_discard_the_interrupting_request() {
    let (mut chat, _sender, _events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    chat.on_realtime_transcript_delta("assistant".into(), "The long answer".into());
    chat.on_realtime_transcript_delta("user".into(), "Actually, ".into());
    let generation = chat.realtime_conversation.input_generation;
    let turn_id = "interruption";
    start_item(
        &mut chat,
        thread_id,
        turn_id,
        user_item("<realtime_delegation><input>make it short</input></realtime_delegation>"),
    );
    chat.on_realtime_transcript_delta("assistant".into(), " keeps going".into());
    chat.on_realtime_transcript_delta("user".into(), "make it short".into());
    chat.on_realtime_transcript_done("assistant".into(), "The long answer keeps going".into());
    chat.on_realtime_transcript_done("user".into(), "Actually, make it short".into());

    let answer = agent_item(
        "short-answer",
        "Short answer",
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
        Ok(AppCommand::RealtimeConversationSpeech { input_generation, text, .. })
            if input_generation == generation && text.as_str() == "Short answer"
    ));
    assert!(ops.try_recv().is_err());
}

#[tokio::test]
async fn completed_user_transcript_keeps_the_old_assistant_turn_suppressed() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    activate_voice(&mut chat);
    chat.on_realtime_transcript_delta("assistant".into(), "Old answer".into());
    chat.on_realtime_transcript_done("user".into(), "Stop, I have a correction".into());
    let generation = chat.realtime_conversation.input_generation;
    chat.on_realtime_transcript_done("assistant".into(), "Old answer".into());
    assert_eq!(
        chat.realtime_conversation.speaker_suppression_generation,
        Some(generation)
    );
    chat.on_realtime_transcript_delta("assistant".into(), "Go".into());
    chat.on_realtime_transcript_done("assistant".into(), "Go ahead".into());
    assert!(
        chat.realtime_conversation
            .speaker_suppression_generation
            .is_none()
    );
}

#[tokio::test]
async fn final_only_assistant_transcript_cannot_release_an_interruption() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    activate_voice(&mut chat);
    chat.realtime_conversation.speaker_level = 1;
    chat.on_realtime_transcript_delta("user".into(), "Stop".into());
    let generation = chat.realtime_conversation.input_generation;
    chat.on_realtime_transcript_done("user".into(), "Stop".into());

    chat.on_realtime_transcript_done("assistant".into(), "Late old answer".into());

    assert_eq!(
        chat.realtime_conversation.speaker_suppression_generation,
        Some(generation)
    );
}

#[tokio::test]
async fn transcript_deltas_track_the_current_speaker() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.realtime_conversation.phase = RealtimeConversationPhase::Active;
    let owner = |chat: &ChatWidget| chat.realtime_conversation.speaker_suppression_generation;

    chat.on_realtime_transcript_delta("user".to_string(), "Hello ".to_string());
    chat.on_realtime_transcript_delta("user".to_string(), "there".to_string());

    assert_eq!(
        (
            chat.realtime_conversation.transcript_role.as_deref(),
            chat.realtime_conversation.transcript.as_str()
        ),
        (Some("user"), "Hello there")
    );
    chat.on_realtime_transcript_done("assistant".to_string(), "Old".to_string());
    assert_eq!(
        chat.realtime_conversation.speaker_suppression_generation,
        None
    );

    chat.on_realtime_transcript_done("user".to_string(), "Hello there".to_string());
    chat.on_realtime_transcript_delta("assistant".to_string(), "Hi".to_string());
    assert!(owner(&chat).is_none());
    chat.on_realtime_transcript_done("assistant".to_string(), "Hi".to_string());

    chat.on_realtime_transcript_delta("user".to_string(), "Again".to_string());
    chat.on_realtime_transcript_done("user".to_string(), "Again".to_string());
    assert!(owner(&chat).is_none());
    chat.on_realtime_transcript_delta("assistant".to_string(), "".to_string());
    assert!(owner(&chat).is_none());
    chat.on_realtime_transcript_delta("assistant".to_string(), "Hi".to_string());
    assert!(owner(&chat).is_none());

    assert_eq!(
        (
            chat.realtime_conversation.transcript_role.as_deref(),
            chat.realtime_conversation.transcript.as_str()
        ),
        (Some("assistant"), "Hi")
    );

    chat.on_realtime_transcript_done("user".to_string(), "Again".to_string());
    let generation = chat.realtime_conversation.input_generation;
    assert_eq!(owner(&chat), Some(generation));
    chat.on_realtime_transcript_done("user".to_string(), " ".to_string());
    assert_eq!(owner(&chat), Some(generation));
    chat.on_realtime_transcript_done("assistant".to_string(), "".to_string());
    assert_eq!(owner(&chat), Some(generation));
    chat.on_realtime_transcript_done("assistant".to_string(), "Hi".to_string());
    assert_eq!(owner(&chat), Some(generation));
    chat.on_realtime_transcript_delta("user".to_string(), "noise".to_string());
    chat.on_realtime_transcript_done("user".to_string(), "".to_string());
    assert!(owner(&chat).is_none());
}

#[tokio::test]
async fn voice_transcripts_stream_in_the_conversation_instead_of_the_footer() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.local_settings.tui.animations = false;
    activate_voice(&mut chat);
    chat.update_realtime_footer();

    chat.on_realtime_transcript_delta("user".to_string(), "pick a ".to_string());
    chat.on_realtime_transcript_delta("user".to_string(), "number".to_string());

    let live = chat
        .active_cell_transcript_lines(/*width*/ 80)
        .unwrap_or_default()
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(live.contains("pick a number"));
    let footer = render_bottom_popup(&chat, /*width*/ 80);
    assert!(footer.contains("voice ● listening"));
    let status_line = footer
        .lines()
        .find(|line| line.contains("voice ● listening"))
        .unwrap_or_default();
    assert!(!status_line.contains("pick a number"));
    assert!(events.try_recv().is_err());

    chat.on_realtime_transcript_done("user".to_string(), "pick a number".to_string());
    commit_realtime_history_events(&mut chat, &mut events);
    assert!(chat.active_cell_transcript_lines(/*width*/ 80).is_none());
    commit_realtime_history_events(&mut chat, &mut events);
    let Ok(AppEvent::InsertHistoryCell(cell)) = events.try_recv() else {
        panic!("the completed voice transcript should become one normal user turn");
    };
    assert!(
        cell.display_lines(/*width*/ 80)
            .iter()
            .any(|line| line.to_string().contains("pick a number"))
    );
    assert!(events.try_recv().is_err());
}

#[tokio::test]
async fn unexpected_voice_close_preserves_partial_transcripts_once() {
    for (role, text) in [
        ("user", "Part of my request"),
        ("assistant", "Part of the answer"),
    ] {
        let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
        activate_voice(&mut chat);
        chat.on_realtime_transcript_delta(role.to_string(), text.to_string());

        chat.on_realtime_conversation_closed(/*reason*/ None);
        chat.on_realtime_transcript_done(role.to_string(), text.to_string());

        let mut appearances = 0;
        commit_realtime_history_events(&mut chat, &mut events);
        while let Ok(event) = events.try_recv() {
            if let AppEvent::InsertHistoryCell(cell) = event {
                appearances +=
                    cell.display_lines(/*width*/ 80)
                        .iter()
                        .any(|line| line.to_string().contains(text)) as usize;
            }
        }
        assert_eq!(appearances, 1, "{role} transcript should be preserved once");
        assert!(chat.active_cell_transcript_lines(/*width*/ 80).is_none());
        assert_eq!(
            chat.realtime_conversation.phase,
            RealtimeConversationPhase::Inactive
        );
    }
}

#[tokio::test]
async fn interleaved_partial_transcripts_survive_voice_close() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    activate_voice(&mut chat);
    seed_interleaved_partials(&mut chat);

    chat.on_realtime_conversation_closed(/*reason*/ None);
    chat.on_realtime_transcript_done("user".into(), "Correction".into());
    chat.on_realtime_transcript_done("assistant".into(), "First answer".into());

    commit_realtime_history_events(&mut chat, &mut events);
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
    assert_eq!(rendered.matches("Correction").count(), 1);
    assert_eq!(rendered.matches("First answer").count(), 1);
    insta::assert_snapshot!("interleaved_partials_on_close", rendered);
}

#[tokio::test]
async fn interleaved_partials_transfer_once_on_thread_switch() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    activate_voice(&mut chat);
    seed_interleaved_partials(&mut chat);

    let cells = chat.take_realtime_transcript_cells_for_replay();
    let rendered = cells
        .iter()
        .map(|record| record.text.as_str())
        .collect::<String>();
    assert_eq!(cells.len(), 2);
    assert_eq!(rendered.matches("Correction").count(), 1);
    assert_eq!(rendered.matches("First answer").count(), 1);
    assert!(chat.take_realtime_transcript_cells_for_replay().is_empty());
}

#[tokio::test]
async fn intentional_stop_preserves_both_interleaved_partials_once() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    activate_voice(&mut chat);
    seed_interleaved_partials(&mut chat);

    chat.stop_realtime_conversation();
    chat.on_realtime_conversation_closed(/*reason*/ None);

    commit_realtime_history_events(&mut chat, &mut events);
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
    assert_eq!(rendered.matches("Correction").count(), 1);
    assert_eq!(rendered.matches("First answer").count(), 1);
    insta::assert_snapshot!("interleaved_partials_on_stop", rendered);
}

#[tokio::test]
async fn stopping_voice_keeps_the_complete_late_final_caption() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    activate_voice(&mut chat);
    chat.on_realtime_transcript_delta("assistant".into(), "First ".into());
    chat.finish_realtime_partial_transcripts();
    chat.realtime_conversation.phase = RealtimeConversationPhase::Stopping;
    chat.on_realtime_transcript_delta("assistant".into(), "and last".into());
    chat.on_realtime_transcript_done("assistant".into(), "First and last".into());
    chat.on_realtime_conversation_closed(Some("requested".into()));

    commit_realtime_history_events(&mut chat, &mut events);
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
    assert_eq!(rendered.matches("First").count(), 2);
    assert_eq!(rendered.matches("and last").count(), 1);
    assert!(ops.try_recv().is_err());
    insta::assert_snapshot!("voice_stop_late_transcript", rendered);
}

#[tokio::test]
async fn separate_late_finals_with_a_shared_prefix_keep_both_full_captions() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    activate_voice(&mut chat);
    chat.realtime_conversation.phase = RealtimeConversationPhase::Stopping;
    chat.on_realtime_transcript_done("assistant".into(), "Hello".into());
    chat.on_realtime_transcript_done("assistant".into(), "Hello again".into());

    commit_realtime_history_events(&mut chat, &mut events);
    let captions = std::iter::from_fn(|| events.try_recv().ok())
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
        .collect::<Vec<_>>();
    assert_eq!(captions, vec!["• Hello", "• Hello again"]);
}

#[tokio::test]
async fn direct_reset_preserves_both_partial_speakers_for_replay() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    activate_voice(&mut chat);
    chat.on_realtime_transcript_delta("assistant".into(), "First ".into());
    chat.on_realtime_transcript_delta("user".into(), "Correction".into());
    chat.on_realtime_transcript_delta("assistant".into(), "answer".into());

    chat.reset_realtime_conversation();
    commit_realtime_history_events(&mut chat, &mut events);

    let records = chat.take_realtime_transcript_cells_for_replay();
    assert_eq!(
        records
            .into_iter()
            .map(|record| (record.role, record.text))
            .collect::<Vec<_>>(),
        vec![
            ("user".to_string(), "Correction".to_string()),
            ("assistant".to_string(), "First answer".to_string())
        ]
    );
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
    assert_eq!(rendered.matches("Correction").count(), 1);
    assert_eq!(rendered.matches("First answer").count(), 1);
}

#[tokio::test]
async fn stopping_voice_preserves_the_live_transcript_once() {
    for command in ["/voice", "/voice stop"] {
        let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
        chat.local_settings.tui.animations = true;
        activate_voice(&mut chat);
        chat.on_realtime_transcript_done("user".to_string(), "Earlier question".to_string());
        chat.on_realtime_transcript_done("assistant".to_string(), "Earlier answer".to_string());
        chat.on_realtime_transcript_delta("assistant".to_string(), "Answer in ".to_string());
        chat.on_realtime_transcript_delta("assistant".to_string(), "progress".to_string());

        if command == "/voice" {
            chat.handle_slash_command_dispatch(crate::slash_command::SlashCommand::Voice);
        } else {
            chat.handle_slash_command_with_args_dispatch(
                crate::slash_command::SlashCommand::Voice,
                "stop".to_string(),
                Vec::new(),
            );
        }
        chat.on_realtime_transcript_done("assistant".to_string(), "Answer in progress".to_string());
        chat.on_realtime_conversation_closed(/*reason*/ None);
        chat.stop_realtime_conversation();

        assert_eq!(
            chat.realtime_conversation.phase,
            RealtimeConversationPhase::Inactive
        );
        commit_realtime_history_events(&mut chat, &mut events);
        assert!(chat.active_cell_transcript_key().is_none());
        let mut rendered = Vec::new();
        commit_realtime_history_events(&mut chat, &mut events);
        while let Ok(event) = events.try_recv() {
            if let AppEvent::InsertHistoryCell(cell) = event {
                assert!(cell.transcript_animation_tick().is_none());
                rendered.extend(
                    cell.display_lines(/*width*/ 80)
                        .into_iter()
                        .map(|line| line.to_string()),
                );
            }
        }
        insta::allow_duplicates! {
            insta::assert_snapshot!(rendered.join("\n"), @r"

            › Earlier question

            • Earlier answer
            • Answer in progress
            ");
        }
    }
}

#[tokio::test]
async fn transcript_completion_waits_for_normal_agent_stream_consolidation() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    activate_voice(&mut chat);
    chat.on_agent_message_delta("normal typed output".to_string());

    chat.on_realtime_transcript_done("assistant".to_string(), "voice output".to_string());
    chat.on_realtime_transcript_delta(
        "assistant".to_string(),
        "unfinished voice output".to_string(),
    );
    chat.stop_realtime_conversation();

    assert_eq!(chat.realtime_conversation.pending_history_cells.len(), 2);
    commit_realtime_history_events(&mut chat, &mut events);
    while let Ok(event) = events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            assert!(
                cell.display_lines(/*width*/ 80)
                    .iter()
                    .all(|line| !line.to_string().contains("voice output"))
            );
        }
    }

    chat.finalize_completed_assistant_message(Some("normal typed output"));
    chat.note_stream_consolidation_completed();
    chat.flush_realtime_transcript_history();

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
    assert_eq!(
        rendered,
        [
            "• normal typed output",
            "• voice output",
            "• unfinished voice output"
        ]
    );
}

#[tokio::test]
async fn transcript_handoff_moves_deferred_repeats_and_partial_once() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.local_settings.tui.animations = false;
    activate_voice(&mut chat);
    chat.on_agent_message_delta("ordinary stream".to_string());
    for _ in 0..super::super::MAX_PENDING_TRANSCRIPT_CELLS {
        chat.on_realtime_transcript_done("assistant".to_string(), "same words".to_string());
    }
    chat.on_realtime_transcript_delta("assistant".to_string(), "partial words".to_string());
    assert_eq!(
        chat.realtime_conversation.pending_history_cells.len(),
        super::super::MAX_PENDING_TRANSCRIPT_CELLS
    );

    let cells = chat.take_realtime_transcript_cells_for_replay();
    assert_eq!(cells.len(), super::super::MAX_PENDING_TRANSCRIPT_CELLS + 1);
    assert!(chat.take_realtime_transcript_cells_for_replay().is_empty());
    chat.restore_realtime_transcript_cells(cells);
    chat.finalize_completed_assistant_message(Some("ordinary stream"));
    chat.note_stream_consolidation_completed();
    chat.flush_realtime_transcript_history();

    commit_realtime_history_events(&mut chat, &mut events);
    let rendered = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(
                cell.transcript_lines(/*width*/ 80)
                    .into_iter()
                    .map(|line| line.to_string())
                    .collect::<String>(),
            ),
            _ => None,
        })
        .collect::<String>();
    assert_eq!(
        rendered.matches("same words").count(),
        super::super::MAX_PENDING_TRANSCRIPT_CELLS
    );
    assert_eq!(rendered.matches("partial words").count(), 0);
    let live = chat
        .realtime_conversation
        .live_transcript_cell
        .as_ref()
        .unwrap();
    assert!(
        live.display_lines(/*width*/ 80)
            .iter()
            .any(|line| line.to_string().contains("partial words"))
    );
}

#[tokio::test]
async fn dropped_deferred_captions_do_not_return_on_thread_replacement() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    activate_voice(&mut chat);
    chat.on_agent_message_delta("ordinary stream".to_string());
    for index in 0..(super::super::MAX_PENDING_TRANSCRIPT_CELLS + 2) {
        chat.on_realtime_transcript_done("assistant".into(), format!("caption {index}"));
    }

    assert_eq!(
        chat.realtime_conversation.pending_history_cells.len(),
        super::super::MAX_PENDING_TRANSCRIPT_CELLS
    );
    let records = chat.take_realtime_transcript_cells_for_replay();
    assert_eq!(records.len(), super::super::MAX_PENDING_TRANSCRIPT_CELLS);
    assert_eq!(
        records.front().map(|record| record.text.as_str()),
        Some("caption 2")
    );
    assert_eq!(
        records.back().map(|record| record.text.as_str()),
        Some("caption 33")
    );
}

#[tokio::test]
async fn transcripts_are_preserved_while_the_peer_is_connecting() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.realtime_conversation.phase = RealtimeConversationPhase::Starting;
    chat.realtime_conversation.backend_started = true;

    chat.on_realtime_transcript_delta("assistant".to_string(), "Hello".to_string());

    assert_eq!(
        (
            chat.realtime_conversation.transcript_role.as_deref(),
            chat.realtime_conversation.transcript.as_str(),
        ),
        (Some("assistant"), "Hello")
    );

    chat.on_realtime_transcript_done("assistant".to_string(), "Hello there".to_string());

    commit_realtime_history_events(&mut chat, &mut events);
    let Ok(AppEvent::InsertHistoryCell(cell)) = events.try_recv() else {
        panic!("voice should preserve transcripts received before the peer connects");
    };
    assert!(
        cell.display_lines(/*width*/ 80)
            .iter()
            .any(|line| line.to_string().contains("Hello there"))
    );
    assert_eq!(
        (
            chat.realtime_conversation.phase,
            chat.realtime_conversation.transcript_role.as_deref(),
            chat.realtime_conversation.transcript.as_str(),
        ),
        (RealtimeConversationPhase::Starting, None, "")
    );
}

#[tokio::test]
async fn transcript_deltas_keep_a_bounded_utf8_suffix() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.realtime_conversation.phase = RealtimeConversationPhase::Active;

    chat.on_realtime_transcript_delta("user".to_string(), "é".repeat(600));
    chat.on_realtime_transcript_delta("user".to_string(), "done".to_string());

    assert_eq!(
        chat.realtime_conversation.transcript,
        format!("{}done", "é".repeat(510))
    );
}

#[tokio::test]
async fn completed_transcript_is_added_to_history_and_clears_the_caption() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.realtime_conversation.phase = RealtimeConversationPhase::Active;
    chat.on_realtime_transcript_delta("assistant".to_string(), "Hello".to_string());

    chat.on_realtime_transcript_done("assistant".to_string(), "Hello there".to_string());
    chat.on_realtime_transcript_done("user".to_string(), "Can you hear me?".to_string());

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

    assert_eq!(
        (
            chat.realtime_conversation.transcript_role.as_deref(),
            chat.realtime_conversation.transcript.as_str(),
        ),
        (None, "")
    );
    insta::assert_snapshot!(rendered.join("\n"));
}

#[tokio::test]
async fn completed_transcript_preserves_the_other_speakers_caption() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.realtime_conversation.phase = RealtimeConversationPhase::Active;
    chat.on_realtime_transcript_delta("assistant".to_string(), "Still speaking".to_string());

    chat.on_realtime_transcript_done("user".to_string(), "Interruption".to_string());

    assert_eq!(
        (
            chat.realtime_conversation.transcript_role.as_deref(),
            chat.realtime_conversation.transcript.as_str(),
        ),
        (Some("assistant"), "Still speaking")
    );
}

#[tokio::test]
async fn live_voice_split_flap_animates_without_changing_final_history() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.local_settings.tui.animations = true;
    activate_voice(&mut chat);

    chat.on_realtime_transcript_delta("assistant".to_string(), "gate 73".to_string());

    let Some(cell) = chat.realtime_conversation.live_transcript_cell.as_ref() else {
        panic!("live voice transcripts should have an animation cell");
    };
    assert_eq!(cell.raw_lines()[0].to_string(), "gate 73");
    assert!(
        chat.active_cell_transcript_key()
            .is_some_and(|key| { key.animation_tick.is_some() })
    );

    chat.on_realtime_transcript_done("assistant".to_string(), "gate 73".to_string());

    commit_realtime_history_events(&mut chat, &mut events);
    assert!(chat.active_cell_transcript_key().is_none());
    commit_realtime_history_events(&mut chat, &mut events);
    let Ok(AppEvent::InsertHistoryCell(cell)) = events.try_recv() else {
        panic!("a completed voice transcript should commit ordinary history");
    };
    assert!(cell.transcript_animation_tick().is_none());
    assert!(
        cell.display_lines(/*width*/ 80)
            .iter()
            .any(|line| { line.to_string().contains("gate 73") })
    );
}

#[tokio::test]
async fn spoken_user_transcript_preserves_red_chevron_and_canonical_history() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    activate_voice(&mut chat);
    chat.on_realtime_transcript_delta("user".to_string(), " hello".to_string());
    let marker = chat
        .realtime_conversation
        .live_transcript_cell
        .as_ref()
        .unwrap()
        .display_lines(/*width*/ 32)
        .into_iter()
        .flat_map(|line| line.spans)
        .find(|span| span.content == "›")
        .expect("genuine spoken user marker");
    assert_eq!(marker.style.fg, Some(ratatui::style::Color::Red));
    assert!(
        marker
            .style
            .add_modifier
            .contains(ratatui::style::Modifier::BOLD)
    );
    chat.local_settings.tui.animations = false;
    chat.on_realtime_transcript_delta("user".to_string(), " world".to_string());
    assert!(
        chat.realtime_conversation
            .live_transcript_cell
            .as_ref()
            .unwrap()
            .display_lines(/*width*/ 32)
            .iter()
            .any(|line| line.to_string() == "› hello world")
    );
    chat.on_realtime_transcript_done("user".to_string(), " hello world".to_string());
    commit_realtime_history_events(&mut chat, &mut events);
    let Ok(AppEvent::InsertHistoryCell(cell)) = events.try_recv() else {
        panic!("completed voice transcript should retain its ordinary history cell");
    };
    assert!(cell.as_any().is::<crate::history_cell::UserHistoryCell>());
    assert_eq!(cell.raw_lines()[0].to_string(), " hello world");
    assert!(
        cell.display_lines(/*width*/ 32)
            .iter()
            .any(|line| line.to_string() == "› hello world")
    );
}

#[tokio::test]
async fn quiet_voice_turns_accept_audio_before_assistant_captions() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    activate_voice(&mut chat);
    for user_delta in [Some("First question"), None, Some("Third question")] {
        if let Some(delta) = user_delta {
            chat.on_realtime_transcript_delta("user".into(), delta.into());
            assert_eq!(
                chat.realtime_conversation.speaker_suppression_generation,
                None
            );
        }
        chat.on_realtime_transcript_done("user".into(), "Question".into());
        // Audio can arrive before captions: keep the helper accepting it now.
        assert_eq!(
            chat.realtime_conversation.speaker_suppression_generation,
            None
        );
        chat.on_realtime_transcript_delta("assistant".into(), "Answer".into());
        chat.on_realtime_transcript_done("assistant".into(), "Answer".into());
    }
}

#[tokio::test]
async fn new_voice_turn_interrupts_uncaptioned_queued_speech() {
    use super::super::PendingRealtimeSpeech;
    use super::super::PendingSpeechState;

    for (state, captioned, expected) in [
        (PendingSpeechState::Queued(1), false, Some(1)),
        (PendingSpeechState::Accepted, false, Some(1)),
        (PendingSpeechState::AwaitingTurn, false, None),
        (PendingSpeechState::Accepted, true, None),
    ] {
        let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
        let thread_id = activate_voice(&mut chat);
        chat.realtime_conversation
            .pending_speech
            .push_back(PendingRealtimeSpeech {
                state,
                captioned,
                input_generation: 0,
                thread_id,
                turn_id: "old-turn".into(),
                item: agent_item("answer", "Old answer", Some(MessagePhase::FinalAnswer)),
            });
        chat.on_realtime_transcript_delta("user".into(), "New question".into());
        assert_eq!(
            chat.realtime_conversation.speaker_suppression_generation,
            expected
        );
    }
}

#[tokio::test]
async fn completed_user_caption_stays_visible_until_history_commit() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    activate_voice(&mut chat);
    chat.local_settings.tui.animations = false;
    chat.on_realtime_transcript_delta("user".into(), "Keep these words visible".into());
    chat.on_realtime_transcript_done("user".into(), "Keep these words visible.".into());

    // A scheduled draw must not clear the caption before its queued history event runs.
    chat.pre_draw_tick();
    let visible = chat.active_cell_transcript_lines(/*width*/ 80).unwrap();
    insta::assert_snapshot!(visible.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n"), @r"

    › Keep these words visible.
    ");
    assert!(chat.active_cell_transcript_key().is_some());
    let viewport = render_bottom_popup(&chat, /*width*/ 80);
    assert!(viewport.contains("Keep these words visible."));

    // Streaming output can postpone insertion, but must not postpone display.
    chat.on_agent_message_delta("An earlier answer".into());
    assert!(chat.take_realtime_transcript_history().is_empty());
    assert!(render_bottom_popup(&chat, /*width*/ 80).contains("Keep these words visible."));
    chat.finalize_completed_assistant_message(Some("An earlier answer"));
    chat.note_stream_consolidation_completed();
    commit_realtime_history_events(&mut chat, &mut events);
    let history = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| {
            if let AppEvent::InsertHistoryCell(cell) = event {
                Some(cell)
            } else {
                None
            }
        })
        .flat_map(|cell| cell.display_lines(/*width*/ 80))
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(history.matches("Keep these words visible.").count(), 1);
    assert!(!render_bottom_popup(&chat, /*width*/ 80).contains("Keep these words visible."));
}

#[tokio::test]
async fn animated_interleaved_captions_keep_settled_words_visible() {
    let words = "Keep these words visible";
    let mut settled = Vec::new();
    for (first, second) in [("user", "assistant"), ("assistant", "user")] {
        let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
        activate_voice(&mut chat);
        chat.local_settings.tui.animations = true;
        chat.on_realtime_transcript_delta(first.into(), words.into());
        // Let the real animation settle before the other speaker's packets arrive.
        tokio::time::sleep(std::time::Duration::from_millis(/*millis*/ 750)).await;
        for (role, delta) in [(second, "Other speaker"), (first, " please"), (second, ".")] {
            chat.on_realtime_transcript_delta(role.into(), delta.into());
            assert!(render_bottom_popup(&chat, /*width*/ 80).contains(words));
            let overlay = chat.active_cell_transcript_lines(/*width*/ 80).unwrap();
            assert!(overlay.iter().any(|line| line.to_string().contains(words)));
            let key = chat.active_cell_transcript_key().unwrap();
            assert!(key.animation_tick.is_some());
        }
        tokio::time::sleep(std::time::Duration::from_millis(/*millis*/ 750)).await;
        let overlay = chat.active_cell_transcript_lines(/*width*/ 80).unwrap();
        settled.push(format!(
            "{first} first:\n{}",
            overlay
                .iter()
                .map(|line| line.to_string().trim_end().to_string())
                .collect::<Vec<_>>()
                .join("\n")
        ));
        chat.on_realtime_transcript_done(first.into(), "Keep these words visible please".into());
        assert!(
            render_bottom_popup(&chat, /*width*/ 80).contains("Keep these words visible please")
        );
        chat.on_realtime_transcript_done(second.into(), "Other speaker.".into());
        let live_count = chat.realtime_conversation.live_transcript_cells().count();
        assert_eq!(live_count, 0);
        let history = chat.take_realtime_transcript_history();
        assert_eq!(history.len(), 2);
        assert!(chat.active_cell_transcript_key().is_none());
    }
    insta::assert_snapshot!(settled.join("\n"), @"
    user first:

    › Keep these words visible please


    • Other speaker.
    assistant first:

    › Other speaker.


    • Keep these words visible please
    ");
}

#[tokio::test]
async fn empty_interleaved_caption_completion_invalidates_overlay() {
    for phase in [
        RealtimeConversationPhase::Active,
        RealtimeConversationPhase::Stopping,
    ] {
        let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
        activate_voice(&mut chat);
        chat.local_settings.tui.animations = false;
        chat.on_realtime_transcript_delta("assistant".into(), "Discard this caption".into());
        chat.on_realtime_transcript_delta("user".into(), "Keep this caption".into());
        chat.realtime_conversation.phase = phase;
        let previous_key = chat.active_cell_transcript_key().unwrap();
        chat.on_realtime_transcript_done("assistant".into(), String::new());
        let current_key = chat.active_cell_transcript_key().unwrap();
        assert_ne!(previous_key, current_key);
        let visible = chat.active_cell_transcript_lines(/*width*/ 80).unwrap();
        assert_eq!(
            visible.iter().map(ToString::to_string).collect::<Vec<_>>(),
            vec!["", "› Keep this caption", ""]
        );
    }
}
