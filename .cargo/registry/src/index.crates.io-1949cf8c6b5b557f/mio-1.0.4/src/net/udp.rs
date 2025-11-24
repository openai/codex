//! Primitives for working with UDP.
//!
//! The types provided in this module are non-blocking by default and are
//! designed to be portable across all supported Mio platforms. As long as the
//! [portability guidelines] are followed, the behavior should be identical no
//! matter the target platform.
//!
//! [portability guidelines]: ../struct.Poll.html#portability

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
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
use std::{fmt, io, net};

use crate::io_source::IoSource;
use crate::{event, sys, Interest, Registry, Token};

/// A User Datagram Protocol socket.
///
/// This is an implementation of a bound UDP socket. This supports both IPv4 and
/// IPv6 addresses, and there is no corresponding notion of a server because UDP
/// is a datagram protocol.
///
/// # Examples
///
#[cfg_attr(feature = "os-poll", doc = "```")]
#[cfg_attr(not(feature = "os-poll"), doc = "```ignore")]
/// # use std::error::Error;
/// #
/// # fn main() -> Result<(), Box<dyn Error>> {
/// // An Echo program:
/// // SENDER -> sends a message.
/// // ECHOER -> listens and prints the message received.
///
/// use mio::net::UdpSocket;
/// use mio::{Events, Interest, Poll, Token};
/// use std::time::Duration;
///
/// const SENDER: Token = Token(0);
/// const ECHOER: Token = Token(1);
///
/// // This operation will fail if the address is in use, so we select different ports for each
/// // socket.
/// let mut sender_socket = UdpSocket::bind("127.0.0.1:0".parse()?)?;
/// let mut echoer_socket = UdpSocket::bind("127.0.0.1:0".parse()?)?;
///
/// // If we do not use connect here, SENDER and ECHOER would need to call send_to and recv_from
/// // respectively.
/// sender_socket.connect(echoer_socket.local_addr()?)?;
///
/// // We need a Poll to check if SENDER is ready to be written into, and if ECHOER is ready to be
/// // read from.
/// let mut poll = Poll::new()?;
///
/// // We register our sockets here so that we can check if they are ready to be written/read.
/// poll.registry().register(&mut sender_socket, SENDER, Interest::WRITABLE)?;
/// poll.registry().register(&mut echoer_socket, ECHOER, Interest::READABLE)?;
///
/// let msg_to_send = [9; 9];
/// let mut buffer = [0; 9];
///
/// let mut events = Events::with_capacity(128);
/// loop {
///     poll.poll(&mut events, Some(Duration::from_millis(100)))?;
///     for event in events.iter() {
///         match event.token() {
///             // Our SENDER is ready to be written into.
///             SENDER => {
///                 let bytes_sent = sender_socket.send(&msg_to_send)?;
///                 assert_eq!(bytes_sent, 9);
///                 println!("sent {:?} -> {:?} bytes", msg_to_send, bytes_sent);
///             },
///             // Our ECHOER is ready to be read from.
///             ECHOER => {
///                 let num_recv = echoer_socket.recv(&mut buffer)?;
///                 println!("echo {:?} -> {:?}", buffer, num_recv);
///                 buffer = [0; 9];
///                 # _ = buffer; // Silence unused assignment warning.
///                 # return Ok(());
///             }
///             _ => unreachable!()
///         }
///     }
/// }
/// # }
/// ```
pub struct UdpSocket {
    inner: IoSource<net::UdpSocket>,
}

impl UdpSocket {
    /// Creates a UDP socket from the given address.
    ///
    /// # Examples
    ///
    #[cfg_attr(feature = "os-poll", doc = "```")]
    #[cfg_attr(not(feature = "os-poll"), doc = "```ignore")]
    /// # use std::error::Error;
    /// #
    /// # fn main() -> Result<(), Box<dyn Error>> {
    /// use mio::net::UdpSocket;
    ///
    /// // We must bind it to an open address.
    /// let socket = match UdpSocket::bind("127.0.0.1:0".parse()?) {
    ///     Ok(new_socket) => new_socket,
    ///     Err(fail) => {
    ///         // We panic! here, but you could try to bind it again on another address.
    ///         panic!("Failed to bind socket. {:?}", fail);
    ///     }
    /// };
    ///
    /// // Our socket was created, but we should not use it before checking it's readiness.
    /// #    drop(socket); // Silence unused variable warning.
    /// #    Ok(())
    /// # }
    /// ```
    pub fn bind(addr: SocketAddr) -> io::Result<UdpSocket> {
        sys::udp::bind(addr).map(UdpSocket::from_std)
    }

