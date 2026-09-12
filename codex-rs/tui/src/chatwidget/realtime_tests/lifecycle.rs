//! Voice startup, shutdown, and retry maintain one active owned session.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn enabling_voice_on_an_open_thread_snapshots_the_new_thread_notice() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.set_feature_enabled(
        codex_features::Feature::RealtimeConversation,
        /*enabled*/ true,
    );
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
    insta::assert_snapshot!(rendered, @"• Voice conversations will be available in new threads.");
}

#[tokio::test]
async fn voice_cannot_start_in_a_side_conversation() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    chat.set_side_conversation_active(/*active*/ true);

    chat.toggle_realtime_conversation();

    commit_realtime_history_events(&mut chat, &mut events);
    let Ok(AppEvent::InsertHistoryCell(cell)) = events.try_recv() else {
        panic!("voice should report that side conversations are unsupported");
    };
    assert!(
        cell.display_lines(/*width*/ 80)
            .iter()
            .any(|line| line.to_string().contains("side conversations"))
    );
    assert!(ops.try_recv().is_err());
    assert_eq!(
        chat.realtime_conversation.phase,
        RealtimeConversationPhase::Inactive
    );
}

#[tokio::test]
async fn mute_during_startup_is_saved_before_the_offer_handle_exists() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.realtime_conversation.phase = RealtimeConversationPhase::Starting;
    chat.realtime_conversation.thread_id = Some(ThreadId::new());

    chat.toggle_realtime_microphone();
    assert!(chat.realtime_conversation.microphone_muted);
    assert!(events.try_recv().is_err());

    chat.toggle_realtime_microphone();
    assert!(!chat.realtime_conversation.microphone_muted);
    assert!(events.try_recv().is_err());
}

#[tokio::test]
async fn voice_cannot_start_on_a_parent_owned_thread() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.blocks_direct_input = true;

    chat.toggle_realtime_conversation();

    assert_eq!(
        chat.realtime_conversation.phase,
        RealtimeConversationPhase::Inactive
    );
    assert!(matches!(
        events.try_recv(),
        Ok(AppEvent::InsertHistoryCell(_))
    ));
}

#[tokio::test]
async fn stopping_while_the_offer_is_pending_resets_the_session() {
    let (mut chat, _sender, _events, mut ops) = make_chatwidget_manual_with_sender().await;
    let (abort, _registration) = AbortHandle::new_pair();
    let observed_abort = abort.clone();
    chat.realtime_conversation.phase = RealtimeConversationPhase::Starting;
    chat.realtime_conversation.startup_abort = Some(abort);

    chat.stop_realtime_conversation();

    assert_eq!(
        (
            chat.realtime_conversation.phase,
            ops.try_recv().ok(),
            observed_abort.is_aborted(),
        ),
        (RealtimeConversationPhase::Inactive, None, true)
    );
}

#[tokio::test]
async fn resetting_pending_voice_aborts_without_reporting_a_backend_session() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    let (abort, _registration) = AbortHandle::new_pair();
    let observed_abort = abort.clone();
    chat.realtime_conversation.phase = RealtimeConversationPhase::Starting;
    chat.realtime_conversation.thread_id = Some(ThreadId::new());
    chat.realtime_conversation.startup_abort = Some(abort);

    assert_eq!(
        (
            chat.reset_realtime_conversation(),
            chat.realtime_conversation.phase,
            observed_abort.is_aborted(),
        ),
        (None, RealtimeConversationPhase::Inactive, true)
    );
}

#[tokio::test]
async fn audio_failure_cancels_pending_voice_and_reports_the_device_error() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    let (abort, _registration) = AbortHandle::new_pair();
    let observed_abort = abort.clone();
    chat.realtime_conversation.phase = RealtimeConversationPhase::Starting;
    chat.realtime_conversation.startup_abort = Some(abort);

    chat.on_realtime_error("speaker stream failed: device disconnected".to_string());

    commit_realtime_history_events(&mut chat, &mut events);
    let Ok(AppEvent::InsertHistoryCell(cell)) = events.try_recv() else {
        panic!("voice should report the speaker failure");
    };
    assert!(
        cell.display_lines(/*width*/ 80)
            .iter()
            .any(|line| line.to_string().contains("device disconnected"))
    );
    assert_eq!(
        (
            chat.realtime_conversation.phase,
            observed_abort.is_aborted()
        ),
        (RealtimeConversationPhase::Inactive, true)
    );
}

#[tokio::test]
async fn voice_waits_for_the_previous_session_to_close_before_restarting() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.realtime_conversation.phase = RealtimeConversationPhase::Stopping;

    chat.toggle_realtime_conversation();
    assert_eq!(
        chat.realtime_conversation.phase,
        RealtimeConversationPhase::Stopping
    );

    chat.on_realtime_conversation_closed(/*reason*/ Some("requested".into()));
    assert_eq!(
        chat.realtime_conversation.phase,
        RealtimeConversationPhase::Inactive
    );
}

