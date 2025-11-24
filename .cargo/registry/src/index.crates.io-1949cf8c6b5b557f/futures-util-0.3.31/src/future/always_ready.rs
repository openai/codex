use super::assert_future;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll};

/// Future for the [`always_ready`](always_ready()) function.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct AlwaysReady<T, F: Fn() -> T>(F);

impl<T, F: Fn() -> T> core::fmt::Debug for AlwaysReady<T, F> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("AlwaysReady").finish()
    }
}

impl<T, F: Fn() -> T + Clone> Clone for AlwaysReady<T, F> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T, F: Fn() -> T + Copy> Copy for AlwaysReady<T, F> {}

impl<T, F: Fn() -> T> Unpin for AlwaysReady<T, F> {}

impl<T, F: Fn() -> T> FusedFuture for AlwaysReady<T, F> {
    fn is_terminated(&self) -> bool {
        false
    }
}

impl<T, F: Fn() -> T> Future for AlwaysReady<T, F> {
    type Output = T;

    #[inline]
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<T> {
        Poll::Ready(self.0())
    }
}

/// Creates a future that is always immediately ready with a value.
///
/// This is particularly useful in avoiding a heap allocation when an API needs [`Box<dyn Future<Output = T>>`],
/// as [`AlwaysReady`] does not have to store a boolean for `is_finished`.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use std::mem::size_of_val;
///
/// use futures::future;
///
/// let a = future::always_ready(|| 1);
/// assert_eq!(size_of_val(&a), 0);
/// assert_eq!(a.await, 1);
/// assert_eq!(a.await, 1);
/// # });
/// ```
pub fn always_ready<T, F: Fn() -> T>(prod: F) -> AlwaysReady<T, F> {
    assert_future::<T, _>(AlwaysReady(prod))
}
