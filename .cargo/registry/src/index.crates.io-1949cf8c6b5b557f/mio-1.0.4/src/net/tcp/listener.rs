use std::net::{self, SocketAddr};
#[cfg(any(unix, target_os = "wasi"))]
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
// TODO: once <https://github.com/rust-lang/rust/issues/126198> is fixed this
// can use `std::os::fd` and be merged with the above.
#[cfg(target_os = "hermit")]
use std::os::hermit::io::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
#[cfg(windows)]
use std::os::windows::io::{
    AsRawSocket, AsSocket, BorrowedSocket, FromRawSocket, IntoRawSocket, OwnedSocket, RawSocket,
};
use std::{fmt, io};

use crate::io_source::IoSource;
use crate::net::TcpStream;
#[cfg(any(unix, target_os = "hermit"))]
use crate::sys::tcp::set_reuseaddr;
#[cfg(not(target_os = "wasi"))]
use crate::sys::tcp::{bind, listen, new_for_addr};
use crate::{event, sys, Interest, Registry, Token};

/// A structure representing a socket server
///
/// # Examples
///
#[cfg_attr(feature = "os-poll", doc = "```")]
#[cfg_attr(not(feature = "os-poll"), doc = "```ignore")]
/// # use std::error::Error;
/// # fn main() -> Result<(), Box<dyn Error>> {
/// use mio::{Events, Interest, Poll, Token};
/// use mio::net::TcpListener;
/// use std::time::Duration;
///
/// let mut listener = TcpListener::bind("127.0.0.1:34255".parse()?)?;
///
/// let mut poll = Poll::new()?;
/// let mut events = Events::with_capacity(128);
///
/// // Register the socket with `Poll`
/// poll.registry().register(&mut listener, Token(0), Interest::READABLE)?;
///
/// poll.poll(&mut events, Some(Duration::from_millis(100)))?;
///
/// // There may be a socket ready to be accepted
/// #     Ok(())
/// # }
/// ```
pub struct TcpListener {
    inner: IoSource<net::TcpListener>,
}

impl TcpListener {
    /// Convenience method to bind a new TCP listener to the specified address
    /// to receive new connections.
    ///
    /// This function will take the following steps:
    ///
    /// 1. Create a new TCP socket.
    /// 2. Set the `SO_REUSEADDR` option on the socket on Unix.
    /// 3. Bind the socket to the specified address.
    /// 4. Calls `listen` on the socket to prepare it to receive new connections.
    #[cfg(not(target_os = "wasi"))]
    pub fn bind(addr: SocketAddr) -> io::Result<TcpListener> {
        let socket = new_for_addr(addr)?;
        #[cfg(any(unix, target_os = "hermit"))]
        let listener = unsafe { TcpListener::from_raw_fd(socket) };
        #[cfg(windows)]
        let listener = unsafe { TcpListener::from_raw_socket(socket as _) };

        // On platforms with Berkeley-derived sockets, this allows to quickly
        // rebind a socket, without needing to wait for the OS to clean up the
        // previous one.
        //
        // On Windows, this allows rebinding sockets which are actively in use,
        // which allows “socket hijacking”, so we explicitly don't set it here.
        // https://docs.microsoft.com/en-us/windows/win32/winsock/using-so-reuseaddr-and-so-exclusiveaddruse
        #[cfg(not(windows))]
        set_reuseaddr(&listener.inner, true)?;

        bind(&listener.inner, addr)?;
        listen(&listener.inner, 1024)?;
        Ok(listener)
    }

    /// Creates a new `TcpListener` from a standard `net::TcpListener`.
    ///
    /// This function is intended to be used to wrap a TCP listener from the
    /// standard library in the Mio equivalent. The conversion assumes nothing
    /// about the underlying listener; ; it is left up to the user to set it
    /// in non-blocking mode.
    pub fn from_std(listener: net::TcpListener) -> TcpListener {
        TcpListener {
            inner: IoSource::new(listener),
        }
    }

