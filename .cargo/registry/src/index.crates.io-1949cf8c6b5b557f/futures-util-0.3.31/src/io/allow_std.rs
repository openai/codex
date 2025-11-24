use futures_core::task::{Context, Poll};
use futures_io::{AsyncBufRead, AsyncRead, AsyncSeek, AsyncWrite, IoSlice, IoSliceMut, SeekFrom};
use std::pin::Pin;
use std::string::String;
use std::vec::Vec;
use std::{fmt, io};

/// A simple wrapper type which allows types which implement only
/// implement `std::io::Read` or `std::io::Write`
/// to be used in contexts which expect an `AsyncRead` or `AsyncWrite`.
///
/// If these types issue an error with the kind `io::ErrorKind::WouldBlock`,
/// it is expected that they will notify the current task on readiness.
/// Synchronous `std` types should not issue errors of this kind and
/// are safe to use in this context. However, using these types with
/// `AllowStdIo` will cause the event loop to block, so they should be used
/// with care.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct AllowStdIo<T>(T);

impl<T> Unpin for AllowStdIo<T> {}

macro_rules! try_with_interrupt {
    ($e:expr) => {
        loop {
            match $e {
                Ok(e) => {
                    break e;
                }
                Err(ref e) if e.kind() == ::std::io::ErrorKind::Interrupted => {
                    continue;
                }
                Err(e) => {
                    return Poll::Ready(Err(e));
                }
            }
        }
    };
}

impl<T> AllowStdIo<T> {
    /// Creates a new `AllowStdIo` from an existing IO object.
    pub fn new(io: T) -> Self {
        Self(io)
    }

    /// Returns a reference to the contained IO object.
    pub fn get_ref(&self) -> &T {
        &self.0
    }

    /// Returns a mutable reference to the contained IO object.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.0
    }

    /// Consumes self and returns the contained IO object.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> io::Write for AllowStdIo<T>
where
    T: io::Write,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.0.write_vectored(bufs)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.0.write_all(buf)
    }
    fn write_fmt(&mut self, fmt: fmt::Arguments<'_>) -> io::Result<()> {
        self.0.write_fmt(fmt)
    }
}

impl<T> AsyncWrite for AllowStdIo<T>
where
    T: io::Write,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(Ok(try_with_interrupt!(self.0.write(buf))))
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(Ok(try_with_interrupt!(self.0.write_vectored(bufs))))
    }

    fn poll_flush(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        try_with_interrupt!(self.0.flush());
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_flush(cx)
    }
}

impl<T> io::Read for AllowStdIo<T>
where
    T: io::Read,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.0.read_vectored(bufs)
    }
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.0.read_to_end(buf)
    }
    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        self.0.read_to_string(buf)
    }
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.0.read_exact(buf)
    }
}

impl<T> AsyncRead for AllowStdIo<T>
where
    T: io::Read,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(Ok(try_with_interrupt!(self.0.read(buf))))
    }

    fn poll_read_vectored(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(Ok(try_with_interrupt!(self.0.read_vectored(bufs))))
    }
}

impl<T> io::Seek for AllowStdIo<T>
where
    T: io::Seek,
{
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.0.seek(pos)
    }
}

impl<T> AsyncSeek for AllowStdIo<T>
where
    T: io::Seek,
{
    fn poll_seek(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        pos: SeekFrom,
    ) -> Poll<io::Result<u64>> {
        Poll::Ready(Ok(try_with_interrupt!(self.0.seek(pos))))
    }
}

impl<T> io::BufRead for AllowStdIo<T>
where
    T: io::BufRead,
{
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.0.fill_buf()
    }
    fn consume(&mut self, amt: usize) {
        self.0.consume(amt)
    }
}

impl<T> AsyncBufRead for AllowStdIo<T>
where
    T: io::BufRead,
{
    fn poll_fill_buf(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        let this: *mut Self = &mut *self as *mut _;
        Poll::Ready(Ok(try_with_interrupt!(unsafe { &mut *this }.0.fill_buf())))
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        self.0.consume(amt)
    }
}
