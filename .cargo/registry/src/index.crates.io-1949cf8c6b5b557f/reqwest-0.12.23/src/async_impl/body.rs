use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{ready, Context, Poll};
use std::time::Duration;

use bytes::Bytes;
use http_body::Body as HttpBody;
use http_body_util::combinators::BoxBody;
use pin_project_lite::pin_project;
#[cfg(feature = "stream")]
use tokio::fs::File;
use tokio::time::Sleep;
#[cfg(feature = "stream")]
use tokio_util::io::ReaderStream;

/// An asynchronous request body.
pub struct Body {
    inner: Inner,
}

enum Inner {
    Reusable(Bytes),
    Streaming(BoxBody<Bytes, Box<dyn std::error::Error + Send + Sync>>),
}

pin_project! {
    /// A body with a total timeout.
    ///
    /// The timeout does not reset upon each chunk, but rather requires the whole
    /// body be streamed before the deadline is reached.
    pub(crate) struct TotalTimeoutBody<B> {
        #[pin]
        inner: B,
        timeout: Pin<Box<Sleep>>,
    }
}

pin_project! {
    pub(crate) struct ReadTimeoutBody<B> {
        #[pin]
        inner: B,
        #[pin]
        sleep: Option<Sleep>,
        timeout: Duration,
    }
}

/// Converts any `impl Body` into a `impl Stream` of just its DATA frames.
#[cfg(any(feature = "stream", feature = "multipart",))]
pub(crate) struct DataStream<B>(pub(crate) B);

impl Body {
    /// Returns a reference to the internal data of the `Body`.
    ///
    /// `None` is returned, if the underlying data is a stream.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match &self.inner {
            Inner::Reusable(bytes) => Some(bytes.as_ref()),
            Inner::Streaming(..) => None,
        }
    }

    /// Wrap a futures `Stream` in a box inside `Body`.
    ///
    /// # Example
    ///
    /// ```
    /// # use reqwest::Body;
    /// # use futures_util;
    /// # fn main() {
    /// let chunks: Vec<Result<_, ::std::io::Error>> = vec![
    ///     Ok("hello"),
    ///     Ok(" "),
    ///     Ok("world"),
    /// ];
    ///
    /// let stream = futures_util::stream::iter(chunks);
    ///
    /// let body = Body::wrap_stream(stream);
    /// # }
    /// ```
    ///
    /// # Optional
    ///
    /// This requires the `stream` feature to be enabled.
    #[cfg(feature = "stream")]
    #[cfg_attr(docsrs, doc(cfg(feature = "stream")))]
    pub fn wrap_stream<S>(stream: S) -> Body
    where
        S: futures_core::stream::TryStream + Send + 'static,
        S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
        Bytes: From<S::Ok>,
    {
        Body::stream(stream)
    }

    #[cfg(any(feature = "stream", feature = "multipart", feature = "blocking"))]
    pub(crate) fn stream<S>(stream: S) -> Body
    where
        S: futures_core::stream::TryStream + Send + 'static,
        S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
        Bytes: From<S::Ok>,
    {
        use futures_util::TryStreamExt;
        use http_body::Frame;
        use http_body_util::StreamBody;

        let body = http_body_util::BodyExt::boxed(StreamBody::new(sync_wrapper::SyncStream::new(
            stream
                .map_ok(|d| Frame::data(Bytes::from(d)))
                .map_err(Into::into),
        )));
        Body {
            inner: Inner::Streaming(body),
        }
    }

    pub(crate) fn empty() -> Body {
        Body::reusable(Bytes::new())
    }

    pub(crate) fn reusable(chunk: Bytes) -> Body {
        Body {
            inner: Inner::Reusable(chunk),
        }
    }

    /// Wrap a [`HttpBody`] in a box inside `Body`.
    ///
    /// # Example
    ///
    /// ```
    /// # use reqwest::Body;
    /// # use futures_util;
    /// # fn main() {
    /// let content = "hello,world!".to_string();
    ///
    /// let body = Body::wrap(content);
    /// # }
    /// ```
    pub fn wrap<B>(inner: B) -> Body
    where
        B: HttpBody + Send + Sync + 'static,
        B::Data: Into<Bytes>,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        use http_body_util::BodyExt;

        let boxed = IntoBytesBody { inner }.map_err(Into::into).boxed();

        Body {
            inner: Inner::Streaming(boxed),
        }
    }

    pub(crate) fn try_clone(&self) -> Option<Body> {
        match self.inner {
            Inner::Reusable(ref chunk) => Some(Body::reusable(chunk.clone())),
            Inner::Streaming { .. } => None,
        }
    }

    #[cfg(feature = "multipart")]
    pub(crate) fn into_stream(self) -> DataStream<Body> {
        DataStream(self)
    }

    #[cfg(feature = "multipart")]
    pub(crate) fn content_length(&self) -> Option<u64> {
        match self.inner {
            Inner::Reusable(ref bytes) => Some(bytes.len() as u64),
            Inner::Streaming(ref body) => body.size_hint().exact(),
        }
    }
}

