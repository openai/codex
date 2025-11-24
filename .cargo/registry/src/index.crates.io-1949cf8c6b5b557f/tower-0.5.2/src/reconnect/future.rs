use pin_project_lite::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

pin_project! {
    /// Future that resolves to the response or failure to connect.
    #[derive(Debug)]
    pub struct ResponseFuture<F, E> {
        #[pin]
        inner: Inner<F, E>,
    }
}

pin_project! {
    #[project = InnerProj]
    #[derive(Debug)]
    enum Inner<F, E> {
        Future {
            #[pin]
            fut: F,
        },
        Error {
            error: Option<E>,
        },
    }
}

impl<F, E> Inner<F, E> {
    fn future(fut: F) -> Self {
        Self::Future { fut }
    }

    fn error(error: Option<E>) -> Self {
        Self::Error { error }
    }
}

impl<F, E> ResponseFuture<F, E> {
    pub(crate) fn new(inner: F) -> Self {
        ResponseFuture {
            inner: Inner::future(inner),
        }
    }

    pub(crate) fn error(error: E) -> Self {
        ResponseFuture {
            inner: Inner::error(Some(error)),
        }
    }
}

impl<F, T, E, ME> Future for ResponseFuture<F, ME>
where
    F: Future<Output = Result<T, E>>,
    E: Into<crate::BoxError>,
    ME: Into<crate::BoxError>,
{
    type Output = Result<T, crate::BoxError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.project();
        match me.inner.project() {
            InnerProj::Future { fut } => fut.poll(cx).map_err(Into::into),
            InnerProj::Error { error } => {
                let e = error.take().expect("Polled after ready.").into();
                Poll::Ready(Err(e))
            }
        }
    }
}
