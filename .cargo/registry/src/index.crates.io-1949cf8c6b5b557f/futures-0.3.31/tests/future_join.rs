use futures::executor::block_on;
use futures::future::{self, Future};
use std::task::Poll;

/// This tests verifies (through miri) that self-referencing
/// futures are not invalidated when joining them.
#[test]
fn futures_join_macro_self_referential() {
    block_on(async { futures::join!(yield_now(), trouble()) });
}

async fn trouble() {
    let lucky_number = 42;
    let problematic_variable = &lucky_number;

    yield_now().await;

    // problematic dereference
    let _ = { *problematic_variable };
}

fn yield_now() -> impl Future<Output = ()> {
    let mut yielded = false;
    future::poll_fn(move |cx| {
        if core::mem::replace(&mut yielded, true) {
            Poll::Ready(())
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    })
}
