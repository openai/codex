#![cfg(test)]

//! Checks interrupt replies against the order in which the client sees terminal events.

use super::*;
use crate::bespoke_event_handling::apply_bespoke_event_handling;
use crate::outgoing_message::OutgoingEnvelope;
use crate::outgoing_message::OutgoingMessage;
use app_test_support::create_mock_responses_server_repeating_assistant;
use codex_app_server_protocol::ThreadQueueAddParams;
use codex_app_server_protocol::ThreadQueueStartParams;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::NoopExtensionEventSink;
use codex_extension_api::TurnLifecycleContributor;
use codex_extension_api::TurnStartInput;
use codex_extension_api::TurnStopInput;
use codex_protocol::protocol::Event;
use codex_queue_extension::QueuedItemService;
use codex_thread_store::LocalQueueStore;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;
use tokio::sync::Notify;

#[test]
fn late_completion_keeps_a_finished_review_delegate_ineligible_for_interrupt() {
    let mut state = ThreadState::default();
    let event = |kind, id| serde_json::from_value(json!({"type": kind, "turn_id": id})).unwrap();
    state.track_current_turn_event("previous", &event("turn_started", "previous"));
    state.track_current_turn_event("review", &event("turn_started", "delegate"));
    assert_eq!(
        interrupt_terminal_turn_id(&state, "delegate"),
        Some("review")
    );
    for id in ["review", "previous"] {
        state.track_current_turn_event(id, &event("turn_complete", id));
        state.take_turn_summary(id);
    }
    assert_eq!(interrupt_terminal_turn_id(&state, "delegate"), None);
}

#[derive(Default)]
struct TerminalGates {
    parent: std::sync::OnceLock<String>,
    delegate_start: Notify,
    run_delegate: Notify,
    stopped: Notify,
    finish: Notify,
}

impl TurnLifecycleContributor for TerminalGates {
    fn on_turn_start<'a>(&'a self, input: TurnStartInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if self
                .parent
                .get()
                .is_some_and(|id| id != input.thread_store.level_id())
            {
                self.delegate_start.notify_one();
                self.run_delegate.notified().await;
            }
        })
    }

    fn on_turn_stop<'a>(&'a self, _: TurnStopInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async {
            self.stopped.notify_one();
            self.finish.notified().await;
        })
    }
}