    /// Creates a new `UdpSocket` from a standard `net::UdpSocket`.
    ///
    /// This function is intended to be used to wrap a UDP socket from the
    /// standard library in the Mio equivalent. The conversion assumes nothing
    /// about the underlying socket; it is left up to the user to set it in
    /// non-blocking mode.
    pub fn from_std(socket: net::UdpSocket) -> UdpSocket {
        UdpSocket {
            inner: IoSource::new(socket),
        }
    }

    /// Returns the socket address that this socket was created from.
    ///
    /// # Examples
    ///
    // This assertion is almost, but not quite, universal.  It fails on
    // shared-IP FreeBSD jails.  It's hard for mio to know whether we're jailed,
    // so simply disable the test on FreeBSD.
    #[cfg_attr(all(feature = "os-poll", not(target_os = "freebsd")), doc = "```")]
    #[cfg_attr(
        any(not(feature = "os-poll"), target_os = "freebsd"),
        doc = "```ignore"
    )]
    /// # use std::error::Error;
    /// #
    /// # fn main() -> Result<(), Box<dyn Error>> {
    /// use mio::net::UdpSocket;
    ///
    /// let addr = "127.0.0.1:0".parse()?;
    /// let socket = UdpSocket::bind(addr)?;
    /// assert_eq!(socket.local_addr()?.ip(), addr.ip());
    /// #    Ok(())
    /// # }
    /// ```
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    /// Returns the socket address of the remote peer this socket was connected to.
    ///
    /// # Examples
    ///
    #[cfg_attr(feature = "os-poll", doc = "```")]
    #[cfg_attr(not(feature = "os-poll"), doc = "```ignore")]
    /// # use std::error::Error;
    /// #
    /// # fn main() -> Result<(), Box<dyn Error>> {
    /// use mio::net::UdpSocket;
    ///
    /// let addr = "127.0.0.1:0".parse()?;
    /// let peer_addr = "127.0.0.1:11100".parse()?;
    /// let socket = UdpSocket::bind(addr)?;
    /// socket.connect(peer_addr)?;
    /// assert_eq!(socket.peer_addr()?.ip(), peer_addr.ip());
    /// #    Ok(())
    /// # }
    /// ```
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.inner.peer_addr()
    }

    /// Sends data on the socket to the given address. On success, returns the
    /// number of bytes written.
    ///
    /// Address type can be any implementor of `ToSocketAddrs` trait. See its
    /// documentation for concrete examples.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use std::error::Error;
    /// # fn main() -> Result<(), Box<dyn Error>> {
    /// use mio::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:0".parse()?)?;
    ///
    /// // We must check if the socket is writable before calling send_to,
    /// // or we could run into a WouldBlock error.
    ///
    /// let bytes_sent = socket.send_to(&[9; 9], "127.0.0.1:11100".parse()?)?;
    /// assert_eq!(bytes_sent, 9);
    /// #
    /// #    Ok(())
    /// # }
    /// ```
    pub fn send_to(&self, buf: &[u8], target: SocketAddr) -> io::Result<usize> {
        self.inner.do_io(|inner| inner.send_to(buf, target))
    }

    /// Receives data from the socket. On success, returns the number of bytes
    /// read and the address from whence the data came.
    ///
    /// # Notes
    ///
    /// On Windows, if the data is larger than the buffer specified, the buffer
    /// is filled with the first part of the data, and recv_from returns the error
    /// WSAEMSGSIZE(10040). The excess data is lost.
    /// Make sure to always use a sufficiently large buffer to hold the
    /// maximum UDP packet size, which can be up to 65536 bytes in size.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use std::error::Error;
    /// #
    /// # fn main() -> Result<(), Box<dyn Error>> {
    /// use mio::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:0".parse()?)?;
    ///
    /// // We must check if the socket is readable before calling recv_from,
    /// // or we could run into a WouldBlock error.
    ///
    /// let mut buf = [0; 9];
    /// let (num_recv, from_addr) = socket.recv_from(&mut buf)?;
    /// println!("Received {:?} -> {:?} bytes from {:?}", buf, num_recv, from_addr);
    /// #
    /// #    Ok(())
    /// # }
    /// ```
    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.inner.do_io(|inner| inner.recv_from(buf))
    }

    /// Receives data from the socket, without removing it from the input queue.
    /// On success, returns the number of bytes read and the address from whence
    /// the data came.
    ///
    /// # Notes
    ///
    /// On Windows, if the data is larger than the buffer specified, the buffer
    /// is filled with the first part of the data, and peek_from returns the error
    /// WSAEMSGSIZE(10040). The excess data is lost.
    /// Make sure to always use a sufficiently large buffer to hold the
    /// maximum UDP packet size, which can be up to 65536 bytes in size.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use std::error::Error;
    /// #
    /// # fn main() -> Result<(), Box<dyn Error>> {
    /// use mio::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:0".parse()?)?;
    ///
    /// // We must check if the socket is readable before calling recv_from,
    /// // or we could run into a WouldBlock error.
    ///
    /// let mut buf = [0; 9];
    /// let (num_recv, from_addr) = socket.peek_from(&mut buf)?;
    /// println!("Received {:?} -> {:?} bytes from {:?}", buf, num_recv, from_addr);
    /// #
    /// #    Ok(())
    /// # }
    /// ```
    pub fn peek_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.inner.do_io(|inner| inner.peek_from(buf))
    }

    /// Sends data on the socket to the address previously bound via connect(). On success,
    /// returns the number of bytes written.
    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.inner.do_io(|inner| inner.send(buf))
    }

    /// Receives data from the socket previously bound with connect(). On success, returns
    /// the number of bytes read.
    ///
    /// # Notes
    ///
    /// On Windows, if the data is larger than the buffer specified, the buffer
    /// is filled with the first part of the data, and recv returns the error
    /// WSAEMSGSIZE(10040). The excess data is lost.
    /// Make sure to always use a sufficiently large buffer to hold the
    /// maximum UDP packet size, which can be up to 65536 bytes in size.
    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.do_io(|inner| inner.recv(buf))
    }

    /// Receives data from the socket, without removing it from the input queue.
    /// On success, returns the number of bytes read.
    ///
    /// # Notes
    ///
    /// On Windows, if the data is larger than the buffer specified, the buffer
    /// is filled with the first part of the data, and peek returns the error
    /// WSAEMSGSIZE(10040). The excess data is lost.
    /// Make sure to always use a sufficiently large buffer to hold the
    /// maximum UDP packet size, which can be up to 65536 bytes in size.
    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.do_io(|inner| inner.peek(buf))
    }

    /// Connects the UDP socket setting the default destination for `send()`
    /// and limiting packets that are read via `recv` from the address specified
    /// in `addr`.
    ///
    /// This may return a `WouldBlock` in which case the socket connection
    /// cannot be completed immediately, it usually means there are insufficient
    /// entries in the routing cache.
    pub fn connect(&self, addr: SocketAddr) -> io::Result<()> {
        self.inner.connect(addr)
    }

    /// Sets the value of the `SO_BROADCAST` option for this socket.
    ///
    /// When enabled, this socket is allowed to send packets to a broadcast
    /// address.
    ///
    /// # Examples
    ///
    #[cfg_attr(feature = "os-poll", doc = "```")]
    #[cfg_attr(not(feature = "os-poll"), doc = "```ignore")]
    /// # use std::error::Error;
    /// #
    /// # fn main() -> Result<(), Box<dyn Error>> {
    /// use mio::net::UdpSocket;
    ///
    /// let broadcast_socket = UdpSocket::bind("127.0.0.1:0".parse()?)?;
    /// if broadcast_socket.broadcast()? == false {
    ///     broadcast_socket.set_broadcast(true)?;
    /// }
    ///
    /// assert_eq!(broadcast_socket.broadcast()?, true);
    /// #
    /// #    Ok(())
    /// # }
    /// ```
    pub fn set_broadcast(&self, on: bool) -> io::Result<()> {
        self.inner.set_broadcast(on)
    }

    /// Gets the value of the `SO_BROADCAST` option for this socket.
    ///
    /// For more information about this option, see
    /// [`set_broadcast`][link].
    ///
    /// [link]: #method.set_broadcast
    ///
    /// # Examples
    ///
    #[cfg_attr(feature = "os-poll", doc = "```")]
    #[cfg_attr(not(feature = "os-poll"), doc = "```ignore")]
    /// # use std::error::Error;
    /// #
    /// # fn main() -> Result<(), Box<dyn Error>> {
    /// use mio::net::UdpSocket;
    ///
    /// let broadcast_socket = UdpSocket::bind("127.0.0.1:0".parse()?)?;
    /// assert_eq!(broadcast_socket.broadcast()?, false);
    /// #
    /// #    Ok(())
    /// # }
    /// ```
    pub fn broadcast(&self) -> io::Result<bool> {
        self.inner.broadcast()
    }

    /// Sets the value of the `IP_MULTICAST_LOOP` option for this socket.
    ///
    /// If enabled, multicast packets will be looped back to the local socket.
    /// Note that this may not have any affect on IPv6 sockets.
    pub fn set_multicast_loop_v4(&self, on: bool) -> io::Result<()> {
        self.inner.set_multicast_loop_v4(on)
    }

    /// Gets the value of the `IP_MULTICAST_LOOP` option for this socket.
    ///
    /// For more information about this option, see
    /// [`set_multicast_loop_v4`][link].
    ///
    /// [link]: #method.set_multicast_loop_v4
    pub fn multicast_loop_v4(&self) -> io::Result<bool> {
        self.inner.multicast_loop_v4()
    }

    /// Sets the value of the `IP_MULTICAST_TTL` option for this socket.
    ///
    /// Indicates the time-to-live value of outgoing multicast packets for
    /// this socket. The default value is 1 which means that multicast packets
    /// don't leave the local network unless explicitly requested.
    ///
    /// Note that this may not have any affect on IPv6 sockets.
    pub fn set_multicast_ttl_v4(&self, ttl: u32) -> io::Result<()> {
        self.inner.set_multicast_ttl_v4(ttl)
    }

    /// Gets the value of the `IP_MULTICAST_TTL` option for this socket.
    ///
    /// For more information about this option, see
    /// [`set_multicast_ttl_v4`][link].
    ///
    /// [link]: #method.set_multicast_ttl_v4
    pub fn multicast_ttl_v4(&self) -> io::Result<u32> {
        self.inner.multicast_ttl_v4()
    }

    /// Sets the value of the `IPV6_MULTICAST_LOOP` option for this socket.
    ///
    /// Controls whether this socket sees the multicast packets it sends itself.
    /// Note that this may not have any affect on IPv4 sockets.
    pub fn set_multicast_loop_v6(&self, on: bool) -> io::Result<()> {
        self.inner.set_multicast_loop_v6(on)
    }

    /// Gets the value of the `IPV6_MULTICAST_LOOP` option for this socket.
    ///
    /// For more information about this option, see
    /// [`set_multicast_loop_v6`][link].
    ///
    /// [link]: #method.set_multicast_loop_v6
    pub fn multicast_loop_v6(&self) -> io::Result<bool> {
        self.inner.multicast_loop_v6()
    }

    /// Sets the value for the `IP_TTL` option on this socket.
    ///
    /// This value sets the time-to-live field that is used in every packet sent
    /// from this socket.
    ///
    /// # Examples
    ///
    #[cfg_attr(feature = "os-poll", doc = "```")]
    #[cfg_attr(not(feature = "os-poll"), doc = "```ignore")]
    /// # use std::error::Error;
    /// #
    /// # fn main() -> Result<(), Box<dyn Error>> {
    /// use mio::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:0".parse()?)?;
    /// if socket.ttl()? < 255 {
    ///     socket.set_ttl(255)?;
    /// }
    ///
    /// assert_eq!(socket.ttl()?, 255);
    /// #
    /// #    Ok(())
    /// # }
    /// ```
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.inner.set_ttl(ttl)
    }

    /// Gets the value of the `IP_TTL` option for this socket.
    ///
    /// For more information about this option, see [`set_ttl`][link].
    ///
    /// [link]: #method.set_ttl
    ///
    /// # Examples
    ///
    #[cfg_attr(feature = "os-poll", doc = "```")]
    #[cfg_attr(not(feature = "os-poll"), doc = "```ignore")]
    /// # use std::error::Error;
    /// #
    /// # fn main() -> Result<(), Box<dyn Error>> {
    /// use mio::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:0".parse()?)?;
    /// socket.set_ttl(255)?;
    ///
    /// assert_eq!(socket.ttl()?, 255);
    /// #
    /// #    Ok(())
    /// # }
    /// ```
    pub fn ttl(&self) -> io::Result<u32> {
        self.inner.ttl()
    }

    /// Executes an operation of the `IP_ADD_MEMBERSHIP` type.
    ///
    /// This function specifies a new multicast group for this socket to join.
    /// The address must be a valid multicast address, and `interface` is the
    /// address of the local interface with which the system should join the
    /// multicast group. If it's equal to `INADDR_ANY` then an appropriate
    /// interface is chosen by the system.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn join_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> io::Result<()> {
        self.inner.join_multicast_v4(multiaddr, interface)
    }

    /// Executes an operation of the `IPV6_ADD_MEMBERSHIP` type.
    ///
    /// This function specifies a new multicast group for this socket to join.
    /// The address must be a valid multicast address, and `interface` is the
    /// index of the interface to join/leave (or 0 to indicate any interface).
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn join_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32) -> io::Result<()> {
        self.inner.join_multicast_v6(multiaddr, interface)
    }

    /// Executes an operation of the `IP_DROP_MEMBERSHIP` type.
    ///
    /// For more information about this option, see
    /// [`join_multicast_v4`][link].
    ///
    /// [link]: #method.join_multicast_v4
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn leave_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> io::Result<()> {
        self.inner.leave_multicast_v4(multiaddr, interface)
    }

    /// Executes an operation of the `IPV6_DROP_MEMBERSHIP` type.
    ///
    /// For more information about this option, see
    /// [`join_multicast_v6`][link].
    ///
    /// [link]: #method.join_multicast_v6
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn leave_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32) -> io::Result<()> {
        self.inner.leave_multicast_v6(multiaddr, interface)
    }

    /// Get the value of the `IPV6_V6ONLY` option on this socket.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn only_v6(&self) -> io::Result<bool> {
        sys::udp::only_v6(&self.inner)
    }

    /// Get the value of the `SO_ERROR` option on this socket.
    ///
    /// This will retrieve the stored error in the underlying socket, clearing
    /// the field in the process. This can be useful for checking errors between
    /// calls.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.inner.take_error()
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
    #[cfg_attr(unix, doc = "```no_run")]
    #[cfg_attr(windows, doc = "```ignore")]
    /// # use std::error::Error;
    /// #
    /// # fn main() -> Result<(), Box<dyn Error>> {
    /// use std::io;
    /// #[cfg(any(unix, target_os = "wasi"))]
    /// use std::os::fd::AsRawFd;
    /// #[cfg(windows)]
    /// use std::os::windows::io::AsRawSocket;
    /// use mio::net::UdpSocket;
    ///
    /// let address = "127.0.0.1:8080".parse().unwrap();
    /// let dgram = UdpSocket::bind(address)?;
    ///
    /// // Wait until the dgram is readable...
    ///
    /// // Read from the dgram using a direct libc call, of course the
    /// // `io::Read` implementation would be easier to use.
    /// let mut buf = [0; 512];
    /// let n = dgram.try_io(|| {
    ///     let buf_ptr = &mut buf as *mut _ as *mut _;
    ///     #[cfg(unix)]
    ///     let res = unsafe { libc::recv(dgram.as_raw_fd(), buf_ptr, buf.len(), 0) };
    ///     #[cfg(windows)]
    ///     let res = unsafe { libc::recvfrom(dgram.as_raw_socket() as usize, buf_ptr, buf.len() as i32, 0, std::ptr::null_mut(), std::ptr::null_mut()) };
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

