use super::assert_future;
use core::marker;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll};

/// Future for the [`pending()`] function.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Pending<T> {
    _data: marker::PhantomData<T>,
}

impl<T> FusedFuture for Pending<T> {
    fn is_terminated(&self) -> bool {
        true
    }
}

/// Creates a future which never resolves, representing a computation that never
/// finishes.
///
/// The returned future will forever return [`Poll::Pending`].
///
/// # Examples
///
/// ```ignore
/// # futures::executor::block_on(async {
/// use futures::future;
///
/// let future = future::pending();
/// let () = future.await;
/// unreachable!();
/// # });
/// ```
#[cfg_attr(docsrs, doc(alias = "never"))]
pub fn pending<T>() -> Pending<T> {
    assert_future::<T, _>(Pending { _data: marker::PhantomData })
}

impl<T> Future for Pending<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<T> {
        Poll::Pending
    }
}

impl<T> Unpin for Pending<T> {}

impl<T> Clone for Pending<T> {
    fn clone(&self) -> Self {
        pending()
    }
}
