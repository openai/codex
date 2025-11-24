#![cfg(feature = "registry")]
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing_core::{
    span::{Attributes, Id, Record},
    subscriber::Interest,
    Event, LevelFilter, Metadata, Subscriber,
};
use tracing_subscriber::{layer, prelude::*, reload::*};

pub struct NopSubscriber;
fn event() {
    tracing::info!("my event");
}

impl Subscriber for NopSubscriber {
    fn register_callsite(&self, _: &'static Metadata<'static>) -> Interest {
        Interest::never()
    }

    fn enabled(&self, _: &Metadata<'_>) -> bool {
        false
    }

    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _: &Id, _: &Record<'_>) {}
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn event(&self, _: &Event<'_>) {}
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}
}

/// Running these two tests in parallel will cause flaky failures, since they are both modifying the MAX_LEVEL value.
/// "cargo test -- --test-threads=1 fixes it, but it runs all tests in serial.
/// The only way to run tests in serial in a single file is this way.
#[test]
fn run_all_reload_test() {
    reload_handle();
    reload_filter();
}

fn reload_handle() {
    static FILTER1_CALLS: AtomicUsize = AtomicUsize::new(0);
    static FILTER2_CALLS: AtomicUsize = AtomicUsize::new(0);

    enum Filter {
        One,
        Two,
    }

    impl<S: Subscriber> tracing_subscriber::Layer<S> for Filter {
        fn register_callsite(&self, m: &Metadata<'_>) -> Interest {
            println!("REGISTER: {:?}", m);
            Interest::sometimes()
        }

        fn enabled(&self, m: &Metadata<'_>, _: layer::Context<'_, S>) -> bool {
            println!("ENABLED: {:?}", m);
            match self {
                Filter::One => FILTER1_CALLS.fetch_add(1, Ordering::SeqCst),
                Filter::Two => FILTER2_CALLS.fetch_add(1, Ordering::SeqCst),
            };
            true
        }

        fn max_level_hint(&self) -> Option<LevelFilter> {
            match self {
                Filter::One => Some(LevelFilter::INFO),
                Filter::Two => Some(LevelFilter::DEBUG),
            }
        }
    }

    let (layer, handle) = Layer::new(Filter::One);

    let subscriber = tracing_core::dispatcher::Dispatch::new(layer.with_subscriber(NopSubscriber));

    tracing_core::dispatcher::with_default(&subscriber, || {
        assert_eq!(FILTER1_CALLS.load(Ordering::SeqCst), 0);
        assert_eq!(FILTER2_CALLS.load(Ordering::SeqCst), 0);

        event();

        assert_eq!(FILTER1_CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(FILTER2_CALLS.load(Ordering::SeqCst), 0);

        assert_eq!(LevelFilter::current(), LevelFilter::INFO);
        handle.reload(Filter::Two).expect("should reload");
        assert_eq!(LevelFilter::current(), LevelFilter::DEBUG);

        event();

        assert_eq!(FILTER1_CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(FILTER2_CALLS.load(Ordering::SeqCst), 1);
    })
}

fn reload_filter() {
    struct NopLayer;
    impl<S: Subscriber> tracing_subscriber::Layer<S> for NopLayer {
        fn register_callsite(&self, _m: &Metadata<'_>) -> Interest {
            Interest::sometimes()
        }

        fn enabled(&self, _m: &Metadata<'_>, _: layer::Context<'_, S>) -> bool {
            true
        }
    }

    static FILTER1_CALLS: AtomicUsize = AtomicUsize::new(0);
    static FILTER2_CALLS: AtomicUsize = AtomicUsize::new(0);

    enum Filter {
        One,
        Two,
    }

    impl<S: Subscriber> tracing_subscriber::layer::Filter<S> for Filter {
        fn enabled(&self, m: &Metadata<'_>, _: &layer::Context<'_, S>) -> bool {
            println!("ENABLED: {:?}", m);
            match self {
                Filter::One => FILTER1_CALLS.fetch_add(1, Ordering::SeqCst),
                Filter::Two => FILTER2_CALLS.fetch_add(1, Ordering::SeqCst),
            };
            true
        }

        fn max_level_hint(&self) -> Option<LevelFilter> {
            match self {
                Filter::One => Some(LevelFilter::INFO),
                Filter::Two => Some(LevelFilter::DEBUG),
            }
        }
    }

    let (filter, handle) = Layer::new(Filter::One);

    let dispatcher = tracing_core::Dispatch::new(
        tracing_subscriber::registry().with(NopLayer.with_filter(filter)),
    );

    tracing_core::dispatcher::with_default(&dispatcher, || {
        assert_eq!(FILTER1_CALLS.load(Ordering::SeqCst), 0);
        assert_eq!(FILTER2_CALLS.load(Ordering::SeqCst), 0);

        event();

        assert_eq!(FILTER1_CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(FILTER2_CALLS.load(Ordering::SeqCst), 0);

        assert_eq!(LevelFilter::current(), LevelFilter::INFO);
        handle.reload(Filter::Two).expect("should reload");
        assert_eq!(LevelFilter::current(), LevelFilter::DEBUG);

        event();

        assert_eq!(FILTER1_CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(FILTER2_CALLS.load(Ordering::SeqCst), 1);
    })
}
