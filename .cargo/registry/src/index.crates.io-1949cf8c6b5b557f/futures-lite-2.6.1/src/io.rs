//! Tools and combinators for I/O.
//!
//! # Examples
//!
//! ```
//! use futures_lite::io::{self, AsyncReadExt};
//!
//! # spin_on::spin_on(async {
//! let input: &[u8] = b"hello";
//! let mut reader = io::BufReader::new(input);
//!
//! let mut contents = String::new();
//! reader.read_to_string(&mut contents).await?;
//! # std::io::Result::Ok(()) });
//! ```

#[doc(no_inline)]
pub use std::io::{Error, ErrorKind, Result, SeekFrom};

#[doc(no_inline)]
pub use futures_io::{AsyncBufRead, AsyncRead, AsyncSeek, AsyncWrite};

use std::borrow::{Borrow, BorrowMut};
use std::boxed::Box;
use std::cmp;
use std::fmt;
use std::future::Future;
use std::io::{IoSlice, IoSliceMut};
use std::mem;
use std::pin::Pin;
use std::string::String;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::vec;
use std::vec::Vec;

use futures_core::stream::Stream;
use pin_project_lite::pin_project;

use crate::future;
use crate::ready;

const DEFAULT_BUF_SIZE: usize = 8 * 1024;

/// Copies the entire contents of a reader into a writer.
///
/// This function will read data from `reader` and write it into `writer` in a streaming fashion
/// until `reader` returns EOF.
///
/// On success, returns the total number of bytes copied.
///
/// # Examples
///
/// ```
/// use futures_lite::io::{self, BufReader, BufWriter};
///
/// # spin_on::spin_on(async {
/// let input: &[u8] = b"hello";
/// let reader = BufReader::new(input);
///
/// let mut output = Vec::new();
/// let writer = BufWriter::new(&mut output);
///
/// io::copy(reader, writer).await?;
/// # std::io::Result::Ok(()) });
/// ```
pub async fn copy<R, W>(reader: R, writer: W) -> Result<u64>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    pin_project! {
        struct CopyFuture<R, W> {
            #[pin]
            reader: R,
            #[pin]
            writer: W,
            amt: u64,
        }
    }

    impl<R, W> Future for CopyFuture<R, W>
    where
        R: AsyncBufRead,
        W: AsyncWrite,
    {
        type Output = Result<u64>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let mut this = self.project();
            loop {
                let buffer = ready!(this.reader.as_mut().poll_fill_buf(cx))?;
                if buffer.is_empty() {
                    ready!(this.writer.as_mut().poll_flush(cx))?;
                    return Poll::Ready(Ok(*this.amt));
                }

                let i = ready!(this.writer.as_mut().poll_write(cx, buffer))?;
                if i == 0 {
                    return Poll::Ready(Err(ErrorKind::WriteZero.into()));
                }
                *this.amt += i as u64;
                this.reader.as_mut().consume(i);
            }
        }
    }

    let future = CopyFuture {
        reader: BufReader::new(reader),
        writer,
        amt: 0,
    };
    future.await
}

/// Asserts that a type implementing [`std::io`] traits can be used as an async type.
///
/// The underlying I/O handle should never block nor return the [`ErrorKind::WouldBlock`] error.
/// This is usually the case for in-memory buffered I/O.
///
/// # Examples
///
/// ```
/// use futures_lite::io::{AssertAsync, AsyncReadExt};
///
/// let reader: &[u8] = b"hello";
///
/// # spin_on::spin_on(async {
/// let mut async_reader = AssertAsync::new(reader);
/// let mut contents = String::new();
///
/// // This line works in async manner - note that there is await:
/// async_reader.read_to_string(&mut contents).await?;
/// # std::io::Result::Ok(()) });
/// ```
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct AssertAsync<T>(T);

impl<T> Unpin for AssertAsync<T> {}

impl<T> AssertAsync<T> {
    /// Wraps an I/O handle implementing [`std::io`] traits.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::AssertAsync;
    ///
    /// let reader: &[u8] = b"hello";
    ///
    /// let async_reader = AssertAsync::new(reader);
    /// ```
    #[inline(always)]
    pub fn new(io: T) -> Self {
        AssertAsync(io)
    }

    /// Gets a reference to the inner I/O handle.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::AssertAsync;
    ///
    /// let reader: &[u8] = b"hello";
    ///
    /// let async_reader = AssertAsync::new(reader);
    /// let r = async_reader.get_ref();
    /// ```
    #[inline(always)]
    pub fn get_ref(&self) -> &T {
        &self.0
    }

    /// Gets a mutable reference to the inner I/O handle.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::AssertAsync;
    ///
    /// let reader: &[u8] = b"hello";
    ///
    /// let mut async_reader = AssertAsync::new(reader);
    /// let r = async_reader.get_mut();
    /// ```
    #[inline(always)]
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.0
    }

    /// Extracts the inner I/O handle.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::AssertAsync;
    ///
    /// let reader: &[u8] = b"hello";
    ///
    /// let async_reader = AssertAsync::new(reader);
    /// let inner = async_reader.into_inner();
    /// ```
    #[inline(always)]
    pub fn into_inner(self) -> T {
        self.0
    }
}

fn assert_async_wrapio<F, T>(mut f: F) -> Poll<std::io::Result<T>>
where
    F: FnMut() -> std::io::Result<T>,
{
    loop {
        match f() {
            Err(err) if err.kind() == ErrorKind::Interrupted => {}
            res => return Poll::Ready(res),
        }
    }
}

impl<T: std::io::Read> AsyncRead for AssertAsync<T> {
    #[inline]
    fn poll_read(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        assert_async_wrapio(move || self.0.read(buf))
    }

    #[inline]
    fn poll_read_vectored(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<Result<usize>> {
        assert_async_wrapio(move || self.0.read_vectored(bufs))
    }
}

impl<T: std::io::Write> AsyncWrite for AssertAsync<T> {
    #[inline]
    fn poll_write(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        assert_async_wrapio(move || self.0.write(buf))
    }

    #[inline]
    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<Result<usize>> {
        assert_async_wrapio(move || self.0.write_vectored(bufs))
    }

    #[inline]
    fn poll_flush(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
        assert_async_wrapio(move || self.0.flush())
    }

    #[inline]
    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        self.poll_flush(cx)
    }
}

impl<T: std::io::Seek> AsyncSeek for AssertAsync<T> {
    #[inline]
    fn poll_seek(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        pos: SeekFrom,
    ) -> Poll<Result<u64>> {
        assert_async_wrapio(move || self.0.seek(pos))
    }
}

/// A wrapper around a type that implements `AsyncRead` or `AsyncWrite` that converts `Pending`
/// polls to `WouldBlock` errors.
///
/// This wrapper can be used as a compatibility layer between `AsyncRead` and `Read`, for types
/// that take `Read` as a parameter.
///
/// # Examples
///
/// ```
/// use std::io::Read;
/// use std::task::{Poll, Context};
///
/// fn poll_for_io(cx: &mut Context<'_>) -> Poll<usize> {
///     // Assume we have a library that's built around `Read` and `Write` traits.
///     use cooltls::Session;
///
///     // We want to use it with our writer that implements `AsyncWrite`.
///     let writer = Stream::new();
///
///     // First, we wrap our `Writer` with `AsyncAsSync` to convert `Pending` polls to `WouldBlock`.
///     use futures_lite::io::AsyncAsSync;
///     let writer = AsyncAsSync::new(cx, writer);
///
///     // Now, we can use it with `cooltls`.
///     let mut session = Session::new(writer);
///
///     // Match on the result of `read()` and translate it to poll.
///     match session.read(&mut [0; 1024]) {
///         Ok(n) => Poll::Ready(n),
///         Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => Poll::Pending,
///         Err(err) => panic!("unexpected error: {}", err),
///     }
/// }
///
/// // Usually, poll-based functions are best wrapped using `poll_fn`.
/// use futures_lite::future::poll_fn;
/// # futures_lite::future::block_on(async {
/// poll_fn(|cx| poll_for_io(cx)).await;
/// # });
/// # struct Stream;
/// # impl Stream {
/// #     fn new() -> Stream {
/// #         Stream
/// #     }
/// # }
/// # impl futures_lite::io::AsyncRead for Stream {
/// #     fn poll_read(self: std::pin::Pin<&mut Self>, _: &mut Context<'_>, _: &mut [u8]) -> Poll<std::io::Result<usize>> {
/// #         Poll::Ready(Ok(0))
/// #     }
/// # }
/// # mod cooltls {
/// #     pub struct Session<W> {
/// #         reader: W,
/// #     }
/// #     impl<W> Session<W> {
/// #         pub fn new(reader: W) -> Session<W> {
/// #             Session { reader }
/// #         }
/// #     }
/// #     impl<W: std::io::Read> std::io::Read for Session<W> {
/// #         fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
/// #             self.reader.read(buf)
/// #         }
/// #     }
/// # }
/// ```
#[derive(Debug)]
pub struct AsyncAsSync<'r, 'ctx, T> {
    /// The context we are using to poll the future.
    pub context: &'r mut Context<'ctx>,

    /// The actual reader/writer we are wrapping.
    pub inner: T,
}

impl<'r, 'ctx, T> AsyncAsSync<'r, 'ctx, T> {
    /// Wraps an I/O handle implementing [`AsyncRead`] or [`AsyncWrite`] traits.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::AsyncAsSync;
    /// use std::task::Context;
    /// use waker_fn::waker_fn;
    ///
    /// let reader: &[u8] = b"hello";
    /// let waker = waker_fn(|| {});
    /// let mut context = Context::from_waker(&waker);
    ///
    /// let async_reader = AsyncAsSync::new(&mut context, reader);
    /// ```
    #[inline]
    pub fn new(context: &'r mut Context<'ctx>, inner: T) -> Self {
        AsyncAsSync { context, inner }
    }

