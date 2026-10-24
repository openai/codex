#![cfg(test)]
//! Verifies dependency publication at real execution-lock handoffs.

use super::super::DirectToolTimings;
use super::super::ToolBatchTiming;
use super::*;
use codex_utils_critical_path::CriticalPath;
use codex_utils_critical_path::PathDurations;
use codex_utils_critical_path::WorkKind;
use pretty_assertions::assert_eq;
use std::future::Future;
use std::task::Context;
use std::task::Waker;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio_util::either::Either;

#[test_case::test_case(ToolAdmission::Parallel; "parallel_after_exclusive")]
#[test_case::test_case(ToolAdmission::Exclusive; "exclusive_after_parallel")]
#[tokio::test]
async fn lock_handoff_publishes_review_history_before_admitting_waiter(admission: ToolAdmission) {
    let start = Instant::now() - Duration::from_secs(10);
    let tool_start = start + Duration::from_secs(1);
    let shared_initial_path = CriticalPath::default().work(start, tool_start, WorkKind::Inference);
    let timings = DirectToolTimings::default();
    let batch = ToolBatchTiming::default();
    let gate = ToolGateTiming::default();
    let lock = RwLock::new(());
    let (releasing_lock, releasing_admission) = match admission {
        ToolAdmission::Parallel => (Either::Left(lock.write().await), ToolAdmission::Exclusive),
        ToolAdmission::Exclusive => (Either::Right(lock.read().await), ToolAdmission::Parallel),
    };
    let releasing_work = timings.start_turn_direct_call(
        "releasing",
        "turn",
        batch.clone(),
        tool_start,
        Some(shared_initial_path.clone()),
    );
    let releasing_timing = gate.admit(releasing_work, releasing_admission, tool_start);
    // Fixed historical boundaries keep attribution exact without sleeping for a review.
    {
        let mut state = timings.state();
        let work = state.unique_work("releasing").unwrap();
        work.record_until(start + Duration::from_secs(2));
        work.active_auto_reviews = 1;
        work.record_until(start + Duration::from_secs(5));
        work.active_auto_reviews = 0;
    }
    let waiting_work = timings.start_turn_direct_call(
        "waiting",
        "turn",
        batch.clone(),
        tool_start,
        Some(shared_initial_path),
    );
    let acquire_waiting_lock = async {
        match admission {
            ToolAdmission::Parallel => Either::Left(lock.read().await),
            ToolAdmission::Exclusive => Either::Right(lock.write().await),
        }
    };
    tokio::pin!(acquire_waiting_lock);
    let mut context = Context::from_waker(Waker::noop());
    assert!(
        acquire_waiting_lock
            .as_mut()
            .poll(&mut context)
            .is_pending()
    );

    // Runtime cleanup must publish the dependency while it still holds the execution lock.
    drop(releasing_timing);
    assert!(batch.completed().is_none());
    assert!(
        acquire_waiting_lock
            .as_mut()
            .poll(&mut context)
            .is_pending()
    );
    drop(releasing_lock);
    let waiting_lock = acquire_waiting_lock.await;
    let waiting_timing = gate.admit(waiting_work, admission, tool_start);
    assert!(batch.completed().is_none());
    drop(waiting_timing);
    drop(waiting_lock);

    let completed = batch.completed().unwrap();
    assert_eq!(
        completed.path.unwrap().durations(start, completed.ended_at),
        PathDurations {
            inference: Duration::from_secs(1),
            auto_review: Duration::from_secs(3),
            tools: completed.ended_at.duration_since(start) - Duration::from_secs(4),
            ..PathDurations::default()
        }
    );
    assert!(timings.state().calls.is_empty());
}
