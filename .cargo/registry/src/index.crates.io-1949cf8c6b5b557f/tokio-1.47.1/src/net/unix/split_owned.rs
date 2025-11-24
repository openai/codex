//! `UnixStream` owned split support.
//!
//! A `UnixStream` can be split into an `OwnedReadHalf` and a `OwnedWriteHalf`
//! with the `UnixStream::into_split` method.  `OwnedReadHalf` implements
//! `AsyncRead` while `OwnedWriteHalf` implements `AsyncWrite`.
//!
//! Compared to the generic split of `AsyncRead + AsyncWrite`, this specialized
//! split has no associated overhead and enforces all invariants at the type
//! level.

use crate::io::{AsyncRead, AsyncWrite, Interest, ReadBuf, Ready};
use crate::net::UnixStream;

use crate::net::unix::SocketAddr;
use std::error::Error;
use std::net::Shutdown;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::{fmt, io};

cfg_io_util! {
    use bytes::BufMut;
}

/// Owned read half of a [`UnixStream`], created by [`into_split`].
///
/// Reading from an `OwnedReadHalf` is usually done using the convenience methods found
/// on the [`AsyncReadExt`] trait.
///
/// [`UnixStream`]: crate::net::UnixStream
/// [`into_split`]: crate::net::UnixStream::into_split()
/// [`AsyncReadExt`]: trait@crate::io::AsyncReadExt
#[derive(Debug)]
pub struct OwnedReadHalf {
    inner: Arc<UnixStream>,
}

/// Owned write half of a [`UnixStream`], created by [`into_split`].
///
/// Note that in the [`AsyncWrite`] implementation of this type,
/// [`poll_shutdown`] will shut down the stream in the write direction.
/// Dropping the write half will also shut down the write half of the stream.
///
/// Writing to an `OwnedWriteHalf` is usually done using the convenience methods
/// found on the [`AsyncWriteExt`] trait.
///
/// [`UnixStream`]: crate::net::UnixStream
/// [`into_split`]: crate::net::UnixStream::into_split()
/// [`AsyncWrite`]: trait@crate::io::AsyncWrite
/// [`poll_shutdown`]: fn@crate::io::AsyncWrite::poll_shutdown
/// [`AsyncWriteExt`]: trait@crate::io::AsyncWriteExt
#[derive(Debug)]
pub struct OwnedWriteHalf {
    inner: Arc<UnixStream>,
    shutdown_on_drop: bool,
}

pub(crate) fn split_owned(stream: UnixStream) -> (OwnedReadHalf, OwnedWriteHalf) {
    let arc = Arc::new(stream);
    let read = OwnedReadHalf {
        inner: Arc::clone(&arc),
    };
    let write = OwnedWriteHalf {
        inner: arc,
        shutdown_on_drop: true,
    };
    (read, write)
}

pub(crate) fn reunite(
    read: OwnedReadHalf,
    write: OwnedWriteHalf,
) -> Result<UnixStream, ReuniteError> {
    if Arc::ptr_eq(&read.inner, &write.inner) {
        write.forget();
        // This unwrap cannot fail as the api does not allow creating more than two Arcs,
        // and we just dropped the other half.
        Ok(Arc::try_unwrap(read.inner).expect("UnixStream: try_unwrap failed in reunite"))
    } else {
        Err(ReuniteError(read, write))
    }
}

/// Error indicating that two halves were not from the same socket, and thus could
/// not be reunited.
#[derive(Debug)]
pub struct ReuniteError(pub OwnedReadHalf, pub OwnedWriteHalf);

impl fmt::Display for ReuniteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "tried to reunite halves that are not from the same socket"
        )
    }
}

impl Error for ReuniteError {}

impl OwnedReadHalf {
    /// Attempts to put the two halves of a `UnixStream` back together and
    /// recover the original socket. Succeeds only if the two halves
    /// originated from the same call to [`into_split`].
    ///
    /// [`into_split`]: crate::net::UnixStream::into_split()
    pub fn reunite(self, other: OwnedWriteHalf) -> Result<UnixStream, ReuniteError> {
        reunite(self, other)
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
    /// This function is equivalent to [`UnixStream::ready`].
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
        self.inner.ready(interest).await
    }

    /// Waits for the socket to become readable.
    ///
    /// This function is equivalent to `ready(Interest::READABLE)` and is usually
    /// paired with `try_read()`.
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. Once a readiness event occurs, the method
    /// will continue to return immediately until the readiness event is
    /// consumed by an attempt to read that fails with `WouldBlock` or
    /// `Poll::Pending`.
    pub async fn readable(&self) -> io::Result<()> {
        self.inner.readable().await
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
        self.inner.try_read(buf)
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
            self.inner.try_read_buf(buf)
        }
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
        self.inner.try_read_vectored(bufs)
    }

    /// Returns the socket address of the remote half of this connection.
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.inner.peer_addr()
    }

    /// Returns the socket address of the local half of this connection.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }
}

impl AsyncRead for OwnedReadHalf {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.inner.poll_read_priv(cx, buf)
    }
}

impl OwnedWriteHalf {
    /// Attempts to put the two halves of a `UnixStream` back together and
    /// recover the original socket. Succeeds only if the two halves
    /// originated from the same call to [`into_split`].
    ///
    /// [`into_split`]: crate::net::UnixStream::into_split()
    pub fn reunite(self, other: OwnedReadHalf) -> Result<UnixStream, ReuniteError> {
        reunite(other, self)
    }

    /// Destroys the write half, but don't close the write half of the stream
    /// until the read half is dropped. If the read half has already been
    /// dropped, this closes the stream.
    pub fn forget(mut self) {
        self.shutdown_on_drop = false;
        drop(self);
    }

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
    /// This function is equivalent to [`UnixStream::ready`].
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
        self.inner.ready(interest).await
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
        self.inner.writable().await
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
        self.inner.try_write(buf)
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
    pub fn try_write_vectored(&self, buf: &[io::IoSlice<'_>]) -> io::Result<usize> {
        self.inner.try_write_vectored(buf)
    }

    /// Returns the socket address of the remote half of this connection.
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.inner.peer_addr()
    }

    /// Returns the socket address of the local half of this connection.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }
}

impl Drop for OwnedWriteHalf {
    fn drop(&mut self) {
        if self.shutdown_on_drop {
            let _ = self.inner.shutdown_std(Shutdown::Write);
        }
    }
}

impl AsyncWrite for OwnedWriteHalf {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.inner.poll_write_priv(cx, buf)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        self.inner.poll_write_vectored_priv(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    #[inline]
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        // flush is a no-op
        Poll::Ready(Ok(()))
    }

    // `poll_shutdown` on a write half shutdowns the stream in the "write" direction.
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        let res = self.inner.shutdown_std(Shutdown::Write);
        if res.is_ok() {
            Pin::into_inner(self).shutdown_on_drop = false;
        }
        res.into()
    }
}

impl AsRef<UnixStream> for OwnedReadHalf {
    fn as_ref(&self) -> &UnixStream {
        &self.inner
    }
}

impl AsRef<UnixStream> for OwnedWriteHalf {
    fn as_ref(&self) -> &UnixStream {
        &self.inner
    }
}
