//! Application-specific request completion semantics.

use futures_core::ready;
use pin_project_lite::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

/// Attaches `H`-typed completion tracker to `V` typed values.
///
/// Handles (of type `H`) are intended to be RAII guards that primarily implement [`Drop`] and update
/// load metric state as they are dropped. This trait allows implementors to "forward" the handle
/// to later parts of the request-handling pipeline, so that the handle is only dropped when the
/// request has truly completed.
///
/// This utility allows load metrics to have a protocol-agnostic means to track streams past their
/// initial response future. For example, if `V` represents an HTTP response type, an
/// implementation could add `H`-typed handles to each response's extensions to detect when all the
/// response's extensions have been dropped.
///
/// A base `impl<H, V> TrackCompletion<H, V> for CompleteOnResponse` is provided to drop the handle
/// once the response future is resolved. This is appropriate when a response is discrete and
/// cannot comprise multiple messages.
///
/// In many cases, the `Output` type is simply `V`. However, [`TrackCompletion`] may alter the type
/// in order to instrument it appropriately. For example, an HTTP [`TrackCompletion`] may modify
/// the body type: so a [`TrackCompletion`] that takes values of type
/// [`http::Response<A>`][response] may output values of type [`http::Response<B>`][response].
///
/// [response]: https://docs.rs/http/latest/http/response/struct.Response.html
pub trait TrackCompletion<H, V>: Clone {
    /// The instrumented value type.
    type Output;

    /// Attaches a `H`-typed handle to a `V`-typed value.
    fn track_completion(&self, handle: H, value: V) -> Self::Output;
}

/// A [`TrackCompletion`] implementation that considers the request completed when the response
/// future is resolved.
#[derive(Clone, Copy, Debug, Default)]
#[non_exhaustive]
pub struct CompleteOnResponse;

pin_project! {
    /// Attaches a `C`-typed completion tracker to the result of an `F`-typed [`Future`].
    #[derive(Debug)]
    pub struct TrackCompletionFuture<F, C, H> {
        #[pin]
        future: F,
        handle: Option<H>,
        completion: C,
    }
}

// ===== impl InstrumentFuture =====

impl<F, C, H> TrackCompletionFuture<F, C, H> {
    /// Wraps a future, propagating the tracker into its value if successful.
    pub const fn new(completion: C, handle: H, future: F) -> Self {
        TrackCompletionFuture {
            future,
            completion,
            handle: Some(handle),
        }
    }
}

impl<F, C, H, T, E> Future for TrackCompletionFuture<F, C, H>
where
    F: Future<Output = Result<T, E>>,
    C: TrackCompletion<H, T>,
{
    type Output = Result<C::Output, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let rsp = ready!(this.future.poll(cx))?;
        let h = this.handle.take().expect("handle");
        Poll::Ready(Ok(this.completion.track_completion(h, rsp)))
    }
}

// ===== CompleteOnResponse =====

impl<H, V> TrackCompletion<H, V> for CompleteOnResponse {
    type Output = V;

    fn track_completion(&self, handle: H, value: V) -> V {
        drop(handle);
        value
    }
}
