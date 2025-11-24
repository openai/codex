use bytes::Bytes;
use http::{HeaderValue, Response, StatusCode};
use http_body::{Body, SizeHint};
use http_body_util::Full;
use pin_project_lite::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project! {
    /// Response body for [`RequestBodyLimit`].
    ///
    /// [`RequestBodyLimit`]: super::RequestBodyLimit
    pub struct ResponseBody<B> {
        #[pin]
        inner: ResponseBodyInner<B>
    }
}

impl<B> ResponseBody<B> {
    fn payload_too_large() -> Self {
        Self {
            inner: ResponseBodyInner::PayloadTooLarge {
                body: Full::from(BODY),
            },
        }
    }

    pub(crate) fn new(body: B) -> Self {
        Self {
            inner: ResponseBodyInner::Body { body },
        }
    }
}

pin_project! {
    #[project = BodyProj]
    enum ResponseBodyInner<B> {
        PayloadTooLarge {
            #[pin]
            body: Full<Bytes>,
        },
        Body {
            #[pin]
            body: B
        }
    }
}

impl<B> Body for ResponseBody<B>
where
    B: Body<Data = Bytes>,
{
    type Data = Bytes;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        match self.project().inner.project() {
            BodyProj::PayloadTooLarge { body } => body.poll_frame(cx).map_err(|err| match err {}),
            BodyProj::Body { body } => body.poll_frame(cx),
        }
    }

    fn is_end_stream(&self) -> bool {
        match &self.inner {
            ResponseBodyInner::PayloadTooLarge { body } => body.is_end_stream(),
            ResponseBodyInner::Body { body } => body.is_end_stream(),
        }
    }

    fn size_hint(&self) -> SizeHint {
        match &self.inner {
            ResponseBodyInner::PayloadTooLarge { body } => body.size_hint(),
            ResponseBodyInner::Body { body } => body.size_hint(),
        }
    }
}

const BODY: &[u8] = b"length limit exceeded";

pub(crate) fn create_error_response<B>() -> Response<ResponseBody<B>>
where
    B: Body,
{
    let mut res = Response::new(ResponseBody::payload_too_large());
    *res.status_mut() = StatusCode::PAYLOAD_TOO_LARGE;

    #[allow(clippy::declare_interior_mutable_const)]
    const TEXT_PLAIN: HeaderValue = HeaderValue::from_static("text/plain; charset=utf-8");
    res.headers_mut()
        .insert(http::header::CONTENT_TYPE, TEXT_PLAIN);

    res
}
