use super::*;
use codex_utils_critical_path::PathDurations;
use pretty_assertions::assert_eq;
use std::time::Duration;

#[test]
fn parallel_winner_retains_its_review_and_shared_initial_path() {
    let now = Instant::now();
    let start = now - Duration::from_secs(10);
    let shared_initial_path =
        CriticalPath::default().work(start, start + Duration::from_secs(1), WorkKind::Sampling);
    let batch = ToolBatchTiming::default();
    let timings = DirectToolTimings::default();
    let losing = timings.start_direct_call(
        "loser",
        batch.clone(),
        start + Duration::from_secs(1),
        Some(shared_initial_path.clone()),
    );
    let winner = timings.start_direct_call(
        "winner",
        batch.clone(),
        start + Duration::from_secs(1),
        Some(shared_initial_path),
    );
    {
        let mut state = timings.state();
        let work = state.unique_work("winner").unwrap();
        work.record_until(start + Duration::from_secs(2));
        work.active_auto_reviews = vec![ReviewExecution::default()];
        work.record_until(start + Duration::from_secs(5));
        work.active_auto_reviews.clear();
    }
    drop(losing);
    assert!(batch.completed().is_none());
    drop(winner);
    let completed = batch.completed().unwrap();
    assert_eq!(
        completed.path.unwrap().durations(start, completed.ended_at),
        PathDurations {
            sampling: Duration::from_secs(1),
            auto_review: Duration::from_secs(3),
            auto_review_overhead: None,
            tools: completed.ended_at.duration_since(start) - Duration::from_secs(4),
            ..PathDurations::default()
        }
    );
}

#[test]
fn duplicate_ids_and_late_review_drop_do_not_complete_another_invocation() {
    let timings = DirectToolTimings::default();
    let batch = ToolBatchTiming::default();
    let start = Instant::now();
    let first =
        timings.start_direct_call("same", batch.clone(), start, Some(CriticalPath::default()));
    let review = first.start_review().unwrap();
    let second =
        timings.start_direct_call("same", batch.clone(), start, Some(CriticalPath::default()));

    drop(first);
    drop(review);
    assert_eq!(
        timings
            .state()
            .unique_work("same")
            .unwrap()
            .active_auto_reviews
            .len(),
        0
    );
    assert!(batch.completed().is_none());
    drop(second);
    assert!(batch.completed().unwrap().path.is_none());
    assert!(timings.state().calls.is_empty());
}

#[test]
fn late_review_start_does_not_attach_to_reused_call_id() {
    let timings = DirectToolTimings::default();
    let batch = ToolBatchTiming::default();
    let first = timings.start_direct_call(
        "same",
        batch.clone(),
        Instant::now(),
        Some(CriticalPath::default()),
    );
    let review = first.start_review().unwrap();
    first.finish().unwrap();
    let started_at = Instant::now();
    let second = timings.start_direct_call(
        "same",
        batch.clone(),
        started_at,
        Some(CriticalPath::default()),
    );
    assert!(first.start_review().is_none());
    drop(review);
    drop(first);
    assert!(batch.completed().is_none());
    drop(second);
    let completed = batch.completed().unwrap();
    assert_eq!(
        completed
            .path
            .unwrap()
            .durations(started_at, completed.ended_at),
        PathDurations {
            tools: completed.ended_at.duration_since(started_at),
            ..PathDurations::default()
        }
    );
    assert!(timings.state().calls.is_empty());
}

#[test]
fn cancellation_completes_once_and_review_can_outlive_call() {
    let timings = DirectToolTimings::default();
    let batch = ToolBatchTiming::default();
    let work = timings.start_direct_call(
        "cancelled",
        batch.clone(),
        Instant::now(),
        Some(CriticalPath::default()),
    );
    let review = work.start_review().unwrap();
    let first = work.finish().unwrap();
    drop(work);
    drop(review);
    assert_eq!(batch.completed().unwrap().ended_at, first.ended_at);
    assert!(timings.state().calls.is_empty());
}

#[test]
fn missing_dependency_remains_unknown() {
    let start = Instant::now() - Duration::from_secs(1);
    let timings = DirectToolTimings::default();
    let batch = ToolBatchTiming::default();
    let work =
        timings.start_direct_call("call", batch.clone(), start, Some(CriticalPath::default()));
    timings.join(
        "call",
        start,
        start + Duration::from_millis(500),
        /*dependency*/ None,
    );
    drop(work);
    let completed = batch.completed().unwrap();
    assert_eq!(
        completed
            .path
            .unwrap()
            .durations(start, completed.ended_at)
            .unknown,
        Duration::from_millis(500)
    );
}

