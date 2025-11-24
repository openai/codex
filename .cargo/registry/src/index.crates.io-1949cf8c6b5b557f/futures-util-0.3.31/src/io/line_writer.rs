use super::buf_writer::BufWriter;
use futures_core::ready;
use futures_core::task::{Context, Poll};
use futures_io::AsyncWrite;
use futures_io::IoSlice;
use pin_project_lite::pin_project;
use std::io;
use std::pin::Pin;

pin_project! {
/// Wrap a writer, like [`BufWriter`] does, but prioritizes buffering lines
///
/// This was written based on `std::io::LineWriter` which goes into further details
/// explaining the code.
///
/// Buffering is actually done using `BufWriter`. This class will leverage `BufWriter`
/// to write on-each-line.
#[derive(Debug)]
pub struct LineWriter<W: AsyncWrite> {
    #[pin]
    buf_writer: BufWriter<W>,
}
}

impl<W: AsyncWrite> LineWriter<W> {
    /// Create a new `LineWriter` with default buffer capacity. The default is currently 1KB
    /// which was taken from `std::io::LineWriter`
    pub fn new(inner: W) -> Self {
        Self::with_capacity(1024, inner)
    }

    /// Creates a new `LineWriter` with the specified buffer capacity.
    pub fn with_capacity(capacity: usize, inner: W) -> Self {
        Self { buf_writer: BufWriter::with_capacity(capacity, inner) }
    }

    /// Flush `buf_writer` if last char is "new line"
    fn flush_if_completed_line(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.project();
        match this.buf_writer.buffer().last().copied() {
            Some(b'\n') => this.buf_writer.flush_buf(cx),
            _ => Poll::Ready(Ok(())),
        }
    }

    /// Returns a reference to `buf_writer`'s internally buffered data.
    pub fn buffer(&self) -> &[u8] {
        self.buf_writer.buffer()
    }

    /// Acquires a reference to the underlying sink or stream that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> &W {
        self.buf_writer.get_ref()
    }
}

impl<W: AsyncWrite> AsyncWrite for LineWriter<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let mut this = self.as_mut().project();
        let newline_index = match memchr::memrchr(b'\n', buf) {
            None => {
                ready!(self.as_mut().flush_if_completed_line(cx)?);
                return self.project().buf_writer.poll_write(cx, buf);
            }
            Some(newline_index) => newline_index + 1,
        };

        ready!(this.buf_writer.as_mut().poll_flush(cx)?);

        let lines = &buf[..newline_index];

        let flushed = { ready!(this.buf_writer.as_mut().inner_poll_write(cx, lines))? };

        if flushed == 0 {
            return Poll::Ready(Ok(0));
        }

        let tail = if flushed >= newline_index {
            &buf[flushed..]
        } else if newline_index - flushed <= this.buf_writer.capacity() {
            &buf[flushed..newline_index]
        } else {
            let scan_area = &buf[flushed..];
            let scan_area = &scan_area[..this.buf_writer.capacity()];
            match memchr::memrchr(b'\n', scan_area) {
                Some(newline_index) => &scan_area[..newline_index + 1],
                None => scan_area,
            }
        };

        let buffered = this.buf_writer.as_mut().write_to_buf(tail);
        Poll::Ready(Ok(flushed + buffered))
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        let mut this = self.as_mut().project();
        // `is_write_vectored()` is handled in original code, but not in this crate
        // see https://github.com/rust-lang/rust/issues/70436

        let last_newline_buf_idx = bufs
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, buf)| memchr::memchr(b'\n', buf).map(|_| i));
        let last_newline_buf_idx = match last_newline_buf_idx {
            None => {
                ready!(self.as_mut().flush_if_completed_line(cx)?);
                return self.project().buf_writer.poll_write_vectored(cx, bufs);
            }
            Some(i) => i,
        };

        ready!(this.buf_writer.as_mut().poll_flush(cx)?);

        let (lines, tail) = bufs.split_at(last_newline_buf_idx + 1);

        let flushed = { ready!(this.buf_writer.as_mut().inner_poll_write_vectored(cx, lines))? };
        if flushed == 0 {
            return Poll::Ready(Ok(0));
        }

        let lines_len = lines.iter().map(|buf| buf.len()).sum();
        if flushed < lines_len {
            return Poll::Ready(Ok(flushed));
        }

        let buffered: usize = tail
            .iter()
            .filter(|buf| !buf.is_empty())
            .map(|buf| this.buf_writer.as_mut().write_to_buf(buf))
            .take_while(|&n| n > 0)
            .sum();

        Poll::Ready(Ok(flushed + buffered))
    }

    /// Forward to `buf_writer` 's `BufWriter::poll_flush()`
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.as_mut().project().buf_writer.poll_flush(cx)
    }

    /// Forward to `buf_writer` 's `BufWriter::poll_close()`
    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.as_mut().project().buf_writer.poll_close(cx)
    }
}
