use super::assert_future;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll};

/// Future for the [`lazy`] function.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Lazy<F> {
    f: Option<F>,
}

// safe because we never generate `Pin<&mut F>`
impl<F> Unpin for Lazy<F> {}

/// Creates a new future that allows delayed execution of a closure.
///
/// The provided closure is only run once the future is polled.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::future;
///
/// let a = future::lazy(|_| 1);
/// assert_eq!(a.await, 1);
///
/// let b = future::lazy(|_| -> i32 {
///     panic!("oh no!")
/// });
/// drop(b); // closure is never run
/// # });
/// ```
pub fn lazy<F, R>(f: F) -> Lazy<F>
where
    F: FnOnce(&mut Context<'_>) -> R,
{
    assert_future::<R, _>(Lazy { f: Some(f) })
}

impl<F, R> FusedFuture for Lazy<F>
where
    F: FnOnce(&mut Context<'_>) -> R,
{
    fn is_terminated(&self) -> bool {
        self.f.is_none()
    }
}

impl<F, R> Future for Lazy<F>
where
    F: FnOnce(&mut Context<'_>) -> R,
{
    type Output = R;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<R> {
        Poll::Ready((self.f.take().expect("Lazy polled after completion"))(cx))
    }
}
