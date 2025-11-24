//! Compatibility between the `tokio::io` and `futures-io` versions of the
//! `AsyncRead` and `AsyncWrite` traits.
//!
//! ## Bridging Tokio and Futures I/O with `compat()`
//!
//! The [`compat()`] function provides a compatibility layer that allows types implementing
//! [`tokio::io::AsyncRead`] or [`tokio::io::AsyncWrite`] to be used as their
//! [`futures::io::AsyncRead`] or [`futures::io::AsyncWrite`] counterparts — and vice versa.
//!
//! This is especially useful when working with libraries that expect I/O types from one ecosystem
//! (usually `futures`) but you are using types from the other (usually `tokio`).
//!
//! ## Compatibility Overview
//!
//! | Inner Type Implements...    | `Compat<T>` Implements...   |
//! |-----------------------------|-----------------------------|
//! | [`tokio::io::AsyncRead`]    | [`futures::io::AsyncRead`]  |
//! | [`futures::io::AsyncRead`]  | [`tokio::io::AsyncRead`]    |
//! | [`tokio::io::AsyncWrite`]   | [`futures::io::AsyncWrite`] |
//! | [`futures::io::AsyncWrite`] | [`tokio::io::AsyncWrite`]   |
//!
//! ## Feature Flag
//!
//! This functionality is available through the `compat` feature flag:
//!
//! ```toml
//! tokio-util = { version = "...", features = ["compat"] }
//! ```
//!
//! ## Example 1: Tokio -> Futures (`AsyncRead`)
//!
//! This example demonstrates sending data over a [`tokio::net::TcpStream`] and using
//! [`futures::io::AsyncReadExt::read`] from the `futures` crate to read it after adapting the
//! stream via [`compat()`].
//!
//! ```no_run
//! use tokio::net::{TcpListener, TcpStream};
//! use tokio::io::AsyncWriteExt;
//! use tokio_util::compat::TokioAsyncReadCompatExt;
//! use futures::io::AsyncReadExt;
//!
//! #[tokio::main]
//! async fn main() -> std::io::Result<()> {
//!     let listener = TcpListener::bind("127.0.0.1:8081").await?;
//!
//!     tokio::spawn(async {
//!         let mut client = TcpStream::connect("127.0.0.1:8081").await.unwrap();
//!         client.write_all(b"Hello World").await.unwrap();
//!     });
//!
//!     let (stream, _) = listener.accept().await?;
//!
//!     // Adapt `tokio::TcpStream` to be used with `futures::io::AsyncReadExt`
//!     let mut compat_stream = stream.compat();
//!     let mut buffer = [0; 20];
//!     let n = compat_stream.read(&mut buffer).await?;
//!     println!("Received: {}", String::from_utf8_lossy(&buffer[..n]));
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Example 2: Futures -> Tokio (`AsyncRead`)
//!
//! The reverse is also possible: you can take a [`futures::io::AsyncRead`] (e.g. a cursor) and
//! adapt it to be used with [`tokio::io::AsyncReadExt::read_to_end`]
//!
//! ```
//! use futures::io::Cursor;
//! use tokio_util::compat::FuturesAsyncReadCompatExt;
//! use tokio::io::AsyncReadExt;
//!
//! fn main() {
//!     let future = async {
//!         let reader = Cursor::new(b"Hello from futures");
//!         let mut compat_reader = reader.compat();
//!         let mut buf = Vec::new();
//!         compat_reader.read_to_end(&mut buf).await.unwrap();
//!         assert_eq!(&buf, b"Hello from futures");
//!     };
//!
//!     // Run the future inside a Tokio runtime
//!     tokio::runtime::Runtime::new().unwrap().block_on(future);
//! }
//! ```
//!
//! ## Common Use Cases
//!
//! - Using `tokio` sockets with `async-tungstenite`, `async-compression`, or `futures-rs`-based
//!   libraries.
//! - Bridging I/O interfaces between mixed-ecosystem libraries.
//! - Avoiding rewrites or duplication of I/O code in async environments.
//!
//! ## See Also
//!
//! - [`Compat`] type
//! - [`TokioAsyncReadCompatExt`]
//! - [`FuturesAsyncReadCompatExt`]
//! - [`tokio::io`]
//! - [`futures::io`]
//!
//! [`futures::io`]: https://docs.rs/futures/latest/futures/io/
//! [`futures::io::AsyncRead`]: https://docs.rs/futures/latest/futures/io/trait.AsyncRead.html
//! [`futures::io::AsyncWrite`]: https://docs.rs/futures/latest/futures/io/trait.AsyncWrite.html
//! [`futures::io::AsyncReadExt::read`]: https://docs.rs/futures/latest/futures/io/trait.AsyncReadExt.html#method.read
//! [`compat()`]: TokioAsyncReadCompatExt::compat

