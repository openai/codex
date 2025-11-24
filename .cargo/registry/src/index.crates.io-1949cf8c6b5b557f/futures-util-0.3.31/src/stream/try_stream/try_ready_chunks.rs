use crate::stream::{Fuse, IntoStream, StreamExt};

use alloc::vec::Vec;
use core::fmt;
use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream, TryStream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`try_ready_chunks`](super::TryStreamExt::try_ready_chunks) method.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct TryReadyChunks<St: TryStream> {
        #[pin]
        stream: Fuse<IntoStream<St>>,
        cap: usize, // https://github.com/rust-lang/futures-rs/issues/1475
    }
}

impl<St: TryStream> TryReadyChunks<St> {
    pub(super) fn new(stream: St, capacity: usize) -> Self {
        assert!(capacity > 0);

        Self { stream: IntoStream::new(stream).fuse(), cap: capacity }
    }

    delegate_access_inner!(stream, St, (. .));
}

type TryReadyChunksStreamError<St> =
    TryReadyChunksError<<St as TryStream>::Ok, <St as TryStream>::Error>;

impl<St: TryStream> Stream for TryReadyChunks<St> {
    type Item = Result<Vec<St::Ok>, TryReadyChunksStreamError<St>>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.as_mut().project();

        let mut items: Vec<St::Ok> = Vec::new();

        loop {
            match this.stream.as_mut().poll_next(cx) {
                // Flush all the collected data if the underlying stream doesn't
                // contain more ready values
                Poll::Pending => {
                    return if items.is_empty() {
                        Poll::Pending
                    } else {
                        Poll::Ready(Some(Ok(items)))
                    }
                }

                // Push the ready item into the buffer and check whether it is full.
                // If so, return the buffer.
                Poll::Ready(Some(Ok(item))) => {
                    if items.is_empty() {
                        items.reserve_exact(*this.cap);
                    }
                    items.push(item);
                    if items.len() >= *this.cap {
                        return Poll::Ready(Some(Ok(items)));
                    }
                }

                // Return the already collected items and the error.
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(TryReadyChunksError(items, e))));
                }

                // Since the underlying stream ran out of values, return what we
                // have buffered, if we have anything.
                Poll::Ready(None) => {
                    let last = if items.is_empty() { None } else { Some(Ok(items)) };
                    return Poll::Ready(last);
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower, upper) = self.stream.size_hint();
        let lower = lower / self.cap;
        (lower, upper)
    }
}

impl<St: TryStream + FusedStream> FusedStream for TryReadyChunks<St> {
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Item> Sink<Item> for TryReadyChunks<S>
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
pub struct TryReadyChunksError<T, E>(pub Vec<T>, pub E);

impl<T, E: fmt::Debug> fmt::Debug for TryReadyChunksError<T, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.1.fmt(f)
    }
}

impl<T, E: fmt::Display> fmt::Display for TryReadyChunksError<T, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.1.fmt(f)
    }
}

#[cfg(feature = "std")]
impl<T, E: fmt::Debug + fmt::Display> std::error::Error for TryReadyChunksError<T, E> {}
