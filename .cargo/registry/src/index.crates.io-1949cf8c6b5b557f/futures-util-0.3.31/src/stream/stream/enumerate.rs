use core::pin::Pin;
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`enumerate`](super::StreamExt::enumerate) method.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Enumerate<St> {
        #[pin]
        stream: St,
        count: usize,
    }
}

impl<St: Stream> Enumerate<St> {
    pub(super) fn new(stream: St) -> Self {
        Self { stream, count: 0 }
    }

    delegate_access_inner!(stream, St, ());
}

impl<St: Stream + FusedStream> FusedStream for Enumerate<St> {
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St: Stream> Stream for Enumerate<St> {
    type Item = (usize, St::Item);

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();

        match ready!(this.stream.poll_next(cx)) {
            Some(item) => {
                let prev_count = *this.count;
                *this.count += 1;
                Poll::Ready(Some((prev_count, item)))
            }
            None => Poll::Ready(None),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Item> Sink<Item> for Enumerate<S>
where
    S: Stream + Sink<Item>,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