#[test]
fn human_wait_takes_precedence_over_reviews_and_overlapping_waits() {
    let start = Instant::now();
    let at = |seconds| start + Duration::from_secs(seconds);
    let timings = DirectToolTimings::default();
    let batch = ToolBatchTiming::default();
    let _guard = timings.start_direct_call("call", batch, start, Some(CriticalPath::default()));
    let mut state = timings.state();
    let work = state.unique_work("call").unwrap();
    work.record_until(at(1));
    work.active_auto_reviews = vec![ReviewExecution::default()];
    work.record_until(at(2));
    work.active_user_waits = 1;
    work.record_until(at(3));
    work.active_user_waits = 2;
    work.record_until(at(4));
    work.active_user_waits = 1;
    work.record_until(at(5));
    work.active_user_waits = 0;
    work.record_until(at(6));
    work.active_auto_reviews.clear();
    work.record_until(at(7));
    assert_eq!(
        work.path.as_ref().unwrap().durations(start, at(7)),
        PathDurations {
            tools: Duration::from_secs(2),
            auto_review: Duration::from_secs(2),
            auto_review_overhead: None,
            user_wait: Duration::from_secs(3),
            ..PathDurations::default()
        }
    );
}

#[test]
fn user_wait_guards_balance_and_do_not_follow_reused_call_ids() {
    let timings = DirectToolTimings::default();
    let batch = ToolBatchTiming::default();
    let first = timings.start_direct_call(
        "call",
        batch.clone(),
        Instant::now(),
        Some(CriticalPath::default()),
    );
    let first_wait = first.start_user_wait().unwrap();
    let nested_wait = first.start_user_wait().unwrap();
    assert_eq!(
        timings
            .state()
            .unique_work("call")
            .unwrap()
            .active_user_waits,
        2
    );
    drop(nested_wait);
    assert_eq!(
        timings
            .state()
            .unique_work("call")
            .unwrap()
            .active_user_waits,
        1
    );
    first.finish().unwrap();
    let next = timings.start_direct_call(
        "call",
        batch.clone(),
        Instant::now(),
        Some(CriticalPath::default()),
    );
    assert!(first.start_user_wait().is_none());
    drop(first_wait);
    assert_eq!(
        timings
            .state()
            .unique_work("call")
            .unwrap()
            .active_user_waits,
        0
    );
    drop(next);
    assert!(batch.completed().is_some());
}

#[test]
fn observed_cached_review_is_all_overhead_and_human_wait_has_no_observer() {
    let timings = DirectToolTimings::default();
    let batch = ToolBatchTiming::default();
    let start = Instant::now();
    let invocation =
        timings.start_direct_call("cached", batch, start, Some(CriticalPath::default()));
    let first = invocation.start_review().unwrap();
    let second = invocation.start_review().unwrap();
    let first_observer = first.review_execution().unwrap();
    let second_observer = second.review_execution().unwrap();
    assert_ne!(first_observer, second_observer);
    let human = invocation.start_user_wait().unwrap();
    assert!(human.review_execution().is_none());
    drop(first);
    assert_eq!(
        timings
            .state()
            .unique_work("cached")
            .unwrap()
            .active_auto_reviews,
        vec![second_observer]
    );
    drop(human);
    drop(second);
    let completed = invocation.finish().unwrap();
    let durations = completed.path.unwrap().durations(start, completed.ended_at);
    assert_eq!(durations.auto_review_overhead, Some(durations.auto_review));
    assert!(durations.auto_review > Duration::ZERO);
    assert!(durations.user_wait > Duration::ZERO);
}

#[test]
fn invocation_finish_freezes_unfinished_review_and_id_reuse_gets_new_observation() {
    let timings = DirectToolTimings::default();
    let batch = ToolBatchTiming::default();
    let start = Instant::now();
    let first =
        timings.start_direct_call("same", batch.clone(), start, Some(CriticalPath::default()));
    let old_guard = first.start_review().unwrap();
    let old_observer = old_guard.review_execution().unwrap();
    let child_start = Instant::now();
    old_observer.started(child_start);
    let first_result = first.finish().unwrap();
    let old_path = first_result.path.unwrap();
    let old_durations = old_path.durations(start, first_result.ended_at);
    assert_eq!(old_durations.auto_review_overhead, None);

    let next_start = Instant::now();
    let next = timings.start_direct_call("same", batch, next_start, Some(CriticalPath::default()));
    let next_guard = next.start_review().unwrap();
    let next_observer = next_guard.review_execution().unwrap();
    assert_ne!(old_observer, next_observer);
    old_observer.completed(child_start, Instant::now());
    drop(old_guard);
    assert_eq!(
        old_path.durations(start, first_result.ended_at),
        old_durations
    );
    assert_eq!(
        timings
            .state()
            .unique_work("same")
            .unwrap()
            .active_auto_reviews,
        vec![next_observer]
    );
    drop(next_guard);
    let next_result = next.finish().unwrap();
    let durations = next_result
        .path
        .unwrap()
        .durations(next_start, next_result.ended_at);
    assert_eq!(durations.auto_review_overhead, Some(durations.auto_review));
}
