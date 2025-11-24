use super::assert_future;
use core::pin::Pin;
use futures_core::task::{Context, Poll};
use futures_core::{FusedFuture, Future, Stream};
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`poll_immediate`](poll_immediate()) function.
    ///
    /// It will never return [Poll::Pending](core::task::Poll::Pending)
    #[derive(Debug, Clone)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct PollImmediate<T> {
        #[pin]
        future: Option<T>
    }
}

impl<T, F> Future for PollImmediate<F>
where
    F: Future<Output = T>,
{
    type Output = Option<T>;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        let mut this = self.project();
        let inner =
            this.future.as_mut().as_pin_mut().expect("PollImmediate polled after completion");
        match inner.poll(cx) {
            Poll::Ready(t) => {
                this.future.set(None);
                Poll::Ready(Some(t))
            }
            Poll::Pending => Poll::Ready(None),
        }
    }
}

impl<T: Future> FusedFuture for PollImmediate<T> {
    fn is_terminated(&self) -> bool {
        self.future.is_none()
    }
}

/// A [Stream](crate::stream::Stream) implementation that can be polled repeatedly until the future is done.
/// The stream will never return [Poll::Pending](core::task::Poll::Pending)
/// so polling it in a tight loop is worse than using a blocking synchronous function.
/// ```
/// # futures::executor::block_on(async {
/// use futures::task::Poll;
/// use futures::{StreamExt, future, pin_mut};
/// use future::FusedFuture;
///
/// let f = async { 1_u32 };
/// pin_mut!(f);
/// let mut r = future::poll_immediate(f);
/// assert_eq!(r.next().await, Some(Poll::Ready(1)));
///
/// let f = async {futures::pending!(); 42_u8};
/// pin_mut!(f);
/// let mut p = future::poll_immediate(f);
/// assert_eq!(p.next().await, Some(Poll::Pending));
/// assert!(!p.is_terminated());
/// assert_eq!(p.next().await, Some(Poll::Ready(42)));
/// assert!(p.is_terminated());
/// assert_eq!(p.next().await, None);
/// # });
/// ```
impl<T, F> Stream for PollImmediate<F>
where
    F: Future<Output = T>,
{
    type Item = Poll<T>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        match this.future.as_mut().as_pin_mut() {
            // inner is gone, so we can signal that the stream is closed.
            None => Poll::Ready(None),
            Some(fut) => Poll::Ready(Some(fut.poll(cx).map(|t| {
                this.future.set(None);
                t
            }))),
        }
    }
}

/// Creates a future that is immediately ready with an Option of a value.
/// Specifically this means that [poll](core::future::Future::poll()) always returns [Poll::Ready](core::task::Poll::Ready).
///
/// # Caution
///
/// When consuming the future by this function, note the following:
///
/// - This function does not guarantee that the future will run to completion, so it is generally incompatible with passing the non-cancellation-safe future by value.
/// - Even if the future is cancellation-safe, creating and dropping new futures frequently may lead to performance problems.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::future;
///
/// let r = future::poll_immediate(async { 1_u32 });
/// assert_eq!(r.await, Some(1));
///
/// let p = future::poll_immediate(future::pending::<i32>());
/// assert_eq!(p.await, None);
/// # });
/// ```
///
/// ### Reusing a future
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::{future, pin_mut};
/// let f = async {futures::pending!(); 42_u8};
/// pin_mut!(f);
/// assert_eq!(None, future::poll_immediate(&mut f).await);
/// assert_eq!(42, f.await);
/// # });
/// ```
pub fn poll_immediate<F: Future>(f: F) -> PollImmediate<F> {
    assert_future::<Option<F::Output>, PollImmediate<F>>(PollImmediate { future: Some(f) })
}
