use super::DEFAULT_BUF_SIZE;
use futures_core::future::Future;
use futures_core::ready;
use futures_core::task::{Context, Poll};
use futures_io::{AsyncBufRead, AsyncRead, AsyncSeek, AsyncWrite, IoSliceMut, SeekFrom};
use pin_project_lite::pin_project;
use std::boxed::Box;
use std::io::{self, Read};
use std::pin::Pin;
use std::vec;
use std::{cmp, fmt};

pin_project! {
    /// The `BufReader` struct adds buffering to any reader.
    ///
    /// It can be excessively inefficient to work directly with a [`AsyncRead`]
    /// instance. A `BufReader` performs large, infrequent reads on the underlying
    /// [`AsyncRead`] and maintains an in-memory buffer of the results.
    ///
    /// `BufReader` can improve the speed of programs that make *small* and
    /// *repeated* read calls to the same file or network socket. It does not
    /// help when reading very large amounts at once, or reading just one or a few
    /// times. It also provides no advantage when reading from a source that is
    /// already in memory, like a `Vec<u8>`.
    ///
    /// When the `BufReader` is dropped, the contents of its buffer will be
    /// discarded. Creating multiple instances of a `BufReader` on the same
    /// stream can cause data loss.
    ///
    /// [`AsyncRead`]: futures_io::AsyncRead
    ///
    // TODO: Examples
    pub struct BufReader<R> {
        #[pin]
        inner: R,
        buffer: Box<[u8]>,
        pos: usize,
        cap: usize,
    }
}

impl<R: AsyncRead> BufReader<R> {
    /// Creates a new `BufReader` with a default buffer capacity. The default is currently 8 KB,
    /// but may change in the future.
    pub fn new(inner: R) -> Self {
        Self::with_capacity(DEFAULT_BUF_SIZE, inner)
    }

    /// Creates a new `BufReader` with the specified buffer capacity.
    pub fn with_capacity(capacity: usize, inner: R) -> Self {
        // TODO: consider using Box<[u8]>::new_uninit_slice once it stabilized
        let buffer = vec![0; capacity];
        Self { inner, buffer: buffer.into_boxed_slice(), pos: 0, cap: 0 }
    }
}

impl<R> BufReader<R> {
    delegate_access_inner!(inner, R, ());

    /// Returns a reference to the internally buffered data.
    ///
    /// Unlike `fill_buf`, this will not attempt to fill the buffer if it is empty.
    pub fn buffer(&self) -> &[u8] {
        &self.buffer[self.pos..self.cap]
    }

    /// Invalidates all data in the internal buffer.
    #[inline]
    fn discard_buffer(self: Pin<&mut Self>) {
        let this = self.project();
        *this.pos = 0;
        *this.cap = 0;
    }
}

impl<R: AsyncRead + AsyncSeek> BufReader<R> {
    /// Seeks relative to the current position. If the new position lies within the buffer,
    /// the buffer will not be flushed, allowing for more efficient seeks.
    /// This method does not return the location of the underlying reader, so the caller
    /// must track this information themselves if it is required.
    pub fn seek_relative(self: Pin<&mut Self>, offset: i64) -> SeeKRelative<'_, R> {
        SeeKRelative { inner: self, offset, first: true }
    }

    /// Attempts to seek relative to the current position. If the new position lies within the buffer,
    /// the buffer will not be flushed, allowing for more efficient seeks.
    /// This method does not return the location of the underlying reader, so the caller
    /// must track this information themselves if it is required.
    pub fn poll_seek_relative(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        offset: i64,
    ) -> Poll<io::Result<()>> {
        let pos = self.pos as u64;
        if offset < 0 {
            if let Some(new_pos) = pos.checked_sub((-offset) as u64) {
                *self.project().pos = new_pos as usize;
                return Poll::Ready(Ok(()));
            }
        } else if let Some(new_pos) = pos.checked_add(offset as u64) {
            if new_pos <= self.cap as u64 {
                *self.project().pos = new_pos as usize;
                return Poll::Ready(Ok(()));
            }
        }
        self.poll_seek(cx, SeekFrom::Current(offset)).map(|res| res.map(|_| ()))
    }
}

impl<R: AsyncRead> AsyncRead for BufReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        // If we don't have any buffered data and we're doing a massive read
        // (larger than our internal buffer), bypass our internal buffer
        // entirely.
        if self.pos == self.cap && buf.len() >= self.buffer.len() {
            let res = ready!(self.as_mut().project().inner.poll_read(cx, buf));
            self.discard_buffer();
            return Poll::Ready(res);
        }
        let mut rem = ready!(self.as_mut().poll_fill_buf(cx))?;
        let nread = rem.read(buf)?;
        self.consume(nread);
        Poll::Ready(Ok(nread))
    }

    fn poll_read_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<io::Result<usize>> {
        let total_len = bufs.iter().map(|b| b.len()).sum::<usize>();
        if self.pos == self.cap && total_len >= self.buffer.len() {
            let res = ready!(self.as_mut().project().inner.poll_read_vectored(cx, bufs));
            self.discard_buffer();
            return Poll::Ready(res);
        }
        let mut rem = ready!(self.as_mut().poll_fill_buf(cx))?;
        let nread = rem.read_vectored(bufs)?;
        self.consume(nread);
        Poll::Ready(Ok(nread))
    }
}

