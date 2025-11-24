//! Tests for time resource instrumentation.
//!
//! These tests ensure that the instrumentation for tokio
//! synchronization primitives is correct.
#![warn(rust_2018_idioms)]
#![cfg(all(tokio_unstable, feature = "tracing", target_has_atomic = "64"))]

use std::time::Duration;

use tracing_mock::{expect, subscriber};

#[tokio::test]
async fn test_sleep_creates_span() {
    let sleep_span_id = expect::id();
    let sleep_span = expect::span()
        .with_id(sleep_span_id.clone())
        .named("runtime.resource")
        .with_target("tokio::time::sleep");

    let state_update = expect::event()
        .with_target("runtime::resource::state_update")
        .with_fields(
            expect::field("duration")
                // FIXME(hds): This value isn't stable and doesn't seem to make sense. We're not
                //             going to test on it until the resource instrumentation has been
                //             refactored and we know that we're getting a stable value here.
                //.with_value(&(7_u64 + 1))
                .and(expect::field("duration.op").with_value(&"override")),
        );

    let async_op_span_id = expect::id();
    let async_op_span = expect::span()
        .with_id(async_op_span_id.clone())
        .named("runtime.resource.async_op")
        .with_target("tokio::time::sleep");

    let async_op_poll_span = expect::span()
        .named("runtime.resource.async_op.poll")
        .with_target("tokio::time::sleep");

    let (subscriber, handle) = subscriber::mock()
        .new_span(sleep_span.clone().with_ancestry(expect::is_explicit_root()))
        .enter(sleep_span.clone())
        .event(state_update)
        .new_span(
            async_op_span
                .clone()
                .with_ancestry(expect::has_contextual_parent(&sleep_span_id))
                .with_fields(expect::field("source").with_value(&"Sleep::new_timeout")),
        )
        .exit(sleep_span.clone())
        .enter(async_op_span.clone())
        .new_span(
            async_op_poll_span
                .clone()
                .with_ancestry(expect::has_contextual_parent(&async_op_span_id)),
        )
        .exit(async_op_span.clone())
        .drop_span(async_op_span)
        .drop_span(async_op_poll_span)
        .drop_span(sleep_span)
        .run_with_handle();

    {
        let _guard = tracing::subscriber::set_default(subscriber);

        _ = tokio::time::sleep(Duration::from_millis(7));
    }

    handle.assert_finished();
}
