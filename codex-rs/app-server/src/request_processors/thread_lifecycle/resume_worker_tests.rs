use super::*;

#[test]
fn busy_history_read_retries_once_then_fails_closed_after_idle_transition() {
    assert_eq!(
        classify_busy_history_read(/*attempt*/ 0, /*thread_became_idle*/ false),
        BusyHistoryReadDisposition::ReturnBusy
    );
    assert_eq!(
        classify_busy_history_read(/*attempt*/ 0, /*thread_became_idle*/ true),
        BusyHistoryReadDisposition::RetryIdle
    );
    assert_eq!(
        classify_busy_history_read(/*attempt*/ 1, /*thread_became_idle*/ true),
        BusyHistoryReadDisposition::Conflict
    );
}

#[test]
fn resume_status_projects_buffered_terminal_turn_before_listener_tracking() {
    let complete = EventMsg::TurnComplete(TurnCompleteEvent {
        turn_id: "completed-turn".to_string(),
        last_agent_message: None,
        completed_at: Some(2),
        duration_ms: Some(1_000),
        time_to_first_token_ms: None,
    });
    let aborted = EventMsg::TurnAborted(TurnAbortedEvent {
        turn_id: Some("aborted-turn".to_string()),
        reason: TurnAbortReason::Interrupted,
        completed_at: Some(2),
        duration_ms: Some(1_000),
    });

    for terminal_event in [&complete, &aborted] {
        assert_eq!(
            project_thread_status_after_buffered_events(
                ThreadStatus::Active {
                    active_flags: vec![ThreadActiveFlag::WaitingOnUserInput],
                },
                /*has_live_in_progress_turn*/ false,
                [terminal_event],
            ),
            ThreadStatus::Idle
        );
    }
}

#[test]
fn resume_status_keeps_active_when_a_later_turn_started_or_is_live() {
    let complete = EventMsg::TurnComplete(TurnCompleteEvent {
        turn_id: "completed-turn".to_string(),
        last_agent_message: None,
        completed_at: Some(2),
        duration_ms: Some(1_000),
        time_to_first_token_ms: None,
    });
    let started = EventMsg::TurnStarted(TurnStartedEvent {
        turn_id: "next-turn".to_string(),
        trace_id: None,
        started_at: Some(3),
        model_context_window: None,
        collaboration_mode_kind: Default::default(),
    });
    let active_status = ThreadStatus::Active {
        active_flags: vec![ThreadActiveFlag::WaitingOnApproval],
    };

    assert_eq!(
        project_thread_status_after_buffered_events(
            active_status.clone(),
            /*has_live_in_progress_turn*/ false,
            [&complete, &started],
        ),
        active_status
    );
    assert_eq!(
        project_thread_status_after_buffered_events(
            ThreadStatus::Idle,
            /*has_live_in_progress_turn*/ true,
            [&complete],
        ),
        ThreadStatus::Active {
            active_flags: Vec::new(),
        }
    );
}

#[test]
fn rejected_resume_begin_does_not_defer_following_listener_command() {
    let mut resume_in_flight = false;

    // A Begin that rejects before spawning its worker returns `None`; no Finish command is
    // guaranteed in that case, so the listener must remain available for ordinary commands.
    apply_listener_command_transition(&mut resume_in_flight, ListenerCommandTransition::None);

    assert!(!resume_in_flight);
    assert!(!should_defer_listener_command(
        resume_in_flight,
        &ThreadListenerCommand::EmitThreadGoalCleared,
    ));
}