    /// Attempt to shutdown the I/O handle.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::AsyncAsSync;
    /// use std::task::Context;
    /// use waker_fn::waker_fn;
    ///
    /// let reader: Vec<u8> = b"hello".to_vec();
    /// let waker = waker_fn(|| {});
    /// let mut context = Context::from_waker(&waker);
    ///
    /// let mut async_reader = AsyncAsSync::new(&mut context, reader);
    /// async_reader.close().unwrap();
    /// ```
    #[inline]
    pub fn close(&mut self) -> Result<()>
    where
        T: AsyncWrite + Unpin,
    {
        self.poll_with(|io, cx| io.poll_close(cx))
    }

    /// Poll this `AsyncAsSync` for some function.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncAsSync, AsyncRead};
    /// use std::task::Context;
    /// use waker_fn::waker_fn;
    ///
    /// let reader: &[u8] = b"hello";
    /// let waker = waker_fn(|| {});
    /// let mut context = Context::from_waker(&waker);
    ///
    /// let mut async_reader = AsyncAsSync::new(&mut context, reader);
    /// let r = async_reader.poll_with(|io, cx| io.poll_read(cx, &mut [0; 1024]));
    /// assert_eq!(r.unwrap(), 5);
    /// ```
    #[inline]
    pub fn poll_with<R>(
        &mut self,
        f: impl FnOnce(Pin<&mut T>, &mut Context<'_>) -> Poll<Result<R>>,
    ) -> Result<R>
    where
        T: Unpin,
    {
        match f(Pin::new(&mut self.inner), self.context) {
            Poll::Ready(res) => res,
            Poll::Pending => Err(ErrorKind::WouldBlock.into()),
        }
    }
}

impl<T: AsyncRead + Unpin> std::io::Read for AsyncAsSync<'_, '_, T> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.poll_with(|io, cx| io.poll_read(cx, buf))
    }

    #[inline]
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize> {
        self.poll_with(|io, cx| io.poll_read_vectored(cx, bufs))
    }
}

impl<T: AsyncWrite + Unpin> std::io::Write for AsyncAsSync<'_, '_, T> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.poll_with(|io, cx| io.poll_write(cx, buf))
    }

    #[inline]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> Result<usize> {
        self.poll_with(|io, cx| io.poll_write_vectored(cx, bufs))
    }

    #[inline]
    fn flush(&mut self) -> Result<()> {
        self.poll_with(|io, cx| io.poll_flush(cx))
    }
}

impl<T: AsyncSeek + Unpin> std::io::Seek for AsyncAsSync<'_, '_, T> {
    #[inline]
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        self.poll_with(|io, cx| io.poll_seek(cx, pos))
    }
}

impl<T> AsRef<T> for AsyncAsSync<'_, '_, T> {
    #[inline]
    fn as_ref(&self) -> &T {
        &self.inner
    }
}

impl<T> AsMut<T> for AsyncAsSync<'_, '_, T> {
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T> Borrow<T> for AsyncAsSync<'_, '_, T> {
    #[inline]
    fn borrow(&self) -> &T {
        &self.inner
    }
}

impl<T> BorrowMut<T> for AsyncAsSync<'_, '_, T> {
    #[inline]
    fn borrow_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

/// Blocks on all async I/O operations and implements [`std::io`] traits.
///
/// Sometimes async I/O needs to be used in a blocking manner. If calling [`future::block_on()`]
/// manually all the time becomes too tedious, use this type for more convenient blocking on async
/// I/O operations.
///
/// This type implements traits [`Read`][`std::io::Read`], [`Write`][`std::io::Write`], or
/// [`Seek`][`std::io::Seek`] if the inner type implements [`AsyncRead`], [`AsyncWrite`], or
/// [`AsyncSeek`], respectively.
///
/// If writing data through the [`Write`][`std::io::Write`] trait, make sure to flush before
/// dropping the [`BlockOn`] handle or some buffered data might get lost.
///
/// # Examples
///
/// ```
/// use futures_lite::io::BlockOn;
/// use futures_lite::pin;
/// use std::io::Read;
///
/// let reader: &[u8] = b"hello";
/// pin!(reader);
///
/// let mut blocking_reader = BlockOn::new(reader);
/// let mut contents = String::new();
///
/// // This line blocks - note that there is no await:
/// blocking_reader.read_to_string(&mut contents)?;
/// # std::io::Result::Ok(())
/// ```
#[derive(Debug)]
pub struct BlockOn<T>(T);

impl<T> BlockOn<T> {
    /// Wraps an async I/O handle into a blocking interface.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::BlockOn;
    /// use futures_lite::pin;
    ///
    /// let reader: &[u8] = b"hello";
    /// pin!(reader);
    ///
    /// let blocking_reader = BlockOn::new(reader);
    /// ```
    pub fn new(io: T) -> BlockOn<T> {
        BlockOn(io)
    }

    /// Gets a reference to the async I/O handle.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::BlockOn;
    /// use futures_lite::pin;
    ///
    /// let reader: &[u8] = b"hello";
    /// pin!(reader);
    ///
    /// let blocking_reader = BlockOn::new(reader);
    /// let r = blocking_reader.get_ref();
    /// ```
    pub fn get_ref(&self) -> &T {
        &self.0
    }

    /// Gets a mutable reference to the async I/O handle.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::BlockOn;
    /// use futures_lite::pin;
    ///
    /// let reader: &[u8] = b"hello";
    /// pin!(reader);
    ///
    /// let mut blocking_reader = BlockOn::new(reader);
    /// let r = blocking_reader.get_mut();
    /// ```
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.0
    }

    /// Extracts the inner async I/O handle.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::BlockOn;
    /// use futures_lite::pin;
    ///
    /// let reader: &[u8] = b"hello";
    /// pin!(reader);
    ///
    /// let blocking_reader = BlockOn::new(reader);
    /// let inner = blocking_reader.into_inner();
    /// ```
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T: AsyncRead + Unpin> std::io::Read for BlockOn<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        future::block_on(self.0.read(buf))
    }
}

impl<T: AsyncBufRead + Unpin> std::io::BufRead for BlockOn<T> {
    fn fill_buf(&mut self) -> Result<&[u8]> {
        future::block_on(self.0.fill_buf())
    }

    fn consume(&mut self, amt: usize) {
        Pin::new(&mut self.0).consume(amt)
    }
}

impl<T: AsyncWrite + Unpin> std::io::Write for BlockOn<T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        future::block_on(self.0.write(buf))
    }

    fn flush(&mut self) -> Result<()> {
        future::block_on(self.0.flush())
    }
}

impl<T: AsyncSeek + Unpin> std::io::Seek for BlockOn<T> {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        future::block_on(self.0.seek(pos))
    }
}

pin_project! {
    /// Adds buffering to a reader.
    ///
    /// It can be excessively inefficient to work directly with an [`AsyncRead`] instance. A
    /// [`BufReader`] performs large, infrequent reads on the underlying [`AsyncRead`] and
    /// maintains an in-memory buffer of the incoming byte stream.
    ///
    /// [`BufReader`] can improve the speed of programs that make *small* and *repeated* reads to
    /// the same file or networking socket. It does not help when reading very large amounts at
    /// once, or reading just once or a few times. It also provides no advantage when reading from
    /// a source that is already in memory, like a `Vec<u8>`.
    ///
    /// When a [`BufReader`] is dropped, the contents of its buffer are discarded. Creating
    /// multiple instances of [`BufReader`] on the same reader can cause data loss.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncBufReadExt, BufReader};
    ///
    /// # spin_on::spin_on(async {
    /// let input: &[u8] = b"hello";
    /// let mut reader = BufReader::new(input);
    ///
    /// let mut line = String::new();
    /// reader.read_line(&mut line).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub struct BufReader<R> {
        #[pin]
        inner: R,
        buf: Box<[u8]>,
        pos: usize,
        cap: usize,
    }
}

impl<R: AsyncRead> BufReader<R> {
    /// Creates a buffered reader with the default buffer capacity.
    ///
    /// The default capacity is currently 8 KB, but that may change in the future.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::BufReader;
    ///
    /// let input: &[u8] = b"hello";
    /// let reader = BufReader::new(input);
    /// ```
    pub fn new(inner: R) -> BufReader<R> {
        BufReader::with_capacity(DEFAULT_BUF_SIZE, inner)
    }

    /// Creates a buffered reader with the specified capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::BufReader;
    ///
    /// let input: &[u8] = b"hello";
    /// let reader = BufReader::with_capacity(1024, input);
    /// ```
    pub fn with_capacity(capacity: usize, inner: R) -> BufReader<R> {
        BufReader {
            inner,
            buf: vec![0; capacity].into_boxed_slice(),
            pos: 0,
            cap: 0,
        }
    }
}

impl<R> BufReader<R> {
    /// Gets a reference to the underlying reader.
    ///
    /// It is not advisable to directly read from the underlying reader.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::BufReader;
    ///
    /// let input: &[u8] = b"hello";
    /// let reader = BufReader::new(input);
    ///
    /// let r = reader.get_ref();
    /// ```
    pub fn get_ref(&self) -> &R {
        &self.inner
    }

    /// Gets a mutable reference to the underlying reader.
    ///
    /// It is not advisable to directly read from the underlying reader.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::BufReader;
    ///
    /// let input: &[u8] = b"hello";
    /// let mut reader = BufReader::new(input);
    ///
    /// let r = reader.get_mut();
    /// ```
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    /// Gets a pinned mutable reference to the underlying reader.
    ///
    /// It is not advisable to directly read from the underlying reader.
    fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut R> {
        self.project().inner
    }

    /// Returns a reference to the internal buffer.
    ///
    /// This method will not attempt to fill the buffer if it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::BufReader;
    ///
    /// let input: &[u8] = b"hello";
    /// let reader = BufReader::new(input);
    ///
    /// // The internal buffer is empty until the first read request.
    /// assert_eq!(reader.buffer(), &[]);
    /// ```
    pub fn buffer(&self) -> &[u8] {
        &self.buf[self.pos..self.cap]
    }

    /// Unwraps the buffered reader, returning the underlying reader.
    ///
    /// Note that any leftover data in the internal buffer will be lost.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::BufReader;
    ///
    /// let input: &[u8] = b"hello";
    /// let reader = BufReader::new(input);
    ///
    /// assert_eq!(reader.into_inner(), input);
    /// ```
    pub fn into_inner(self) -> R {
        self.inner
    }