#[tokio::test]
async fn canceled_offer_is_ignored_after_a_new_start_attempt() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.realtime_conversation.phase = RealtimeConversationPhase::Starting;
    chat.realtime_conversation.attempt_id = 2;

    chat.on_realtime_webrtc_offer_created(
        ThreadId::new(),
        /*attempt_id*/ 1,
        Err("canceled offer failed".to_string()),
    );

    assert_eq!(
        (
            chat.realtime_conversation.phase,
            chat.realtime_conversation.attempt_id,
            events.try_recv().is_err()
        ),
        (RealtimeConversationPhase::Starting, 2, true)
    );
}

#[tokio::test]
async fn voice_becomes_active_only_after_backend_and_current_peer_are_ready() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.realtime_conversation.phase = RealtimeConversationPhase::Starting;
    chat.realtime_conversation.attempt_id = 2;

    chat.on_realtime_webrtc_connected(/*attempt_id*/ 1, Ok(()));
    assert_eq!(
        chat.realtime_conversation.phase,
        RealtimeConversationPhase::Starting
    );

    chat.on_realtime_webrtc_connected(/*attempt_id*/ 2, Ok(()));
    assert_eq!(
        chat.realtime_conversation.phase,
        RealtimeConversationPhase::Starting
    );

    chat.on_realtime_conversation_started();
    assert_eq!(
        chat.realtime_conversation.phase,
        RealtimeConversationPhase::Active
    );
    commit_realtime_history_events(&mut chat, &mut events);
    let Ok(AppEvent::InsertHistoryCell(cell)) = events.try_recv() else {
        panic!("voice start should insert its history banner");
    };
    insta::assert_snapshot!(
        "voice_start_banner",
        cell.display_lines(/*width*/ 80)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[tokio::test]
async fn normal_voice_close_renders_the_ended_message() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    activate_voice(&mut chat);
    while events.try_recv().is_ok() {}

    chat.on_realtime_conversation_closed(Some("transport_closed".into()));

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
    insta::assert_snapshot!("voice_ended_message", rendered);
}

#[tokio::test]
async fn voice_waits_for_peer_when_backend_starts_first() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.realtime_conversation.phase = RealtimeConversationPhase::Starting;
    chat.realtime_conversation.attempt_id = 2;

    chat.on_realtime_conversation_started();
    assert_eq!(
        chat.realtime_conversation.phase,
        RealtimeConversationPhase::Starting
    );

    chat.on_realtime_webrtc_connected(/*attempt_id*/ 2, Ok(()));
    assert_eq!(
        chat.realtime_conversation.phase,
        RealtimeConversationPhase::Active
    );
}

#[tokio::test]
async fn startup_retry_waits_for_closed_then_uses_a_fresh_attempt_and_preserves_mute() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    chat.realtime_conversation.phase = RealtimeConversationPhase::Starting;
    chat.realtime_conversation.attempt_id = 0;
    chat.realtime_conversation.microphone_muted = true;
    chat.on_realtime_webrtc_connected(
        /*attempt_id*/ 0,
        Err(codex_realtime_webrtc::ConnectionError::NegotiationTimedOut),
    );
    assert!(
        matches!(ops.try_recv().unwrap(), AppCommand::RealtimeConversationStop { thread_id: id } if id == thread_id)
    );
    assert_eq!(
        (
            chat.realtime_conversation.phase,
            chat.realtime_conversation.startup_retry
        ),
        (
            RealtimeConversationPhase::Stopping,
            super::super::StartupRetry::WaitingForStop
        )
    );
    assert!(ops.try_recv().is_err());
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
    insta::assert_snapshot!("voice_startup_retry", rendered.join("\n"));
    chat.on_realtime_conversation_closed(Some("transport_closed".into()));
    assert_eq!(
        chat.realtime_conversation.phase,
        RealtimeConversationPhase::Stopping
    );
    assert_eq!(chat.realtime_conversation.attempt_id, 0);
    chat.on_realtime_conversation_closed(Some("requested".into()));
    let new_attempt = chat.realtime_conversation.attempt_id;
    assert_ne!(new_attempt, 0);
    assert_eq!(
        (
            chat.realtime_conversation.phase,
            chat.realtime_conversation.startup_retry,
            chat.realtime_conversation.microphone_muted
        ),
        (
            RealtimeConversationPhase::Starting,
            super::super::StartupRetry::Used,
            true
        )
    );
    // A completion from the first attempt cannot finish or fail the retry.
    chat.on_realtime_webrtc_connected(
        /*attempt_id*/ 0,
        Err(codex_realtime_webrtc::ConnectionError::NegotiationTimedOut),
    );
    assert_eq!(
        chat.realtime_conversation.phase,
        RealtimeConversationPhase::Starting
    );
    chat.on_realtime_webrtc_connected(new_attempt, Ok(()));
    chat.on_realtime_conversation_started();
    assert_eq!(
        chat.realtime_conversation.phase,
        RealtimeConversationPhase::Active
    );
    chat.reset_realtime_conversation();
}

