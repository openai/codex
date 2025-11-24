use super::body::create_error_response;
use super::ResponseBody;
use http::Response;
use http_body::Body;
use pin_project_lite::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

pin_project! {
    /// Response future for [`RequestBodyLimit`].
    ///
    /// [`RequestBodyLimit`]: super::RequestBodyLimit
    pub struct ResponseFuture<F> {
        #[pin]
        inner: ResponseFutureInner<F>,
    }
}

impl<F> ResponseFuture<F> {
    pub(crate) fn payload_too_large() -> Self {
        Self {
            inner: ResponseFutureInner::PayloadTooLarge,
        }
    }

    pub(crate) fn new(future: F) -> Self {
        Self {
            inner: ResponseFutureInner::Future { future },
        }
    }
}

pin_project! {
    #[project = ResFutProj]
    enum ResponseFutureInner<F> {
        PayloadTooLarge,
        Future {
            #[pin]
            future: F,
        }
    }
}

impl<ResBody, F, E> Future for ResponseFuture<F>
where
    ResBody: Body,
    F: Future<Output = Result<Response<ResBody>, E>>,
{
    type Output = Result<Response<ResponseBody<ResBody>>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let res = match self.project().inner.project() {
            ResFutProj::PayloadTooLarge => create_error_response(),
            ResFutProj::Future { future } => ready!(future.poll(cx))?.map(ResponseBody::new),
        };

        Poll::Ready(Ok(res))
    }
}