use pin_project_lite::pin_project;
use std::io;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

pin_project! {
    /// A compatibility layer that allows conversion between the
    /// `tokio::io` and `futures-io` `AsyncRead` and `AsyncWrite` traits.
    #[derive(Copy, Clone, Debug)]
    pub struct Compat<T> {
        #[pin]
        inner: T,
        seek_pos: Option<io::SeekFrom>,
    }
}

/// Extension trait that allows converting a type implementing
/// `futures_io::AsyncRead` to implement `tokio::io::AsyncRead`.
pub trait FuturesAsyncReadCompatExt: futures_io::AsyncRead {
    /// Wraps `self` with a compatibility layer that implements
    /// `tokio_io::AsyncRead`.
    fn compat(self) -> Compat<Self>
    where
        Self: Sized,
    {
        Compat::new(self)
    }
}

impl<T: futures_io::AsyncRead> FuturesAsyncReadCompatExt for T {}

/// Extension trait that allows converting a type implementing
/// `futures_io::AsyncWrite` to implement `tokio::io::AsyncWrite`.
pub trait FuturesAsyncWriteCompatExt: futures_io::AsyncWrite {
    /// Wraps `self` with a compatibility layer that implements
    /// `tokio::io::AsyncWrite`.
    fn compat_write(self) -> Compat<Self>
    where
        Self: Sized,
    {
        Compat::new(self)
    }
}

impl<T: futures_io::AsyncWrite> FuturesAsyncWriteCompatExt for T {}

/// Extension trait that allows converting a type implementing
/// `tokio::io::AsyncRead` to implement `futures_io::AsyncRead`.
pub trait TokioAsyncReadCompatExt: tokio::io::AsyncRead {
    /// Wraps `self` with a compatibility layer that implements
    /// `futures_io::AsyncRead`.
    fn compat(self) -> Compat<Self>
    where
        Self: Sized,
    {
        Compat::new(self)
    }
}

impl<T: tokio::io::AsyncRead> TokioAsyncReadCompatExt for T {}

/// Extension trait that allows converting a type implementing
/// `tokio::io::AsyncWrite` to implement `futures_io::AsyncWrite`.
pub trait TokioAsyncWriteCompatExt: tokio::io::AsyncWrite {
    /// Wraps `self` with a compatibility layer that implements
    /// `futures_io::AsyncWrite`.
    fn compat_write(self) -> Compat<Self>
    where
        Self: Sized,
    {
        Compat::new(self)
    }
}

impl<T: tokio::io::AsyncWrite> TokioAsyncWriteCompatExt for T {}

// === impl Compat ===

impl<T> Compat<T> {
    fn new(inner: T) -> Self {
        Self {
            inner,
            seek_pos: None,
        }
    }

    /// Get a reference to the `Future`, `Stream`, `AsyncRead`, or `AsyncWrite` object
    /// contained within.
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Get a mutable reference to the `Future`, `Stream`, `AsyncRead`, or `AsyncWrite` object
    /// contained within.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Returns the wrapped item.
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T> tokio::io::AsyncRead for Compat<T>
where
    T: futures_io::AsyncRead,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // We can't trust the inner type to not peak at the bytes,
        // so we must defensively initialize the buffer.
        let slice = buf.initialize_unfilled();
        let n = ready!(futures_io::AsyncRead::poll_read(
            self.project().inner,
            cx,
            slice
        ))?;
        buf.advance(n);
        Poll::Ready(Ok(()))
    }
}