    /// Invalidates all data in the internal buffer.
    #[inline]
    fn discard_buffer(self: Pin<&mut Self>) {
        let this = self.project();
        *this.pos = 0;
        *this.cap = 0;
    }
}

impl<R: AsyncRead> AsyncRead for BufReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        // If we don't have any buffered data and we're doing a massive read
        // (larger than our internal buffer), bypass our internal buffer
        // entirely.
        if self.pos == self.cap && buf.len() >= self.buf.len() {
            let res = ready!(self.as_mut().get_pin_mut().poll_read(cx, buf));
            self.discard_buffer();
            return Poll::Ready(res);
        }
        let mut rem = ready!(self.as_mut().poll_fill_buf(cx))?;
        let nread = std::io::Read::read(&mut rem, buf)?;
        self.consume(nread);
        Poll::Ready(Ok(nread))
    }

    fn poll_read_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<Result<usize>> {
        let total_len = bufs.iter().map(|b| b.len()).sum::<usize>();
        if self.pos == self.cap && total_len >= self.buf.len() {
            let res = ready!(self.as_mut().get_pin_mut().poll_read_vectored(cx, bufs));
            self.discard_buffer();
            return Poll::Ready(res);
        }
        let mut rem = ready!(self.as_mut().poll_fill_buf(cx))?;
        let nread = std::io::Read::read_vectored(&mut rem, bufs)?;
        self.consume(nread);
        Poll::Ready(Ok(nread))
    }
}

impl<R: AsyncRead> AsyncBufRead for BufReader<R> {
    fn poll_fill_buf<'a>(self: Pin<&'a mut Self>, cx: &mut Context<'_>) -> Poll<Result<&'a [u8]>> {
        let mut this = self.project();

        // If we've reached the end of our internal buffer then we need to fetch
        // some more data from the underlying reader.
        // Branch using `>=` instead of the more correct `==`
        // to tell the compiler that the pos..cap slice is always valid.
        if *this.pos >= *this.cap {
            debug_assert!(*this.pos == *this.cap);
            *this.cap = ready!(this.inner.as_mut().poll_read(cx, this.buf))?;
            *this.pos = 0;
        }
        Poll::Ready(Ok(&this.buf[*this.pos..*this.cap]))
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        let this = self.project();
        *this.pos = cmp::min(*this.pos + amt, *this.cap);
    }
}

impl<R: fmt::Debug> fmt::Debug for BufReader<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BufReader")
            .field("reader", &self.inner)
            .field(
                "buffer",
                &format_args!("{}/{}", self.cap - self.pos, self.buf.len()),
            )
            .finish()
    }
}

impl<R: AsyncSeek> AsyncSeek for BufReader<R> {
    /// Seeks to an offset, in bytes, in the underlying reader.
    ///
    /// The position used for seeking with [`SeekFrom::Current`] is the position the underlying
    /// reader would be at if the [`BufReader`] had no internal buffer.
    ///
    /// Seeking always discards the internal buffer, even if the seek position would otherwise fall
    /// within it. This guarantees that calling [`into_inner()`][`BufReader::into_inner()`]
    /// immediately after a seek yields the underlying reader at the same position.
    ///
    /// See [`AsyncSeek`] for more details.
    ///
    /// Note: In the edge case where you're seeking with `SeekFrom::Current(n)` where `n` minus the
    /// internal buffer length overflows an `i64`, two seeks will be performed instead of one. If
    /// the second seek returns `Err`, the underlying reader will be left at the same position it
    /// would have if you called [`seek()`][`AsyncSeekExt::seek()`] with `SeekFrom::Current(0)`.
    fn poll_seek(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        pos: SeekFrom,
    ) -> Poll<Result<u64>> {
        let result: u64;
        if let SeekFrom::Current(n) = pos {
            let remainder = (self.cap - self.pos) as i64;
            // it should be safe to assume that remainder fits within an i64 as the alternative
            // means we managed to allocate 8 exbibytes and that's absurd.
            // But it's not out of the realm of possibility for some weird underlying reader to
            // support seeking by i64::min_value() so we need to handle underflow when subtracting
            // remainder.
            if let Some(offset) = n.checked_sub(remainder) {
                result = ready!(self
                    .as_mut()
                    .get_pin_mut()
                    .poll_seek(cx, SeekFrom::Current(offset)))?;
            } else {
                // seek backwards by our remainder, and then by the offset
                ready!(self
                    .as_mut()
                    .get_pin_mut()
                    .poll_seek(cx, SeekFrom::Current(-remainder)))?;
                self.as_mut().discard_buffer();
                result = ready!(self
                    .as_mut()
                    .get_pin_mut()
                    .poll_seek(cx, SeekFrom::Current(n)))?;
            }
        } else {
            // Seeking with Start/End doesn't care about our buffer length.
            result = ready!(self.as_mut().get_pin_mut().poll_seek(cx, pos))?;
        }
        self.discard_buffer();
        Poll::Ready(Ok(result))
    }
}

impl<R: AsyncWrite> AsyncWrite for BufReader<R> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        self.as_mut().get_pin_mut().poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        self.as_mut().get_pin_mut().poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        self.as_mut().get_pin_mut().poll_close(cx)
    }
}

pin_project! {
    /// Adds buffering to a writer.
    ///
    /// It can be excessively inefficient to work directly with something that implements
    /// [`AsyncWrite`]. For example, every call to [`write()`][`AsyncWriteExt::write()`] on a TCP
    /// stream results in a system call. A [`BufWriter`] keeps an in-memory buffer of data and
    /// writes it to the underlying writer in large, infrequent batches.
    ///
    /// [`BufWriter`] can improve the speed of programs that make *small* and *repeated* writes to
    /// the same file or networking socket. It does not help when writing very large amounts at
    /// once, or writing just once or a few times. It also provides no advantage when writing to a
    /// destination that is in memory, like a `Vec<u8>`.
    ///
    /// Unlike [`std::io::BufWriter`], this type does not write out the contents of its buffer when
    /// it is dropped. Therefore, it is important that users explicitly flush the buffer before
    /// dropping the [`BufWriter`].
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncWriteExt, BufWriter};
    ///
    /// # spin_on::spin_on(async {
    /// let mut output = Vec::new();
    /// let mut writer = BufWriter::new(&mut output);
    ///
    /// writer.write_all(b"hello").await?;
    /// writer.flush().await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub struct BufWriter<W> {
        #[pin]
        inner: W,
        buf: Vec<u8>,
        written: usize,
    }
}

impl<W: AsyncWrite> BufWriter<W> {
    /// Creates a buffered writer with the default buffer capacity.
    ///
    /// The default capacity is currently 8 KB, but that may change in the future.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::BufWriter;
    ///
    /// let mut output = Vec::new();
    /// let writer = BufWriter::new(&mut output);
    /// ```
    pub fn new(inner: W) -> BufWriter<W> {
        BufWriter::with_capacity(DEFAULT_BUF_SIZE, inner)
    }

    /// Creates a buffered writer with the specified buffer capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::BufWriter;
    ///
    /// let mut output = Vec::new();
    /// let writer = BufWriter::with_capacity(100, &mut output);
    /// ```
    pub fn with_capacity(capacity: usize, inner: W) -> BufWriter<W> {
        BufWriter {
            inner,
            buf: Vec::with_capacity(capacity),
            written: 0,
        }
    }

    /// Gets a reference to the underlying writer.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::BufWriter;
    ///
    /// let mut output = Vec::new();
    /// let writer = BufWriter::new(&mut output);
    ///
    /// let r = writer.get_ref();
    /// ```
    pub fn get_ref(&self) -> &W {
        &self.inner
    }

    /// Gets a mutable reference to the underlying writer.
    ///
    /// It is not advisable to directly write to the underlying writer.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::BufWriter;
    ///
    /// let mut output = Vec::new();
    /// let mut writer = BufWriter::new(&mut output);
    ///
    /// let r = writer.get_mut();
    /// ```
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.inner
    }

    /// Gets a pinned mutable reference to the underlying writer.
    ///
    /// It is not not advisable to directly write to the underlying writer.
    fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut W> {
        self.project().inner
    }

    /// Unwraps the buffered writer, returning the underlying writer.
    ///
    /// Note that any leftover data in the internal buffer will be lost. If you don't want to lose
    /// that data, flush the buffered writer before unwrapping it.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncWriteExt, BufWriter};
    ///
    /// # spin_on::spin_on(async {
    /// let mut output = vec![1, 2, 3];
    /// let mut writer = BufWriter::new(&mut output);
    ///
    /// writer.write_all(&[4]).await?;
    /// writer.flush().await?;
    /// assert_eq!(writer.into_inner(), &[1, 2, 3, 4]);
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn into_inner(self) -> W {
        self.inner
    }

    /// Returns a reference to the internal buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::BufWriter;
    ///
    /// let mut output = Vec::new();
    /// let writer = BufWriter::new(&mut output);
    ///
    /// // The internal buffer is empty until the first write request.
    /// assert_eq!(writer.buffer(), &[]);
    /// ```
    pub fn buffer(&self) -> &[u8] {
        &self.buf
    }

    /// Flush the buffer.
    fn poll_flush_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        let mut this = self.project();
        let len = this.buf.len();
        let mut ret = Ok(());

        while *this.written < len {
            match this
                .inner
                .as_mut()
                .poll_write(cx, &this.buf[*this.written..])
            {
                Poll::Ready(Ok(0)) => {
                    ret = Err(Error::new(
                        ErrorKind::WriteZero,
                        "Failed to write buffered data",
                    ));
                    break;
                }
                Poll::Ready(Ok(n)) => *this.written += n,
                Poll::Ready(Err(ref e)) if e.kind() == ErrorKind::Interrupted => {}
                Poll::Ready(Err(e)) => {
                    ret = Err(e);
                    break;
                }
                Poll::Pending => return Poll::Pending,
            }
        }

        if *this.written > 0 {
            this.buf.drain(..*this.written);
        }
        *this.written = 0;

        Poll::Ready(ret)
    }
}