#[tokio::test]
async fn startup_retry_is_cancelled_while_waiting_for_backend_close() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    activate_voice(&mut chat);
    chat.realtime_conversation.phase = RealtimeConversationPhase::Starting;
    chat.on_realtime_webrtc_connected(
        /*attempt_id*/ 0,
        Err(codex_realtime_webrtc::ConnectionError::NegotiationTimedOut),
    );
    chat.toggle_realtime_conversation();
    chat.on_realtime_conversation_closed(Some("transport_closed".into()));
    assert_eq!(
        chat.realtime_conversation.phase,
        RealtimeConversationPhase::Stopping
    );
    chat.on_realtime_conversation_closed(Some("requested".into()));
    assert_eq!(
        (
            chat.realtime_conversation.phase,
            chat.realtime_conversation.attempt_id
        ),
        (RealtimeConversationPhase::Inactive, 0)
    );
}

#[tokio::test]
async fn startup_transport_close_before_peer_timeout_retries_once_and_ignores_old_result() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    activate_voice(&mut chat);
    chat.realtime_conversation.phase = RealtimeConversationPhase::Starting;
    chat.realtime_conversation.attempt_id = 0;
    // The close can also overtake the backend's started notification.
    chat.realtime_conversation.backend_started = false;
    chat.realtime_conversation.microphone_muted = true;

    chat.on_realtime_conversation_closed(Some("transport_closed".into()));
    let retry_attempt = chat.realtime_conversation.attempt_id;
    assert_ne!(retry_attempt, 0);
    assert_eq!(
        (
            chat.realtime_conversation.phase,
            chat.realtime_conversation.startup_retry,
            chat.realtime_conversation.microphone_muted,
        ),
        (
            RealtimeConversationPhase::Starting,
            super::super::StartupRetry::Used,
            true,
        )
    );
    assert!(
        ops.try_recv().is_err(),
        "closed backend needs no extra stop"
    );
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
    insta::assert_snapshot!("voice_startup_transport_close_retry", rendered);

    chat.on_realtime_webrtc_connected(
        /*attempt_id*/ 0,
        Err(codex_realtime_webrtc::ConnectionError::NegotiationTimedOut),
    );
    assert_eq!(
        (
            chat.realtime_conversation.phase,
            chat.realtime_conversation.attempt_id
        ),
        (RealtimeConversationPhase::Starting, retry_attempt)
    );
    chat.on_realtime_conversation_started();
    chat.on_realtime_webrtc_connected(retry_attempt, Ok(()));
    assert_eq!(
        chat.realtime_conversation.phase,
        RealtimeConversationPhase::Active
    );
    chat.on_realtime_conversation_closed(Some("transport_closed".into()));
    assert_eq!(
        chat.realtime_conversation.phase,
        RealtimeConversationPhase::Inactive
    );
    assert!(ops.try_recv().is_err(), "retry budget must be used");
}

#[tokio::test]
async fn startup_retry_never_retries_twice_or_retries_other_errors_or_active_sessions() {
    for (phase, budget, error) in [
        (
            RealtimeConversationPhase::Starting,
            super::super::StartupRetry::Used,
            codex_realtime_webrtc::ConnectionError::NegotiationTimedOut,
        ),
        (
            RealtimeConversationPhase::Starting,
            super::super::StartupRetry::Available,
            codex_realtime_webrtc::ConnectionError::Failed,
        ),
        (
            RealtimeConversationPhase::Active,
            super::super::StartupRetry::Available,
            codex_realtime_webrtc::ConnectionError::NegotiationTimedOut,
        ),
    ] {
        let (mut chat, _sender, _events, mut ops) = make_chatwidget_manual_with_sender().await;
        let thread_id = activate_voice(&mut chat);
        chat.realtime_conversation.phase = phase;
        chat.realtime_conversation.startup_retry = budget;
        chat.on_realtime_webrtc_connected(/*attempt_id*/ 0, Err(error));
        assert_ne!(
            chat.realtime_conversation.startup_retry,
            super::super::StartupRetry::WaitingForStop
        );
        if phase == RealtimeConversationPhase::Starting {
            assert!(matches!(
                ops.try_recv(),
                Ok(AppCommand::RealtimeConversationStop { thread_id: stopped }) if stopped == thread_id
            ));
        }
        assert!(ops.try_recv().is_err(), "no retry may be queued");
    }
}

#[tokio::test]
async fn failure_cleanup_does_not_attribute_stop_to_the_user() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    // A local failure initiates backend cleanup; its acknowledgement is still "requested".
    chat.realtime_conversation.phase = RealtimeConversationPhase::Stopping;
    chat.realtime_conversation.failure_recorded = true;
    chat.on_realtime_error(format!(
        "Failed to connect voice mode: {}",
        codex_realtime_webrtc::ConnectionError::AudioDevices
    ));
    chat.on_realtime_conversation_closed(Some("requested".into()));
    assert_eq!(
        chat.realtime_conversation.phase,
        RealtimeConversationPhase::Inactive
    );
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
    insta::assert_snapshot!("voice_device_failure_cleanup", rendered);
}
