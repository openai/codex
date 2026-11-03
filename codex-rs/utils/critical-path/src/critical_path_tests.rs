//! Reducer coverage for exclusive attribution, parallel joins, ties, and incomplete evidence.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn join_replaces_overlapping_work_with_the_full_winning_branch() {
    let turn_start = Instant::now();
    let time_after_turn_start = |milliseconds| turn_start + Duration::from_millis(milliseconds);
    let shared_initial_path = CriticalPath::default().work(
        time_after_turn_start(0),
        time_after_turn_start(100),
        WorkKind::Sampling,
    );
    let reviewed_tool_path = shared_initial_path
        .clone()
        .work(
            time_after_turn_start(100),
            time_after_turn_start(300),
            WorkKind::AutoReview,
        )
        .work(
            time_after_turn_start(300),
            time_after_turn_start(400),
            WorkKind::Tools,
        );
    let parallel_tool_path = shared_initial_path.clone().work(
        time_after_turn_start(100),
        time_after_turn_start(700),
        WorkKind::Tools,
    );
    let critical_path = shared_initial_path
        .work(
            time_after_turn_start(100),
            time_after_turn_start(200),
            WorkKind::Sampling,
        )
        .join(
            time_after_turn_start(200),
            time_after_turn_start(700),
            reviewed_tool_path.latest(parallel_tool_path),
        );
    assert_eq!(
        critical_path.durations(time_after_turn_start(0), time_after_turn_start(700)),
        PathDurations {
            sampling: Duration::from_millis(100),
            tools: Duration::from_millis(600),
            ..PathDurations::default()
        }
    );
}

#[test]
fn nested_serial_dependencies_preserve_exclusive_categories() {
    let turn_start = Instant::now();
    let time_after_turn_start = |milliseconds| turn_start + Duration::from_millis(milliseconds);
    let shared_initial_path = CriticalPath::default().work(
        time_after_turn_start(0),
        time_after_turn_start(100),
        WorkKind::Sampling,
    );
    let child_path = shared_initial_path
        .clone()
        .work(
            time_after_turn_start(100),
            time_after_turn_start(300),
            WorkKind::AutoReview,
        )
        .work(
            time_after_turn_start(300),
            time_after_turn_start(400),
            WorkKind::Tools,
        )
        .work(
            time_after_turn_start(400),
            time_after_turn_start(500),
            WorkKind::Compaction,
        );
    let critical_path = shared_initial_path
        .work(
            time_after_turn_start(100),
            time_after_turn_start(200),
            WorkKind::Sampling,
        )
        .join(
            time_after_turn_start(200),
            time_after_turn_start(600),
            child_path,
        );
    assert_eq!(
        critical_path.durations(time_after_turn_start(0), time_after_turn_start(600)),
        PathDurations {
            sampling: Duration::from_millis(100),
            compaction: Duration::from_millis(100),
            auto_review: Duration::from_millis(200),
            auto_review_overhead: None,
            tools: Duration::from_millis(200),
            ..PathDurations::default()
        }
    );
}

#[test]
fn equal_finish_times_select_one_path_without_double_counting() {
    let turn_start = Instant::now();
    let turn_end = turn_start + Duration::from_millis(100);
    let reviewed_tool_path =
        CriticalPath::default().work(turn_start, turn_end, WorkKind::AutoReview);
    let inference_path = CriticalPath::default().work(turn_start, turn_end, WorkKind::Sampling);
    // A tie has multiple valid paths. Stable left selection does not sum both.
    assert_eq!(
        reviewed_tool_path
            .clone()
            .latest(inference_path)
            .durations(turn_start, turn_end),
        reviewed_tool_path.durations(turn_start, turn_end)
    );
}

#[test]
fn path_retention_exhaustion_marks_unknown_instead_of_claiming_zero() {
    let turn_start = Instant::now();
    let mut critical_path = CriticalPath::default();
    for segment_index in 0..=MAX_PATH_DEPTH {
        critical_path = critical_path.work(
            turn_start + Duration::from_micros(segment_index as u64),
            turn_start + Duration::from_micros(segment_index as u64 + 1),
            WorkKind::Sampling,
        );
    }
    let turn_end = turn_start + Duration::from_micros(MAX_PATH_DEPTH as u64 + 1);
    assert_eq!(
        critical_path.durations(turn_start, turn_end),
        PathDurations {
            unknown: turn_end.duration_since(turn_start),
            auto_review_overhead: None,
            ..Default::default()
        }
    );
}

