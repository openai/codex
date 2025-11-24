//! Definition of the TryMaybeDone combinator

use super::assert_future;
use core::mem;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future, TryFuture};
use futures_core::ready;
use futures_core::task::{Context, Poll};

/// A future that may have completed with an error.
///
/// This is created by the [`try_maybe_done()`] function.
#[derive(Debug)]
pub enum TryMaybeDone<Fut: TryFuture> {
    /// A not-yet-completed future
    Future(/* #[pin] */ Fut),
    /// The output of the completed future
    Done(Fut::Ok),
    /// The empty variant after the result of a [`TryMaybeDone`] has been
    /// taken using the [`take_output`](TryMaybeDone::take_output) method,
    /// or if the future returned an error.
    Gone,
}

impl<Fut: TryFuture + Unpin> Unpin for TryMaybeDone<Fut> {}

/// Wraps a future into a `TryMaybeDone`
pub fn try_maybe_done<Fut: TryFuture>(future: Fut) -> TryMaybeDone<Fut> {
    assert_future::<Result<(), Fut::Error>, _>(TryMaybeDone::Future(future))
}

impl<Fut: TryFuture> TryMaybeDone<Fut> {
    /// Returns an [`Option`] containing a mutable reference to the output of the future.
    /// The output of this method will be [`Some`] if and only if the inner
    /// future has completed successfully and [`take_output`](TryMaybeDone::take_output)
    /// has not yet been called.
    #[inline]
    pub fn output_mut(self: Pin<&mut Self>) -> Option<&mut Fut::Ok> {
        unsafe {
            match self.get_unchecked_mut() {
                Self::Done(res) => Some(res),
                _ => None,
            }
        }
    }

    /// Attempt to take the output of a `TryMaybeDone` without driving it
    /// towards completion.
    #[inline]
    pub fn take_output(self: Pin<&mut Self>) -> Option<Fut::Ok> {
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

impl<Fut: TryFuture> FusedFuture for TryMaybeDone<Fut> {
    fn is_terminated(&self) -> bool {
        match self {
            Self::Future(_) => false,
            Self::Done(_) | Self::Gone => true,
        }
    }
}

impl<Fut: TryFuture> Future for TryMaybeDone<Fut> {
    type Output = Result<(), Fut::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe {
            match self.as_mut().get_unchecked_mut() {
                Self::Future(f) => match ready!(Pin::new_unchecked(f).try_poll(cx)) {
                    Ok(res) => self.set(Self::Done(res)),
                    Err(e) => {
                        self.set(Self::Gone);
                        return Poll::Ready(Err(e));
                    }
                },
                Self::Done(_) => {}
                Self::Gone => panic!("TryMaybeDone polled after value taken"),
            }
        }
        Poll::Ready(Ok(()))
    }
}