impl<W: fmt::Debug> fmt::Debug for BufWriter<W> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BufWriter")
            .field("writer", &self.inner)
            .field("buf", &self.buf)
            .finish()
    }
}

impl<W: AsyncWrite> AsyncWrite for BufWriter<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        if self.buf.len() + buf.len() > self.buf.capacity() {
            ready!(self.as_mut().poll_flush_buf(cx))?;
        }
        if buf.len() >= self.buf.capacity() {
            self.get_pin_mut().poll_write(cx, buf)
        } else {
            Pin::new(&mut *self.project().buf).poll_write(cx, buf)
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        ready!(self.as_mut().poll_flush_buf(cx))?;
        self.get_pin_mut().poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        ready!(self.as_mut().poll_flush_buf(cx))?;
        self.get_pin_mut().poll_close(cx)
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
    ) -> Poll<Result<u64>> {
        ready!(self.as_mut().poll_flush_buf(cx))?;
        self.get_pin_mut().poll_seek(cx, pos)
    }
}

/// Gives an in-memory buffer a cursor for reading and writing.
///
/// # Examples
///
/// ```
/// use futures_lite::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, Cursor, SeekFrom};
///
/// # spin_on::spin_on(async {
/// let mut bytes = b"hello".to_vec();
/// let mut cursor = Cursor::new(&mut bytes);
///
/// // Overwrite 'h' with 'H'.
/// cursor.write_all(b"H").await?;
///
/// // Move the cursor one byte forward.
/// cursor.seek(SeekFrom::Current(1)).await?;
///
/// // Read a byte.
/// let mut byte = [0];
/// cursor.read_exact(&mut byte).await?;
/// assert_eq!(&byte, b"l");
///
/// // Check the final buffer.
/// assert_eq!(bytes, b"Hello");
/// # std::io::Result::Ok(()) });
/// ```
#[derive(Clone, Debug, Default)]
pub struct Cursor<T> {
    inner: std::io::Cursor<T>,
}

impl<T> Cursor<T> {
    /// Creates a cursor for an in-memory buffer.
    ///
    /// Cursor's initial position is 0 even if the underlying buffer is not empty. Writing using
    /// [`Cursor`] will overwrite the existing contents unless the cursor is moved to the end of
    /// the buffer using [`set_position()`][Cursor::set_position()`] or
    /// [`seek()`][`AsyncSeekExt::seek()`].
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::Cursor;
    ///
    /// let cursor = Cursor::new(Vec::<u8>::new());
    /// ```
    pub fn new(inner: T) -> Cursor<T> {
        Cursor {
            inner: std::io::Cursor::new(inner),
        }
    }

    /// Gets a reference to the underlying buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::Cursor;
    ///
    /// let cursor = Cursor::new(Vec::<u8>::new());
    /// let r = cursor.get_ref();
    /// ```
    pub fn get_ref(&self) -> &T {
        self.inner.get_ref()
    }

    /// Gets a mutable reference to the underlying buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::Cursor;
    ///
    /// let mut cursor = Cursor::new(Vec::<u8>::new());
    /// let r = cursor.get_mut();
    /// ```
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }

    /// Unwraps the cursor, returning the underlying buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::Cursor;
    ///
    /// let cursor = Cursor::new(vec![1, 2, 3]);
    /// assert_eq!(cursor.into_inner(), [1, 2, 3]);
    /// ```
    pub fn into_inner(self) -> T {
        self.inner.into_inner()
    }

    /// Returns the current position of this cursor.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncSeekExt, Cursor, SeekFrom};
    ///
    /// # spin_on::spin_on(async {
    /// let mut cursor = Cursor::new(b"hello");
    /// assert_eq!(cursor.position(), 0);
    ///
    /// cursor.seek(SeekFrom::Start(2)).await?;
    /// assert_eq!(cursor.position(), 2);
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn position(&self) -> u64 {
        self.inner.position()
    }

    /// Sets the position of this cursor.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::Cursor;
    ///
    /// let mut cursor = Cursor::new(b"hello");
    /// assert_eq!(cursor.position(), 0);
    ///
    /// cursor.set_position(2);
    /// assert_eq!(cursor.position(), 2);
    /// ```
    pub fn set_position(&mut self, pos: u64) {
        self.inner.set_position(pos)
    }
}

impl<T> AsyncSeek for Cursor<T>
where
    T: AsRef<[u8]> + Unpin,
{
    fn poll_seek(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        pos: SeekFrom,
    ) -> Poll<Result<u64>> {
        Poll::Ready(std::io::Seek::seek(&mut self.inner, pos))
    }
}

impl<T> AsyncRead for Cursor<T>
where
    T: AsRef<[u8]> + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        Poll::Ready(std::io::Read::read(&mut self.inner, buf))
    }

    fn poll_read_vectored(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<Result<usize>> {
        Poll::Ready(std::io::Read::read_vectored(&mut self.inner, bufs))
    }
}

impl<T> AsyncBufRead for Cursor<T>
where
    T: AsRef<[u8]> + Unpin,
{
    fn poll_fill_buf(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<&[u8]>> {
        Poll::Ready(std::io::BufRead::fill_buf(&mut self.get_mut().inner))
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        std::io::BufRead::consume(&mut self.inner, amt)
    }
}

impl AsyncWrite for Cursor<&mut [u8]> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        Poll::Ready(std::io::Write::write(&mut self.inner, buf))
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<Result<usize>> {
        Poll::Ready(std::io::Write::write_vectored(&mut self.inner, bufs))
    }

    fn poll_flush(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(std::io::Write::flush(&mut self.inner))
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        self.poll_flush(cx)
    }
}

impl AsyncWrite for Cursor<&mut Vec<u8>> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        Poll::Ready(std::io::Write::write(&mut self.inner, buf))
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        self.poll_flush(cx)
    }

    fn poll_flush(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(std::io::Write::flush(&mut self.inner))
    }
}

impl AsyncWrite for Cursor<Vec<u8>> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        Poll::Ready(std::io::Write::write(&mut self.inner, buf))
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        self.poll_flush(cx)
    }

    fn poll_flush(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(std::io::Write::flush(&mut self.inner))
    }
}

/// Creates an empty reader.
///
/// # Examples
///
/// ```
/// use futures_lite::io::{self, AsyncReadExt};
///
/// # spin_on::spin_on(async {
/// let mut reader = io::empty();
///
/// let mut contents = Vec::new();
/// reader.read_to_end(&mut contents).await?;
/// assert!(contents.is_empty());
/// # std::io::Result::Ok(()) });
/// ```
pub fn empty() -> Empty {
    Empty { _private: () }
}

/// Reader for the [`empty()`] function.
pub struct Empty {
    _private: (),
}

impl fmt::Debug for Empty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Empty { .. }")
    }
}

impl AsyncRead for Empty {
    #[inline]
    fn poll_read(self: Pin<&mut Self>, _: &mut Context<'_>, _: &mut [u8]) -> Poll<Result<usize>> {
        Poll::Ready(Ok(0))
    }
}

impl AsyncBufRead for Empty {
    #[inline]
    fn poll_fill_buf<'a>(self: Pin<&'a mut Self>, _: &mut Context<'_>) -> Poll<Result<&'a [u8]>> {
        Poll::Ready(Ok(&[]))
    }

    #[inline]
    fn consume(self: Pin<&mut Self>, _: usize) {}
}

/// Creates an infinite reader that reads the same byte repeatedly.
///
/// # Examples
///
/// ```
/// use futures_lite::io::{self, AsyncReadExt};
///
/// # spin_on::spin_on(async {
/// let mut reader = io::repeat(b'a');
///
/// let mut contents = vec![0; 5];
/// reader.read_exact(&mut contents).await?;
/// assert_eq!(contents, b"aaaaa");
/// # std::io::Result::Ok(()) });
/// ```
pub fn repeat(byte: u8) -> Repeat {
    Repeat { byte }
}

/// Reader for the [`repeat()`] function.
#[derive(Debug)]
pub struct Repeat {
    byte: u8,
}

impl AsyncRead for Repeat {
    #[inline]
    fn poll_read(self: Pin<&mut Self>, _: &mut Context<'_>, buf: &mut [u8]) -> Poll<Result<usize>> {
        for b in &mut *buf {
            *b = self.byte;
        }
        Poll::Ready(Ok(buf.len()))
    }
}

/// Creates a writer that consumes and drops all data.
///
/// # Examples
///
/// ```
/// use futures_lite::io::{self, AsyncWriteExt};
///
/// # spin_on::spin_on(async {
/// let mut writer = io::sink();
/// writer.write_all(b"hello").await?;
/// # std::io::Result::Ok(()) });
/// ```
pub fn sink() -> Sink {
    Sink { _private: () }
}

/// Writer for the [`sink()`] function.
#[derive(Debug)]
pub struct Sink {
    _private: (),
}

impl AsyncWrite for Sink {
    #[inline]
    fn poll_write(self: Pin<&mut Self>, _: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        Poll::Ready(Ok(buf.len()))
    }

    #[inline]
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }
}

/// Extension trait for [`AsyncBufRead`].
pub trait AsyncBufReadExt: AsyncBufRead {
    /// Returns the contents of the internal buffer, filling it with more data if empty.
    ///
    /// If the stream has reached EOF, an empty buffer will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncBufReadExt, BufReader};
    /// use std::pin::Pin;
    ///
    /// # spin_on::spin_on(async {
    /// let input: &[u8] = b"hello world";
    /// let mut reader = BufReader::with_capacity(5, input);
    ///
    /// assert_eq!(reader.fill_buf().await?, b"hello");
    /// reader.consume(2);
    /// assert_eq!(reader.fill_buf().await?, b"llo");
    /// reader.consume(3);
    /// assert_eq!(reader.fill_buf().await?, b" worl");
    /// # std::io::Result::Ok(()) });
    /// ```
    fn fill_buf(&mut self) -> FillBuf<'_, Self>
    where
        Self: Unpin,
    {
        FillBuf { reader: Some(self) }
    }

