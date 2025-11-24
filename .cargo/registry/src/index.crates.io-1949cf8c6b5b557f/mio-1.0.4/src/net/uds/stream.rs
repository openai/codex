use std::fmt;
use std::io::{self, IoSlice, IoSliceMut, Read, Write};
use std::net::Shutdown;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::os::unix::net::{self, SocketAddr};
use std::path::Path;

use crate::io_source::IoSource;
use crate::{event, sys, Interest, Registry, Token};

/// A non-blocking Unix stream socket.
pub struct UnixStream {
    inner: IoSource<net::UnixStream>,
}

impl UnixStream {
    /// Connects to the socket named by `path`.
    ///
    /// This may return a `WouldBlock` in which case the socket connection
    /// cannot be completed immediately. Usually it means the backlog is full.
    pub fn connect<P: AsRef<Path>>(path: P) -> io::Result<UnixStream> {
        let addr = SocketAddr::from_pathname(path)?;
        UnixStream::connect_addr(&addr)
    }

    /// Connects to the socket named by `address`.
    ///
    /// This may return a `WouldBlock` in which case the socket connection
    /// cannot be completed immediately. Usually it means the backlog is full.
    pub fn connect_addr(address: &SocketAddr) -> io::Result<UnixStream> {
        sys::uds::stream::connect_addr(address).map(UnixStream::from_std)
    }

    /// Creates a new `UnixStream` from a standard `net::UnixStream`.
    ///
    /// This function is intended to be used to wrap a Unix stream from the
    /// standard library in the Mio equivalent. The conversion assumes nothing
    /// about the underlying stream; it is left up to the user to set it in
    /// non-blocking mode.
    ///
    /// # Note
    ///
    /// The Unix stream here will not have `connect` called on it, so it
    /// should already be connected via some other means (be it manually, or
    /// the standard library).
    pub fn from_std(stream: net::UnixStream) -> UnixStream {
        UnixStream {
            inner: IoSource::new(stream),
        }
    }

    /// Creates an unnamed pair of connected sockets.
    ///
    /// Returns two `UnixStream`s which are connected to each other.
    pub fn pair() -> io::Result<(UnixStream, UnixStream)> {
        sys::uds::stream::pair().map(|(stream1, stream2)| {
            (UnixStream::from_std(stream1), UnixStream::from_std(stream2))
        })
    }

    /// Returns the socket address of the local half of this connection.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    /// Returns the socket address of the remote half of this connection.
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.inner.peer_addr()
    }

    /// Returns the value of the `SO_ERROR` option.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.inner.take_error()
    }

    /// Shuts down the read, write, or both halves of this connection.
    ///
    /// This function will cause all pending and future I/O calls on the
    /// specified portions to immediately return with an appropriate value
    /// (see the documentation of `Shutdown`).
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.inner.shutdown(how)
    }

    /// Execute an I/O operation ensuring that the socket receives more events
    /// if it hits a [`WouldBlock`] error.
    ///
    /// # Notes
    ///
    /// This method is required to be called for **all** I/O operations to
    /// ensure the user will receive events once the socket is ready again after
    /// returning a [`WouldBlock`] error.
    ///
    /// [`WouldBlock`]: io::ErrorKind::WouldBlock
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// #
    /// # fn main() -> Result<(), Box<dyn Error>> {
    /// use std::io;
    /// use std::os::fd::AsRawFd;
    /// use mio::net::UnixStream;
    ///
    /// let (stream1, stream2) = UnixStream::pair()?;
    ///
    /// // Wait until the stream is writable...
    ///
    /// // Write to the stream using a direct libc call, of course the
    /// // `io::Write` implementation would be easier to use.
    /// let buf = b"hello";
    /// let n = stream1.try_io(|| {
    ///     let buf_ptr = &buf as *const _ as *const _;
    ///     let res = unsafe { libc::send(stream1.as_raw_fd(), buf_ptr, buf.len(), 0) };
    ///     if res != -1 {
    ///         Ok(res as usize)
    ///     } else {
    ///         // If EAGAIN or EWOULDBLOCK is set by libc::send, the closure
    ///         // should return `WouldBlock` error.
    ///         Err(io::Error::last_os_error())
    ///     }
    /// })?;
    /// eprintln!("write {} bytes", n);
    ///
    /// // Wait until the stream is readable...
    ///
    /// // Read from the stream using a direct libc call, of course the
    /// // `io::Read` implementation would be easier to use.
    /// let mut buf = [0; 512];
    /// let n = stream2.try_io(|| {
    ///     let buf_ptr = &mut buf as *mut _ as *mut _;
    ///     let res = unsafe { libc::recv(stream2.as_raw_fd(), buf_ptr, buf.len(), 0) };
    ///     if res != -1 {
    ///         Ok(res as usize)
    ///     } else {
    ///         // If EAGAIN or EWOULDBLOCK is set by libc::recv, the closure
    ///         // should return `WouldBlock` error.
    ///         Err(io::Error::last_os_error())
    ///     }
    /// })?;
    /// eprintln!("read {} bytes", n);
    /// # Ok(())
    /// # }
    /// ```
    pub fn try_io<F, T>(&self, f: F) -> io::Result<T>
    where
        F: FnOnce() -> io::Result<T>,
    {
        self.inner.do_io(|_| f())
    }
}

impl Read for UnixStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.do_io(|mut inner| inner.read(buf))
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.inner.do_io(|mut inner| inner.read_vectored(bufs))
    }
}

impl<'a> Read for &'a UnixStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.do_io(|mut inner| inner.read(buf))
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.inner.do_io(|mut inner| inner.read_vectored(bufs))
    }
}

impl Write for UnixStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.do_io(|mut inner| inner.write(buf))
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.inner.do_io(|mut inner| inner.write_vectored(bufs))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.do_io(|mut inner| inner.flush())
    }
}

impl<'a> Write for &'a UnixStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.do_io(|mut inner| inner.write(buf))
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.inner.do_io(|mut inner| inner.write_vectored(bufs))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.do_io(|mut inner| inner.flush())
    }
}

impl event::Source for UnixStream {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        self.inner.register(registry, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        self.inner.reregister(registry, token, interests)
    }

    fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
        self.inner.deregister(registry)
    }
}

impl fmt::Debug for UnixStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl IntoRawFd for UnixStream {
    fn into_raw_fd(self) -> RawFd {
        self.inner.into_inner().into_raw_fd()
    }
}

impl AsRawFd for UnixStream {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

impl FromRawFd for UnixStream {
    /// Converts a `RawFd` to a `UnixStream`.
    ///
    /// # Notes
    ///
    /// The caller is responsible for ensuring that the socket is in
    /// non-blocking mode.
    unsafe fn from_raw_fd(fd: RawFd) -> UnixStream {
        UnixStream::from_std(FromRawFd::from_raw_fd(fd))
    }
}

impl From<UnixStream> for net::UnixStream {
    fn from(stream: UnixStream) -> Self {
        // Safety: This is safe since we are extracting the raw fd from a well-constructed
        // mio::net::uds::UnixStream which ensures that we actually pass in a valid file
        // descriptor/socket
        unsafe { net::UnixStream::from_raw_fd(stream.into_raw_fd()) }
    }
}

impl From<UnixStream> for OwnedFd {
    fn from(unix_stream: UnixStream) -> Self {
        unix_stream.inner.into_inner().into()
    }
}

impl AsFd for UnixStream {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.inner.as_fd()
    }
}

impl From<OwnedFd> for UnixStream {
    fn from(fd: OwnedFd) -> Self {
        UnixStream::from_std(From::from(fd))
    }
}
