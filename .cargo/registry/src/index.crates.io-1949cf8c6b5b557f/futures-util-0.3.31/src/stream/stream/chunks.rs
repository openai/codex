use crate::stream::Fuse;
use alloc::vec::Vec;
use core::mem;
use core::pin::Pin;
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`chunks`](super::StreamExt::chunks) method.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Chunks<St: Stream> {
        #[pin]
        stream: Fuse<St>,
        items: Vec<St::Item>,
        cap: usize, // https://github.com/rust-lang/futures-rs/issues/1475
    }
}

impl<St: Stream> Chunks<St> {
    pub(super) fn new(stream: St, capacity: usize) -> Self {
        assert!(capacity > 0);

        Self {
            stream: super::Fuse::new(stream),
            items: Vec::with_capacity(capacity),
            cap: capacity,
        }
    }

    fn take(self: Pin<&mut Self>) -> Vec<St::Item> {
        let cap = self.cap;
        mem::replace(self.project().items, Vec::with_capacity(cap))
    }

    delegate_access_inner!(stream, St, (.));
}

impl<St: Stream> Stream for Chunks<St> {
    type Item = Vec<St::Item>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.as_mut().project();
        loop {
            match ready!(this.stream.as_mut().poll_next(cx)) {
                // Push the item into the buffer and check whether it is full.
                // If so, replace our buffer with a new and empty one and return
                // the full one.
                Some(item) => {
                    this.items.push(item);
                    if this.items.len() >= *this.cap {
                        return Poll::Ready(Some(self.take()));
                    }
                }

                // Since the underlying stream ran out of values, return what we
                // have buffered, if we have anything.
                None => {
                    let last = if this.items.is_empty() {
                        None
                    } else {
                        let full_buf = mem::take(this.items);
                        Some(full_buf)
                    };

                    return Poll::Ready(last);
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let chunk_len = usize::from(!self.items.is_empty());
        let (lower, upper) = self.stream.size_hint();
        let lower = (lower / self.cap).saturating_add(chunk_len);
        let upper = match upper {
            Some(x) => x.checked_add(chunk_len),
            None => None,
        };
        (lower, upper)
    }
}

impl<St: FusedStream> FusedStream for Chunks<St> {
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated() && self.items.is_empty()
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Item> Sink<Item> for Chunks<S>
where
    S: Stream + Sink<Item>,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