impl<T> futures_io::AsyncRead for Compat<T>
where
    T: tokio::io::AsyncRead,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        slice: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let mut buf = tokio::io::ReadBuf::new(slice);
        ready!(tokio::io::AsyncRead::poll_read(
            self.project().inner,
            cx,
            &mut buf
        ))?;
        Poll::Ready(Ok(buf.filled().len()))
    }
}

impl<T> tokio::io::AsyncBufRead for Compat<T>
where
    T: futures_io::AsyncBufRead,
{
    fn poll_fill_buf<'a>(
        self: Pin<&'a mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<&'a [u8]>> {
        futures_io::AsyncBufRead::poll_fill_buf(self.project().inner, cx)
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        futures_io::AsyncBufRead::consume(self.project().inner, amt)
    }
}

impl<T> futures_io::AsyncBufRead for Compat<T>
where
    T: tokio::io::AsyncBufRead,
{
    fn poll_fill_buf<'a>(
        self: Pin<&'a mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<&'a [u8]>> {
        tokio::io::AsyncBufRead::poll_fill_buf(self.project().inner, cx)
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        tokio::io::AsyncBufRead::consume(self.project().inner, amt)
    }
}

impl<T> tokio::io::AsyncWrite for Compat<T>
where
    T: futures_io::AsyncWrite,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        futures_io::AsyncWrite::poll_write(self.project().inner, cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        futures_io::AsyncWrite::poll_flush(self.project().inner, cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        futures_io::AsyncWrite::poll_close(self.project().inner, cx)
    }
}

impl<T> futures_io::AsyncWrite for Compat<T>
where
    T: tokio::io::AsyncWrite,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        tokio::io::AsyncWrite::poll_write(self.project().inner, cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        tokio::io::AsyncWrite::poll_flush(self.project().inner, cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        tokio::io::AsyncWrite::poll_shutdown(self.project().inner, cx)
    }
}

impl<T: tokio::io::AsyncSeek> futures_io::AsyncSeek for Compat<T> {
    fn poll_seek(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        pos: io::SeekFrom,
    ) -> Poll<io::Result<u64>> {
        if self.seek_pos != Some(pos) {
            // Ensure previous seeks have finished before starting a new one
            ready!(self.as_mut().project().inner.poll_complete(cx))?;
            self.as_mut().project().inner.start_seek(pos)?;
            *self.as_mut().project().seek_pos = Some(pos);
        }
        let res = ready!(self.as_mut().project().inner.poll_complete(cx));
        *self.as_mut().project().seek_pos = None;
        Poll::Ready(res)
    }
}

impl<T: futures_io::AsyncSeek> tokio::io::AsyncSeek for Compat<T> {
    fn start_seek(mut self: Pin<&mut Self>, pos: io::SeekFrom) -> io::Result<()> {
        *self.as_mut().project().seek_pos = Some(pos);
        Ok(())
    }

    fn poll_complete(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<u64>> {
        let pos = match self.seek_pos {
            None => {
                // tokio 1.x AsyncSeek recommends calling poll_complete before start_seek.
                // We don't have to guarantee that the value returned by
                // poll_complete called without start_seek is correct,
                // so we'll return 0.
                return Poll::Ready(Ok(0));
            }
            Some(pos) => pos,
        };
        let res = ready!(self.as_mut().project().inner.poll_seek(cx, pos));
        *self.as_mut().project().seek_pos = None;
        Poll::Ready(res)
    }
}

#[cfg(unix)]
impl<T: std::os::unix::io::AsRawFd> std::os::unix::io::AsRawFd for Compat<T> {
    fn as_raw_fd(&self) -> std::os::unix::io::RawFd {
        self.inner.as_raw_fd()
    }
}

#[cfg(windows)]
impl<T: std::os::windows::io::AsRawHandle> std::os::windows::io::AsRawHandle for Compat<T> {
    fn as_raw_handle(&self) -> std::os::windows::io::RawHandle {
        self.inner.as_raw_handle()
    }
}
