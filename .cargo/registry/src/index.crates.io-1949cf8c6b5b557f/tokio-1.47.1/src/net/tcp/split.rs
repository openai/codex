//! `TcpStream` split support.
//!
//! A `TcpStream` can be split into a `ReadHalf` and a
//! `WriteHalf` with the `TcpStream::split` method. `ReadHalf`
//! implements `AsyncRead` while `WriteHalf` implements `AsyncWrite`.
//!
//! Compared to the generic split of `AsyncRead + AsyncWrite`, this specialized
//! split has no associated overhead and enforces all invariants at the type
//! level.

use crate::io::{AsyncRead, AsyncWrite, Interest, ReadBuf, Ready};
use crate::net::TcpStream;

use std::future::poll_fn;
use std::io;
use std::net::{Shutdown, SocketAddr};
use std::pin::Pin;
use std::task::{Context, Poll};

cfg_io_util! {
    use bytes::BufMut;
}

/// Borrowed read half of a [`TcpStream`], created by [`split`].
///
/// Reading from a `ReadHalf` is usually done using the convenience methods found on the
/// [`AsyncReadExt`] trait.
///
/// [`TcpStream`]: TcpStream
/// [`split`]: TcpStream::split()
/// [`AsyncReadExt`]: trait@crate::io::AsyncReadExt
#[derive(Debug)]
pub struct ReadHalf<'a>(&'a TcpStream);

/// Borrowed write half of a [`TcpStream`], created by [`split`].
///
/// Note that in the [`AsyncWrite`] implementation of this type, [`poll_shutdown`] will
/// shut down the TCP stream in the write direction.
///
/// Writing to an `WriteHalf` is usually done using the convenience methods found
/// on the [`AsyncWriteExt`] trait.
///
/// [`TcpStream`]: TcpStream
/// [`split`]: TcpStream::split()
/// [`AsyncWrite`]: trait@crate::io::AsyncWrite
/// [`poll_shutdown`]: fn@crate::io::AsyncWrite::poll_shutdown
/// [`AsyncWriteExt`]: trait@crate::io::AsyncWriteExt
#[derive(Debug)]
pub struct WriteHalf<'a>(&'a TcpStream);

pub(crate) fn split(stream: &mut TcpStream) -> (ReadHalf<'_>, WriteHalf<'_>) {
    (ReadHalf(&*stream), WriteHalf(&*stream))
}

