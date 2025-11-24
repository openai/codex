mod common;

use common::*;
use tracing_core::dispatcher::*;
#[test]
fn global_dispatch() {
    set_global_default(Dispatch::new(TestSubscriberA)).expect("global dispatch set failed");
    get_default(|current| {
        assert!(
            current.is::<TestSubscriberA>(),
            "global dispatch get failed"
        )
    });

    #[cfg(feature = "std")]
    with_default(&Dispatch::new(TestSubscriberB), || {
        get_default(|current| {
            assert!(
                current.is::<TestSubscriberB>(),
                "thread-local override of global dispatch failed"
            )
        });
    });

    get_default(|current| {
        assert!(
            current.is::<TestSubscriberA>(),
            "reset to global override failed"
        )
    });

    set_global_default(Dispatch::new(TestSubscriberA))
        .expect_err("double global dispatch set succeeded");
}
