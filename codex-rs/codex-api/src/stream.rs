use crate::common::ResponseEvent;
use crate::common::ResponseStream;
use crate::error::ApiError;
use futures::Stream;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

pub struct AggregatedStream {
    inner: ResponseStream,
}

impl Stream for AggregatedStream {
    type Item = Result<ResponseEvent, crate::error::ApiError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

pub trait AggregateStreamExt: Stream<Item = Result<ResponseEvent, ApiError>> + Sized {
    fn aggregate(self) -> AggregatedStream;
}

impl AggregateStreamExt for ResponseStream {
    fn aggregate(self) -> AggregatedStream {
        AggregatedStream { inner: self }
    }
}