impl ReadHalf<'_> {
    /// Attempts to receive data on the socket, without removing that data from
    /// the queue, registering the current task for wakeup if data is not yet
    /// available.
    ///
    /// Note that on multiple calls to `poll_peek` or `poll_read`, only the
    /// `Waker` from the `Context` passed to the most recent call is scheduled
    /// to receive a wakeup.
    ///
    /// See the [`TcpStream::poll_peek`] level documentation for more details.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::io::{self, ReadBuf};
    /// use tokio::net::TcpStream;
    ///
    /// use std::future::poll_fn;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     let mut stream = TcpStream::connect("127.0.0.1:8000").await?;
    ///     let (mut read_half, _) = stream.split();
    ///     let mut buf = [0; 10];
    ///     let mut buf = ReadBuf::new(&mut buf);
    ///
    ///     poll_fn(|cx| {
    ///         read_half.poll_peek(cx, &mut buf)
    ///     }).await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    ///
    /// [`TcpStream::poll_peek`]: TcpStream::poll_peek
    pub fn poll_peek(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<usize>> {
        self.0.poll_peek(cx, buf)
    }

    /// Receives data on the socket from the remote address to which it is
    /// connected, without removing that data from the queue. On success,
    /// returns the number of bytes peeked.
    ///
    /// See the [`TcpStream::peek`] level documentation for more details.
    ///
    /// [`TcpStream::peek`]: TcpStream::peek
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::TcpStream;
    /// use tokio::io::AsyncReadExt;
    /// use std::error::Error;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     // Connect to a peer
    ///     let mut stream = TcpStream::connect("127.0.0.1:8080").await?;
    ///     let (mut read_half, _) = stream.split();
    ///
    ///     let mut b1 = [0; 10];
    ///     let mut b2 = [0; 10];
    ///
    ///     // Peek at the data
    ///     let n = read_half.peek(&mut b1).await?;
    ///
    ///     // Read the data
    ///     assert_eq!(n, read_half.read(&mut b2[..n]).await?);
    ///     assert_eq!(&b1[..n], &b2[..n]);
    ///
    ///     Ok(())
    /// }
    /// ```
    ///
    /// The [`read`] method is defined on the [`AsyncReadExt`] trait.
    ///
    /// [`read`]: fn@crate::io::AsyncReadExt::read
    /// [`AsyncReadExt`]: trait@crate::io::AsyncReadExt
    pub async fn peek(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut buf = ReadBuf::new(buf);
        poll_fn(|cx| self.poll_peek(cx, &mut buf)).await
    }

    /// Waits for any of the requested ready states.
    ///
    /// This function is usually paired with [`try_read()`]. It can be used instead
    /// of [`readable()`] to check the returned ready set for [`Ready::READABLE`]
    /// and [`Ready::READ_CLOSED`] events.
    ///
    /// The function may complete without the socket being ready. This is a
    /// false-positive and attempting an operation will return with
    /// `io::ErrorKind::WouldBlock`. The function can also return with an empty
    /// [`Ready`] set, so you should always check the returned value and possibly
    /// wait again if the requested states are not set.
    ///
    /// This function is equivalent to [`TcpStream::ready`].
    ///
    /// [`try_read()`]: Self::try_read
    /// [`readable()`]: Self::readable
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. Once a readiness event occurs, the method
    /// will continue to return immediately until the readiness event is
    /// consumed by an attempt to read or write that fails with `WouldBlock` or
    /// `Poll::Pending`.
    pub async fn ready(&self, interest: Interest) -> io::Result<Ready> {
        self.0.ready(interest).await
    }

    /// Waits for the socket to become readable.
    ///
    /// This function is equivalent to `ready(Interest::READABLE)` and is usually
    /// paired with `try_read()`.
    ///
    /// This function is also equivalent to [`TcpStream::ready`].
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. Once a readiness event occurs, the method
    /// will continue to return immediately until the readiness event is
    /// consumed by an attempt to read that fails with `WouldBlock` or
    /// `Poll::Pending`.
    pub async fn readable(&self) -> io::Result<()> {
        self.0.readable().await
    }

    /// Tries to read data from the stream into the provided buffer, returning how
    /// many bytes were read.
    ///
    /// Receives any pending data from the socket but does not wait for new data
    /// to arrive. On success, returns the number of bytes read. Because
    /// `try_read()` is non-blocking, the buffer does not have to be stored by
    /// the async task and can exist entirely on the stack.
    ///
    /// Usually, [`readable()`] or [`ready()`] is used with this function.
    ///
    /// [`readable()`]: Self::readable()
    /// [`ready()`]: Self::ready()
    ///
    /// # Return
    ///
    /// If data is successfully read, `Ok(n)` is returned, where `n` is the
    /// number of bytes read. If `n` is `0`, then it can indicate one of two scenarios:
    ///
    /// 1. The stream's read half is closed and will no longer yield data.
    /// 2. The specified buffer was 0 bytes in length.
    ///
    /// If the stream is not ready to read data,
    /// `Err(io::ErrorKind::WouldBlock)` is returned.
    pub fn try_read(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.try_read(buf)
    }

    /// Tries to read data from the stream into the provided buffers, returning
    /// how many bytes were read.
    ///
    /// Data is copied to fill each buffer in order, with the final buffer
    /// written to possibly being only partially filled. This method behaves
    /// equivalently to a single call to [`try_read()`] with concatenated
    /// buffers.
    ///
    /// Receives any pending data from the socket but does not wait for new data
    /// to arrive. On success, returns the number of bytes read. Because
    /// `try_read_vectored()` is non-blocking, the buffer does not have to be
    /// stored by the async task and can exist entirely on the stack.
    ///
    /// Usually, [`readable()`] or [`ready()`] is used with this function.
    ///
    /// [`try_read()`]: Self::try_read()
    /// [`readable()`]: Self::readable()
    /// [`ready()`]: Self::ready()
    ///
    /// # Return
    ///
    /// If data is successfully read, `Ok(n)` is returned, where `n` is the
    /// number of bytes read. `Ok(0)` indicates the stream's read half is closed
    /// and will no longer yield data. If the stream is not ready to read data
    /// `Err(io::ErrorKind::WouldBlock)` is returned.
    pub fn try_read_vectored(&self, bufs: &mut [io::IoSliceMut<'_>]) -> io::Result<usize> {
        self.0.try_read_vectored(bufs)
    }

    cfg_io_util! {
        /// Tries to read data from the stream into the provided buffer, advancing the
        /// buffer's internal cursor, returning how many bytes were read.
        ///
        /// Receives any pending data from the socket but does not wait for new data
        /// to arrive. On success, returns the number of bytes read. Because
        /// `try_read_buf()` is non-blocking, the buffer does not have to be stored by
        /// the async task and can exist entirely on the stack.
        ///
        /// Usually, [`readable()`] or [`ready()`] is used with this function.
        ///
        /// [`readable()`]: Self::readable()
        /// [`ready()`]: Self::ready()
        ///
        /// # Return
        ///
        /// If data is successfully read, `Ok(n)` is returned, where `n` is the
        /// number of bytes read. `Ok(0)` indicates the stream's read half is closed
        /// and will no longer yield data. If the stream is not ready to read data
        /// `Err(io::ErrorKind::WouldBlock)` is returned.
        pub fn try_read_buf<B: BufMut>(&self, buf: &mut B) -> io::Result<usize> {
            self.0.try_read_buf(buf)
        }
    }

    /// Returns the remote address that this stream is connected to.
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.0.peer_addr()
    }

    /// Returns the local address that this stream is bound to.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.0.local_addr()
    }
}

