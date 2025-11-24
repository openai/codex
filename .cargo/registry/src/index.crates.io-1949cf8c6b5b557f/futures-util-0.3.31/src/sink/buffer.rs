use alloc::collections::VecDeque;
use core::pin::Pin;
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Sink for the [`buffer`](super::SinkExt::buffer) method.
    #[derive(Debug)]
    #[must_use = "sinks do nothing unless polled"]
    pub struct Buffer<Si, Item> {
        #[pin]
        sink: Si,
        buf: VecDeque<Item>,

        // Track capacity separately from the `VecDeque`, which may be rounded up
        capacity: usize,
    }
}

impl<Si: Sink<Item>, Item> Buffer<Si, Item> {
    pub(super) fn new(sink: Si, capacity: usize) -> Self {
        Self { sink, buf: VecDeque::with_capacity(capacity), capacity }
    }

    delegate_access_inner!(sink, Si, ());

    fn try_empty_buffer(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Si::Error>> {
        let mut this = self.project();
        ready!(this.sink.as_mut().poll_ready(cx))?;
        while let Some(item) = this.buf.pop_front() {
            this.sink.as_mut().start_send(item)?;
            if !this.buf.is_empty() {
                ready!(this.sink.as_mut().poll_ready(cx))?;
            }
        }
        Poll::Ready(Ok(()))
    }
}

// Forwarding impl of Stream from the underlying sink
impl<S, Item> Stream for Buffer<S, Item>
where
    S: Sink<Item> + Stream,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<S::Item>> {
        self.project().sink.poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.sink.size_hint()
    }
}

impl<S, Item> FusedStream for Buffer<S, Item>
where
    S: Sink<Item> + FusedStream,
{
    fn is_terminated(&self) -> bool {
        self.sink.is_terminated()
    }
}

impl<Si: Sink<Item>, Item> Sink<Item> for Buffer<Si, Item> {
    type Error = Si::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.capacity == 0 {
            return self.project().sink.poll_ready(cx);
        }

        let _ = self.as_mut().try_empty_buffer(cx)?;

        if self.buf.len() >= self.capacity {
            Poll::Pending
        } else {
            Poll::Ready(Ok(()))
        }
    }

    fn start_send(self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error> {
        if self.capacity == 0 {
            self.project().sink.start_send(item)
        } else {
            self.project().buf.push_back(item);
            Ok(())
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().try_empty_buffer(cx))?;
        debug_assert!(self.buf.is_empty());
        self.project().sink.poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().try_empty_buffer(cx))?;
        debug_assert!(self.buf.is_empty());
        self.project().sink.poll_close(cx)
    }
}