    /// Consumes `amt` buffered bytes.
    ///
    /// This method does not perform any I/O, it simply consumes some amount of bytes from the
    /// internal buffer.
    ///
    /// The `amt` must be <= the number of bytes in the buffer returned by
    /// [`fill_buf()`][`AsyncBufReadExt::fill_buf()`].
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncBufReadExt, BufReader};
    /// use std::pin::Pin;
    ///
    /// # spin_on::spin_on(async {
    /// let input: &[u8] = b"hello";
    /// let mut reader = BufReader::with_capacity(4, input);
    ///
    /// assert_eq!(reader.fill_buf().await?, b"hell");
    /// reader.consume(2);
    /// assert_eq!(reader.fill_buf().await?, b"ll");
    /// # std::io::Result::Ok(()) });
    /// ```
    fn consume(&mut self, amt: usize)
    where
        Self: Unpin,
    {
        AsyncBufRead::consume(Pin::new(self), amt);
    }

    /// Reads all bytes and appends them into `buf` until the delimiter `byte` or EOF is found.
    ///
    /// This method will read bytes from the underlying stream until the delimiter or EOF is
    /// found. All bytes up to and including the delimiter (if found) will be appended to `buf`.
    ///
    /// If successful, returns the total number of bytes read.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncBufReadExt, BufReader};
    ///
    /// # spin_on::spin_on(async {
    /// let input: &[u8] = b"hello";
    /// let mut reader = BufReader::new(input);
    ///
    /// let mut buf = Vec::new();
    /// let n = reader.read_until(b'\n', &mut buf).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    fn read_until<'a>(&'a mut self, byte: u8, buf: &'a mut Vec<u8>) -> ReadUntilFuture<'a, Self>
    where
        Self: Unpin,
    {
        ReadUntilFuture {
            reader: self,
            byte,
            buf,
            read: 0,
        }
    }

    /// Reads all bytes and appends them into `buf` until a newline (the 0xA byte) or EOF is found.
    ///
    /// This method will read bytes from the underlying stream until the newline delimiter (the
    /// 0xA byte) or EOF is found. All bytes up to, and including, the newline delimiter (if found)
    /// will be appended to `buf`.
    ///
    /// If successful, returns the total number of bytes read.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncBufReadExt, BufReader};
    ///
    /// # spin_on::spin_on(async {
    /// let input: &[u8] = b"hello";
    /// let mut reader = BufReader::new(input);
    ///
    /// let mut line = String::new();
    /// let n = reader.read_line(&mut line).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    fn read_line<'a>(&'a mut self, buf: &'a mut String) -> ReadLineFuture<'a, Self>
    where
        Self: Unpin,
    {
        ReadLineFuture {
            reader: self,
            buf,
            bytes: Vec::new(),
            read: 0,
        }
    }

    /// Returns a stream over the lines of this byte stream.
    ///
    /// The stream returned from this method yields items of type
    /// [`io::Result`][`super::io::Result`]`<`[`String`]`>`.
    /// Each string returned will *not* have a newline byte (the 0xA byte) or CRLF (0xD, 0xA bytes)
    /// at the end.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncBufReadExt, BufReader};
    /// use futures_lite::stream::StreamExt;
    ///
    /// # spin_on::spin_on(async {
    /// let input: &[u8] = b"hello\nworld\n";
    /// let mut reader = BufReader::new(input);
    /// let mut lines = reader.lines();
    ///
    /// while let Some(line) = lines.next().await {
    ///     println!("{}", line?);
    /// }
    /// # std::io::Result::Ok(()) });
    /// ```
    fn lines(self) -> Lines<Self>
    where
        Self: Sized,
    {
        Lines {
            reader: self,
            buf: String::new(),
            bytes: Vec::new(),
            read: 0,
        }
    }

    /// Returns a stream over the contents of this reader split on the specified `byte`.
    ///
    /// The stream returned from this method yields items of type
    /// [`io::Result`][`super::io::Result`]`<`[`Vec<u8>`][`Vec`]`>`.
    /// Each vector returned will *not* have the delimiter byte at the end.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncBufReadExt, Cursor};
    /// use futures_lite::stream::StreamExt;
    ///
    /// # spin_on::spin_on(async {
    /// let cursor = Cursor::new(b"lorem-ipsum-dolor");
    /// let items: Vec<Vec<u8>> = cursor.split(b'-').try_collect().await?;
    ///
    /// assert_eq!(items[0], b"lorem");
    /// assert_eq!(items[1], b"ipsum");
    /// assert_eq!(items[2], b"dolor");
    /// # std::io::Result::Ok(()) });
    /// ```
    fn split(self, byte: u8) -> Split<Self>
    where
        Self: Sized,
    {
        Split {
            reader: self,
            buf: Vec::new(),
            delim: byte,
            read: 0,
        }
    }
}

impl<R: AsyncBufRead + ?Sized> AsyncBufReadExt for R {}

/// Future for the [`AsyncBufReadExt::fill_buf()`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct FillBuf<'a, R: ?Sized> {
    reader: Option<&'a mut R>,
}

impl<R: ?Sized> Unpin for FillBuf<'_, R> {}

impl<'a, R> Future for FillBuf<'a, R>
where
    R: AsyncBufRead + Unpin + ?Sized,
{
    type Output = Result<&'a [u8]>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;
        let reader = this
            .reader
            .take()
            .expect("polled `FillBuf` after completion");

        match Pin::new(&mut *reader).poll_fill_buf(cx) {
            Poll::Ready(Ok(_)) => match Pin::new(reader).poll_fill_buf(cx) {
                Poll::Ready(Ok(slice)) => Poll::Ready(Ok(slice)),
                poll => panic!("`poll_fill_buf()` was ready but now it isn't: {:?}", poll),
            },
            Poll::Ready(Err(err)) => Poll::Ready(Err(err)),
            Poll::Pending => {
                this.reader = Some(reader);
                Poll::Pending
            }
        }
    }
}

/// Future for the [`AsyncBufReadExt::read_until()`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ReadUntilFuture<'a, R: Unpin + ?Sized> {
    reader: &'a mut R,
    byte: u8,
    buf: &'a mut Vec<u8>,
    read: usize,
}

impl<R: Unpin + ?Sized> Unpin for ReadUntilFuture<'_, R> {}

impl<R: AsyncBufRead + Unpin + ?Sized> Future for ReadUntilFuture<'_, R> {
    type Output = Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            reader,
            byte,
            buf,
            read,
        } = &mut *self;
        read_until_internal(Pin::new(reader), cx, *byte, buf, read)
    }
}

fn read_until_internal<R: AsyncBufReadExt + ?Sized>(
    mut reader: Pin<&mut R>,
    cx: &mut Context<'_>,
    byte: u8,
    buf: &mut Vec<u8>,
    read: &mut usize,
) -> Poll<Result<usize>> {
    loop {
        let (done, used) = {
            let available = ready!(reader.as_mut().poll_fill_buf(cx))?;

            if let Some(i) = memchr(byte, available) {
                buf.extend_from_slice(&available[..=i]);
                (true, i + 1)
            } else {
                buf.extend_from_slice(available);
                (false, available.len())
            }
        };

        reader.as_mut().consume(used);
        *read += used;

        if done || used == 0 {
            return Poll::Ready(Ok(mem::replace(read, 0)));
        }
    }
}

/// Future for the [`AsyncBufReadExt::read_line()`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ReadLineFuture<'a, R: Unpin + ?Sized> {
    reader: &'a mut R,
    buf: &'a mut String,
    bytes: Vec<u8>,
    read: usize,
}

impl<R: Unpin + ?Sized> Unpin for ReadLineFuture<'_, R> {}

impl<R: AsyncBufRead + Unpin + ?Sized> Future for ReadLineFuture<'_, R> {
    type Output = Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            reader,
            buf,
            bytes,
            read,
        } = &mut *self;
        read_line_internal(Pin::new(reader), cx, buf, bytes, read)
    }
}

pin_project! {
    /// Stream for the [`AsyncBufReadExt::lines()`] method.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Lines<R> {
        #[pin]
        reader: R,
        buf: String,
        bytes: Vec<u8>,
        read: usize,
    }
}

impl<R: AsyncBufRead> Stream for Lines<R> {
    type Item = Result<String>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();

        let n = ready!(read_line_internal(
            this.reader,
            cx,
            this.buf,
            this.bytes,
            this.read
        ))?;
        if n == 0 && this.buf.is_empty() {
            return Poll::Ready(None);
        }

        if this.buf.ends_with('\n') {
            this.buf.pop();
            if this.buf.ends_with('\r') {
                this.buf.pop();
            }
        }
        Poll::Ready(Some(Ok(mem::take(this.buf))))
    }
}

fn read_line_internal<R: AsyncBufRead + ?Sized>(
    reader: Pin<&mut R>,
    cx: &mut Context<'_>,
    buf: &mut String,
    bytes: &mut Vec<u8>,
    read: &mut usize,
) -> Poll<Result<usize>> {
    let ret = ready!(read_until_internal(reader, cx, b'\n', bytes, read));

    match String::from_utf8(mem::take(bytes)) {
        Ok(s) => {
            debug_assert!(buf.is_empty());
            debug_assert_eq!(*read, 0);
            *buf = s;
            Poll::Ready(ret)
        }
        Err(_) => Poll::Ready(ret.and_then(|_| {
            Err(Error::new(
                ErrorKind::InvalidData,
                "stream did not contain valid UTF-8",
            ))
        })),
    }
}

pin_project! {
    /// Stream for the [`AsyncBufReadExt::split()`] method.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Split<R> {
        #[pin]
        reader: R,
        buf: Vec<u8>,
        read: usize,
        delim: u8,
    }
}

impl<R: AsyncBufRead> Stream for Split<R> {
    type Item = Result<Vec<u8>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();

        let n = ready!(read_until_internal(
            this.reader,
            cx,
            *this.delim,
            this.buf,
            this.read
        ))?;
        if n == 0 && this.buf.is_empty() {
            return Poll::Ready(None);
        }

        if this.buf[this.buf.len() - 1] == *this.delim {
            this.buf.pop();
        }
        Poll::Ready(Some(Ok(mem::take(this.buf))))
    }
}