#[tokio::test]
async fn blocked_resume_worker_drains_high_volume_exec_deltas_to_existing_subscriber() {
    let cancellation_token = CancellationToken::new();
    let (worker_started_tx, worker_started_rx) = oneshot::channel();
    let blocked_worker = tokio::spawn(run_cancelable_resume_worker(
        cancellation_token.child_token(),
        async move {
            let _ = worker_started_tx.send(());
            futures::future::pending::<()>().await;
        },
    ));
    worker_started_rx
        .await
        .expect("resume worker should reach blocked storage phase");

    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
    let queued_depth = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let consumer_queued_depth = std::sync::Arc::clone(&queued_depth);
    let (subscriber_tx, mut subscriber_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let drain = tokio::spawn(async move {
        let mut buffered = Vec::new();
        let mut exec_delta_replay = ResumeExecDeltaReplay::default();
        while let Some(event) = event_rx.recv().await {
            consumer_queued_depth.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            match route_resume_in_flight_event(
                event,
                /*has_buffered_prefix*/ !buffered.is_empty(),
            ) {
                ResumeInFlightEvent::DispatchImmediately(event) => {
                    exec_delta_replay.retain(&event);
                    subscriber_tx
                        .send(event.id)
                        .expect("existing subscriber should remain attached");
                }
                ResumeInFlightEvent::Buffer(event) => buffered.push(event),
            }
        }
        (buffered, exec_delta_replay)
    });
    let delta = Event {
        id: "large-output-turn".to_string(),
        msg: EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
            call_id: "large-output-command".to_string(),
            stream: ExecOutputStream::Stdout,
            chunk: vec![b'x'; 8 * 1024],
        }),
    };
    let mut max_queue_depth = 0;
    for _ in 0..100 {
        for _ in 0..100 {
            let queue_depth = queued_depth.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            event_tx
                .send(delta.clone())
                .expect("listener channel should stay open");
            max_queue_depth = max_queue_depth.max(queue_depth);
        }
        timeout(Duration::from_secs(1), async {
            while queued_depth.load(std::sync::atomic::Ordering::SeqCst) != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("listener should drain each output batch while storage remains blocked");
        assert!(!blocked_worker.is_finished());
    }
    queued_depth.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    event_tx
        .send(Event {
            id: "ordinary-event".to_string(),
            msg: EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "ordinary-event".to_string(),
                trace_id: None,
                started_at: None,
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
        })
        .expect("listener channel should stay open");
    drop(event_tx);

    let (buffered, exec_delta_replay) = drain.await.expect("resume event drain should not panic");
    assert!(
        max_queue_depth <= 100,
        "queue depth should stay bounded by one producer batch, got {max_queue_depth}"
    );
    let mut subscriber_event_count = 0;
    while subscriber_rx.try_recv().is_ok() {
        subscriber_event_count += 1;
    }
    assert_eq!(subscriber_event_count, 10_000);
    assert_eq!(buffered.len(), 1);
    assert!(matches!(&buffered[0].event.msg, EventMsg::TurnStarted(_)));
    assert!(exec_delta_replay.is_truncated());
    assert!(exec_delta_replay.payload_bytes() <= DEFAULT_OUTPUT_BYTES_CAP);
    let replayed_events = exec_delta_replay.into_events();
    assert!(replayed_events.len() <= RESUME_EXEC_DELTA_REPLAY_MAX_EVENTS);
    let EventMsg::ExecCommandOutputDelta(last_replayed_delta) = &replayed_events
        .last()
        .expect("replay should retain output")
        .event
        .msg
    else {
        panic!("exec replay must contain only output deltas");
    };
    assert_eq!(
        last_replayed_delta.chunk,
        RESUME_EXEC_DELTA_REPLAY_TRUNCATION_MARKER
    );

    cancellation_token.cancel();
    blocked_worker
        .await
        .expect("cancelled resume worker should not panic");
}

#[tokio::test]
async fn resume_exec_delta_replay_targets_only_the_joiner_and_skips_covered_output() {
    let conversation_id = ThreadId::new();
    let connection_id = ConnectionId(17);
    let (tx, mut rx) = tokio::sync::mpsc::channel(4);
    let outgoing = Arc::new(OutgoingMessageSender::new(
        tx,
        codex_analytics::AnalyticsEventsClient::disabled(),
    ));
    let mut replay = ResumeExecDeltaReplay::default();
    for chunk in [b"missing".to_vec(), b"covered".to_vec()] {
        replay.retain(&Event {
            id: "active-turn".to_string(),
            msg: EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
                call_id: "active-command".to_string(),
                stream: ExecOutputStream::Stdout,
                chunk,
            }),
        });
    }
    replay.events_mut()[1].represented_in_resume_snapshot = true;

    dispatch_replayed_exec_deltas_to_connection(replay, connection_id, conversation_id, &outgoing)
        .await;

    let envelope = rx
        .recv()
        .await
        .expect("joiner should receive retained output");
    let OutgoingEnvelope::ToConnection {
        connection_id: actual_connection_id,
        message:
            OutgoingMessage::AppServerNotification(ServerNotification::CommandExecutionOutputDelta(
                notification,
            )),
        ..
    } = envelope
    else {
        panic!("expected one connection-scoped exec output notification");
    };
    assert_eq!(actual_connection_id, connection_id);
    assert_eq!(notification.thread_id, conversation_id.to_string());
    assert_eq!(notification.turn_id, "active-turn");
    assert_eq!(notification.item_id, "active-command");
    assert_eq!(notification.delta, "missing");
    assert!(
        rx.try_recv().is_err(),
        "covered output must not be replayed"
    );
}

