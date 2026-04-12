use super::make_session_and_context_with_rx;
use crate::injected_message::MessagePayload;
use crate::timers::MAX_ACTIVE_TIMERS_PER_THREAD;
use crate::timers::ThreadTimerTrigger;
use crate::timers::TimerDelivery;
use codex_features::Feature;
use std::sync::Arc;

#[tokio::test]
async fn dropping_session_cancels_timer_tasks() {
    let (session, _, _) = make_session_and_context_with_rx().await;
    let cancel_token = session.timer_tasks_cancellation_token.clone();

    drop(session);

    assert!(cancel_token.is_cancelled());
}

#[tokio::test]
async fn maybe_start_pending_timer_claims_only_one_timer_while_start_is_in_progress() {
    let (mut session, _, _) = make_session_and_context_with_rx().await;
    Arc::get_mut(&mut session)
        .expect("session should have no other references")
        .features
        .enable(Feature::Timers)
        .expect("test config should allow feature update");
    let config = {
        let state = session.state.lock().await;
        state
            .session_configuration
            .original_config_do_not_use
            .clone()
    };
    let state_db = codex_state::StateRuntime::init(
        config.sqlite_home.clone(),
        config.model_provider_id.clone(),
    )
    .await
    .expect("state db should open");
    let now = chrono::Utc::now();
    let trigger = ThreadTimerTrigger::Delay {
        seconds: 10,
        repeat: Some(true),
    };
    {
        let mut timers = session.timers.lock().await;
        for (id, content) in [("timer-1", "first"), ("timer-2", "second")] {
            timers
                .create_timer(
                    crate::timers::CreateTimer {
                        id: id.to_string(),
                        trigger: trigger.clone(),
                        payload: MessagePayload {
                            content: content.to_string(),
                            instructions: None,
                            meta: Default::default(),
                        },
                        delivery: TimerDelivery::AfterTurn,
                        now,
                    },
                    /*timer_cancel*/ None,
                )
                .expect("timer should be created");
            timers.mark_timer_due(id, now);
        }
    }
    for timer_id in ["timer-1", "timer-2"] {
        let persisted_timer = session
            .timers
            .lock()
            .await
            .persisted_timer(timer_id)
            .expect("timer should be in memory");
        let timer = persisted_timer.timer;
        state_db
            .create_thread_timer(&codex_state::ThreadTimerCreateParams {
                id: timer.id,
                thread_id: session.conversation_id.to_string(),
                source: "agent".to_string(),
                client_id: "codex-cli".to_string(),
                trigger_json: serde_json::to_string(&timer.trigger)
                    .expect("trigger should serialize"),
                content: timer.content,
                instructions: timer.instructions,
                meta_json: serde_json::to_string(&timer.meta).expect("metadata should serialize"),
                delivery: timer.delivery.as_str().to_string(),
                created_at: timer.created_at,
                next_run_at: timer.next_run_at,
                last_run_at: timer.last_run_at,
                pending_run: persisted_timer.pending_run,
            })
            .await
            .expect("timer should be persisted");
    }

    let first = Arc::clone(&session);
    let second = Arc::clone(&session);
    tokio::join!(
        first.maybe_start_pending_timer(),
        second.maybe_start_pending_timer()
    );

    let timers = session.timers.lock().await.list_timers();
    assert_eq!(
        timers
            .iter()
            .filter(|timer| timer.last_run_at.is_some())
            .count(),
        1
    );
}

#[tokio::test]
async fn create_timer_rejects_when_sqlite_thread_timer_limit_is_reached() {
    let (mut session, _, _) = make_session_and_context_with_rx().await;
    Arc::get_mut(&mut session)
        .expect("session should have no other references")
        .features
        .enable(Feature::Timers)
        .expect("test config should allow feature update");
    let config = {
        let state = session.state.lock().await;
        state
            .session_configuration
            .original_config_do_not_use
            .clone()
    };
    let state_db = codex_state::StateRuntime::init(
        config.sqlite_home.clone(),
        config.model_provider_id.clone(),
    )
    .await
    .expect("state db should open");
    let thread_id = session.conversation_id.to_string();
    for index in 0..MAX_ACTIVE_TIMERS_PER_THREAD {
        state_db
            .create_thread_timer(&test_timer_params(&thread_id, &format!("timer-{index}")))
            .await
            .expect("seed timer");
    }

    let err = session
        .create_timer(
            ThreadTimerTrigger::Delay {
                seconds: 10,
                repeat: None,
            },
            MessagePayload {
                content: "overflow".to_string(),
                instructions: None,
                meta: Default::default(),
            },
            TimerDelivery::AfterTurn,
        )
        .await
        .expect_err("timer creation should reject full sqlite timer set");

    assert_eq!(
        err,
        format!(
            "too many active timers; each thread supports at most {MAX_ACTIVE_TIMERS_PER_THREAD} timers"
        )
    );
    assert_eq!(
        state_db
            .list_thread_timers(&thread_id)
            .await
            .expect("list timers")
            .len(),
        MAX_ACTIVE_TIMERS_PER_THREAD
    );
}

fn test_timer_params(thread_id: &str, id: &str) -> codex_state::ThreadTimerCreateParams {
    codex_state::ThreadTimerCreateParams {
        id: id.to_string(),
        thread_id: thread_id.to_string(),
        source: "agent".to_string(),
        client_id: "codex-cli".to_string(),
        trigger_json: r#"{"kind":"delay","seconds":10}"#.to_string(),
        content: "existing timer".to_string(),
        instructions: None,
        meta_json: "{}".to_string(),
        delivery: TimerDelivery::AfterTurn.as_str().to_string(),
        created_at: 100,
        next_run_at: Some(200),
        last_run_at: None,
        pending_run: false,
    }
}
