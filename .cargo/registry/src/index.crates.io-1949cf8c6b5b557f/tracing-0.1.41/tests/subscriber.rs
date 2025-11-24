// These tests require the thread-local scoped dispatcher, which only works when
// we have a standard library. The behaviour being tested should be the same
// with the standard lib disabled.
//
// The alternative would be for each of these tests to be defined in a separate
// file, which is :(
#![cfg(feature = "std")]
use tracing::{
    field::display,
    span::{Attributes, Id, Record},
    subscriber::{with_default, Interest, Subscriber},
    Event, Level, Metadata,
};
use tracing_mock::{expect, subscriber};

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[test]
fn event_macros_dont_infinite_loop() {
    // This test ensures that an event macro within a subscriber
    // won't cause an infinite loop of events.
    struct TestSubscriber;
    impl Subscriber for TestSubscriber {
        fn register_callsite(&self, _: &Metadata<'_>) -> Interest {
            // Always return sometimes so that `enabled` will be called
            // (which can loop).
            Interest::sometimes()
        }

        fn enabled(&self, meta: &Metadata<'_>) -> bool {
            assert!(meta.fields().iter().any(|f| f.name() == "foo"));
            tracing::event!(Level::TRACE, bar = false);
            true
        }

        fn new_span(&self, _: &Attributes<'_>) -> Id {
            Id::from_u64(0xAAAA)
        }

        fn record(&self, _: &Id, _: &Record<'_>) {}

        fn record_follows_from(&self, _: &Id, _: &Id) {}

        fn event(&self, event: &Event<'_>) {
            assert!(event.metadata().fields().iter().any(|f| f.name() == "foo"));
            tracing::event!(Level::TRACE, baz = false);
        }

        fn enter(&self, _: &Id) {}

        fn exit(&self, _: &Id) {}
    }

    with_default(TestSubscriber, || {
        tracing::event!(Level::TRACE, foo = false);
    })
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[test]
fn boxed_subscriber() {
    let (subscriber, handle) = subscriber::mock()
        .new_span(
            expect::span().named("foo").with_fields(
                expect::field("bar")
                    .with_value(&display("hello from my span"))
                    .only(),
            ),
        )
        .enter(expect::span().named("foo"))
        .exit(expect::span().named("foo"))
        .drop_span(expect::span().named("foo"))
        .only()
        .run_with_handle();
    let subscriber: Box<dyn Subscriber + Send + Sync + 'static> = Box::new(subscriber);

    with_default(subscriber, || {
        let from = "my span";
        let span = tracing::span!(
            Level::TRACE,
            "foo",
            bar = format_args!("hello from {}", from)
        );
        span.in_scope(|| {});
    });

    handle.assert_finished();
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[test]
fn arced_subscriber() {
    use std::sync::Arc;

    let (subscriber, handle) = subscriber::mock()
        .new_span(
            expect::span().named("foo").with_fields(
                expect::field("bar")
                    .with_value(&display("hello from my span"))
                    .only(),
            ),
        )
        .enter(expect::span().named("foo"))
        .exit(expect::span().named("foo"))
        .drop_span(expect::span().named("foo"))
        .event(
            expect::event()
                .with_fields(expect::field("message").with_value(&display("hello from my event"))),
        )
        .only()
        .run_with_handle();
    let subscriber: Arc<dyn Subscriber + Send + Sync + 'static> = Arc::new(subscriber);

    // Test using a clone of the `Arc`ed subscriber
    with_default(subscriber.clone(), || {
        let from = "my span";
        let span = tracing::span!(
            Level::TRACE,
            "foo",
            bar = format_args!("hello from {}", from)
        );
        span.in_scope(|| {});
    });

    with_default(subscriber, || {
        tracing::info!("hello from my event");
    });

    handle.assert_finished();
}
