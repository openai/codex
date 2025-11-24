//! Middleware for shedding load when inner services aren't ready.

use std::task::{Context, Poll};
use tower_service::Service;

pub mod error;
pub mod future;
mod layer;

use self::future::ResponseFuture;
pub use self::layer::LoadShedLayer;

/// A [`Service`] that sheds load when the inner service isn't ready.
///
/// [`Service`]: crate::Service
#[derive(Debug)]
pub struct LoadShed<S> {
    inner: S,
    is_ready: bool,
}

// ===== impl LoadShed =====

impl<S> LoadShed<S> {
    /// Wraps a service in [`LoadShed`] middleware.
    pub const fn new(inner: S) -> Self {
        LoadShed {
            inner,
            is_ready: false,
        }
    }
}

impl<S, Req> Service<Req> for LoadShed<S>
where
    S: Service<Req>,
    S::Error: Into<crate::BoxError>,
{
    type Response = S::Response;
    type Error = crate::BoxError;
    type Future = ResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // We check for readiness here, so that we can know in `call` if
        // the inner service is overloaded or not.
        self.is_ready = match self.inner.poll_ready(cx) {
            Poll::Ready(Err(e)) => return Poll::Ready(Err(e.into())),
            r => r.is_ready(),
        };

        // But we always report Ready, so that layers above don't wait until
        // the inner service is ready (the entire point of this layer!)
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Req) -> Self::Future {
        if self.is_ready {
            // readiness only counts once, you need to check again!
            self.is_ready = false;
            ResponseFuture::called(self.inner.call(req))
        } else {
            ResponseFuture::overloaded()
        }
    }
}

impl<S: Clone> Clone for LoadShed<S> {
    fn clone(&self) -> Self {
        LoadShed {
            inner: self.inner.clone(),
            // new clones shouldn't carry the readiness state, as a cloneable
            // inner service likely tracks readiness per clone.
            is_ready: false,
        }
    }
}
