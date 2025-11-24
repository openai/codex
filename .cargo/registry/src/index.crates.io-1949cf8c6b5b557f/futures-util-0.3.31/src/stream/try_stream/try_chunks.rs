use crate::stream::{Fuse, IntoStream, StreamExt};

use alloc::vec::Vec;
use core::pin::Pin;
use core::{fmt, mem};
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream, TryStream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`try_chunks`](super::TryStreamExt::try_chunks) method.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct TryChunks<St: TryStream> {
        #[pin]
        stream: Fuse<IntoStream<St>>,
        items: Vec<St::Ok>,
        cap: usize, // https://github.com/rust-lang/futures-rs/issues/1475
    }
}

impl<St: TryStream> TryChunks<St> {
    pub(super) fn new(stream: St, capacity: usize) -> Self {
        assert!(capacity > 0);

        Self {
            stream: IntoStream::new(stream).fuse(),
            items: Vec::with_capacity(capacity),
            cap: capacity,
        }
    }

    fn take(self: Pin<&mut Self>) -> Vec<St::Ok> {
        let cap = self.cap;
        mem::replace(self.project().items, Vec::with_capacity(cap))
    }

    delegate_access_inner!(stream, St, (. .));
}

type TryChunksStreamError<St> = TryChunksError<<St as TryStream>::Ok, <St as TryStream>::Error>;

impl<St: TryStream> Stream for TryChunks<St> {
    type Item = Result<Vec<St::Ok>, TryChunksStreamError<St>>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.as_mut().project();
        loop {
            match ready!(this.stream.as_mut().try_poll_next(cx)) {
                // Push the item into the buffer and check whether it is full.
                // If so, replace our buffer with a new and empty one and return
                // the full one.
                Some(item) => match item {
                    Ok(item) => {
                        this.items.push(item);
                        if this.items.len() >= *this.cap {
                            return Poll::Ready(Some(Ok(self.take())));
                        }
                    }
                    Err(e) => {
                        return Poll::Ready(Some(Err(TryChunksError(self.take(), e))));
                    }
                },

                // Since the underlying stream ran out of values, return what we
                // have buffered, if we have anything.
                None => {
                    let last = if this.items.is_empty() {
                        None
                    } else {
                        let full_buf = mem::take(this.items);
                        Some(full_buf)
                    };

                    return Poll::Ready(last.map(Ok));
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

impl<St: TryStream + FusedStream> FusedStream for TryChunks<St> {
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated() && self.items.is_empty()
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Item> Sink<Item> for TryChunks<S>
where
    S: TryStream + Sink<Item>,
{
    type Error = <S as Sink<Item>>::Error;

    delegate_sink!(stream, Item);
}

/// Error indicating, that while chunk was collected inner stream produced an error.
///
/// Contains all items that were collected before an error occurred, and the stream error itself.
#[derive(PartialEq, Eq)]
pub struct TryChunksError<T, E>(pub Vec<T>, pub E);

impl<T, E: fmt::Debug> fmt::Debug for TryChunksError<T, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.1.fmt(f)
    }
}

impl<T, E: fmt::Display> fmt::Display for TryChunksError<T, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.1.fmt(f)
    }
}

#[cfg(feature = "std")]
impl<T, E: fmt::Debug + fmt::Display> std::error::Error for TryChunksError<T, E> {}
