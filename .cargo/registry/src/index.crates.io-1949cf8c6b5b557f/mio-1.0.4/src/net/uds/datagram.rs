use std::net::Shutdown;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::os::unix::net::{self, SocketAddr};
use std::path::Path;
use std::{fmt, io};

use crate::io_source::IoSource;
use crate::{event, sys, Interest, Registry, Token};

/// A Unix datagram socket.
pub struct UnixDatagram {
    inner: IoSource<net::UnixDatagram>,
}

impl UnixDatagram {
    /// Creates a Unix datagram socket bound to the given path.
    pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<UnixDatagram> {
        let addr = SocketAddr::from_pathname(path)?;
        UnixDatagram::bind_addr(&addr)
    }

    /// Creates a new `UnixDatagram` bound to the specified socket `address`.
    pub fn bind_addr(address: &SocketAddr) -> io::Result<UnixDatagram> {
        sys::uds::datagram::bind_addr(address).map(UnixDatagram::from_std)
    }

    /// Creates a new `UnixDatagram` from a standard `net::UnixDatagram`.
    ///
    /// This function is intended to be used to wrap a Unix datagram from the
    /// standard library in the Mio equivalent. The conversion assumes nothing
    /// about the underlying datagram; it is left up to the user to set it in
    /// non-blocking mode.
    pub fn from_std(socket: net::UnixDatagram) -> UnixDatagram {
        UnixDatagram {
            inner: IoSource::new(socket),
        }
    }

    /// Connects the socket to the specified address.
    ///
    /// This may return a `WouldBlock` in which case the socket connection
    /// cannot be completed immediately.
    pub fn connect<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        self.inner.connect(path)
    }

    /// Creates a Unix Datagram socket which is not bound to any address.
    pub fn unbound() -> io::Result<UnixDatagram> {
        sys::uds::datagram::unbound().map(UnixDatagram::from_std)
    }

    /// Create an unnamed pair of connected sockets.
    pub fn pair() -> io::Result<(UnixDatagram, UnixDatagram)> {
        sys::uds::datagram::pair().map(|(socket1, socket2)| {
            (
                UnixDatagram::from_std(socket1),
                UnixDatagram::from_std(socket2),
            )
        })
    }

    /// Returns the address of this socket.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    /// Returns the address of this socket's peer.
    ///
    /// The `connect` method will connect the socket to a peer.
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.inner.peer_addr()
    }

    /// Receives data from the socket.
    ///
    /// On success, returns the number of bytes read and the address from
    /// whence the data came.
    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.inner.do_io(|inner| inner.recv_from(buf))
    }

    /// Receives data from the socket.
    ///
    /// On success, returns the number of bytes read.
    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.do_io(|inner| inner.recv(buf))
    }

    /// Sends data on the socket to the specified address.
    ///
    /// On success, returns the number of bytes written.
    pub fn send_to<P: AsRef<Path>>(&self, buf: &[u8], path: P) -> io::Result<usize> {
        self.inner.do_io(|inner| inner.send_to(buf, path))
    }

    /// Sends data on the socket to the socket's peer.
    ///
    /// The peer address may be set by the `connect` method, and this method
    /// will return an error if the socket has not already been connected.
    ///
    /// On success, returns the number of bytes written.
    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.inner.do_io(|inner| inner.send(buf))
    }

    /// Returns the value of the `SO_ERROR` option.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.inner.take_error()
    }

    /// Shut down the read, write, or both halves of this connection.
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
    /// use mio::net::UnixDatagram;
    ///
    /// let (dgram1, dgram2) = UnixDatagram::pair()?;
    ///
    /// // Wait until the dgram is writable...
    ///
    /// // Write to the dgram using a direct libc call, of course the
    /// // `io::Write` implementation would be easier to use.
    /// let buf = b"hello";
    /// let n = dgram1.try_io(|| {
    ///     let buf_ptr = &buf as *const _ as *const _;
    ///     let res = unsafe { libc::send(dgram1.as_raw_fd(), buf_ptr, buf.len(), 0) };
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
    /// // Wait until the dgram is readable...
    ///
    /// // Read from the dgram using a direct libc call, of course the
    /// // `io::Read` implementation would be easier to use.
    /// let mut buf = [0; 512];
    /// let n = dgram2.try_io(|| {
    ///     let buf_ptr = &mut buf as *mut _ as *mut _;
    ///     let res = unsafe { libc::recv(dgram2.as_raw_fd(), buf_ptr, buf.len(), 0) };
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

impl event::Source for UnixDatagram {
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

impl fmt::Debug for UnixDatagram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl IntoRawFd for UnixDatagram {
    fn into_raw_fd(self) -> RawFd {
        self.inner.into_inner().into_raw_fd()
    }
}

impl AsRawFd for UnixDatagram {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

impl FromRawFd for UnixDatagram {
    /// Converts a `RawFd` to a `UnixDatagram`.
    ///
    /// # Notes
    ///
    /// The caller is responsible for ensuring that the socket is in
    /// non-blocking mode.
    unsafe fn from_raw_fd(fd: RawFd) -> UnixDatagram {
        UnixDatagram::from_std(FromRawFd::from_raw_fd(fd))
    }
}

impl From<UnixDatagram> for net::UnixDatagram {
    fn from(datagram: UnixDatagram) -> Self {
        // Safety: This is safe since we are extracting the raw fd from a well-constructed
        // mio::net::uds::UnixListener which ensures that we actually pass in a valid file
        // descriptor/socket
        unsafe { net::UnixDatagram::from_raw_fd(datagram.into_raw_fd()) }
    }
}

impl From<UnixDatagram> for OwnedFd {
    fn from(unix_datagram: UnixDatagram) -> Self {
        unix_datagram.inner.into_inner().into()
    }
}

impl AsFd for UnixDatagram {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.inner.as_fd()
    }
}

impl From<OwnedFd> for UnixDatagram {
    fn from(fd: OwnedFd) -> Self {
        UnixDatagram::from_std(From::from(fd))
    }
}
