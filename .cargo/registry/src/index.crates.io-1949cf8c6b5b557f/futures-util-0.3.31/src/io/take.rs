use futures_core::ready;
use futures_core::task::{Context, Poll};
use futures_io::{AsyncBufRead, AsyncRead};
use pin_project_lite::pin_project;
use std::pin::Pin;
use std::{cmp, io};

pin_project! {
    /// Reader for the [`take`](super::AsyncReadExt::take) method.
    #[derive(Debug)]
    #[must_use = "readers do nothing unless you `.await` or poll them"]
    pub struct Take<R> {
        #[pin]
        inner: R,
        limit: u64,
    }
}

impl<R: AsyncRead> Take<R> {
    pub(super) fn new(inner: R, limit: u64) -> Self {
        Self { inner, limit }
    }

    /// Returns the remaining number of bytes that can be
    /// read before this instance will return EOF.
    ///
    /// # Note
    ///
    /// This instance may reach `EOF` after reading fewer bytes than indicated by
    /// this method if the underlying [`AsyncRead`] instance reaches EOF.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::io::{AsyncReadExt, Cursor};
    ///
    /// let reader = Cursor::new(&b"12345678"[..]);
    /// let mut buffer = [0; 2];
    ///
    /// let mut take = reader.take(4);
    /// let n = take.read(&mut buffer).await?;
    ///
    /// assert_eq!(take.limit(), 2);
    /// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
    /// ```
    pub fn limit(&self) -> u64 {
        self.limit
    }

    /// Sets the number of bytes that can be read before this instance will
    /// return EOF. This is the same as constructing a new `Take` instance, so
    /// the amount of bytes read and the previous limit value don't matter when
    /// calling this method.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::io::{AsyncReadExt, Cursor};
    ///
    /// let reader = Cursor::new(&b"12345678"[..]);
    /// let mut buffer = [0; 4];
    ///
    /// let mut take = reader.take(4);
    /// let n = take.read(&mut buffer).await?;
    ///
    /// assert_eq!(n, 4);
    /// assert_eq!(take.limit(), 0);
    ///
    /// take.set_limit(10);
    /// let n = take.read(&mut buffer).await?;
    /// assert_eq!(n, 4);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
    /// ```
    pub fn set_limit(&mut self, limit: u64) {
        self.limit = limit
    }

    delegate_access_inner!(inner, R, ());
}

impl<R: AsyncRead> AsyncRead for Take<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, io::Error>> {
        let this = self.project();

        if *this.limit == 0 {
            return Poll::Ready(Ok(0));
        }

        let max = cmp::min(buf.len() as u64, *this.limit) as usize;
        let n = ready!(this.inner.poll_read(cx, &mut buf[..max]))?;
        *this.limit -= n as u64;
        Poll::Ready(Ok(n))
    }
}

impl<R: AsyncBufRead> AsyncBufRead for Take<R> {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        let this = self.project();

        // Don't call into inner reader at all at EOF because it may still block
        if *this.limit == 0 {
            return Poll::Ready(Ok(&[]));
        }

        let buf = ready!(this.inner.poll_fill_buf(cx)?);
        let cap = cmp::min(buf.len() as u64, *this.limit) as usize;
        Poll::Ready(Ok(&buf[..cap]))
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        let this = self.project();

        // Don't let callers reset the limit by passing an overlarge value
        let amt = cmp::min(amt as u64, *this.limit) as usize;
        *this.limit -= amt as u64;
        this.inner.consume(amt);
    }
}