/// Extension trait for [`AsyncRead`].
pub trait AsyncReadExt: AsyncRead {
    /// Reads some bytes from the byte stream.
    ///
    /// On success, returns the total number of bytes read.
    ///
    /// If the return value is `Ok(n)`, then it must be guaranteed that
    /// `0 <= n <= buf.len()`. A nonzero `n` value indicates that the buffer has been
    /// filled with `n` bytes of data. If `n` is `0`, then it can indicate one of two
    /// scenarios:
    ///
    /// 1. This reader has reached its "end of file" and will likely no longer be able to
    ///    produce bytes. Note that this does not mean that the reader will always no
    ///    longer be able to produce bytes.
    /// 2. The buffer specified was 0 bytes in length.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncReadExt, BufReader};
    ///
    /// # spin_on::spin_on(async {
    /// let input: &[u8] = b"hello";
    /// let mut reader = BufReader::new(input);
    ///
    /// let mut buf = vec![0; 1024];
    /// let n = reader.read(&mut buf).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> ReadFuture<'a, Self>
    where
        Self: Unpin,
    {
        ReadFuture { reader: self, buf }
    }

    /// Like [`read()`][`AsyncReadExt::read()`], except it reads into a slice of buffers.
    ///
    /// Data is copied to fill each buffer in order, with the final buffer possibly being
    /// only partially filled. This method must behave same as a single call to
    /// [`read()`][`AsyncReadExt::read()`] with the buffers concatenated would.
    fn read_vectored<'a>(
        &'a mut self,
        bufs: &'a mut [IoSliceMut<'a>],
    ) -> ReadVectoredFuture<'a, Self>
    where
        Self: Unpin,
    {
        ReadVectoredFuture { reader: self, bufs }
    }

    /// Reads the entire contents and appends them to a [`Vec`].
    ///
    /// On success, returns the total number of bytes read.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncReadExt, Cursor};
    ///
    /// # spin_on::spin_on(async {
    /// let mut reader = Cursor::new(vec![1, 2, 3]);
    /// let mut contents = Vec::new();
    ///
    /// let n = reader.read_to_end(&mut contents).await?;
    /// assert_eq!(n, 3);
    /// assert_eq!(contents, [1, 2, 3]);
    /// # std::io::Result::Ok(()) });
    /// ```
    fn read_to_end<'a>(&'a mut self, buf: &'a mut Vec<u8>) -> ReadToEndFuture<'a, Self>
    where
        Self: Unpin,
    {
        let start_len = buf.len();
        ReadToEndFuture {
            reader: self,
            buf,
            start_len,
        }
    }

    /// Reads the entire contents and appends them to a [`String`].
    ///
    /// On success, returns the total number of bytes read.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncReadExt, Cursor};
    ///
    /// # spin_on::spin_on(async {
    /// let mut reader = Cursor::new(&b"hello");
    /// let mut contents = String::new();
    ///
    /// let n = reader.read_to_string(&mut contents).await?;
    /// assert_eq!(n, 5);
    /// assert_eq!(contents, "hello");
    /// # std::io::Result::Ok(()) });
    /// ```
    fn read_to_string<'a>(&'a mut self, buf: &'a mut String) -> ReadToStringFuture<'a, Self>
    where
        Self: Unpin,
    {
        ReadToStringFuture {
            reader: self,
            buf,
            bytes: Vec::new(),
            start_len: 0,
        }
    }

    /// Reads the exact number of bytes required to fill `buf`.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncReadExt, Cursor};
    ///
    /// # spin_on::spin_on(async {
    /// let mut reader = Cursor::new(&b"hello");
    /// let mut contents = vec![0; 3];
    ///
    /// reader.read_exact(&mut contents).await?;
    /// assert_eq!(contents, b"hel");
    /// # std::io::Result::Ok(()) });
    /// ```
    fn read_exact<'a>(&'a mut self, buf: &'a mut [u8]) -> ReadExactFuture<'a, Self>
    where
        Self: Unpin,
    {
        ReadExactFuture { reader: self, buf }
    }

    /// Creates an adapter which will read at most `limit` bytes from it.
    ///
    /// This method returns a new instance of [`AsyncRead`] which will read at most
    /// `limit` bytes, after which it will always return `Ok(0)` indicating EOF.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncReadExt, Cursor};
    ///
    /// # spin_on::spin_on(async {
    /// let mut reader = Cursor::new(&b"hello");
    /// let mut contents = String::new();
    ///
    /// let n = reader.take(3).read_to_string(&mut contents).await?;
    /// assert_eq!(n, 3);
    /// assert_eq!(contents, "hel");
    /// # std::io::Result::Ok(()) });
    /// ```
    fn take(self, limit: u64) -> Take<Self>
    where
        Self: Sized,
    {
        Take { inner: self, limit }
    }

    /// Converts this [`AsyncRead`] into a [`Stream`] of bytes.
    ///
    /// The returned type implements [`Stream`] where `Item` is `io::Result<u8>`.
    ///
    /// ```
    /// use futures_lite::io::{AsyncReadExt, Cursor};
    /// use futures_lite::stream::StreamExt;
    ///
    /// # spin_on::spin_on(async {
    /// let reader = Cursor::new(&b"hello");
    /// let mut bytes = reader.bytes();
    ///
    /// while let Some(byte) = bytes.next().await {
    ///     println!("byte: {}", byte?);
    /// }
    /// # std::io::Result::Ok(()) });
    /// ```
    fn bytes(self) -> Bytes<Self>
    where
        Self: Sized,
    {
        Bytes { inner: self }
    }

    /// Creates an adapter which will chain this stream with another.
    ///
    /// The returned [`AsyncRead`] instance will first read all bytes from this reader
    /// until EOF is found, and then continue with `next`.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncReadExt, Cursor};
    ///
    /// # spin_on::spin_on(async {
    /// let r1 = Cursor::new(&b"hello");
    /// let r2 = Cursor::new(&b"world");
    /// let mut reader = r1.chain(r2);
    ///
    /// let mut contents = String::new();
    /// reader.read_to_string(&mut contents).await?;
    /// assert_eq!(contents, "helloworld");
    /// # std::io::Result::Ok(()) });
    /// ```
    fn chain<R: AsyncRead>(self, next: R) -> Chain<Self, R>
    where
        Self: Sized,
    {
        Chain {
            first: self,
            second: next,
            done_first: false,
        }
    }

    /// Boxes the reader and changes its type to `dyn AsyncRead + Send + 'a`.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::AsyncReadExt;
    ///
    /// let reader = [1, 2, 3].boxed_reader();
    /// ```
    #[cfg(feature = "alloc")]
    fn boxed_reader<'a>(self) -> Pin<Box<dyn AsyncRead + Send + 'a>>
    where
        Self: Sized + Send + 'a,
    {
        Box::pin(self)
    }
}

impl<R: AsyncRead + ?Sized> AsyncReadExt for R {}

/// Future for the [`AsyncReadExt::read()`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ReadFuture<'a, R: Unpin + ?Sized> {
    reader: &'a mut R,
    buf: &'a mut [u8],
}

impl<R: Unpin + ?Sized> Unpin for ReadFuture<'_, R> {}

impl<R: AsyncRead + Unpin + ?Sized> Future for ReadFuture<'_, R> {
    type Output = Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { reader, buf } = &mut *self;
        Pin::new(reader).poll_read(cx, buf)
    }
}

/// Future for the [`AsyncReadExt::read_vectored()`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ReadVectoredFuture<'a, R: Unpin + ?Sized> {
    reader: &'a mut R,
    bufs: &'a mut [IoSliceMut<'a>],
}

impl<R: Unpin + ?Sized> Unpin for ReadVectoredFuture<'_, R> {}

impl<R: AsyncRead + Unpin + ?Sized> Future for ReadVectoredFuture<'_, R> {
    type Output = Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { reader, bufs } = &mut *self;
        Pin::new(reader).poll_read_vectored(cx, bufs)
    }
}

/// Future for the [`AsyncReadExt::read_to_end()`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ReadToEndFuture<'a, R: Unpin + ?Sized> {
    reader: &'a mut R,
    buf: &'a mut Vec<u8>,
    start_len: usize,
}

impl<R: Unpin + ?Sized> Unpin for ReadToEndFuture<'_, R> {}

impl<R: AsyncRead + Unpin + ?Sized> Future for ReadToEndFuture<'_, R> {
    type Output = Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            reader,
            buf,
            start_len,
        } = &mut *self;
        read_to_end_internal(Pin::new(reader), cx, buf, *start_len)
    }
}

/// Future for the [`AsyncReadExt::read_to_string()`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ReadToStringFuture<'a, R: Unpin + ?Sized> {
    reader: &'a mut R,
    buf: &'a mut String,
    bytes: Vec<u8>,
    start_len: usize,
}

impl<R: Unpin + ?Sized> Unpin for ReadToStringFuture<'_, R> {}

impl<R: AsyncRead + Unpin + ?Sized> Future for ReadToStringFuture<'_, R> {
    type Output = Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            reader,
            buf,
            bytes,
            start_len,
        } = &mut *self;
        let reader = Pin::new(reader);

        let ret = ready!(read_to_end_internal(reader, cx, bytes, *start_len));

        match String::from_utf8(mem::take(bytes)) {
            Ok(s) => {
                debug_assert!(buf.is_empty());
                **buf = s;
                Poll::Ready(ret)
            }
            Err(_) => Poll::Ready(ret.and_then(|_| {
                Err(Error::new(
                    ErrorKind::InvalidData,
                    "stream did not contain valid UTF-8",
                ))
            })),
        }
    }
}

