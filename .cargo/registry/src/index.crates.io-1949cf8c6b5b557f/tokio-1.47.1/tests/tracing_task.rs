//! Tests for task instrumentation.
//!
//! These tests ensure that the instrumentation for task spawning and task
//! lifecycles is correct.
#![warn(rust_2018_idioms)]
#![cfg(all(tokio_unstable, feature = "tracing", target_has_atomic = "64"))]

use std::{mem, time::Duration};

use tokio::task;
use tracing_mock::{expect, span::NewSpan, subscriber};

#[tokio::test]
async fn task_spawn_creates_span() {
    let task_span = expect::span()
        .named("runtime.spawn")
        .with_target("tokio::task");

    let (subscriber, handle) = subscriber::mock()
        .new_span(&task_span)
        .enter(&task_span)
        .exit(&task_span)
        // The task span is entered once more when it gets dropped
        .enter(&task_span)
        .exit(&task_span)
        .drop_span(task_span)
        .run_with_handle();

    {
        let _guard = tracing::subscriber::set_default(subscriber);
        tokio::spawn(futures::future::ready(()))
            .await
            .expect("failed to await join handle");
    }

    handle.assert_finished();
}

#[tokio::test]
async fn task_spawn_loc_file_recorded() {
    let task_span = expect::span()
        .named("runtime.spawn")
        .with_target("tokio::task")
        .with_fields(expect::field("loc.file").with_value(&file!()));

    let (subscriber, handle) = subscriber::mock().new_span(task_span).run_with_handle();

    {
        let _guard = tracing::subscriber::set_default(subscriber);

        tokio::spawn(futures::future::ready(()))
            .await
            .expect("failed to await join handle");
    }

    handle.assert_finished();
}

#[tokio::test]
async fn task_builder_name_recorded() {
    let task_span = expect_task_named("test-task");

    let (subscriber, handle) = subscriber::mock().new_span(task_span).run_with_handle();

    {
        let _guard = tracing::subscriber::set_default(subscriber);
        task::Builder::new()
            .name("test-task")
            .spawn(futures::future::ready(()))
            .unwrap()
            .await
            .expect("failed to await join handle");
    }

    handle.assert_finished();
}

#[tokio::test]
async fn task_builder_loc_file_recorded() {
    let task_span = expect::span()
        .named("runtime.spawn")
        .with_target("tokio::task")
        .with_fields(expect::field("loc.file").with_value(&file!()));

    let (subscriber, handle) = subscriber::mock().new_span(task_span).run_with_handle();

    {
        let _guard = tracing::subscriber::set_default(subscriber);

        task::Builder::new()
            .spawn(futures::future::ready(()))
            .unwrap()
            .await
            .expect("failed to await join handle");
    }

    handle.assert_finished();
}

#[tokio::test]
async fn task_spawn_sizes_recorded() {
    let future = futures::future::ready(());
    let size = mem::size_of_val(&future) as u64;

    let task_span = expect::span()
        .named("runtime.spawn")
        .with_target("tokio::task")
        // TODO(hds): check that original_size.bytes is NOT recorded when this can be done in
        // tracing-mock without listing every other field.
        .with_fields(expect::field("size.bytes").with_value(&size));

    let (subscriber, handle) = subscriber::mock().new_span(task_span).run_with_handle();

    {
        let _guard = tracing::subscriber::set_default(subscriber);

        task::Builder::new()
            .spawn(future)
            .unwrap()
            .await
            .expect("failed to await join handle");
    }

    handle.assert_finished();
}

#[tokio::test]
async fn task_big_spawn_sizes_recorded() {
    let future = {
        async fn big<const N: usize>() {
            let mut a = [0_u8; N];
            for (idx, item) in a.iter_mut().enumerate() {
                *item = (idx % 256) as u8;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
            for (idx, item) in a.iter_mut().enumerate() {
                assert_eq!(*item, (idx % 256) as u8);
            }
        }

        // This is larger than the release auto-boxing threshold
        big::<20_000>()
    };

    fn boxed_size<T>(_: &T) -> usize {
        mem::size_of::<Box<T>>()
    }
    let size = mem::size_of_val(&future) as u64;
    let boxed_size = boxed_size(&future);

    let task_span = expect::span()
        .named("runtime.spawn")
        .with_target("tokio::task")
        .with_fields(
            expect::field("size.bytes")
                .with_value(&boxed_size)
                .and(expect::field("original_size.bytes").with_value(&size)),
        );

    let (subscriber, handle) = subscriber::mock().new_span(task_span).run_with_handle();

    {
        let _guard = tracing::subscriber::set_default(subscriber);

        task::Builder::new()
            .spawn(future)
            .unwrap()
            .await
            .expect("failed to await join handle");
    }

    handle.assert_finished();
}

/// Expect a task with name
///
/// This is a convenience function to create the expectation for a new task
/// with the `task.name` field set to the provided name.
fn expect_task_named(name: &str) -> NewSpan {
    expect::span()
        .named("runtime.spawn")
        .with_target("tokio::task")
        .with_fields(
            expect::field("task.name").with_value(&tracing::field::debug(format_args!("{}", name))),
        )
}
