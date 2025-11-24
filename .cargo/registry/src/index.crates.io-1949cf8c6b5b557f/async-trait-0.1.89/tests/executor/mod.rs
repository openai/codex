use std::future::Future;
use std::pin::Pin;
use std::ptr;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

// Executor for a future that resolves immediately (test only).
#[allow(clippy::missing_panics_doc)]
pub fn block_on_simple<F: Future>(mut fut: F) -> F::Output {
    unsafe fn clone(_null: *const ()) -> RawWaker {
        unimplemented!()
    }

    unsafe fn wake(_null: *const ()) {
        unimplemented!()
    }

    unsafe fn wake_by_ref(_null: *const ()) {
        unimplemented!()
    }

    unsafe fn drop(_null: *const ()) {}

    let data = ptr::null();
    let vtable = &RawWakerVTable::new(clone, wake, wake_by_ref, drop);
    let raw_waker = RawWaker::new(data, vtable);
    let waker = unsafe { Waker::from_raw(raw_waker) };
    let mut cx = Context::from_waker(&waker);

    // fut does not move until it gets dropped.
    let fut = unsafe { Pin::new_unchecked(&mut fut) };

    match fut.poll(&mut cx) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("future did not resolve immediately"),
    }
}