impl WriteHalf<'_> {
    /// Waits for any of the requested ready states.
    ///
    /// This function is usually paired with [`try_write()`]. It can be used instead
    /// of [`writable()`] to check the returned ready set for [`Ready::WRITABLE`]
    /// and [`Ready::WRITE_CLOSED`] events.
    ///
    /// The function may complete without the socket being ready. This is a
    /// false-positive and attempting an operation will return with
    /// `io::ErrorKind::WouldBlock`. The function can also return with an empty
    /// [`Ready`] set, so you should always check the returned value and possibly
    /// wait again if the requested states are not set.
    ///
    /// This function is equivalent to [`TcpStream::ready`].
    ///
    /// [`try_write()`]: Self::try_write
    /// [`writable()`]: Self::writable
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. Once a readiness event occurs, the method
    /// will continue to return immediately until the readiness event is
    /// consumed by an attempt to read or write that fails with `WouldBlock` or
    /// `Poll::Pending`.
    pub async fn ready(&self, interest: Interest) -> io::Result<Ready> {
        self.0.ready(interest).await
    }

    /// Waits for the socket to become writable.
    ///
    /// This function is equivalent to `ready(Interest::WRITABLE)` and is usually
    /// paired with `try_write()`.
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. Once a readiness event occurs, the method
    /// will continue to return immediately until the readiness event is
    /// consumed by an attempt to write that fails with `WouldBlock` or
    /// `Poll::Pending`.
    pub async fn writable(&self) -> io::Result<()> {
        self.0.writable().await
    }

    /// Tries to write a buffer to the stream, returning how many bytes were
    /// written.
    ///
    /// The function will attempt to write the entire contents of `buf`, but
    /// only part of the buffer may be written.
    ///
    /// This function is usually paired with `writable()`.
    ///
    /// # Return
    ///
    /// If data is successfully written, `Ok(n)` is returned, where `n` is the
    /// number of bytes written. If the stream is not ready to write data,
    /// `Err(io::ErrorKind::WouldBlock)` is returned.
    pub fn try_write(&self, buf: &[u8]) -> io::Result<usize> {
        self.0.try_write(buf)
    }

    /// Tries to write several buffers to the stream, returning how many bytes
    /// were written.
    ///
    /// Data is written from each buffer in order, with the final buffer read
    /// from possible being only partially consumed. This method behaves
    /// equivalently to a single call to [`try_write()`] with concatenated
    /// buffers.
    ///
    /// This function is usually paired with `writable()`.
    ///
    /// [`try_write()`]: Self::try_write()
    ///
    /// # Return
    ///
    /// If data is successfully written, `Ok(n)` is returned, where `n` is the
    /// number of bytes written. If the stream is not ready to write data,
    /// `Err(io::ErrorKind::WouldBlock)` is returned.
    pub fn try_write_vectored(&self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        self.0.try_write_vectored(bufs)
    }

    /// Returns the remote address that this stream is connected to.
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.0.peer_addr()
    }

    /// Returns the local address that this stream is bound to.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.0.local_addr()
    }
}

impl AsyncRead for ReadHalf<'_> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.0.poll_read_priv(cx, buf)
    }
}

impl AsyncWrite for WriteHalf<'_> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.0.poll_write_priv(cx, buf)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        self.0.poll_write_vectored_priv(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.0.is_write_vectored()
    }

    #[inline]
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        // tcp flush is a no-op
        Poll::Ready(Ok(()))
    }

    // `poll_shutdown` on a write half shutdowns the stream in the "write" direction.
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.0.shutdown_std(Shutdown::Write).into()
    }
}

impl AsRef<TcpStream> for ReadHalf<'_> {
    fn as_ref(&self) -> &TcpStream {
        self.0
    }
}

impl AsRef<TcpStream> for WriteHalf<'_> {
    fn as_ref(&self) -> &TcpStream {
        self.0
    }
}