impl<R: AsyncRead> AsyncBufRead for BufReader<R> {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        let this = self.project();

        // If we've reached the end of our internal buffer then we need to fetch
        // some more data from the underlying reader.
        // Branch using `>=` instead of the more correct `==`
        // to tell the compiler that the pos..cap slice is always valid.
        if *this.pos >= *this.cap {
            debug_assert!(*this.pos == *this.cap);
            *this.cap = ready!(this.inner.poll_read(cx, this.buffer))?;
            *this.pos = 0;
        }
        Poll::Ready(Ok(&this.buffer[*this.pos..*this.cap]))
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        *self.project().pos = cmp::min(self.pos + amt, self.cap);
    }
}

impl<R: AsyncWrite> AsyncWrite for BufReader<R> {
    delegate_async_write!(inner);
}

impl<R: fmt::Debug> fmt::Debug for BufReader<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BufReader")
            .field("reader", &self.inner)
            .field("buffer", &format_args!("{}/{}", self.cap - self.pos, self.buffer.len()))
            .finish()
    }
}

impl<R: AsyncRead + AsyncSeek> AsyncSeek for BufReader<R> {
    /// Seek to an offset, in bytes, in the underlying reader.
    ///
    /// The position used for seeking with `SeekFrom::Current(_)` is the
    /// position the underlying reader would be at if the `BufReader` had no
    /// internal buffer.
    ///
    /// Seeking always discards the internal buffer, even if the seek position
    /// would otherwise fall within it. This guarantees that calling
    /// `.into_inner()` immediately after a seek yields the underlying reader
    /// at the same position.
    ///
    /// To seek without discarding the internal buffer, use
    /// [`BufReader::seek_relative`](BufReader::seek_relative) or
    /// [`BufReader::poll_seek_relative`](BufReader::poll_seek_relative).
    ///
    /// See [`AsyncSeek`](futures_io::AsyncSeek) for more details.
    ///
    /// Note: In the edge case where you're seeking with `SeekFrom::Current(n)`
    /// where `n` minus the internal buffer length overflows an `i64`, two
    /// seeks will be performed instead of one. If the second seek returns
    /// `Err`, the underlying reader will be left at the same position it would
    /// have if you called `seek` with `SeekFrom::Current(0)`.
    fn poll_seek(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        pos: SeekFrom,
    ) -> Poll<io::Result<u64>> {
        let result: u64;
        if let SeekFrom::Current(n) = pos {
            let remainder = (self.cap - self.pos) as i64;
            // it should be safe to assume that remainder fits within an i64 as the alternative
            // means we managed to allocate 8 exbibytes and that's absurd.
            // But it's not out of the realm of possibility for some weird underlying reader to
            // support seeking by i64::MIN so we need to handle underflow when subtracting
            // remainder.
            if let Some(offset) = n.checked_sub(remainder) {
                result =
                    ready!(self.as_mut().project().inner.poll_seek(cx, SeekFrom::Current(offset)))?;
            } else {
                // seek backwards by our remainder, and then by the offset
                ready!(self.as_mut().project().inner.poll_seek(cx, SeekFrom::Current(-remainder)))?;
                self.as_mut().discard_buffer();
                result = ready!(self.as_mut().project().inner.poll_seek(cx, SeekFrom::Current(n)))?;
            }
        } else {
            // Seeking with Start/End doesn't care about our buffer length.
            result = ready!(self.as_mut().project().inner.poll_seek(cx, pos))?;
        }
        self.discard_buffer();
        Poll::Ready(Ok(result))
    }
}

/// Future for the [`BufReader::seek_relative`](self::BufReader::seek_relative) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct SeeKRelative<'a, R> {
    inner: Pin<&'a mut BufReader<R>>,
    offset: i64,
    first: bool,
}

impl<R> Future for SeeKRelative<'_, R>
where
    R: AsyncRead + AsyncSeek,
{
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let offset = self.offset;
        if self.first {
            self.first = false;
            self.inner.as_mut().poll_seek_relative(cx, offset)
        } else {
            self.inner
                .as_mut()
                .as_mut()
                .poll_seek(cx, SeekFrom::Current(offset))
                .map(|res| res.map(|_| ()))
        }
    }
}
