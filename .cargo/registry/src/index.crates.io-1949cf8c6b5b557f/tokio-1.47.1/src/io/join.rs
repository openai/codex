//! Join two values implementing `AsyncRead` and `AsyncWrite` into a single one.

use crate::io::{AsyncBufRead, AsyncRead, AsyncWrite, ReadBuf};

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Join two values implementing `AsyncRead` and `AsyncWrite` into a
/// single handle.
pub fn join<R, W>(reader: R, writer: W) -> Join<R, W>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    Join { reader, writer }
}

pin_project_lite::pin_project! {
    /// Joins two values implementing `AsyncRead` and `AsyncWrite` into a
    /// single handle.
    #[derive(Debug)]
    pub struct Join<R, W> {
        #[pin]
        reader: R,
        #[pin]
        writer: W,
    }
}

impl<R, W> Join<R, W>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    /// Splits this `Join` back into its `AsyncRead` and `AsyncWrite`
    /// components.
    pub fn into_inner(self) -> (R, W) {
        (self.reader, self.writer)
    }

    /// Returns a reference to the inner reader.
    pub fn reader(&self) -> &R {
        &self.reader
    }

    /// Returns a reference to the inner writer.
    pub fn writer(&self) -> &W {
        &self.writer
    }

    /// Returns a mutable reference to the inner reader.
    pub fn reader_mut(&mut self) -> &mut R {
        &mut self.reader
    }

    /// Returns a mutable reference to the inner writer.
    pub fn writer_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    /// Returns a pinned mutable reference to the inner reader.
    pub fn reader_pin_mut(self: Pin<&mut Self>) -> Pin<&mut R> {
        self.project().reader
    }

    /// Returns a pinned mutable reference to the inner writer.
    pub fn writer_pin_mut(self: Pin<&mut Self>) -> Pin<&mut W> {
        self.project().writer
    }
}

impl<R, W> AsyncRead for Join<R, W>
where
    R: AsyncRead,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<(), io::Error>> {
        self.project().reader.poll_read(cx, buf)
    }
}

impl<R, W> AsyncWrite for Join<R, W>
where
    W: AsyncWrite,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        self.project().writer.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.project().writer.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.project().writer.poll_shutdown(cx)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        self.project().writer.poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.writer.is_write_vectored()
    }
}

impl<R, W> AsyncBufRead for Join<R, W>
where
    R: AsyncBufRead,
{
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        self.project().reader.poll_fill_buf(cx)
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        self.project().reader.consume(amt)
    }
}
