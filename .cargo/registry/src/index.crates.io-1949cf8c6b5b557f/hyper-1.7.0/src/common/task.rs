use std::task::{Context, Poll};
#[cfg(feature = "client")]
use std::task::{RawWaker, RawWakerVTable, Waker};

/// A function to help "yield" a future, such that it is re-scheduled immediately.
///
/// Useful for spin counts, so a future doesn't hog too much time.
pub(crate) fn yield_now(cx: &mut Context<'_>) -> Poll<std::convert::Infallible> {
    cx.waker().wake_by_ref();
    Poll::Pending
}

// TODO: replace with `std::task::Waker::noop()` once MSRV >= 1.85
#[cfg(feature = "client")]
fn noop_waker() -> Waker {
    const NOOP_RAW_WAKER: RawWaker = RawWaker::new(std::ptr::null(), &NOOP_VTABLE);
    const NOOP_VTABLE: RawWakerVTable = RawWakerVTable::new(
        // `clone` returns the same noop waker again
        |_: *const ()| NOOP_RAW_WAKER,
        // `wake`, `wake_by_ref`, and `drop` do nothing
        |_: *const ()| {},
        |_: *const ()| {},
        |_: *const ()| {},
    );

    // SAFETY: all functions in the vtable are safe to call, and Waker's safety does not require
    // them to actually do anything.
    unsafe { Waker::from_raw(NOOP_RAW_WAKER) }
}

/// Poll the future once and return `Some` if it is ready, else `None`.
///
/// If the future wasn't ready, it future likely can't be driven to completion any more: the polling
/// uses a no-op waker, so knowledge of what the pending future was waiting for is lost.
#[cfg(feature = "client")]
pub(crate) fn now_or_never<F: std::future::Future>(fut: F) -> Option<F::Output> {
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    // TODO: replace with std::pin::pin! and drop pin-utils once MSRV >= 1.68
    pin_utils::pin_mut!(fut);
    match fut.poll(&mut cx) {
        Poll::Ready(res) => Some(res),
        Poll::Pending => None,
    }
}