// This uses an adaptive system to extend the vector when it fills. We want to
// avoid paying to allocate and zero a huge chunk of memory if the reader only
// has 4 bytes while still making large reads if the reader does have a ton
// of data to return. Simply tacking on an extra DEFAULT_BUF_SIZE space every
// time is 4,500 times (!) slower than this if the reader has a very small
// amount of data to return.
//
// Because we're extending the buffer with uninitialized data for trusted
// readers, we need to make sure to truncate that if any of this panics.
fn read_to_end_internal<R: AsyncRead + ?Sized>(
    mut rd: Pin<&mut R>,
    cx: &mut Context<'_>,
    buf: &mut Vec<u8>,
    start_len: usize,
) -> Poll<Result<usize>> {
    struct Guard<'a> {
        buf: &'a mut Vec<u8>,
        len: usize,
    }

    impl Drop for Guard<'_> {
        fn drop(&mut self) {
            self.buf.resize(self.len, 0);
        }
    }

    let mut g = Guard {
        len: buf.len(),
        buf,
    };
    let ret;
    loop {
        if g.len == g.buf.len() {
            g.buf.reserve(32);
            let capacity = g.buf.capacity();
            g.buf.resize(capacity, 0);
        }

        match ready!(rd.as_mut().poll_read(cx, &mut g.buf[g.len..])) {
            Ok(0) => {
                ret = Poll::Ready(Ok(g.len - start_len));
                break;
            }
            Ok(n) => g.len += n,
            Err(e) => {
                ret = Poll::Ready(Err(e));
                break;
            }
        }
    }

    ret
}

/// Future for the [`AsyncReadExt::read_exact()`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ReadExactFuture<'a, R: Unpin + ?Sized> {
    reader: &'a mut R,
    buf: &'a mut [u8],
}

impl<R: Unpin + ?Sized> Unpin for ReadExactFuture<'_, R> {}

impl<R: AsyncRead + Unpin + ?Sized> Future for ReadExactFuture<'_, R> {
    type Output = Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { reader, buf } = &mut *self;

        while !buf.is_empty() {
            let n = ready!(Pin::new(&mut *reader).poll_read(cx, buf))?;
            let (_, rest) = mem::take(buf).split_at_mut(n);
            *buf = rest;

            if n == 0 {
                return Poll::Ready(Err(ErrorKind::UnexpectedEof.into()));
            }
        }

        Poll::Ready(Ok(()))
    }
}

pin_project! {
    /// Reader for the [`AsyncReadExt::take()`] method.
    #[derive(Debug)]
    pub struct Take<R> {
        #[pin]
        inner: R,
        limit: u64,
    }
}

impl<R> Take<R> {
    /// Returns the number of bytes before this adapter will return EOF.
    ///
    /// Note that EOF may be reached sooner if the underlying reader is shorter than the limit.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncReadExt, Cursor};
    ///
    /// let reader = Cursor::new("hello");
    ///
    /// let reader = reader.take(3);
    /// assert_eq!(reader.limit(), 3);
    /// ```
    pub fn limit(&self) -> u64 {
        self.limit
    }

    /// Puts a limit on the number of bytes.
    ///
    /// Changing the limit is equivalent to creating a new adapter with [`AsyncReadExt::take()`].
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncReadExt, Cursor};
    ///
    /// let reader = Cursor::new("hello");
    ///
    /// let mut reader = reader.take(10);
    /// assert_eq!(reader.limit(), 10);
    ///
    /// reader.set_limit(3);
    /// assert_eq!(reader.limit(), 3);
    /// ```
    pub fn set_limit(&mut self, limit: u64) {
        self.limit = limit;
    }

    /// Gets a reference to the underlying reader.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncReadExt, Cursor};
    ///
    /// let reader = Cursor::new("hello");
    ///
    /// let reader = reader.take(3);
    /// let r = reader.get_ref();
    /// ```
    pub fn get_ref(&self) -> &R {
        &self.inner
    }

    /// Gets a mutable reference to the underlying reader.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncReadExt, Cursor};
    ///
    /// let reader = Cursor::new("hello");
    ///
    /// let mut reader = reader.take(3);
    /// let r = reader.get_mut();
    /// ```
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    /// Unwraps the adapter, returning the underlying reader.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncReadExt, Cursor};
    ///
    /// let reader = Cursor::new("hello");
    ///
    /// let reader = reader.take(3);
    /// let reader = reader.into_inner();
    /// ```
    pub fn into_inner(self) -> R {
        self.inner
    }
}

impl<R: AsyncRead> AsyncRead for Take<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        let this = self.project();
        take_read_internal(this.inner, cx, buf, this.limit)
    }
}

fn take_read_internal<R: AsyncRead + ?Sized>(
    mut rd: Pin<&mut R>,
    cx: &mut Context<'_>,
    buf: &mut [u8],
    limit: &mut u64,
) -> Poll<Result<usize>> {
    // Don't call into inner reader at all at EOF because it may still block
    if *limit == 0 {
        return Poll::Ready(Ok(0));
    }

    let max = cmp::min(buf.len() as u64, *limit) as usize;

    match ready!(rd.as_mut().poll_read(cx, &mut buf[..max])) {
        Ok(n) => {
            *limit -= n as u64;
            Poll::Ready(Ok(n))
        }
        Err(e) => Poll::Ready(Err(e)),
    }
}

impl<R: AsyncBufRead> AsyncBufRead for Take<R> {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<&[u8]>> {
        let this = self.project();

        if *this.limit == 0 {
            return Poll::Ready(Ok(&[]));
        }

        match ready!(this.inner.poll_fill_buf(cx)) {
            Ok(buf) => {
                let cap = cmp::min(buf.len() as u64, *this.limit) as usize;
                Poll::Ready(Ok(&buf[..cap]))
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        let this = self.project();
        // Don't let callers reset the limit by passing an overlarge value
        let amt = cmp::min(amt as u64, *this.limit) as usize;
        *this.limit -= amt as u64;

        this.inner.consume(amt);
    }
}

pin_project! {
    /// Reader for the [`AsyncReadExt::bytes()`] method.
    #[derive(Debug)]
    pub struct Bytes<R> {
        #[pin]
        inner: R,
    }
}

impl<R: AsyncRead + Unpin> Stream for Bytes<R> {
    type Item = Result<u8>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut byte = 0;

        let rd = Pin::new(&mut self.inner);

        match ready!(rd.poll_read(cx, std::slice::from_mut(&mut byte))) {
            Ok(0) => Poll::Ready(None),
            Ok(..) => Poll::Ready(Some(Ok(byte))),
            Err(ref e) if e.kind() == ErrorKind::Interrupted => Poll::Pending,
            Err(e) => Poll::Ready(Some(Err(e))),
        }
    }
}

impl<R: AsyncRead> AsyncRead for Bytes<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        self.project().inner.poll_read(cx, buf)
    }

    fn poll_read_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<Result<usize>> {
        self.project().inner.poll_read_vectored(cx, bufs)
    }
}

pin_project! {
    /// Reader for the [`AsyncReadExt::chain()`] method.
    pub struct Chain<R1, R2> {
        #[pin]
        first: R1,
        #[pin]
        second: R2,
        done_first: bool,
    }
}

impl<R1, R2> Chain<R1, R2> {
    /// Gets references to the underlying readers.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncReadExt, Cursor};
    ///
    /// let r1 = Cursor::new(b"hello");
    /// let r2 = Cursor::new(b"world");
    ///
    /// let reader = r1.chain(r2);
    /// let (r1, r2) = reader.get_ref();
    /// ```
    pub fn get_ref(&self) -> (&R1, &R2) {
        (&self.first, &self.second)
    }

    /// Gets mutable references to the underlying readers.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncReadExt, Cursor};
    ///
    /// let r1 = Cursor::new(b"hello");
    /// let r2 = Cursor::new(b"world");
    ///
    /// let mut reader = r1.chain(r2);
    /// let (r1, r2) = reader.get_mut();
    /// ```
    pub fn get_mut(&mut self) -> (&mut R1, &mut R2) {
        (&mut self.first, &mut self.second)
    }

    /// Unwraps the adapter, returning the underlying readers.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncReadExt, Cursor};
    ///
    /// let r1 = Cursor::new(b"hello");
    /// let r2 = Cursor::new(b"world");
    ///
    /// let reader = r1.chain(r2);
    /// let (r1, r2) = reader.into_inner();
    /// ```
    pub fn into_inner(self) -> (R1, R2) {
        (self.first, self.second)
    }
}

impl<R1: fmt::Debug, R2: fmt::Debug> fmt::Debug for Chain<R1, R2> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Chain")
            .field("r1", &self.first)
            .field("r2", &self.second)
            .finish()
    }
}

impl<R1: AsyncRead, R2: AsyncRead> AsyncRead for Chain<R1, R2> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        let this = self.project();
        if !*this.done_first {
            match ready!(this.first.poll_read(cx, buf)) {
                Ok(0) if !buf.is_empty() => *this.done_first = true,
                Ok(n) => return Poll::Ready(Ok(n)),
                Err(err) => return Poll::Ready(Err(err)),
            }
        }

        this.second.poll_read(cx, buf)
    }

    fn poll_read_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<Result<usize>> {
        let this = self.project();
        if !*this.done_first {
            match ready!(this.first.poll_read_vectored(cx, bufs)) {
                Ok(0) if !bufs.is_empty() => *this.done_first = true,
                Ok(n) => return Poll::Ready(Ok(n)),
                Err(err) => return Poll::Ready(Err(err)),
            }
        }

        this.second.poll_read_vectored(cx, bufs)
    }
}

impl<R1: AsyncBufRead, R2: AsyncBufRead> AsyncBufRead for Chain<R1, R2> {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<&[u8]>> {
        let this = self.project();
        if !*this.done_first {
            match ready!(this.first.poll_fill_buf(cx)) {
                Ok([]) => *this.done_first = true,
                Ok(buf) => return Poll::Ready(Ok(buf)),
                Err(err) => return Poll::Ready(Err(err)),
            }
        }

        this.second.poll_fill_buf(cx)
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        let this = self.project();
        if !*this.done_first {
            this.first.consume(amt)
        } else {
            this.second.consume(amt)
        }
    }
}

/// Extension trait for [`AsyncSeek`].
pub trait AsyncSeekExt: AsyncSeek {
    /// Seeks to a new position in a byte stream.
    ///
    /// Returns the new position in the byte stream.
    ///
    /// A seek beyond the end of stream is allowed, but behavior is defined by the implementation.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncSeekExt, Cursor, SeekFrom};
    ///
    /// # spin_on::spin_on(async {
    /// let mut cursor = Cursor::new("hello");
    ///
    /// // Move the cursor to the end.
    /// cursor.seek(SeekFrom::End(0)).await?;
    ///
    /// // Check the current position.
    /// assert_eq!(cursor.seek(SeekFrom::Current(0)).await?, 5);
    /// # std::io::Result::Ok(()) });
    /// ```
    fn seek(&mut self, pos: SeekFrom) -> SeekFuture<'_, Self>
    where
        Self: Unpin,
    {
        SeekFuture { seeker: self, pos }
    }
}

