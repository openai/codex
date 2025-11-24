use super::assert_future;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll};

/// Future for the [`ready`](ready()) function.
#[derive(Debug, Clone)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Ready<T>(Option<T>);

impl<T> Ready<T> {
    /// Unwraps the value from this immediately ready future.
    #[inline]
    pub fn into_inner(mut self) -> T {
        self.0.take().unwrap()
    }
}

impl<T> Unpin for Ready<T> {}

impl<T> FusedFuture for Ready<T> {
    fn is_terminated(&self) -> bool {
        self.0.is_none()
    }
}

impl<T> Future for Ready<T> {
    type Output = T;

    #[inline]
    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<T> {
        Poll::Ready(self.0.take().expect("Ready polled after completion"))
    }
}

/// Creates a future that is immediately ready with a value.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::future;
///
/// let a = future::ready(1);
/// assert_eq!(a.await, 1);
/// # });
/// ```
pub fn ready<T>(t: T) -> Ready<T> {
    assert_future::<T, _>(Ready(Some(t)))
}

/// Create a future that is immediately ready with a success value.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::future;
///
/// let a = future::ok::<i32, i32>(1);
/// assert_eq!(a.await, Ok(1));
/// # });
/// ```
pub fn ok<T, E>(t: T) -> Ready<Result<T, E>> {
    Ready(Some(Ok(t)))
}

/// Create a future that is immediately ready with an error value.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::future;
///
/// let a = future::err::<i32, i32>(1);
/// assert_eq!(a.await, Err(1));
/// # });
/// ```
pub fn err<T, E>(err: E) -> Ready<Result<T, E>> {
    Ready(Some(Err(err)))
}