#[test]
fn reviewed_branch_wins_even_when_some_review_overlapped_parent_inference() {
    let turn_start = Instant::now();
    let time_after_turn_start = |milliseconds| turn_start + Duration::from_millis(milliseconds);
    let shared_initial_path = CriticalPath::default().work(
        time_after_turn_start(0),
        time_after_turn_start(100),
        WorkKind::Sampling,
    );
    let reviewed_tool_path = shared_initial_path
        .clone()
        .work(
            time_after_turn_start(100),
            time_after_turn_start(300),
            WorkKind::AutoReview,
        )
        .work(
            time_after_turn_start(300),
            time_after_turn_start(400),
            WorkKind::Tools,
        );
    let parallel_tool_path = shared_initial_path.clone().work(
        time_after_turn_start(100),
        time_after_turn_start(350),
        WorkKind::Tools,
    );
    let critical_path = shared_initial_path
        .work(
            time_after_turn_start(100),
            time_after_turn_start(200),
            WorkKind::Sampling,
        )
        .join(
            time_after_turn_start(200),
            time_after_turn_start(400),
            reviewed_tool_path.latest(parallel_tool_path),
        );
    assert_eq!(
        critical_path.durations(time_after_turn_start(0), time_after_turn_start(400)),
        PathDurations {
            sampling: Duration::from_millis(100),
            auto_review: Duration::from_millis(200),
            auto_review_overhead: None,
            tools: Duration::from_millis(100),
            ..Default::default()
        }
    );
}

#[test]
fn durations_clip_work_to_the_window_and_attribute_gaps_to_overhead() {
    let turn_start = Instant::now();
    let time_after_turn_start = |milliseconds| turn_start + Duration::from_millis(milliseconds);
    let critical_path = CriticalPath::default()
        .work(
            time_after_turn_start(10),
            time_after_turn_start(30),
            WorkKind::Sampling,
        )
        .work(
            time_after_turn_start(50),
            time_after_turn_start(90),
            WorkKind::AutoReview,
        );

    assert_eq!(
        critical_path.durations(time_after_turn_start(0), time_after_turn_start(100)),
        PathDurations {
            sampling: Duration::from_millis(20),
            auto_review: Duration::from_millis(40),
            auto_review_overhead: None,
            overhead: Duration::from_millis(40),
            ..PathDurations::default()
        }
    );
    assert_eq!(
        critical_path.durations(time_after_turn_start(20), time_after_turn_start(80)),
        PathDurations {
            sampling: Duration::from_millis(10),
            auto_review: Duration::from_millis(30),
            auto_review_overhead: None,
            overhead: Duration::from_millis(20),
            ..PathDurations::default()
        }
    );
}

#[test]
fn overlapping_work_only_attributes_time_after_the_existing_path() {
    let turn_start = Instant::now();
    let time_after_turn_start = |milliseconds| turn_start + Duration::from_millis(milliseconds);
    let critical_path = CriticalPath::default()
        .work(
            time_after_turn_start(0),
            time_after_turn_start(10),
            WorkKind::Sampling,
        )
        .work(
            time_after_turn_start(5),
            time_after_turn_start(20),
            WorkKind::Tools,
        );

    assert_eq!(
        critical_path.durations(time_after_turn_start(0), time_after_turn_start(20)),
        PathDurations {
            sampling: Duration::from_millis(10),
            tools: Duration::from_millis(10),
            ..PathDurations::default()
        }
    );
    assert_eq!(
        critical_path.clone().work(
            time_after_turn_start(5),
            time_after_turn_start(15),
            WorkKind::AutoReview
        ),
        critical_path
    );
    assert_eq!(
        critical_path.clone().work(
            time_after_turn_start(20),
            time_after_turn_start(20),
            WorkKind::Unknown
        ),
        critical_path
    );
}

