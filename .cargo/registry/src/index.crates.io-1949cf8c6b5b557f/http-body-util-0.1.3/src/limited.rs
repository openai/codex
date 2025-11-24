use bytes::Buf;
use http_body::{Body, Frame, SizeHint};
use pin_project_lite::pin_project;
use std::error::Error;
use std::fmt;
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project! {
    /// A length limited body.
    ///
    /// This body will return an error if more than the configured number
    /// of bytes are returned on polling the wrapped body.
    #[derive(Clone, Copy, Debug)]
    pub struct Limited<B> {
        remaining: usize,
        #[pin]
        inner: B,
    }
}

impl<B> Limited<B> {
    /// Create a new `Limited`.
    pub fn new(inner: B, limit: usize) -> Self {
        Self {
            remaining: limit,
            inner,
        }
    }
}

impl<B> Body for Limited<B>
where
    B: Body,
    B::Error: Into<Box<dyn Error + Send + Sync>>,
{
    type Data = B::Data;
    type Error = Box<dyn Error + Send + Sync>;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.project();
        let res = match this.inner.poll_frame(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(None) => None,
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    if data.remaining() > *this.remaining {
                        *this.remaining = 0;
                        Some(Err(LengthLimitError.into()))
                    } else {
                        *this.remaining -= data.remaining();
                        Some(Ok(frame))
                    }
                } else {
                    Some(Ok(frame))
                }
            }
            Poll::Ready(Some(Err(err))) => Some(Err(err.into())),
        };

        Poll::Ready(res)
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        use std::convert::TryFrom;
        match u64::try_from(self.remaining) {
            Ok(n) => {
                let mut hint = self.inner.size_hint();
                if hint.lower() >= n {
                    hint.set_exact(n)
                } else if let Some(max) = hint.upper() {
                    hint.set_upper(n.min(max))
                } else {
                    hint.set_upper(n)
                }
                hint
            }
            Err(_) => self.inner.size_hint(),
        }
    }
}

/// An error returned when body length exceeds the configured limit.
#[derive(Debug)]
#[non_exhaustive]
pub struct LengthLimitError;

impl fmt::Display for LengthLimitError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("length limit exceeded")
    }
}

impl Error for LengthLimitError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BodyExt, Full, StreamBody};
    use bytes::Bytes;
    use std::convert::Infallible;

    #[tokio::test]
    async fn read_for_body_under_limit_returns_data() {
        const DATA: &[u8] = b"testing";
        let inner = Full::new(Bytes::from(DATA));
        let body = &mut Limited::new(inner, 8);

        let mut hint = SizeHint::new();
        hint.set_upper(7);
        assert_eq!(body.size_hint().upper(), hint.upper());

        let data = body.frame().await.unwrap().unwrap().into_data().unwrap();
        assert_eq!(data, DATA);
        hint.set_upper(0);
        assert_eq!(body.size_hint().upper(), hint.upper());

        assert!(body.frame().await.is_none());
    }

    #[tokio::test]
    async fn read_for_body_over_limit_returns_error() {
        const DATA: &[u8] = b"testing a string that is too long";
        let inner = Full::new(Bytes::from(DATA));
        let body = &mut Limited::new(inner, 8);

        let mut hint = SizeHint::new();
        hint.set_upper(8);
        assert_eq!(body.size_hint().upper(), hint.upper());

        let error = body.frame().await.unwrap().unwrap_err();
        assert!(matches!(error.downcast_ref(), Some(LengthLimitError)));
    }

    fn body_from_iter<I>(into_iter: I) -> impl Body<Data = Bytes, Error = Infallible>
    where
        I: IntoIterator,
        I::Item: Into<Bytes> + 'static,
        I::IntoIter: Send + 'static,
    {
        let iter = into_iter
            .into_iter()
            .map(|it| Frame::data(it.into()))
            .map(Ok::<_, Infallible>);

        StreamBody::new(futures_util::stream::iter(iter))
    }

    #[tokio::test]
    async fn read_for_chunked_body_around_limit_returns_first_chunk_but_returns_error_on_over_limit_chunk(
    ) {
        const DATA: [&[u8]; 2] = [b"testing ", b"a string that is too long"];
        let inner = body_from_iter(DATA);
        let body = &mut Limited::new(inner, 8);

        let mut hint = SizeHint::new();
        hint.set_upper(8);
        assert_eq!(body.size_hint().upper(), hint.upper());

        let data = body.frame().await.unwrap().unwrap().into_data().unwrap();
        assert_eq!(data, DATA[0]);
        hint.set_upper(0);
        assert_eq!(body.size_hint().upper(), hint.upper());

        let error = body.frame().await.unwrap().unwrap_err();
        assert!(matches!(error.downcast_ref(), Some(LengthLimitError)));
    }

    #[tokio::test]
    async fn read_for_chunked_body_over_limit_on_first_chunk_returns_error() {
        const DATA: [&[u8]; 2] = [b"testing a string", b" that is too long"];
        let inner = body_from_iter(DATA);
        let body = &mut Limited::new(inner, 8);

        let mut hint = SizeHint::new();
        hint.set_upper(8);
        assert_eq!(body.size_hint().upper(), hint.upper());

        let error = body.frame().await.unwrap().unwrap_err();
        assert!(matches!(error.downcast_ref(), Some(LengthLimitError)));
    }

    #[tokio::test]
    async fn read_for_chunked_body_under_limit_is_okay() {
        const DATA: [&[u8]; 2] = [b"test", b"ing!"];
        let inner = body_from_iter(DATA);
        let body = &mut Limited::new(inner, 8);

        let mut hint = SizeHint::new();
        hint.set_upper(8);
        assert_eq!(body.size_hint().upper(), hint.upper());

        let data = body.frame().await.unwrap().unwrap().into_data().unwrap();
        assert_eq!(data, DATA[0]);
        hint.set_upper(4);
        assert_eq!(body.size_hint().upper(), hint.upper());

        let data = body.frame().await.unwrap().unwrap().into_data().unwrap();
        assert_eq!(data, DATA[1]);
        hint.set_upper(0);
        assert_eq!(body.size_hint().upper(), hint.upper());

        assert!(body.frame().await.is_none());
    }

    struct SomeTrailers;

    impl Body for SomeTrailers {
        type Data = Bytes;
        type Error = Infallible;

        fn poll_frame(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
            Poll::Ready(Some(Ok(Frame::trailers(http::HeaderMap::new()))))
        }
    }

    #[tokio::test]
    async fn read_for_trailers_propagates_inner_trailers() {
        let body = &mut Limited::new(SomeTrailers, 8);
        let frame = body.frame().await.unwrap().unwrap();
        assert!(frame.is_trailers());
    }

    #[derive(Debug)]
    struct ErrorBodyError;

    impl fmt::Display for ErrorBodyError {
        fn fmt(&self, _f: &mut fmt::Formatter) -> fmt::Result {
            Ok(())
        }
    }

    impl Error for ErrorBodyError {}

    struct ErrorBody;

    impl Body for ErrorBody {
        type Data = &'static [u8];
        type Error = ErrorBodyError;

        fn poll_frame(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
            Poll::Ready(Some(Err(ErrorBodyError)))
        }
    }

    #[tokio::test]
    async fn read_for_body_returning_error_propagates_error() {
        let body = &mut Limited::new(ErrorBody, 8);
        let error = body.frame().await.unwrap().unwrap_err();
        assert!(matches!(error.downcast_ref(), Some(ErrorBodyError)));
    }
}
