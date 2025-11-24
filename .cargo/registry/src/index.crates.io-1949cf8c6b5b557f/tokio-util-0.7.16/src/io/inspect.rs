use pin_project_lite::pin_project;
use std::io::{IoSlice, Result};
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pin_project! {
    /// An adapter that lets you inspect the data that's being read.
    ///
    /// This is useful for things like hashing data as it's read in.
    pub struct InspectReader<R, F> {
        #[pin]
        reader: R,
        f: F,
    }
}

impl<R, F> InspectReader<R, F> {
    /// Create a new `InspectReader`, wrapping `reader` and calling `f` for the
    /// new data supplied by each read call.
    ///
    /// The closure will only be called with an empty slice if the inner reader
    /// returns without reading data into the buffer. This happens at EOF, or if
    /// `poll_read` is called with a zero-size buffer.
    pub fn new(reader: R, f: F) -> InspectReader<R, F>
    where
        R: AsyncRead,
        F: FnMut(&[u8]),
    {
        InspectReader { reader, f }
    }

    /// Consumes the `InspectReader`, returning the wrapped reader
    pub fn into_inner(self) -> R {
        self.reader
    }
}

impl<R: AsyncRead, F: FnMut(&[u8])> AsyncRead for InspectReader<R, F> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        let me = self.project();
        let filled_length = buf.filled().len();
        ready!(me.reader.poll_read(cx, buf))?;
        (me.f)(&buf.filled()[filled_length..]);
        Poll::Ready(Ok(()))
    }
}

impl<R: AsyncWrite, F> AsyncWrite for InspectReader<R, F> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::result::Result<usize, std::io::Error>> {
        self.project().reader.poll_write(cx, buf)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::result::Result<(), std::io::Error>> {
        self.project().reader.poll_flush(cx)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::result::Result<(), std::io::Error>> {
        self.project().reader.poll_shutdown(cx)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<Result<usize>> {
        self.project().reader.poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.reader.is_write_vectored()
    }
}

pin_project! {
    /// An adapter that lets you inspect the data that's being written.
    ///
    /// This is useful for things like hashing data as it's written out.
    pub struct InspectWriter<W, F> {
        #[pin]
        writer: W,
        f: F,
    }
}

impl<W, F> InspectWriter<W, F> {
    /// Create a new `InspectWriter`, wrapping `write` and calling `f` for the
    /// data successfully written by each write call.
    ///
    /// The closure `f` will never be called with an empty slice. A vectored
    /// write can result in multiple calls to `f` - at most one call to `f` per
    /// buffer supplied to `poll_write_vectored`.
    pub fn new(writer: W, f: F) -> InspectWriter<W, F>
    where
        W: AsyncWrite,
        F: FnMut(&[u8]),
    {
        InspectWriter { writer, f }
    }

    /// Consumes the `InspectWriter`, returning the wrapped writer
    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: AsyncWrite, F: FnMut(&[u8])> AsyncWrite for InspectWriter<W, F> {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        let me = self.project();
        let res = me.writer.poll_write(cx, buf);
        if let Poll::Ready(Ok(count)) = res {
            if count != 0 {
                (me.f)(&buf[..count]);
            }
        }
        res
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        let me = self.project();
        me.writer.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        let me = self.project();
        me.writer.poll_shutdown(cx)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<Result<usize>> {
        let me = self.project();
        let res = me.writer.poll_write_vectored(cx, bufs);
        if let Poll::Ready(Ok(mut count)) = res {
            for buf in bufs {
                if count == 0 {
                    break;
                }
                let size = count.min(buf.len());
                if size != 0 {
                    (me.f)(&buf[..size]);
                    count -= size;
                }
            }
        }
        res
    }

    fn is_write_vectored(&self) -> bool {
        self.writer.is_write_vectored()
    }
}

impl<W: AsyncRead, F> AsyncRead for InspectWriter<W, F> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        self.project().writer.poll_read(cx, buf)
    }
}