impl event::Source for UdpSocket {
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

impl fmt::Debug for UdpSocket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

#[cfg(any(unix, target_os = "hermit", target_os = "wasi"))]
impl IntoRawFd for UdpSocket {
    fn into_raw_fd(self) -> RawFd {
        self.inner.into_inner().into_raw_fd()
    }
}

#[cfg(any(unix, target_os = "hermit", target_os = "wasi"))]
impl AsRawFd for UdpSocket {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

#[cfg(any(unix, target_os = "hermit", target_os = "wasi"))]
impl FromRawFd for UdpSocket {
    /// Converts a `RawFd` to a `UdpSocket`.
    ///
    /// # Notes
    ///
    /// The caller is responsible for ensuring that the socket is in
    /// non-blocking mode.
    unsafe fn from_raw_fd(fd: RawFd) -> UdpSocket {
        UdpSocket::from_std(FromRawFd::from_raw_fd(fd))
    }
}

#[cfg(any(unix, target_os = "hermit", target_os = "wasi"))]
impl From<UdpSocket> for OwnedFd {
    fn from(udp_socket: UdpSocket) -> Self {
        udp_socket.inner.into_inner().into()
    }
}

#[cfg(any(unix, target_os = "hermit", target_os = "wasi"))]
impl AsFd for UdpSocket {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.inner.as_fd()
    }
}

#[cfg(any(unix, target_os = "hermit", target_os = "wasi"))]
impl From<OwnedFd> for UdpSocket {
    /// Converts a `RawFd` to a `UdpSocket`.
    ///
    /// # Notes
    ///
    /// The caller is responsible for ensuring that the socket is in
    /// non-blocking mode.
    fn from(fd: OwnedFd) -> Self {
        UdpSocket::from_std(From::from(fd))
    }
}

#[cfg(windows)]
impl IntoRawSocket for UdpSocket {
    fn into_raw_socket(self) -> RawSocket {
        self.inner.into_inner().into_raw_socket()
    }
}

#[cfg(windows)]
impl AsRawSocket for UdpSocket {
    fn as_raw_socket(&self) -> RawSocket {
        self.inner.as_raw_socket()
    }
}

#[cfg(windows)]
impl FromRawSocket for UdpSocket {
    /// Converts a `RawSocket` to a `UdpSocket`.
    ///
    /// # Notes
    ///
    /// The caller is responsible for ensuring that the socket is in
    /// non-blocking mode.
    unsafe fn from_raw_socket(socket: RawSocket) -> UdpSocket {
        UdpSocket::from_std(FromRawSocket::from_raw_socket(socket))
    }
}

#[cfg(windows)]
impl From<UdpSocket> for OwnedSocket {
    fn from(udp_socket: UdpSocket) -> Self {
        udp_socket.inner.into_inner().into()
    }
}

#[cfg(windows)]
impl AsSocket for UdpSocket {
    fn as_socket(&self) -> BorrowedSocket<'_> {
        self.inner.as_socket()
    }
}

#[cfg(windows)]
impl From<OwnedSocket> for UdpSocket {
    /// Converts a `RawSocket` to a `UdpSocket`.
    ///
    /// # Notes
    ///
    /// The caller is responsible for ensuring that the socket is in
    /// non-blocking mode.
    fn from(socket: OwnedSocket) -> Self {
        UdpSocket::from_std(From::from(socket))
    }
}

impl From<UdpSocket> for net::UdpSocket {
    fn from(socket: UdpSocket) -> Self {
        // Safety: This is safe since we are extracting the raw fd from a well-constructed
        // mio::net::UdpSocket which ensures that we actually pass in a valid file
        // descriptor/socket
        unsafe {
            #[cfg(any(unix, target_os = "hermit", target_os = "wasi"))]
            {
                net::UdpSocket::from_raw_fd(socket.into_raw_fd())
            }
            #[cfg(windows)]
            {
                net::UdpSocket::from_raw_socket(socket.into_raw_socket())
            }
        }
    }
}
