mod common;

use common::*;
use tracing_core::{
    dispatcher::{self, Dispatch},
    subscriber::NoSubscriber,
};

/// This test reproduces the following issues:
/// - https://github.com/tokio-rs/tracing/issues/2587
/// - https://github.com/tokio-rs/tracing/issues/2411
/// - https://github.com/tokio-rs/tracing/issues/2436
#[test]
fn local_dispatch_before_init() {
    dispatcher::get_default(|current| assert!(dbg!(current).is::<NoSubscriber>()));

    // Temporarily override the default dispatcher with a scoped dispatcher.
    // Using a scoped dispatcher makes the thread local state attempt to cache
    // the scoped default.
    #[cfg(feature = "std")]
    {
        dispatcher::with_default(&Dispatch::new(TestSubscriberB), || {
            dispatcher::get_default(|current| {
                assert!(
                    dbg!(current).is::<TestSubscriberB>(),
                    "overriden subscriber not set",
                );
            })
        })
    }

    dispatcher::get_default(|current| assert!(current.is::<NoSubscriber>()));

    dispatcher::set_global_default(Dispatch::new(TestSubscriberA))
        .expect("set global dispatch failed");

    dispatcher::get_default(|current| {
        assert!(
            dbg!(current).is::<TestSubscriberA>(),
            "default subscriber not set"
        );
    });
}