impl Default for Body {
    #[inline]
    fn default() -> Body {
        Body::empty()
    }
}

/*
impl From<hyper::Body> for Body {
    #[inline]
    fn from(body: hyper::Body) -> Body {
        Self {
            inner: Inner::Streaming {
                body: Box::pin(WrapHyper(body)),
            },
        }
    }
}
*/

impl From<Bytes> for Body {
    #[inline]
    fn from(bytes: Bytes) -> Body {
        Body::reusable(bytes)
    }
}

impl From<Vec<u8>> for Body {
    #[inline]
    fn from(vec: Vec<u8>) -> Body {
        Body::reusable(vec.into())
    }
}

impl From<&'static [u8]> for Body {
    #[inline]
    fn from(s: &'static [u8]) -> Body {
        Body::reusable(Bytes::from_static(s))
    }
}

impl From<String> for Body {
    #[inline]
    fn from(s: String) -> Body {
        Body::reusable(s.into())
    }
}

impl From<&'static str> for Body {
    #[inline]
    fn from(s: &'static str) -> Body {
        s.as_bytes().into()
    }
}

#[cfg(feature = "stream")]
#[cfg_attr(docsrs, doc(cfg(feature = "stream")))]
impl From<File> for Body {
    #[inline]
    fn from(file: File) -> Body {
        Body::wrap_stream(ReaderStream::new(file))
    }
}

impl fmt::Debug for Body {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Body").finish()
    }
}

impl HttpBody for Body {
    type Data = Bytes;
    type Error = crate::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
    ) -> Poll<Option<Result<hyper::body::Frame<Self::Data>, Self::Error>>> {
        match self.inner {
            Inner::Reusable(ref mut bytes) => {
                let out = bytes.split_off(0);
                if out.is_empty() {
                    Poll::Ready(None)
                } else {
                    Poll::Ready(Some(Ok(hyper::body::Frame::data(out))))
                }
            }
            Inner::Streaming(ref mut body) => Poll::Ready(
                ready!(Pin::new(body).poll_frame(cx))
                    .map(|opt_chunk| opt_chunk.map_err(crate::error::body)),
            ),
        }
    }

    fn size_hint(&self) -> http_body::SizeHint {
        match self.inner {
            Inner::Reusable(ref bytes) => http_body::SizeHint::with_exact(bytes.len() as u64),
            Inner::Streaming(ref body) => body.size_hint(),
        }
    }

    fn is_end_stream(&self) -> bool {
        match self.inner {
            Inner::Reusable(ref bytes) => bytes.is_empty(),
            Inner::Streaming(ref body) => body.is_end_stream(),
        }
    }
}

// ===== impl TotalTimeoutBody =====

pub(crate) fn total_timeout<B>(body: B, timeout: Pin<Box<Sleep>>) -> TotalTimeoutBody<B> {
    TotalTimeoutBody {
        inner: body,
        timeout,
    }
}

pub(crate) fn with_read_timeout<B>(body: B, timeout: Duration) -> ReadTimeoutBody<B> {
    ReadTimeoutBody {
        inner: body,
        sleep: None,
        timeout,
    }
}

impl<B> hyper::body::Body for TotalTimeoutBody<B>
where
    B: hyper::body::Body,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Data = B::Data;
    type Error = crate::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context,
    ) -> Poll<Option<Result<hyper::body::Frame<Self::Data>, Self::Error>>> {
        let this = self.project();
        if let Poll::Ready(()) = this.timeout.as_mut().poll(cx) {
            return Poll::Ready(Some(Err(crate::error::body(crate::error::TimedOut))));
        }
        Poll::Ready(
            ready!(this.inner.poll_frame(cx))
                .map(|opt_chunk| opt_chunk.map_err(crate::error::body)),
        )
    }

    #[inline]
    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }

    #[inline]
    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }
}

