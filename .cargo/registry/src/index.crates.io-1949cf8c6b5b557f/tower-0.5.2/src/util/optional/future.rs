use super::error;
use futures_core::ready;
use pin_project_lite::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

pin_project! {
    /// Response future returned by [`Optional`].
    ///
    /// [`Optional`]: crate::util::Optional
    #[derive(Debug)]
    pub struct ResponseFuture<T> {
        #[pin]
        inner: Option<T>,
    }
}

impl<T> ResponseFuture<T> {
    pub(crate) fn new(inner: Option<T>) -> ResponseFuture<T> {
        ResponseFuture { inner }
    }
}

impl<F, T, E> Future for ResponseFuture<F>
where
    F: Future<Output = Result<T, E>>,
    E: Into<crate::BoxError>,
{
    type Output = Result<T, crate::BoxError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project().inner.as_pin_mut() {
            Some(inner) => Poll::Ready(Ok(ready!(inner.poll(cx)).map_err(Into::into)?)),
            None => Poll::Ready(Err(error::None::new().into())),
        }
    }
}
