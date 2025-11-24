//! Future types

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::ready;
use pin_project_lite::pin_project;

use super::error::Overloaded;

pin_project! {
    /// Future for the [`LoadShed`] service.
    ///
    /// [`LoadShed`]: crate::load_shed::LoadShed
    pub struct ResponseFuture<F> {
        #[pin]
        state: ResponseState<F>,
    }
}

pin_project! {
    #[project = ResponseStateProj]
    enum ResponseState<F> {
        Called {
            #[pin]
            fut: F
        },
        Overloaded,
    }
}

impl<F> ResponseFuture<F> {
    pub(crate) fn called(fut: F) -> Self {
        ResponseFuture {
            state: ResponseState::Called { fut },
        }
    }

    pub(crate) fn overloaded() -> Self {
        ResponseFuture {
            state: ResponseState::Overloaded,
        }
    }
}

impl<F, T, E> Future for ResponseFuture<F>
where
    F: Future<Output = Result<T, E>>,
    E: Into<crate::BoxError>,
{
    type Output = Result<T, crate::BoxError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project().state.project() {
            ResponseStateProj::Called { fut } => {
                Poll::Ready(ready!(fut.poll(cx)).map_err(Into::into))
            }
            ResponseStateProj::Overloaded => Poll::Ready(Err(Overloaded::new().into())),
        }
    }
}

impl<F> fmt::Debug for ResponseFuture<F>
where
    // bounds for future-proofing...
    F: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("ResponseFuture")
    }
}
