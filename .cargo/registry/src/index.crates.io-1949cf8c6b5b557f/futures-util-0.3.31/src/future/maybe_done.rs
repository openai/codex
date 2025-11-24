//! Definition of the MaybeDone combinator

use super::assert_future;
use core::mem;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::ready;
use futures_core::task::{Context, Poll};

/// A future that may have completed.
///
/// This is created by the [`maybe_done()`] function.
#[derive(Debug)]
pub enum MaybeDone<Fut: Future> {
    /// A not-yet-completed future
    Future(/* #[pin] */ Fut),
    /// The output of the completed future
    Done(Fut::Output),
    /// The empty variant after the result of a [`MaybeDone`] has been
    /// taken using the [`take_output`](MaybeDone::take_output) method.
    Gone,
}

impl<Fut: Future + Unpin> Unpin for MaybeDone<Fut> {}

/// Wraps a future into a `MaybeDone`
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::future;
/// use futures::pin_mut;
///
/// let future = future::maybe_done(async { 5 });
/// pin_mut!(future);
/// assert_eq!(future.as_mut().take_output(), None);
/// let () = future.as_mut().await;
/// assert_eq!(future.as_mut().take_output(), Some(5));
/// assert_eq!(future.as_mut().take_output(), None);
/// # });
/// ```
pub fn maybe_done<Fut: Future>(future: Fut) -> MaybeDone<Fut> {
    assert_future::<(), _>(MaybeDone::Future(future))
}

impl<Fut: Future> MaybeDone<Fut> {
    /// Returns an [`Option`] containing a mutable reference to the output of the future.
    /// The output of this method will be [`Some`] if and only if the inner
    /// future has been completed and [`take_output`](MaybeDone::take_output)
    /// has not yet been called.
    #[inline]
    pub fn output_mut(self: Pin<&mut Self>) -> Option<&mut Fut::Output> {
        unsafe {
            match self.get_unchecked_mut() {
                Self::Done(res) => Some(res),
                _ => None,
            }
        }
    }

    /// Attempt to take the output of a `MaybeDone` without driving it
    /// towards completion.
    #[inline]
    pub fn take_output(self: Pin<&mut Self>) -> Option<Fut::Output> {
        match &*self {
            Self::Done(_) => {}
            Self::Future(_) | Self::Gone => return None,
        }
        unsafe {
            match mem::replace(self.get_unchecked_mut(), Self::Gone) {
                Self::Done(output) => Some(output),
                _ => unreachable!(),
            }
        }
    }
}

impl<Fut: Future> FusedFuture for MaybeDone<Fut> {
    fn is_terminated(&self) -> bool {
        match self {
            Self::Future(_) => false,
            Self::Done(_) | Self::Gone => true,
        }
    }
}

impl<Fut: Future> Future for MaybeDone<Fut> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe {
            match self.as_mut().get_unchecked_mut() {
                Self::Future(f) => {
                    let res = ready!(Pin::new_unchecked(f).poll(cx));
                    self.set(Self::Done(res));
                }
                Self::Done(_) => {}
                Self::Gone => panic!("MaybeDone polled after value taken"),
            }
        }
        Poll::Ready(())
    }
}
