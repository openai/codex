use futures::future::{self, FutureExt};
use futures::task::Context;
use futures_test::task::panic_waker;

#[test]
fn fuse() {
    let mut future = future::ready::<i32>(2).fuse();
    let waker = panic_waker();
    let mut cx = Context::from_waker(&waker);
    assert!(future.poll_unpin(&mut cx).is_ready());
    assert!(future.poll_unpin(&mut cx).is_pending());
}