#[test]
fn resume_event_routing_preserves_exec_begin_before_output_delta() {
    let begin = Event {
        id: "turn-with-command".to_string(),
        msg: EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
            call_id: "ordered-command".to_string(),
            process_id: None,
            turn_id: "turn-with-command".to_string(),
            started_at_ms: 1,
            command: vec!["printf".to_string(), "output".to_string()],
            cwd: "file:///tmp".parse().expect("path uri"),
            parsed_cmd: Vec::new(),
            source: ExecCommandSource::Agent,
            interaction_input: None,
        }),
    };
    let delta = Event {
        id: "turn-with-command".to_string(),
        msg: EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
            call_id: "ordered-command".to_string(),
            stream: ExecOutputStream::Stdout,
            chunk: b"output".to_vec(),
        }),
    };
    let mut buffered = Vec::new();
    match route_resume_in_flight_event(begin, /*has_buffered_prefix*/ false) {
        ResumeInFlightEvent::Buffer(event) => buffered.push(event),
        ResumeInFlightEvent::DispatchImmediately(_) => panic!("exec begin must be buffered"),
    }
    match route_resume_in_flight_event(delta, /*has_buffered_prefix*/ true) {
        ResumeInFlightEvent::Buffer(event) => buffered.push(event),
        ResumeInFlightEvent::DispatchImmediately(_) => {
            panic!("output must not overtake a buffered exec begin")
        }
    }

    assert!(matches!(
        &buffered[0].event.msg,
        EventMsg::ExecCommandBegin(_)
    ));
    assert!(matches!(
        &buffered[1].event.msg,
        EventMsg::ExecCommandOutputDelta(_)
    ));
}

#[tokio::test]
async fn resume_worker_reads_metadata_after_acquiring_history_event_cut() {
    let harness = TestCodexHarness::new()
        .await
        .expect("test Codex thread should start");
    let conversation = Arc::clone(&harness.test().codex);
    let conversation_id = harness.test().session_configured.thread_id;
    conversation
        .update_thread_metadata(
            ThreadMetadataPatch {
                name: Some(Some("captured before cut".to_string())),
                ..Default::default()
            },
            /*include_archived*/ true,
        )
        .await
        .expect("initial metadata should update");
    let captured_source_thread = conversation
        .read_thread(
            /*include_archived*/ true, /*include_history*/ false,
        )
        .await
        .expect("request should capture pre-cut metadata");
    assert_eq!(
        captured_source_thread.name.as_deref(),
        Some("captured before cut")
    );

    let held_cut = conversation
        .acquire_history_reconciliation_event_cut()
        .await;
    let (cut_attempt_tx, cut_attempt_rx) = oneshot::channel();
    let (cut_acquired_tx, mut cut_acquired_rx) = oneshot::channel();
    let worker_conversation = Arc::clone(&conversation);
    let worker = tokio::spawn(async move {
        let _ = cut_attempt_tx.send(());
        let worker_cut = worker_conversation
            .acquire_history_reconciliation_event_cut()
            .await;
        let _ = cut_acquired_tx.send(());
        let result =
            read_pending_thread_resume_history(conversation_id, &worker_conversation).await;
        drop(worker_cut);
        result
    });
    cut_attempt_rx
        .await
        .expect("resume worker should attempt to acquire the cut");
    assert!(
        timeout(Duration::from_millis(25), &mut cut_acquired_rx)
            .await
            .is_err(),
        "held cut must block the resume worker after pre-cut metadata capture"
    );

    conversation
        .update_thread_metadata(
            ThreadMetadataPatch {
                name: Some(Some("updated while worker blocked".to_string())),
                ..Default::default()
            },
            /*include_archived*/ true,
        )
        .await
        .expect("external metadata update should not require the history/event cut");
    drop(held_cut);
    cut_acquired_rx
        .await
        .expect("resume worker should acquire the released cut");
    let prepared = worker
        .await
        .expect("resume worker should not panic")
        .expect("resume worker should prepare history and metadata");

    let response_thread = crate::request_processors::thread_processor::thread_from_stored_thread(
        prepared.stored_thread,
        harness.test().config.model_provider_id.as_str(),
        &harness.test().config.cwd,
    )
    .0;
    assert_eq!(
        response_thread.name.as_deref(),
        Some("updated while worker blocked")
    );
    assert_ne!(response_thread.name, captured_source_thread.name);
}

#[tokio::test]
async fn listener_cancellation_drops_blocked_resume_worker_and_completion_state() {
    let cancellation_token = CancellationToken::new();
    let (started_tx, started_rx) = oneshot::channel();
    let (completion_tx, completion_rx) = oneshot::channel::<()>();
    let worker_cancel = cancellation_token.child_token();
    let worker = tokio::spawn(async move {
        run_cancelable_resume_worker(worker_cancel, async move {
            // Mirrors the real worker's cut guards, resume lease, and completion sender: all
            // are owned by this future and must drop if the listener is cancelled mid-read.
            let _completion_tx = completion_tx;
            let _ = started_tx.send(());
            futures::future::pending::<()>().await;
        })
        .await;
    });

    started_rx.await.expect("worker should enter blocked read");
    cancellation_token.cancel();
    timeout(Duration::from_secs(1), worker)
        .await
        .expect("cancelled worker should exit promptly")
        .expect("cancelled worker should not panic");
    assert!(
        completion_rx.await.is_err(),
        "listener cancellation must drop the pending request completion sender"
    );
}
