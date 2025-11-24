use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use pin_project_lite::pin_project;

use crate::sync::{CancellationToken, RunUntilCancelledFuture, RunUntilCancelledFutureOwned};

pin_project! {
    /// A [`Future`] that is resolved once the corresponding [`CancellationToken`]
    /// is cancelled or a given [`Future`] gets resolved.
    ///
    /// This future is immediately resolved if the corresponding [`CancellationToken`]
    /// is already cancelled, otherwise, in case of concurrent completion and
    /// cancellation, this is biased towards the future completion.
    #[must_use = "futures do nothing unless polled"]
    pub struct WithCancellationTokenFuture<'a, F: Future> {
        #[pin]
        run_until_cancelled: Option<RunUntilCancelledFuture<'a, F>>
    }
}

impl<'a, F: Future> WithCancellationTokenFuture<'a, F> {
    pub(crate) fn new(cancellation_token: &'a CancellationToken, future: F) -> Self {
        Self {
            run_until_cancelled: (!cancellation_token.is_cancelled())
                .then(|| RunUntilCancelledFuture::new(cancellation_token, future)),
        }
    }
}

impl<'a, F: Future> Future for WithCancellationTokenFuture<'a, F> {
    type Output = Option<F::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        match this.run_until_cancelled.as_pin_mut() {
            Some(fut) => fut.poll(cx),
            None => Poll::Ready(None),
        }
    }
}

pin_project! {
    /// A [`Future`] that is resolved once the corresponding [`CancellationToken`]
    /// is cancelled or a given [`Future`] gets resolved.
    ///
    /// This future is immediately resolved if the corresponding [`CancellationToken`]
    /// is already cancelled, otherwise, in case of concurrent completion and
    /// cancellation, this is biased towards the future completion.
    #[must_use = "futures do nothing unless polled"]
    pub struct WithCancellationTokenFutureOwned<F: Future> {
        #[pin]
        run_until_cancelled: Option<RunUntilCancelledFutureOwned<F>>
    }
}

impl<F: Future> WithCancellationTokenFutureOwned<F> {
    pub(crate) fn new(cancellation_token: CancellationToken, future: F) -> Self {
        Self {
            run_until_cancelled: (!cancellation_token.is_cancelled())
                .then(|| RunUntilCancelledFutureOwned::new(cancellation_token, future)),
        }
    }
}

impl<F: Future> Future for WithCancellationTokenFutureOwned<F> {
    type Output = Option<F::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        match this.run_until_cancelled.as_pin_mut() {
            Some(fut) => fut.poll(cx),
            None => Poll::Ready(None),
        }
    }
}