#[test]
fn long_path_debug_output_is_bounded_on_a_small_stack() {
    // Match a typical 2 MiB worker stack rather than nextest's larger test stack.
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            let start = Instant::now();
            let mut path = CriticalPath::default();
            for index in 0..10_000 {
                path = path.work(
                    start + Duration::from_micros(index),
                    start + Duration::from_micros(index + 1),
                    WorkKind::Sampling,
                );
            }
            assert!(format!("{path:?}").len() < 512);
            assert!(format!("{path:#?}").len() < 512);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn retained_prefixes_can_be_released_concurrently_on_small_stacks() {
    for _ in 0..8 {
        let start = Instant::now();
        let mut path = CriticalPath::default();
        let mut prefixes = [Vec::new(), Vec::new()];
        for index in 0..MAX_PATH_DEPTH {
            path = path.work(
                start + Duration::from_micros(index as u64),
                start + Duration::from_micros(index as u64 + 1),
                WorkKind::Sampling,
            );
            prefixes[index % 2].push(path.clone());
        }
        // Only the workers retain nodes when they begin releasing prefixes.
        drop(path);
        let ready = Arc::new(std::sync::Barrier::new(3));
        let workers = prefixes.map(|mut prefixes| {
            let ready = Arc::clone(&ready);
            std::thread::Builder::new()
                .stack_size(2 * 1024 * 1024)
                .spawn(move || {
                    ready.wait();
                    // Release newest first so adjacent retained prefixes race
                    // to become the last owner of the shared chain.
                    while let Some(prefix) = prefixes.pop() {
                        drop(prefix);
                    }
                })
                .unwrap()
        });
        ready.wait();
        for worker in workers {
            worker.join().unwrap();
        }
    }
}

#[test]
fn human_wait_is_retained_only_on_the_selected_parallel_path() {
    let start = Instant::now();
    let time_after_turn_start = |milliseconds| start + Duration::from_millis(milliseconds);
    let prefix =
        CriticalPath::default().work(start, time_after_turn_start(100), WorkKind::Sampling);
    let reviewed_tool = prefix
        .clone()
        .work(
            time_after_turn_start(100),
            time_after_turn_start(200),
            WorkKind::AutoReview,
        )
        .work(
            time_after_turn_start(200),
            time_after_turn_start(500),
            WorkKind::UserWait,
        )
        .work(
            time_after_turn_start(500),
            time_after_turn_start(600),
            WorkKind::Tools,
        );
    let parallel_tool = prefix.clone().work(
        time_after_turn_start(100),
        time_after_turn_start(400),
        WorkKind::Tools,
    );
    let path = prefix.join(
        time_after_turn_start(100),
        time_after_turn_start(600),
        parallel_tool.latest(reviewed_tool),
    );
    assert_eq!(
        path.durations(start, time_after_turn_start(600)),
        PathDurations {
            sampling: Duration::from_millis(100),
            auto_review: Duration::from_millis(100),
            auto_review_overhead: None,
            user_wait: Duration::from_millis(300),
            tools: Duration::from_millis(100),
            ..PathDurations::default()
        }
    );
}

fn observed_review(intervals: &[(Instant, Instant)]) -> ReviewExecution {
    let review = ReviewExecution::default();
    review.observe();
    for &(start, end) in intervals {
        review.started(start);
        review.completed(start, end);
    }
    review.seal();
    review
}

#[test]
fn review_overhead_is_the_execution_complement_and_clips_to_the_requested_window() {
    let start = Instant::now();
    let at = |ms| start + Duration::from_millis(ms);
    let review = observed_review(&[(at(1000), at(4633))]);
    let path = CriticalPath::default().auto_review(start, at(5000), vec![review]);
    assert_eq!(
        path.durations(start, at(5000)),
        PathDurations {
            auto_review: Duration::from_millis(5000),
            auto_review_overhead: Some(Duration::from_millis(1367)),
            ..Default::default()
        }
    );
    assert_eq!(
        path.durations(at(500), at(4800)),
        PathDurations {
            auto_review: Duration::from_millis(4300),
            auto_review_overhead: Some(Duration::from_millis(667)),
            ..Default::default()
        }
    );
}

#[test]
fn overlapping_reviews_and_retries_subtract_a_clipped_union_once() {
    let start = Instant::now();
    let at = |ms| start + Duration::from_millis(ms);
    let first = observed_review(&[(at(0), at(30)), (at(25), at(50))]);
    let second = observed_review(&[(at(40), at(60)), (at(80), at(120))]);
    let path = CriticalPath::default().auto_review(at(10), at(100), vec![first, second]);
    assert_eq!(
        path.durations(at(10), at(100)),
        PathDurations {
            auto_review: Duration::from_millis(90),
            auto_review_overhead: Some(Duration::from_millis(20)),
            ..Default::default()
        }
    );
}

#[test]
fn human_wait_overlap_is_excluded_from_both_review_components() {
    let start = Instant::now();
    let at = |ms| start + Duration::from_millis(ms);
    let review = observed_review(&[(at(20), at(80))]);
    let path = CriticalPath::default()
        .auto_review(start, at(40), vec![review.clone()])
        .work(at(40), at(70), WorkKind::UserWait)
        .auto_review(at(70), at(100), vec![review]);
    assert_eq!(
        path.durations(start, at(100)),
        PathDurations {
            auto_review: Duration::from_millis(70),
            auto_review_overhead: Some(Duration::from_millis(40)),
            user_wait: Duration::from_millis(30),
            ..Default::default()
        }
    );
}

#[test]
fn cached_review_has_only_overhead_and_losing_review_has_no_contribution() {
    let start = Instant::now();
    let at = |ms| start + Duration::from_millis(ms);
    let reviewed = CriticalPath::default().auto_review(start, at(10), vec![observed_review(&[])]);
    assert_eq!(
        reviewed.durations(start, at(10)),
        PathDurations {
            auto_review: Duration::from_millis(10),
            auto_review_overhead: Some(Duration::from_millis(10)),
            ..Default::default()
        }
    );
    let winner = CriticalPath::default().work(start, at(20), WorkKind::Tools);
    assert_eq!(
        reviewed.latest(winner).durations(start, at(20)),
        PathDurations {
            tools: Duration::from_millis(20),
            ..Default::default()
        }
    );
}

#[test]
fn incomplete_or_unobserved_reviews_leave_the_subcomponent_unknown() {
    let start = Instant::now();
    let end = start + Duration::from_millis(10);
    let unobserved = ReviewExecution::default();
    let canceled = ReviewExecution::default();
    canceled.observe();
    canceled.started(start);
    let missing = ReviewExecution::default();
    missing.observe();
    missing.mark_unknown();
    for review in [unobserved, canceled, missing] {
        review.seal();
        let path = CriticalPath::default().auto_review(start, end, vec![review.clone()]);
        let expected = PathDurations {
            auto_review: Duration::from_millis(10),
            auto_review_overhead: None,
            ..Default::default()
        };
        assert_eq!(path.durations(start, end), expected);
        review.observe();
        review.completed(start, end);
        assert_eq!(path.durations(start, end), expected);
    }
}

#[test]
fn one_completed_retry_does_not_hide_another_canceled_attempt() {
    let start = Instant::now();
    let at = |ms| start + Duration::from_millis(ms);
    let review = ReviewExecution::default();
    review.observe();
    review.started(at(1));
    review.started(at(2));
    review.completed(at(2), at(5));
    review.seal();
    assert_eq!(
        CriticalPath::default()
            .auto_review(start, at(10), vec![review])
            .durations(start, at(10)),
        PathDurations {
            auto_review: Duration::from_millis(10),
            auto_review_overhead: None,
            ..Default::default()
        }
    );
}

#[test]
fn indistinguishable_concurrent_attempts_remain_unknown_after_one_completes() {
    let start = Instant::now();
    let end = start + Duration::from_millis(10);
    let review = ReviewExecution::default();
    review.observe();
    review.started(start);
    review.started(start);
    review.completed(start, end);
    review.seal();
    assert_eq!(
        CriticalPath::default()
            .auto_review(start, end, vec![review])
            .durations(start, end),
        PathDurations {
            auto_review: Duration::from_millis(10),
            auto_review_overhead: None,
            ..Default::default()
        }
    );
}

#[test]
fn sealing_before_submission_starts_freezes_unknown_attribution() {
    let start = Instant::now();
    let end = start + Duration::from_millis(10);
    let review = ReviewExecution::default();
    review.observe();
    let attempt = review.start_attempt();
    review.seal();
    let path = CriticalPath::default().auto_review(start, end, vec![review.clone()]);
    let expected = PathDurations {
        auto_review: Duration::from_millis(10),
        auto_review_overhead: None,
        ..Default::default()
    };
    assert_eq!(path.durations(start, end), expected);
    review.started(start);
    review.completed(start, end);
    attempt.finish(/*known*/ true);
    assert_eq!(path.durations(start, end), expected);
}

#[test]
fn completed_attempts_resolve_only_when_all_submissions_have_known_outcomes() {
    let start = Instant::now();
    let at = |ms| start + Duration::from_millis(ms);
    for known in [true, false] {
        let review = ReviewExecution::default();
        review.observe();
        let first = review.start_attempt();
        let second = review.start_attempt();
        review.started(at(2));
        review.completed(at(2), at(7));
        first.finish(/*known*/ true);
        let path = CriticalPath::default().auto_review(start, at(10), vec![review.clone()]);
        assert_eq!(path.durations(start, at(10)).auto_review_overhead, None);
        second.finish(known);
        review.seal();
        assert_eq!(
            path.durations(start, at(10)),
            PathDurations {
                auto_review: Duration::from_millis(10),
                auto_review_overhead: known.then_some(Duration::from_millis(5)),
                ..Default::default()
            }
        );
    }
}

#[test]
fn incomplete_attempt_does_not_label_unobserved_execution_as_overhead() {
    for completed in [false, true] {
        let execution = ReviewExecution::default();
        execution.observe();
        let attempt = execution.start_attempt();
        let start = Instant::now();
        let end = start + Duration::from_millis(100);
        if completed {
            execution.started(start);
            execution.completed(start, end);
            attempt.finish(/*known*/ true);
        } else {
            drop(attempt);
        }
        execution.seal();
        let totals = CriticalPath::default()
            .auto_review(start, end, vec![execution])
            .durations(start, end);
        assert_eq!(
            totals.auto_review_overhead,
            completed.then_some(Duration::ZERO)
        );
    }
}
