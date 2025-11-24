//! Future types for the [`Buffer`] middleware.
//!
//! [`Buffer`]: crate::buffer::Buffer

use super::{error::Closed, message};
use futures_core::ready;
use pin_project_lite::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

pin_project! {
    /// Future that completes when the buffered service eventually services the submitted request.
    #[derive(Debug)]
    pub struct ResponseFuture<T> {
        #[pin]
        state: ResponseState<T>,
    }
}

pin_project! {
    #[project = ResponseStateProj]
    #[derive(Debug)]
    enum ResponseState<T> {
        Failed {
            error: Option<crate::BoxError>,
        },
        Rx {
            #[pin]
            rx: message::Rx<T>,
        },
        Poll {
            #[pin]
            fut: T,
        },
    }
}

impl<T> ResponseFuture<T> {
    pub(crate) fn new(rx: message::Rx<T>) -> Self {
        ResponseFuture {
            state: ResponseState::Rx { rx },
        }
    }

    pub(crate) fn failed(err: crate::BoxError) -> Self {
        ResponseFuture {
            state: ResponseState::Failed { error: Some(err) },
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
        let mut this = self.project();

        loop {
            match this.state.as_mut().project() {
                ResponseStateProj::Failed { error } => {
                    return Poll::Ready(Err(error.take().expect("polled after error")));
                }
                ResponseStateProj::Rx { rx } => match ready!(rx.poll(cx)) {
                    Ok(Ok(fut)) => this.state.set(ResponseState::Poll { fut }),
                    Ok(Err(e)) => return Poll::Ready(Err(e.into())),
                    Err(_) => return Poll::Ready(Err(Closed::new().into())),
                },
                ResponseStateProj::Poll { fut } => return fut.poll(cx).map_err(Into::into),
            }
        }
    }
}
