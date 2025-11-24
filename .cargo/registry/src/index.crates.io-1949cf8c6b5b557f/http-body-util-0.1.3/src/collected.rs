use std::{
    convert::Infallible,
    pin::Pin,
    task::{Context, Poll},
};

use bytes::{Buf, Bytes};
use http::HeaderMap;
use http_body::{Body, Frame};

use crate::util::BufList;

/// A collected body produced by [`BodyExt::collect`] which collects all the DATA frames
/// and trailers.
///
/// [`BodyExt::collect`]: crate::BodyExt::collect
#[derive(Debug)]
pub struct Collected<B> {
    bufs: BufList<B>,
    trailers: Option<HeaderMap>,
}

impl<B: Buf> Collected<B> {
    /// If there is a trailers frame buffered, returns a reference to it.
    ///
    /// Returns `None` if the body contained no trailers.
    pub fn trailers(&self) -> Option<&HeaderMap> {
        self.trailers.as_ref()
    }

    /// Aggregate this buffered into a [`Buf`].
    pub fn aggregate(self) -> impl Buf {
        self.bufs
    }

    /// Convert this body into a [`Bytes`].
    pub fn to_bytes(mut self) -> Bytes {
        self.bufs.copy_to_bytes(self.bufs.remaining())
    }

    pub(crate) fn push_frame(&mut self, frame: Frame<B>) {
        let frame = match frame.into_data() {
            Ok(data) => {
                // Only push this frame if it has some data in it, to avoid crashing on
                // `BufList::push`.
                if data.has_remaining() {
                    self.bufs.push(data);
                }
                return;
            }
            Err(frame) => frame,
        };

        if let Ok(trailers) = frame.into_trailers() {
            if let Some(current) = &mut self.trailers {
                current.extend(trailers);
            } else {
                self.trailers = Some(trailers);
            }
        };
    }
}

impl<B: Buf> Body for Collected<B> {
    type Data = B;
    type Error = Infallible;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let frame = if let Some(data) = self.bufs.pop() {
            Frame::data(data)
        } else if let Some(trailers) = self.trailers.take() {
            Frame::trailers(trailers)
        } else {
            return Poll::Ready(None);
        };

        Poll::Ready(Some(Ok(frame)))
    }
}

impl<B> Default for Collected<B> {
    fn default() -> Self {
        Self {
            bufs: BufList::default(),
            trailers: None,
        }
    }
}

impl<B> Unpin for Collected<B> {}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use futures_util::stream;

    use crate::{BodyExt, Full, StreamBody};

    use super::*;

    #[tokio::test]
    async fn full_body() {
        let body = Full::new(&b"hello"[..]);

        let buffered = body.collect().await.unwrap();

        let mut buf = buffered.to_bytes();

        assert_eq!(&buf.copy_to_bytes(buf.remaining())[..], &b"hello"[..]);
    }

    #[tokio::test]
    async fn segmented_body() {
        let bufs = [&b"hello"[..], &b"world"[..], &b"!"[..]];
        let body = StreamBody::new(stream::iter(bufs.map(Frame::data).map(Ok::<_, Infallible>)));

        let buffered = body.collect().await.unwrap();

        let mut buf = buffered.to_bytes();

        assert_eq!(&buf.copy_to_bytes(buf.remaining())[..], b"helloworld!");
    }

    #[tokio::test]
    async fn delayed_segments() {
        let one = stream::once(async { Ok::<_, Infallible>(Frame::data(&b"hello "[..])) });
        let two = stream::once(async {
            // a yield just so its not ready immediately
            tokio::task::yield_now().await;
            Ok::<_, Infallible>(Frame::data(&b"world!"[..]))
        });
        let stream = futures_util::StreamExt::chain(one, two);

        let body = StreamBody::new(stream);

        let buffered = body.collect().await.unwrap();

        let mut buf = buffered.to_bytes();

        assert_eq!(&buf.copy_to_bytes(buf.remaining())[..], b"hello world!");
    }

    #[tokio::test]
    async fn trailers() {
        let mut trailers = HeaderMap::new();
        trailers.insert("this", "a trailer".try_into().unwrap());
        let bufs = [
            Frame::data(&b"hello"[..]),
            Frame::data(&b"world!"[..]),
            Frame::trailers(trailers.clone()),
        ];

        let body = StreamBody::new(stream::iter(bufs.map(Ok::<_, Infallible>)));

        let buffered = body.collect().await.unwrap();

        assert_eq!(&trailers, buffered.trailers().unwrap());

        let mut buf = buffered.to_bytes();

        assert_eq!(&buf.copy_to_bytes(buf.remaining())[..], b"helloworld!");
    }

    /// Test for issue [#88](https://github.com/hyperium/http-body/issues/88).
    #[tokio::test]
    async fn empty_frame() {
        let bufs: [&[u8]; 1] = [&[]];

        let body = StreamBody::new(stream::iter(bufs.map(Frame::data).map(Ok::<_, Infallible>)));
        let buffered = body.collect().await.unwrap();

        assert_eq!(buffered.to_bytes().len(), 0);
    }
}