impl<S: AsyncSeek + ?Sized> AsyncSeekExt for S {}

/// Future for the [`AsyncSeekExt::seek()`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct SeekFuture<'a, S: Unpin + ?Sized> {
    seeker: &'a mut S,
    pos: SeekFrom,
}

impl<S: Unpin + ?Sized> Unpin for SeekFuture<'_, S> {}

impl<S: AsyncSeek + Unpin + ?Sized> Future for SeekFuture<'_, S> {
    type Output = Result<u64>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let pos = self.pos;
        Pin::new(&mut *self.seeker).poll_seek(cx, pos)
    }
}

/// Extension trait for [`AsyncWrite`].
pub trait AsyncWriteExt: AsyncWrite {
    /// Writes some bytes into the byte stream.
    ///
    /// Returns the number of bytes written from the start of the buffer.
    ///
    /// If the return value is `Ok(n)` then it must be guaranteed that
    /// `0 <= n <= buf.len()`. A return value of `0` typically means that the underlying
    /// object is no longer able to accept bytes and will likely not be able to in the
    /// future as well, or that the provided buffer is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncWriteExt, BufWriter};
    ///
    /// # spin_on::spin_on(async {
    /// let mut output = Vec::new();
    /// let mut writer = BufWriter::new(&mut output);
    ///
    /// let n = writer.write(b"hello").await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    fn write<'a>(&'a mut self, buf: &'a [u8]) -> WriteFuture<'a, Self>
    where
        Self: Unpin,
    {
        WriteFuture { writer: self, buf }
    }

    /// Like [`write()`][`AsyncWriteExt::write()`], except that it writes a slice of buffers.
    ///
    /// Data is copied from each buffer in order, with the final buffer possibly being only
    /// partially consumed. This method must behave same as a call to
    /// [`write()`][`AsyncWriteExt::write()`] with the buffers concatenated would.
    fn write_vectored<'a>(&'a mut self, bufs: &'a [IoSlice<'a>]) -> WriteVectoredFuture<'a, Self>
    where
        Self: Unpin,
    {
        WriteVectoredFuture { writer: self, bufs }
    }

    /// Writes an entire buffer into the byte stream.
    ///
    /// This method will keep calling [`write()`][`AsyncWriteExt::write()`] until there is no more
    /// data to be written or an error occurs. It will not return before the entire buffer is
    /// successfully written or an error occurs.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncWriteExt, BufWriter};
    ///
    /// # spin_on::spin_on(async {
    /// let mut output = Vec::new();
    /// let mut writer = BufWriter::new(&mut output);
    ///
    /// let n = writer.write_all(b"hello").await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    fn write_all<'a>(&'a mut self, buf: &'a [u8]) -> WriteAllFuture<'a, Self>
    where
        Self: Unpin,
    {
        WriteAllFuture { writer: self, buf }
    }

    /// Flushes the stream to ensure that all buffered contents reach their destination.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncWriteExt, BufWriter};
    ///
    /// # spin_on::spin_on(async {
    /// let mut output = Vec::new();
    /// let mut writer = BufWriter::new(&mut output);
    ///
    /// writer.write_all(b"hello").await?;
    /// writer.flush().await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    fn flush(&mut self) -> FlushFuture<'_, Self>
    where
        Self: Unpin,
    {
        FlushFuture { writer: self }
    }

    /// Closes the writer.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::{AsyncWriteExt, BufWriter};
    ///
    /// # spin_on::spin_on(async {
    /// let mut output = Vec::new();
    /// let mut writer = BufWriter::new(&mut output);
    ///
    /// writer.close().await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    fn close(&mut self) -> CloseFuture<'_, Self>
    where
        Self: Unpin,
    {
        CloseFuture { writer: self }
    }

    /// Boxes the writer and changes its type to `dyn AsyncWrite + Send + 'a`.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::io::AsyncWriteExt;
    ///
    /// let writer = Vec::<u8>::new().boxed_writer();
    /// ```
    #[cfg(feature = "alloc")]
    fn boxed_writer<'a>(self) -> Pin<Box<dyn AsyncWrite + Send + 'a>>
    where
        Self: Sized + Send + 'a,
    {
        Box::pin(self)
    }
}

impl<W: AsyncWrite + ?Sized> AsyncWriteExt for W {}

/// Future for the [`AsyncWriteExt::write()`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct WriteFuture<'a, W: Unpin + ?Sized> {
    writer: &'a mut W,
    buf: &'a [u8],
}

impl<W: Unpin + ?Sized> Unpin for WriteFuture<'_, W> {}

impl<W: AsyncWrite + Unpin + ?Sized> Future for WriteFuture<'_, W> {
    type Output = Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let buf = self.buf;
        Pin::new(&mut *self.writer).poll_write(cx, buf)
    }
}

/// Future for the [`AsyncWriteExt::write_vectored()`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct WriteVectoredFuture<'a, W: Unpin + ?Sized> {
    writer: &'a mut W,
    bufs: &'a [IoSlice<'a>],
}

impl<W: Unpin + ?Sized> Unpin for WriteVectoredFuture<'_, W> {}

impl<W: AsyncWrite + Unpin + ?Sized> Future for WriteVectoredFuture<'_, W> {
    type Output = Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let bufs = self.bufs;
        Pin::new(&mut *self.writer).poll_write_vectored(cx, bufs)
    }
}

/// Future for the [`AsyncWriteExt::write_all()`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct WriteAllFuture<'a, W: Unpin + ?Sized> {
    writer: &'a mut W,
    buf: &'a [u8],
}

impl<W: Unpin + ?Sized> Unpin for WriteAllFuture<'_, W> {}

impl<W: AsyncWrite + Unpin + ?Sized> Future for WriteAllFuture<'_, W> {
    type Output = Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { writer, buf } = &mut *self;

        while !buf.is_empty() {
            let n = ready!(Pin::new(&mut **writer).poll_write(cx, buf))?;
            let (_, rest) = mem::take(buf).split_at(n);
            *buf = rest;

            if n == 0 {
                return Poll::Ready(Err(ErrorKind::WriteZero.into()));
            }
        }

        Poll::Ready(Ok(()))
    }
}

/// Future for the [`AsyncWriteExt::flush()`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct FlushFuture<'a, W: Unpin + ?Sized> {
    writer: &'a mut W,
}

impl<W: Unpin + ?Sized> Unpin for FlushFuture<'_, W> {}

impl<W: AsyncWrite + Unpin + ?Sized> Future for FlushFuture<'_, W> {
    type Output = Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut *self.writer).poll_flush(cx)
    }
}

/// Future for the [`AsyncWriteExt::close()`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct CloseFuture<'a, W: Unpin + ?Sized> {
    writer: &'a mut W,
}

impl<W: Unpin + ?Sized> Unpin for CloseFuture<'_, W> {}

impl<W: AsyncWrite + Unpin + ?Sized> Future for CloseFuture<'_, W> {
    type Output = Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut *self.writer).poll_close(cx)
    }
}

/// Type alias for `Pin<Box<dyn AsyncRead + Send + 'static>>`.
///
/// # Examples
///
/// ```
/// use futures_lite::io::AsyncReadExt;
///
/// let reader = [1, 2, 3].boxed_reader();
/// ```
#[cfg(feature = "alloc")]
pub type BoxedReader = Pin<Box<dyn AsyncRead + Send + 'static>>;

/// Type alias for `Pin<Box<dyn AsyncWrite + Send + 'static>>`.
///
/// # Examples
///
/// ```
/// use futures_lite::io::AsyncWriteExt;
///
/// let writer = Vec::<u8>::new().boxed_writer();
/// ```
#[cfg(feature = "alloc")]
pub type BoxedWriter = Pin<Box<dyn AsyncWrite + Send + 'static>>;

/// Splits a stream into [`AsyncRead`] and [`AsyncWrite`] halves.
///
/// # Examples
///
/// ```
/// use futures_lite::io::{self, Cursor};
///
/// # spin_on::spin_on(async {
/// let stream = Cursor::new(vec![]);
/// let (mut reader, mut writer) = io::split(stream);
/// # std::io::Result::Ok(()) });
/// ```
pub fn split<T>(stream: T) -> (ReadHalf<T>, WriteHalf<T>)
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    let inner = Arc::new(Mutex::new(stream));
    (ReadHalf(inner.clone()), WriteHalf(inner))
}

/// The read half returned by [`split()`].
#[derive(Debug)]
pub struct ReadHalf<T>(Arc<Mutex<T>>);

/// The write half returned by [`split()`].
#[derive(Debug)]
pub struct WriteHalf<T>(Arc<Mutex<T>>);

impl<T: AsyncRead + Unpin> AsyncRead for ReadHalf<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        let mut inner = self.0.lock().unwrap();
        Pin::new(&mut *inner).poll_read(cx, buf)
    }

    fn poll_read_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<Result<usize>> {
        let mut inner = self.0.lock().unwrap();
        Pin::new(&mut *inner).poll_read_vectored(cx, bufs)
    }
}

impl<T: AsyncWrite + Unpin> AsyncWrite for WriteHalf<T> {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        let mut inner = self.0.lock().unwrap();
        Pin::new(&mut *inner).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        let mut inner = self.0.lock().unwrap();
        Pin::new(&mut *inner).poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        let mut inner = self.0.lock().unwrap();
        Pin::new(&mut *inner).poll_close(cx)
    }
}

#[cfg(feature = "memchr")]
use memchr::memchr;

/// Unoptimized memchr fallback.
#[cfg(not(feature = "memchr"))]
fn memchr(needle: u8, haystack: &[u8]) -> Option<usize> {
    haystack.iter().position(|&b| b == needle)
}