    /// Accepts a new `TcpStream`.
    ///
    /// This may return an `Err(e)` where `e.kind()` is
    /// `io::ErrorKind::WouldBlock`. This means a stream may be ready at a later
    /// point and one should wait for an event before calling `accept` again.
    ///
    /// If an accepted stream is returned, the remote address of the peer is
    /// returned along with it.
    pub fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        self.inner.do_io(|inner| {
            sys::tcp::accept(inner).map(|(stream, addr)| (TcpStream::from_std(stream), addr))
        })
    }

    /// Returns the local socket address of this listener.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    /// Sets the value for the `IP_TTL` option on this socket.
    ///
    /// This value sets the time-to-live field that is used in every packet sent
    /// from this socket.
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.inner.set_ttl(ttl)
    }

    /// Gets the value of the `IP_TTL` option for this socket.
    ///
    /// For more information about this option, see [`set_ttl`][link].
    ///
    /// [link]: #method.set_ttl
    pub fn ttl(&self) -> io::Result<u32> {
        self.inner.ttl()
    }

    /// Get the value of the `SO_ERROR` option on this socket.
    ///
    /// This will retrieve the stored error in the underlying socket, clearing
    /// the field in the process. This can be useful for checking errors between
    /// calls.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.inner.take_error()
    }
}

impl event::Source for TcpListener {
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

impl fmt::Debug for TcpListener {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

#[cfg(any(unix, target_os = "hermit", target_os = "wasi"))]
impl IntoRawFd for TcpListener {
    fn into_raw_fd(self) -> RawFd {
        self.inner.into_inner().into_raw_fd()
    }
}

#[cfg(any(unix, target_os = "hermit", target_os = "wasi"))]
impl AsRawFd for TcpListener {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

#[cfg(any(unix, target_os = "hermit", target_os = "wasi"))]
impl FromRawFd for TcpListener {
    /// Converts a `RawFd` to a `TcpListener`.
    ///
    /// # Notes
    ///
    /// The caller is responsible for ensuring that the socket is in
    /// non-blocking mode.
    unsafe fn from_raw_fd(fd: RawFd) -> TcpListener {
        TcpListener::from_std(FromRawFd::from_raw_fd(fd))
    }
}

#[cfg(any(unix, target_os = "hermit", target_os = "wasi"))]
impl From<TcpListener> for OwnedFd {
    fn from(tcp_listener: TcpListener) -> Self {
        tcp_listener.inner.into_inner().into()
    }
}

#[cfg(any(unix, target_os = "hermit", target_os = "wasi"))]
impl AsFd for TcpListener {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.inner.as_fd()
    }
}

#[cfg(any(unix, target_os = "hermit", target_os = "wasi"))]
impl From<OwnedFd> for TcpListener {
    /// Converts a `RawFd` to a `TcpListener`.
    ///
    /// # Notes
    ///
    /// The caller is responsible for ensuring that the socket is in
    /// non-blocking mode.
    fn from(fd: OwnedFd) -> Self {
        TcpListener::from_std(From::from(fd))
    }
}

#[cfg(windows)]
impl IntoRawSocket for TcpListener {
    fn into_raw_socket(self) -> RawSocket {
        self.inner.into_inner().into_raw_socket()
    }
}

#[cfg(windows)]
impl AsRawSocket for TcpListener {
    fn as_raw_socket(&self) -> RawSocket {
        self.inner.as_raw_socket()
    }
}

#[cfg(windows)]
impl FromRawSocket for TcpListener {
    /// Converts a `RawSocket` to a `TcpListener`.
    ///
    /// # Notes
    ///
    /// The caller is responsible for ensuring that the socket is in
    /// non-blocking mode.
    unsafe fn from_raw_socket(socket: RawSocket) -> TcpListener {
        TcpListener::from_std(FromRawSocket::from_raw_socket(socket))
    }
}

#[cfg(windows)]
impl From<TcpListener> for OwnedSocket {
    fn from(tcp_listener: TcpListener) -> Self {
        tcp_listener.inner.into_inner().into()
    }
}

#[cfg(windows)]
impl AsSocket for TcpListener {
    fn as_socket(&self) -> BorrowedSocket<'_> {
        self.inner.as_socket()
    }
}

#[cfg(windows)]
impl From<OwnedSocket> for TcpListener {
    /// Converts a `RawSocket` to a `TcpListener`.
    ///
    /// # Notes
    ///
    /// The caller is responsible for ensuring that the socket is in
    /// non-blocking mode.
    fn from(socket: OwnedSocket) -> Self {
        TcpListener::from_std(From::from(socket))
    }
}

impl From<TcpListener> for net::TcpListener {
    fn from(listener: TcpListener) -> Self {
        // Safety: This is safe since we are extracting the raw fd from a well-constructed
        // mio::net::TcpListener which ensures that we actually pass in a valid file
        // descriptor/socket
        unsafe {
            #[cfg(any(unix, target_os = "hermit", target_os = "wasi"))]
            {
                net::TcpListener::from_raw_fd(listener.into_raw_fd())
            }
            #[cfg(windows)]
            {
                net::TcpListener::from_raw_socket(listener.into_raw_socket())
            }
        }
    }
}
