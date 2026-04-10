use super::make_session_and_context_with_rx;
use crate::messages::MessagePayload;
use crate::timers::ThreadTimerTrigger;
use crate::timers::TimerDelivery;
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
    let (session, _, _) = make_session_and_context_with_rx().await;
    let now = chrono::Utc::now();
    {
        let mut timers = session.timers.lock().await;
        timers
            .create_timer(
                crate::timers::CreateTimer {
                    id: "timer-1".to_string(),
                    trigger: ThreadTimerTrigger::Delay {
                        seconds: 10,
                        repeat: Some(true),
                    },
                    payload: MessagePayload {
                        content: "first".to_string(),
                        instructions: None,
                        meta: Default::default(),
                    },
                    delivery: TimerDelivery::AfterTurn,
                    now,
                },
                /*timer_cancel*/ None,
            )
            .expect("first timer should be created");
        timers
            .create_timer(
                crate::timers::CreateTimer {
                    id: "timer-2".to_string(),
                    trigger: ThreadTimerTrigger::Delay {
                        seconds: 10,
                        repeat: Some(true),
                    },
                    payload: MessagePayload {
                        content: "second".to_string(),
                        instructions: None,
                        meta: Default::default(),
                    },
                    delivery: TimerDelivery::AfterTurn,
                    now,
                },
                /*timer_cancel*/ None,
            )
            .expect("second timer should be created");
        timers.mark_timer_due("timer-1", now);
        timers.mark_timer_due("timer-2", now);
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
