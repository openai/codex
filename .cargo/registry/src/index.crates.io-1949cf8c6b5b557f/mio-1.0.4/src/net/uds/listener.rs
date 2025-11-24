use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::os::unix::net::{self, SocketAddr};
use std::path::Path;
use std::{fmt, io};

use crate::io_source::IoSource;
use crate::net::UnixStream;
use crate::{event, sys, Interest, Registry, Token};

/// A non-blocking Unix domain socket server.
pub struct UnixListener {
    inner: IoSource<net::UnixListener>,
}

impl UnixListener {
    /// Creates a new `UnixListener` bound to the specified socket `path`.
    pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<UnixListener> {
        let addr = SocketAddr::from_pathname(path)?;
        UnixListener::bind_addr(&addr)
    }

    /// Creates a new `UnixListener` bound to the specified socket `address`.
    pub fn bind_addr(address: &SocketAddr) -> io::Result<UnixListener> {
        sys::uds::listener::bind_addr(address).map(UnixListener::from_std)
    }

    /// Creates a new `UnixListener` from a standard `net::UnixListener`.
    ///
    /// This function is intended to be used to wrap a Unix listener from the
    /// standard library in the Mio equivalent. The conversion assumes nothing
    /// about the underlying listener; it is left up to the user to set it in
    /// non-blocking mode.
    pub fn from_std(listener: net::UnixListener) -> UnixListener {
        UnixListener {
            inner: IoSource::new(listener),
        }
    }

    /// Accepts a new incoming connection to this listener.
    ///
    /// The call is responsible for ensuring that the listening socket is in
    /// non-blocking mode.
    pub fn accept(&self) -> io::Result<(UnixStream, SocketAddr)> {
        sys::uds::listener::accept(&self.inner)
    }

    /// Returns the local socket address of this listener.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    /// Returns the value of the `SO_ERROR` option.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.inner.take_error()
    }
}

impl event::Source for UnixListener {
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

impl fmt::Debug for UnixListener {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl IntoRawFd for UnixListener {
    fn into_raw_fd(self) -> RawFd {
        self.inner.into_inner().into_raw_fd()
    }
}

impl AsRawFd for UnixListener {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

impl FromRawFd for UnixListener {
    /// Converts a `RawFd` to a `UnixListener`.
    ///
    /// # Notes
    ///
    /// The caller is responsible for ensuring that the socket is in
    /// non-blocking mode.
    unsafe fn from_raw_fd(fd: RawFd) -> UnixListener {
        UnixListener::from_std(FromRawFd::from_raw_fd(fd))
    }
}

impl From<UnixListener> for net::UnixListener {
    fn from(listener: UnixListener) -> Self {
        // Safety: This is safe since we are extracting the raw fd from a well-constructed
        // mio::net::uds::UnixListener which ensures that we actually pass in a valid file
        // descriptor/socket
        unsafe { net::UnixListener::from_raw_fd(listener.into_raw_fd()) }
    }
}

impl From<UnixListener> for OwnedFd {
    fn from(unix_listener: UnixListener) -> Self {
        unix_listener.inner.into_inner().into()
    }
}

impl AsFd for UnixListener {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.inner.as_fd()
    }
}

impl From<OwnedFd> for UnixListener {
    fn from(fd: OwnedFd) -> Self {
        UnixListener::from_std(From::from(fd))
    }
}
