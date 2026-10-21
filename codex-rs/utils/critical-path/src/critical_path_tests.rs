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
        WorkKind::Inference,
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
            WorkKind::Inference,
        )
        .join(
            time_after_turn_start(200),
            time_after_turn_start(700),
            reviewed_tool_path.latest(parallel_tool_path),
        );
    assert_eq!(
        critical_path.durations(time_after_turn_start(0), time_after_turn_start(700)),
        PathDurations {
            inference: Duration::from_millis(100),
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
        WorkKind::Inference,
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
            WorkKind::Inference,
        );
    let critical_path = shared_initial_path
        .work(
            time_after_turn_start(100),
            time_after_turn_start(200),
            WorkKind::Inference,
        )
        .join(
            time_after_turn_start(200),
            time_after_turn_start(600),
            child_path,
        );
    assert_eq!(
        critical_path.durations(time_after_turn_start(0), time_after_turn_start(600)),
        PathDurations {
            inference: Duration::from_millis(200),
            auto_review: Duration::from_millis(200),
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
    let inference_path = CriticalPath::default().work(turn_start, turn_end, WorkKind::Inference);
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
            WorkKind::Inference,
        );
    }
    let turn_end = turn_start + Duration::from_micros(MAX_PATH_DEPTH as u64 + 1);
    assert_eq!(
        critical_path.durations(turn_start, turn_end),
        PathDurations {
            unknown: turn_end.duration_since(turn_start),
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
        WorkKind::Inference,
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
            WorkKind::Inference,
        )
        .join(
            time_after_turn_start(200),
            time_after_turn_start(400),
            reviewed_tool_path.latest(parallel_tool_path),
        );
    assert_eq!(
        critical_path.durations(time_after_turn_start(0), time_after_turn_start(400)),
        PathDurations {
            inference: Duration::from_millis(100),
            auto_review: Duration::from_millis(200),
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
            WorkKind::Inference,
        )
        .work(
            time_after_turn_start(50),
            time_after_turn_start(90),
            WorkKind::AutoReview,
        );

    assert_eq!(
        critical_path.durations(time_after_turn_start(0), time_after_turn_start(100)),
        PathDurations {
            inference: Duration::from_millis(20),
            auto_review: Duration::from_millis(40),
            overhead: Duration::from_millis(40),
            ..PathDurations::default()
        }
    );
    assert_eq!(
        critical_path.durations(time_after_turn_start(20), time_after_turn_start(80)),
        PathDurations {
            inference: Duration::from_millis(10),
            auto_review: Duration::from_millis(30),
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
            WorkKind::Inference,
        )
        .work(
            time_after_turn_start(5),
            time_after_turn_start(20),
            WorkKind::Tools,
        );

    assert_eq!(
        critical_path.durations(time_after_turn_start(0), time_after_turn_start(20)),
        PathDurations {
            inference: Duration::from_millis(10),
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
                    WorkKind::Inference,
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
                WorkKind::Inference,
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
        CriticalPath::default().work(start, time_after_turn_start(100), WorkKind::Inference);
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
            inference: Duration::from_millis(100),
            auto_review: Duration::from_millis(100),
            user_wait: Duration::from_millis(300),
            tools: Duration::from_millis(100),
            ..PathDurations::default()
        }
    );
}
