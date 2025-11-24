//! Definition of the [`MaybeDone`] combinator.

use pin_project_lite::pin_project;
use std::future::{Future, IntoFuture};
use std::pin::Pin;
use std::task::{ready, Context, Poll};

pin_project! {
    /// A future that may have completed.
    #[derive(Debug)]
    #[project = MaybeDoneProj]
    #[project_replace = MaybeDoneProjReplace]
    #[repr(C)] // https://github.com/rust-lang/miri/issues/3780
    pub enum MaybeDone<Fut: Future> {
        /// A not-yet-completed future.
        Future { #[pin] future: Fut },
        /// The output of the completed future.
        Done { output: Fut::Output },
        /// The empty variant after the result of a [`MaybeDone`] has been
        /// taken using the [`take_output`](MaybeDone::take_output) method.
        Gone,
    }
}

/// Wraps a future into a `MaybeDone`.
pub fn maybe_done<F: IntoFuture>(future: F) -> MaybeDone<F::IntoFuture> {
    MaybeDone::Future {
        future: future.into_future(),
    }
}

impl<Fut: Future> MaybeDone<Fut> {
    /// Returns an [`Option`] containing a mutable reference to the output of the future.
    /// The output of this method will be [`Some`] if and only if the inner
    /// future has been completed and [`take_output`](MaybeDone::take_output)
    /// has not yet been called.
    pub fn output_mut(self: Pin<&mut Self>) -> Option<&mut Fut::Output> {
        match self.project() {
            MaybeDoneProj::Done { output } => Some(output),
            _ => None,
        }
    }

    /// Attempts to take the output of a `MaybeDone` without driving it
    /// towards completion.
    #[inline]
    pub fn take_output(self: Pin<&mut Self>) -> Option<Fut::Output> {
        match *self {
            MaybeDone::Done { .. } => {}
            MaybeDone::Future { .. } | MaybeDone::Gone => return None,
        };
        if let MaybeDoneProjReplace::Done { output } = self.project_replace(MaybeDone::Gone) {
            Some(output)
        } else {
            unreachable!()
        }
    }
}

impl<Fut: Future> Future for MaybeDone<Fut> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let output = match self.as_mut().project() {
            MaybeDoneProj::Future { future } => ready!(future.poll(cx)),
            MaybeDoneProj::Done { .. } => return Poll::Ready(()),
            MaybeDoneProj::Gone => panic!("MaybeDone polled after value taken"),
        };
        self.set(MaybeDone::Done { output });
        Poll::Ready(())
    }
}

// Test for https://github.com/tokio-rs/tokio/issues/6729
#[cfg(test)]
mod miri_tests {
    use super::maybe_done;

    use std::{
        future::Future,
        pin::Pin,
        sync::Arc,
        task::{Context, Poll, Wake},
    };

    struct ThingAdder<'a> {
        thing: &'a mut String,
    }

    impl Future for ThingAdder<'_> {
        type Output = ();

        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
            unsafe {
                *self.get_unchecked_mut().thing += ", world";
            }
            Poll::Pending
        }
    }

    #[test]
    fn maybe_done_miri() {
        let mut thing = "hello".to_owned();

        // The async block is necessary to trigger the miri failure.
        #[allow(clippy::redundant_async_block)]
        let fut = async move { ThingAdder { thing: &mut thing }.await };

        let mut fut = maybe_done(fut);
        let mut fut = unsafe { Pin::new_unchecked(&mut fut) };

        let waker = Arc::new(DummyWaker).into();
        let mut ctx = Context::from_waker(&waker);
        assert_eq!(fut.as_mut().poll(&mut ctx), Poll::Pending);
        assert_eq!(fut.as_mut().poll(&mut ctx), Poll::Pending);
    }

    struct DummyWaker;

    impl Wake for DummyWaker {
        fn wake(self: Arc<Self>) {}
    }
}
