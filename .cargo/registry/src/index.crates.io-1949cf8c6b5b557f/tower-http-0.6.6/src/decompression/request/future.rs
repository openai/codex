use crate::body::UnsyncBoxBody;
use crate::compression_utils::AcceptEncoding;
use crate::BoxError;
use bytes::Buf;
use http::{header, HeaderValue, Response, StatusCode};
use http_body::Body;
use http_body_util::BodyExt;
use http_body_util::Empty;
use pin_project_lite::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

pin_project! {
    #[derive(Debug)]
    /// Response future of [`RequestDecompression`]
    pub struct RequestDecompressionFuture<F, B, E>
    where
        F: Future<Output = Result<Response<B>, E>>,
        B: Body
    {
        #[pin]
        kind: Kind<F, B, E>,
    }
}

pin_project! {
    #[derive(Debug)]
    #[project = StateProj]
    enum Kind<F, B, E>
    where
        F: Future<Output = Result<Response<B>, E>>,
        B: Body
    {
        Inner {
            #[pin]
            fut: F
        },
        Unsupported {
            #[pin]
            accept: AcceptEncoding
        },
    }
}

impl<F, B, E> RequestDecompressionFuture<F, B, E>
where
    F: Future<Output = Result<Response<B>, E>>,
    B: Body,
{
    #[must_use]
    pub(super) fn unsupported_encoding(accept: AcceptEncoding) -> Self {
        Self {
            kind: Kind::Unsupported { accept },
        }
    }

    #[must_use]
    pub(super) fn inner(fut: F) -> Self {
        Self {
            kind: Kind::Inner { fut },
        }
    }
}

impl<F, B, E> Future for RequestDecompressionFuture<F, B, E>
where
    F: Future<Output = Result<Response<B>, E>>,
    B: Body + Send + 'static,
    B::Data: Buf + 'static,
    B::Error: Into<BoxError> + 'static,
{
    type Output = Result<Response<UnsyncBoxBody<B::Data, BoxError>>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project().kind.project() {
            StateProj::Inner { fut } => fut.poll(cx).map_ok(|res| {
                res.map(|body| UnsyncBoxBody::new(body.map_err(Into::into).boxed_unsync()))
            }),
            StateProj::Unsupported { accept } => {
                let res = Response::builder()
                    .header(
                        header::ACCEPT_ENCODING,
                        accept
                            .to_header_value()
                            .unwrap_or(HeaderValue::from_static("identity")),
                    )
                    .status(StatusCode::UNSUPPORTED_MEDIA_TYPE)
                    .body(UnsyncBoxBody::new(
                        Empty::new().map_err(Into::into).boxed_unsync(),
                    ))
                    .unwrap();
                Poll::Ready(Ok(res))
            }
        }
    }
}