impl<B> hyper::body::Body for ReadTimeoutBody<B>
where
    B: hyper::body::Body,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Data = B::Data;
    type Error = crate::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context,
    ) -> Poll<Option<Result<hyper::body::Frame<Self::Data>, Self::Error>>> {
        let mut this = self.project();

        // Start the `Sleep` if not active.
        let sleep_pinned = if let Some(some) = this.sleep.as_mut().as_pin_mut() {
            some
        } else {
            this.sleep.set(Some(tokio::time::sleep(*this.timeout)));
            this.sleep.as_mut().as_pin_mut().unwrap()
        };

        // Error if the timeout has expired.
        if let Poll::Ready(()) = sleep_pinned.poll(cx) {
            return Poll::Ready(Some(Err(crate::error::body(crate::error::TimedOut))));
        }

        let item = ready!(this.inner.poll_frame(cx))
            .map(|opt_chunk| opt_chunk.map_err(crate::error::body));
        // a ready frame means timeout is reset
        this.sleep.set(None);
        Poll::Ready(item)
    }

    #[inline]
    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }

    #[inline]
    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }
}

pub(crate) type ResponseBody =
    http_body_util::combinators::BoxBody<Bytes, Box<dyn std::error::Error + Send + Sync>>;

pub(crate) fn boxed<B>(body: B) -> ResponseBody
where
    B: hyper::body::Body<Data = Bytes> + Send + Sync + 'static,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    use http_body_util::BodyExt;

    body.map_err(box_err).boxed()
}

pub(crate) fn response<B>(
    body: B,
    deadline: Option<Pin<Box<Sleep>>>,
    read_timeout: Option<Duration>,
) -> ResponseBody
where
    B: hyper::body::Body<Data = Bytes> + Send + Sync + 'static,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    use http_body_util::BodyExt;

    match (deadline, read_timeout) {
        (Some(total), Some(read)) => {
            let body = with_read_timeout(body, read).map_err(box_err);
            total_timeout(body, total).map_err(box_err).boxed()
        }
        (Some(total), None) => total_timeout(body, total).map_err(box_err).boxed(),
        (None, Some(read)) => with_read_timeout(body, read).map_err(box_err).boxed(),
        (None, None) => body.map_err(box_err).boxed(),
    }
}

fn box_err<E>(err: E) -> Box<dyn std::error::Error + Send + Sync>
where
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    err.into()
}

// ===== impl DataStream =====

#[cfg(any(feature = "stream", feature = "multipart",))]
impl<B> futures_core::Stream for DataStream<B>
where
    B: HttpBody<Data = Bytes> + Unpin,
{
    type Item = Result<Bytes, B::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        loop {
            return match ready!(Pin::new(&mut self.0).poll_frame(cx)) {
                Some(Ok(frame)) => {
                    // skip non-data frames
                    if let Ok(buf) = frame.into_data() {
                        Poll::Ready(Some(Ok(buf)))
                    } else {
                        continue;
                    }
                }
                Some(Err(err)) => Poll::Ready(Some(Err(err))),
                None => Poll::Ready(None),
            };
        }
    }
}

// ===== impl IntoBytesBody =====

pin_project! {
    struct IntoBytesBody<B> {
        #[pin]
        inner: B,
    }
}

// We can't use `map_frame()` because that loses the hint data (for good reason).
// But we aren't transforming the data.
impl<B> hyper::body::Body for IntoBytesBody<B>
where
    B: hyper::body::Body,
    B::Data: Into<Bytes>,
{
    type Data = Bytes;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context,
    ) -> Poll<Option<Result<hyper::body::Frame<Self::Data>, Self::Error>>> {
        match ready!(self.project().inner.poll_frame(cx)) {
            Some(Ok(f)) => Poll::Ready(Some(Ok(f.map_data(Into::into)))),
            Some(Err(e)) => Poll::Ready(Some(Err(e))),
            None => Poll::Ready(None),
        }
    }

    #[inline]
    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }

    #[inline]
    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }
}

#[cfg(test)]
mod tests {
    use http_body::Body as _;

    use super::Body;

    #[test]
    fn test_as_bytes() {
        let test_data = b"Test body";
        let body = Body::from(&test_data[..]);
        assert_eq!(body.as_bytes(), Some(&test_data[..]));
    }

    #[test]
    fn body_exact_length() {
        let empty_body = Body::empty();
        assert!(empty_body.is_end_stream());
        assert_eq!(empty_body.size_hint().exact(), Some(0));

        let bytes_body = Body::reusable("abc".into());
        assert!(!bytes_body.is_end_stream());
        assert_eq!(bytes_body.size_hint().exact(), Some(3));

        // can delegate even when wrapped
        let stream_body = Body::wrap(empty_body);
        assert!(stream_body.is_end_stream());
        assert_eq!(stream_body.size_hint().exact(), Some(0));
    }
}
