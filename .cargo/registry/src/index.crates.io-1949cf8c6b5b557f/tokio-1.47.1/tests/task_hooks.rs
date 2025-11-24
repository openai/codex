#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", tokio_unstable, target_has_atomic = "64"))]

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use tokio::runtime::Builder;

const TASKS: usize = 8;
const ITERATIONS: usize = 64;
/// Assert that the spawn task hook always fires when set.
#[test]
fn spawn_task_hook_fires() {
    let count = Arc::new(AtomicUsize::new(0));
    let count2 = Arc::clone(&count);

    let ids = Arc::new(Mutex::new(HashSet::new()));
    let ids2 = Arc::clone(&ids);

    let runtime = Builder::new_current_thread()
        .on_task_spawn(move |data| {
            ids2.lock().unwrap().insert(data.id());

            count2.fetch_add(1, Ordering::SeqCst);
        })
        .build()
        .unwrap();

    for _ in 0..TASKS {
        runtime.spawn(std::future::pending::<()>());
    }

    let count_realized = count.load(Ordering::SeqCst);
    assert_eq!(
        TASKS, count_realized,
        "Total number of spawned task hook invocations was incorrect, expected {TASKS}, got {}",
        count_realized
    );

    let count_ids_realized = ids.lock().unwrap().len();

    assert_eq!(
        TASKS, count_ids_realized,
        "Total number of spawned task hook invocations was incorrect, expected {TASKS}, got {}",
        count_realized
    );
}

/// Assert that the terminate task hook always fires when set.
#[test]
fn terminate_task_hook_fires() {
    let count = Arc::new(AtomicUsize::new(0));
    let count2 = Arc::clone(&count);

    let runtime = Builder::new_current_thread()
        .on_task_terminate(move |_data| {
            count2.fetch_add(1, Ordering::SeqCst);
        })
        .build()
        .unwrap();

    for _ in 0..TASKS {
        runtime.spawn(std::future::ready(()));
    }

    runtime.block_on(async {
        // tick the runtime a bunch to close out tasks
        for _ in 0..ITERATIONS {
            tokio::task::yield_now().await;
        }
    });

    assert_eq!(TASKS, count.load(Ordering::SeqCst));
}

/// Test that the correct spawn location is provided to the task hooks on a
/// current thread runtime.
#[test]
fn task_hook_spawn_location_current_thread() {
    let spawns = Arc::new(AtomicUsize::new(0));
    let poll_starts = Arc::new(AtomicUsize::new(0));
    let poll_ends = Arc::new(AtomicUsize::new(0));

    let runtime = Builder::new_current_thread()
        .on_task_spawn(mk_spawn_location_hook(
            "(current_thread) on_task_spawn",
            &spawns,
        ))
        .on_before_task_poll(mk_spawn_location_hook(
            "(current_thread) on_before_task_poll",
            &poll_starts,
        ))
        .on_after_task_poll(mk_spawn_location_hook(
            "(current_thread) on_after_task_poll",
            &poll_ends,
        ))
        .build()
        .unwrap();

    let task = runtime.spawn(async move { tokio::task::yield_now().await });
    runtime.block_on(async move {
        // Spawn tasks using both `runtime.spawn(...)` and `tokio::spawn(...)`
        // to ensure the correct location is captured in both code paths.
        task.await.unwrap();
        tokio::spawn(async move {}).await.unwrap();

        // tick the runtime a bunch to close out tasks
        for _ in 0..ITERATIONS {
            tokio::task::yield_now().await;
        }
    });

    assert_eq!(spawns.load(Ordering::SeqCst), 2);
    let poll_starts = poll_starts.load(Ordering::SeqCst);
    assert!(poll_starts > 2);
    assert_eq!(poll_starts, poll_ends.load(Ordering::SeqCst));
}

/// Test that the correct spawn location is provided to the task hooks on a
/// multi-thread runtime.
///
/// Testing this separately is necessary as the spawn code paths are different
/// and we should ensure that `#[track_caller]` is passed through correctly
/// for both runtimes.
#[cfg_attr(
    target_os = "wasi",
    ignore = "WASI does not support multi-threaded runtime"
)]
#[test]
fn task_hook_spawn_location_multi_thread() {
    let spawns = Arc::new(AtomicUsize::new(0));
    let poll_starts = Arc::new(AtomicUsize::new(0));
    let poll_ends = Arc::new(AtomicUsize::new(0));

    let runtime = Builder::new_multi_thread()
        .on_task_spawn(mk_spawn_location_hook(
            "(multi_thread) on_task_spawn",
            &spawns,
        ))
        .on_before_task_poll(mk_spawn_location_hook(
            "(multi_thread) on_before_task_poll",
            &poll_starts,
        ))
        .on_after_task_poll(mk_spawn_location_hook(
            "(multi_thread) on_after_task_poll",
            &poll_ends,
        ))
        .build()
        .unwrap();

    let task = runtime.spawn(async move { tokio::task::yield_now().await });
    runtime.block_on(async move {
        // Spawn tasks using both `runtime.spawn(...)` and `tokio::spawn(...)`
        // to ensure the correct location is captured in both code paths.
        task.await.unwrap();
        tokio::spawn(async move {}).await.unwrap();

        // tick the runtime a bunch to close out tasks
        for _ in 0..ITERATIONS {
            tokio::task::yield_now().await;
        }
    });

    // Give the runtime to shut down so that we see all the expected calls to
    // the task hooks.
    runtime.shutdown_timeout(std::time::Duration::from_secs(60));

    // Note: we "read" the counters using `fetch_add(0, SeqCst)` rather than
    // `load(SeqCst)` because read-write-modify operations are guaranteed to
    // observe the latest value, while the load is not.
    // This avoids a race that may cause test flakiness.
    assert_eq!(spawns.fetch_add(0, Ordering::SeqCst), 2);
    let poll_starts = poll_starts.fetch_add(0, Ordering::SeqCst);
    assert!(poll_starts > 2);
    assert_eq!(poll_starts, poll_ends.fetch_add(0, Ordering::SeqCst));
}

fn mk_spawn_location_hook(
    event: &'static str,
    count: &Arc<AtomicUsize>,
) -> impl Fn(&tokio::runtime::TaskMeta<'_>) {
    let count = Arc::clone(&count);
    move |data| {
        eprintln!("{event} ({:?}): {:?}", data.id(), data.spawned_at());
        // Assert that the spawn location is in this file.
        // Don't make assertions about line number/column here, as these
        // may change as new code is added to the test file...
        assert_eq!(
            data.spawned_at().file(),
            file!(),
            "incorrect spawn location in {event} hook",
        );
        count.fetch_add(1, Ordering::SeqCst);
    }
}
