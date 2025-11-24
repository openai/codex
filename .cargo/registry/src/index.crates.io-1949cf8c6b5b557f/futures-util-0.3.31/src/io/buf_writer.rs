use super::DEFAULT_BUF_SIZE;
use futures_core::ready;
use futures_core::task::{Context, Poll};
use futures_io::{AsyncBufRead, AsyncRead, AsyncSeek, AsyncWrite, IoSlice, SeekFrom};
use pin_project_lite::pin_project;
use std::fmt;
use std::io::{self, Write};
use std::pin::Pin;
use std::ptr;
use std::vec::Vec;

pin_project! {
    /// Wraps a writer and buffers its output.
    ///
    /// It can be excessively inefficient to work directly with something that
    /// implements [`AsyncWrite`]. A `BufWriter` keeps an in-memory buffer of data and
    /// writes it to an underlying writer in large, infrequent batches.
    ///
    /// `BufWriter` can improve the speed of programs that make *small* and
    /// *repeated* write calls to the same file or network socket. It does not
    /// help when writing very large amounts at once, or writing just one or a few
    /// times. It also provides no advantage when writing to a destination that is
    /// in memory, like a `Vec<u8>`.
    ///
    /// When the `BufWriter` is dropped, the contents of its buffer will be
    /// discarded. Creating multiple instances of a `BufWriter` on the same
    /// stream can cause data loss. If you need to write out the contents of its
    /// buffer, you must manually call flush before the writer is dropped.
    ///
    /// [`AsyncWrite`]: futures_io::AsyncWrite
    /// [`flush`]: super::AsyncWriteExt::flush
    ///
    // TODO: Examples
    pub struct BufWriter<W> {
        #[pin]
        inner: W,
        buf: Vec<u8>,
        written: usize,
    }
}

impl<W: AsyncWrite> BufWriter<W> {
    /// Creates a new `BufWriter` with a default buffer capacity. The default is currently 8 KB,
    /// but may change in the future.
    pub fn new(inner: W) -> Self {
        Self::with_capacity(DEFAULT_BUF_SIZE, inner)
    }

    /// Creates a new `BufWriter` with the specified buffer capacity.
    pub fn with_capacity(cap: usize, inner: W) -> Self {
        Self { inner, buf: Vec::with_capacity(cap), written: 0 }
    }

    pub(super) fn flush_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let mut this = self.project();

        let len = this.buf.len();
        let mut ret = Ok(());
        while *this.written < len {
            match ready!(this.inner.as_mut().poll_write(cx, &this.buf[*this.written..])) {
                Ok(0) => {
                    ret = Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "failed to write the buffered data",
                    ));
                    break;
                }
                Ok(n) => *this.written += n,
                Err(e) => {
                    ret = Err(e);
                    break;
                }
            }
        }
        if *this.written > 0 {
            this.buf.drain(..*this.written);
        }
        *this.written = 0;
        Poll::Ready(ret)
    }

    /// Write directly using `inner`, bypassing buffering
    pub(super) fn inner_poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.project().inner.poll_write(cx, buf)
    }

    /// Write directly using `inner`, bypassing buffering
    pub(super) fn inner_poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        self.project().inner.poll_write_vectored(cx, bufs)
    }
}

impl<W> BufWriter<W> {
    delegate_access_inner!(inner, W, ());

    /// Returns a reference to the internally buffered data.
    pub fn buffer(&self) -> &[u8] {
        &self.buf
    }

    /// Capacity of `buf`. how many chars can be held in buffer
    pub(super) fn capacity(&self) -> usize {
        self.buf.capacity()
    }

    /// Remaining number of bytes to reach `buf` 's capacity
    #[inline]
    pub(super) fn spare_capacity(&self) -> usize {
        self.buf.capacity() - self.buf.len()
    }

    /// Write a byte slice directly into buffer
    ///
    /// Will truncate the number of bytes written to `spare_capacity()` so you want to
    /// calculate the size of your slice to avoid losing bytes
    ///
    /// Based on `std::io::BufWriter`
    pub(super) fn write_to_buf(self: Pin<&mut Self>, buf: &[u8]) -> usize {
        let available = self.spare_capacity();
        let amt_to_buffer = available.min(buf.len());

        // SAFETY: `amt_to_buffer` is <= buffer's spare capacity by construction.
        unsafe {
            self.write_to_buffer_unchecked(&buf[..amt_to_buffer]);
        }

        amt_to_buffer
    }

    /// Write byte slice directly into `self.buf`
    ///
    /// Based on `std::io::BufWriter`
    #[inline]
    unsafe fn write_to_buffer_unchecked(self: Pin<&mut Self>, buf: &[u8]) {
        debug_assert!(buf.len() <= self.spare_capacity());
        let this = self.project();
        let old_len = this.buf.len();
        let buf_len = buf.len();
        let src = buf.as_ptr();
        unsafe {
            let dst = this.buf.as_mut_ptr().add(old_len);
            ptr::copy_nonoverlapping(src, dst, buf_len);
            this.buf.set_len(old_len + buf_len);
        }
    }
}

impl<W: AsyncWrite> AsyncWrite for BufWriter<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if self.buf.len() + buf.len() > self.buf.capacity() {
            ready!(self.as_mut().flush_buf(cx))?;
        }
        if buf.len() >= self.buf.capacity() {
            self.project().inner.poll_write(cx, buf)
        } else {
            Poll::Ready(self.project().buf.write(buf))
        }
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        let total_len = bufs.iter().map(|b| b.len()).sum::<usize>();
        if self.buf.len() + total_len > self.buf.capacity() {
            ready!(self.as_mut().flush_buf(cx))?;
        }
        if total_len >= self.buf.capacity() {
            self.project().inner.poll_write_vectored(cx, bufs)
        } else {
            Poll::Ready(self.project().buf.write_vectored(bufs))
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        ready!(self.as_mut().flush_buf(cx))?;
        self.project().inner.poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        ready!(self.as_mut().flush_buf(cx))?;
        self.project().inner.poll_close(cx)
    }
}

impl<W: AsyncRead> AsyncRead for BufWriter<W> {
    delegate_async_read!(inner);
}

impl<W: AsyncBufRead> AsyncBufRead for BufWriter<W> {
    delegate_async_buf_read!(inner);
}

impl<W: fmt::Debug> fmt::Debug for BufWriter<W> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BufWriter")
            .field("writer", &self.inner)
            .field("buffer", &format_args!("{}/{}", self.buf.len(), self.buf.capacity()))
            .field("written", &self.written)
            .finish()
    }
}

impl<W: AsyncWrite + AsyncSeek> AsyncSeek for BufWriter<W> {
    /// Seek to the offset, in bytes, in the underlying writer.
    ///
    /// Seeking always writes out the internal buffer before seeking.
    fn poll_seek(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        pos: SeekFrom,
    ) -> Poll<io::Result<u64>> {
        ready!(self.as_mut().flush_buf(cx))?;
        self.project().inner.poll_seek(cx, pos)
    }
}