#[test_case(false; "turn_start")]
#[test_case(true; "thread_queue_start")]
#[tokio::test]
async fn interrupt_reply_waits_for_the_requested_turn(queued: bool) -> anyhow::Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let gates = Arc::new(TerminalGates::default());
    let mut registry = ExtensionRegistryBuilder::new();
    registry.turn_lifecycle_contributor(gates.clone());
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let test = test_codex()
        .with_extensions(Arc::new(registry.build()))
        .build_with_auto_env(&server)
        .await?;
    let thread_id = test.session_configured.thread_id;
    gates.parent.set(thread_id.to_string()).unwrap();
    let connection_id = ConnectionId(1);
    let analytics = AnalyticsEventsClient::disabled();
    let (tx, mut rx) = tokio::sync::mpsc::channel(crate::CHANNEL_CAPACITY);
    let outgoing = Arc::new(OutgoingMessageSender::new(tx, analytics.clone()));
    let watcher = SkillsWatcher::new(
        test.thread_manager.skills_service(),
        &test.config.codex_home,
        outgoing.clone(),
    );
    let processor = TurnRequestProcessor::new(
        test.thread_manager.clone(),
        test.thread_store.clone(),
        outgoing.clone(),
        analytics,
        Arc::new(test.config.clone()),
        ConfigManager::without_managed_config_for_tests(test.config.codex_home.to_path_buf()),
        Default::default(),
        ThreadStateManager::new(),
        ThreadWatchManager::new(),
        watcher.clone(),
        /*turn_cost_worker*/ None,
    );
    let request_id = |id| ConnectionRequestId {
        connection_id,
        request_id: RequestId::Integer(id),
    };
    let turn = TurnStartParams {
        thread_id: thread_id.to_string(),
        input: vec![V2UserInput::Text {
            text: "first".into(),
            text_elements: vec![],
        }],
        ..Default::default()
    };
    let interrupt = async |id, turn_id: &str| {
        processor
            .turn_interrupt_inner(
                &request_id(id),
                TurnInterruptParams {
                    thread_id: thread_id.to_string(),
                    turn_id: turn_id.to_owned(),
                },
            )
            .await
    };
    let first = if queued {
        let store = LocalQueueStore::new(test.codex.state_db().expect("state DB"));
        let service = QueuedItemService::new(
            Arc::new(store),
            Arc::downgrade(&test.thread_manager),
            Arc::new(NoopExtensionEventSink),
        );
        let queue = ThreadQueueRequestProcessor::new(
            test.thread_manager.clone(),
            test.thread_store.clone(),
            outgoing.clone(),
            processor.config_manager.clone(),
            Some(Arc::new(service)),
        );
        let add = ThreadQueueAddParams {
            thread_id: turn.thread_id.clone(),
            input: turn.input,
            client_user_message_id: "queued-first".into(),
            responsesapi_client_metadata: None,
        };
        queue.add(add).await.expect("enqueue");
        let params = ThreadQueueStartParams {
            thread_id: turn.thread_id,
            queued_submission_id: None,
        };
        queue
            .start(&request_id(1), params)
            .await
            .expect("start queued turn")
            .turn
            .id
    } else {
        processor
            .turn_start_inner(
                request_id(1),
                turn,
                /*app_server_client_name*/ None,
                /*app_server_client_version*/ None,
            )
            .await
            .expect("start turn")
            .turn
            .id
    };
    gates.stopped.notified().await;
    assert_eq!(
        interrupt(0, "unknown").await.unwrap_err(),
        invalid_request("no active turn to interrupt")
    );
    assert_eq!(interrupt(2, &first).await, Ok(None));
    gates.finish.notify_one();
    // Hold A's actual terminal event until B has an interrupt waiting for a reply.
    let completed = wait_for_event(
        &test.codex,
        |msg| matches!(msg, EventMsg::TurnComplete(e) if e.turn_id == first && e.error.is_none()),
    )
    .await;

    let params = ReviewStartParams {
        thread_id: thread_id.to_string(),
        target: ApiReviewTarget::UncommittedChanges,
        delivery: None,
    };
    let review = processor
        .review_start(&request_id(3), params)
        .await
        .expect("start review");
    let Some(ClientResponsePayload::ReviewStart(review)) = review else {
        panic!("expected review response")
    };
    let second = review.turn.id;
    gates.delegate_start.notified().await;
    assert_eq!(
        interrupt(5, &first).await.unwrap_err(),
        invalid_request("no active turn to interrupt")
    );
    // The review owns the slot, but Core has not announced its delegate yet.
    assert_eq!(
        interrupt(6, &second).await,
        Err(invalid_request("no active turn to interrupt"))
    );
    gates.run_delegate.notify_one();
    wait_for_event(&test.codex, |msg| matches!(msg, EventMsg::TurnStarted(_))).await;
    // The delegate finishes its model work after the stale interrupt. Its parent is still running.
    gates.stopped.notified().await;
    assert_eq!(interrupt(4, &second).await, Ok(None));
    gates.finish.notify_one();
    let aborted = wait_for_event(
        &test.codex,
        |msg| matches!(msg, EventMsg::TurnAborted(e) if e.turn_id.as_deref() == Some(&second)),
    )
    .await;

    let state = processor.thread_state_manager.thread_state(thread_id).await;
    let outgoing = ThreadScopedOutgoingMessageSender::new(outgoing, vec![connection_id], thread_id);
    let mut receive = || match rx.try_recv().expect("terminal event already processed") {
        OutgoingEnvelope::ToConnection { message, .. } => message,
        other => panic!("unexpected outgoing message {other:?}"),
    };
    for (turn_id, msg, reply, status) in [
        (first.clone(), completed, 2, TurnStatus::Completed),
        (second, aborted, 4, TurnStatus::Interrupted),
    ] {
        state.lock().await.track_current_turn_event(&turn_id, &msg);
        apply_bespoke_event_handling(
            Event {
                id: turn_id.clone(),
                msg,
            },
            thread_id,
            test.codex.clone(),
            test.thread_manager.clone(),
            outgoing.clone(),
            state.clone(),
            processor.thread_watch_manager.clone(),
        )
        .await;
        assert!(
            matches!(receive(), OutgoingMessage::Response(r) if r.id == RequestId::Integer(reply))
        );
        assert!(
            matches!(receive(), OutgoingMessage::AppServerNotification(n)
            if matches!(n.notification, ServerNotification::TurnCompleted(ref done) if done.turn.id == turn_id && done.turn.status == status))
        );
    }
    // Late child activity opens a history turn after Core has finished both turns.
    let activity = serde_json::from_value(json!({
        "type": "sub_agent_activity", "kind": "completed", "event_id": "late-child",
        "agent_thread_id": ThreadId::new(), "agent_path": "/root/worker"
    }))?;
    let history_id = {
        let mut state = state.lock().await;
        state.track_current_turn_event(&first, &activity);
        state.active_turn_id().expect("history turn").to_owned()
    };
    assert_eq!(
        interrupt(7, &history_id).await,
        Err(invalid_request("no active turn to interrupt"))
    );
    assert!(state.lock().await.pending_interrupts.is_empty());
    test.codex.shutdown_and_wait().await?;
    watcher.shutdown();
    Ok(())
}
