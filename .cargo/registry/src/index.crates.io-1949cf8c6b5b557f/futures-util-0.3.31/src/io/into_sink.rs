use futures_core::ready;
use futures_core::task::{Context, Poll};
use futures_io::AsyncWrite;
use futures_sink::Sink;
use pin_project_lite::pin_project;
use std::io;
use std::pin::Pin;

#[derive(Debug)]
struct Block<Item> {
    offset: usize,
    bytes: Item,
}

pin_project! {
    /// Sink for the [`into_sink`](super::AsyncWriteExt::into_sink) method.
    #[must_use = "sinks do nothing unless polled"]
    #[derive(Debug)]
    #[cfg_attr(docsrs, doc(cfg(feature = "sink")))]
    pub struct IntoSink<W, Item> {
        #[pin]
        writer: W,
        // An outstanding block for us to push into the underlying writer, along with an offset of how
        // far into this block we have written already.
        buffer: Option<Block<Item>>,
    }
}

impl<W: AsyncWrite, Item: AsRef<[u8]>> IntoSink<W, Item> {
    pub(super) fn new(writer: W) -> Self {
        Self { writer, buffer: None }
    }

    /// If we have an outstanding block in `buffer` attempt to push it into the writer, does _not_
    /// flush the writer after it succeeds in pushing the block into it.
    fn poll_flush_buffer(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        let mut this = self.project();

        if let Some(buffer) = this.buffer {
            loop {
                let bytes = buffer.bytes.as_ref();
                let written = ready!(this.writer.as_mut().poll_write(cx, &bytes[buffer.offset..]))?;
                buffer.offset += written;
                if buffer.offset == bytes.len() {
                    break;
                }
            }
        }
        *this.buffer = None;
        Poll::Ready(Ok(()))
    }
}

impl<W: AsyncWrite, Item: AsRef<[u8]>> Sink<Item> for IntoSink<W, Item> {
    type Error = io::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.poll_flush_buffer(cx))?;
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error> {
        debug_assert!(self.buffer.is_none());
        *self.project().buffer = Some(Block { offset: 0, bytes: item });
        Ok(())
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().poll_flush_buffer(cx))?;
        ready!(self.project().writer.poll_flush(cx))?;
        Poll::Ready(Ok(()))
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().poll_flush_buffer(cx))?;
        ready!(self.project().writer.poll_close(cx))?;
        Poll::Ready(Ok(()))
    }
}
