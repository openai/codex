use bytes::Buf;
use futures_core::{ready, stream::Stream};
use http_body::{Body, Frame};
use pin_project_lite::pin_project;
use std::{
    pin::Pin,
    task::{Context, Poll},
};

pin_project! {
    /// A body created from a [`Stream`].
    #[derive(Clone, Copy, Debug)]
    pub struct StreamBody<S> {
        #[pin]
        stream: S,
    }
}

impl<S> StreamBody<S> {
    /// Create a new `StreamBody`.
    pub fn new(stream: S) -> Self {
        Self { stream }
    }
}

impl<S, D, E> Body for StreamBody<S>
where
    S: Stream<Item = Result<Frame<D>, E>>,
    D: Buf,
{
    type Data = D;
    type Error = E;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match self.project().stream.poll_next(cx) {
            Poll::Ready(Some(result)) => Poll::Ready(Some(result)),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<S: Stream> Stream for StreamBody<S> {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().stream.poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}

pin_project! {
    /// A stream created from a [`Body`].
    #[derive(Clone, Copy, Debug)]
    pub struct BodyStream<B> {
        #[pin]
        body: B,
    }
}

impl<B> BodyStream<B> {
    /// Create a new `BodyStream`.
    pub fn new(body: B) -> Self {
        Self { body }
    }
}

impl<B> Body for BodyStream<B>
where
    B: Body,
{
    type Data = B::Data;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        self.project().body.poll_frame(cx)
    }
}

impl<B> Stream for BodyStream<B>
where
    B: Body,
{
    type Item = Result<Frame<B::Data>, B::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.project().body.poll_frame(cx) {
            Poll::Ready(Some(frame)) => Poll::Ready(Some(frame)),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

pin_project! {
    /// A data stream created from a [`Body`].
    #[derive(Clone, Copy, Debug)]
    pub struct BodyDataStream<B> {
        #[pin]
        body: B,
    }
}

impl<B> BodyDataStream<B> {
    /// Create a new `BodyDataStream`
    pub fn new(body: B) -> Self {
        Self { body }
    }
}

impl<B> Stream for BodyDataStream<B>
where
    B: Body,
{
    type Item = Result<B::Data, B::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            return match ready!(self.as_mut().project().body.poll_frame(cx)) {
                Some(Ok(frame)) => match frame.into_data() {
                    Ok(bytes) => Poll::Ready(Some(Ok(bytes))),
                    Err(_) => continue,
                },
                Some(Err(err)) => Poll::Ready(Some(Err(err))),
                None => Poll::Ready(None),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{BodyExt, BodyStream, StreamBody};
    use bytes::Bytes;
    use futures_util::StreamExt;
    use http_body::Frame;
    use std::convert::Infallible;

    #[tokio::test]
    async fn body_from_stream() {
        let chunks: Vec<Result<_, Infallible>> = vec![
            Ok(Frame::data(Bytes::from(vec![1]))),
            Ok(Frame::data(Bytes::from(vec![2]))),
            Ok(Frame::data(Bytes::from(vec![3]))),
        ];
        let stream = futures_util::stream::iter(chunks);
        let mut body = StreamBody::new(stream);

        assert_eq!(
            body.frame()
                .await
                .unwrap()
                .unwrap()
                .into_data()
                .unwrap()
                .as_ref(),
            [1]
        );
        assert_eq!(
            body.frame()
                .await
                .unwrap()
                .unwrap()
                .into_data()
                .unwrap()
                .as_ref(),
            [2]
        );
        assert_eq!(
            body.frame()
                .await
                .unwrap()
                .unwrap()
                .into_data()
                .unwrap()
                .as_ref(),
            [3]
        );

        assert!(body.frame().await.is_none());
    }

    #[tokio::test]
    async fn stream_from_body() {
        let chunks: Vec<Result<_, Infallible>> = vec![
            Ok(Frame::data(Bytes::from(vec![1]))),
            Ok(Frame::data(Bytes::from(vec![2]))),
            Ok(Frame::data(Bytes::from(vec![3]))),
        ];
        let stream = futures_util::stream::iter(chunks);
        let body = StreamBody::new(stream);

        let mut stream = BodyStream::new(body);

        assert_eq!(
            stream
                .next()
                .await
                .unwrap()
                .unwrap()
                .into_data()
                .unwrap()
                .as_ref(),
            [1]
        );
        assert_eq!(
            stream
                .next()
                .await
                .unwrap()
                .unwrap()
                .into_data()
                .unwrap()
                .as_ref(),
            [2]
        );
        assert_eq!(
            stream
                .next()
                .await
                .unwrap()
                .unwrap()
                .into_data()
                .unwrap()
                .as_ref(),
            [3]
        );

        assert!(stream.next().await.is_none());
    }
}
