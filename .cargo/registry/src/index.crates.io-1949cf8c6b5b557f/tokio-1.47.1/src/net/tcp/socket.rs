use crate::net::{TcpListener, TcpStream};

use std::fmt;
use std::io;
use std::net::SocketAddr;

#[cfg(unix)]
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, RawFd};
use std::time::Duration;

cfg_windows! {
    use crate::os::windows::io::{AsRawSocket, FromRawSocket, IntoRawSocket, RawSocket, AsSocket, BorrowedSocket};
}

cfg_net! {
    /// A TCP socket that has not yet been converted to a `TcpStream` or
    /// `TcpListener`.
    ///
    /// `TcpSocket` wraps an operating system socket and enables the caller to
    /// configure the socket before establishing a TCP connection or accepting
    /// inbound connections. The caller is able to set socket option and explicitly
    /// bind the socket with a socket address.
    ///
    /// The underlying socket is closed when the `TcpSocket` value is dropped.
    ///
    /// `TcpSocket` should only be used directly if the default configuration used
    /// by `TcpStream::connect` and `TcpListener::bind` does not meet the required
    /// use case.
    ///
    /// Calling `TcpStream::connect("127.0.0.1:8080")` is equivalent to:
    ///
    /// ```no_run
    /// use tokio::net::TcpSocket;
    ///
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     let addr = "127.0.0.1:8080".parse().unwrap();
    ///
    ///     let socket = TcpSocket::new_v4()?;
    ///     let stream = socket.connect(addr).await?;
    /// # drop(stream);
    ///
    ///     Ok(())
    /// }
    /// ```
    ///
    /// Calling `TcpListener::bind("127.0.0.1:8080")` is equivalent to:
    ///
    /// ```no_run
    /// use tokio::net::TcpSocket;
    ///
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     let addr = "127.0.0.1:8080".parse().unwrap();
    ///
    ///     let socket = TcpSocket::new_v4()?;
    ///     // On platforms with Berkeley-derived sockets, this allows to quickly
    ///     // rebind a socket, without needing to wait for the OS to clean up the
    ///     // previous one.
    ///     //
    ///     // On Windows, this allows rebinding sockets which are actively in use,
    ///     // which allows "socket hijacking", so we explicitly don't set it here.
    ///     // https://docs.microsoft.com/en-us/windows/win32/winsock/using-so-reuseaddr-and-so-exclusiveaddruse
    ///     socket.set_reuseaddr(true)?;
    ///     socket.bind(addr)?;
    ///
    ///     let listener = socket.listen(1024)?;
    /// # drop(listener);
    ///
    ///     Ok(())
    /// }
    /// ```
    ///
    /// Setting socket options not explicitly provided by `TcpSocket` may be done by
    /// accessing the `RawFd`/`RawSocket` using [`AsRawFd`]/[`AsRawSocket`] and
    /// setting the option with a crate like [`socket2`].
    ///
    /// [`RawFd`]: https://doc.rust-lang.org/std/os/fd/type.RawFd.html
    /// [`RawSocket`]: https://doc.rust-lang.org/std/os/windows/io/type.RawSocket.html
    /// [`AsRawFd`]: https://doc.rust-lang.org/std/os/fd/trait.AsRawFd.html
    /// [`AsRawSocket`]: https://doc.rust-lang.org/std/os/windows/io/trait.AsRawSocket.html
    /// [`socket2`]: https://docs.rs/socket2/
    #[cfg_attr(docsrs, doc(alias = "connect_std"))]
    pub struct TcpSocket {
        inner: socket2::Socket,
    }
}

impl TcpSocket {
    /// Creates a new socket configured for IPv4.
    ///
    /// Calls `socket(2)` with `AF_INET` and `SOCK_STREAM`.
    ///
    /// # Returns
    ///
    /// On success, the newly created `TcpSocket` is returned. If an error is
    /// encountered, it is returned instead.
    ///
    /// # Examples
    ///
    /// Create a new IPv4 socket and start listening.
    ///
    /// ```no_run
    /// use tokio::net::TcpSocket;
    ///
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     let addr = "127.0.0.1:8080".parse().unwrap();
    ///     let socket = TcpSocket::new_v4()?;
    ///     socket.bind(addr)?;
    ///
    ///     let listener = socket.listen(128)?;
    /// # drop(listener);
    ///     Ok(())
    /// }
    /// ```
    pub fn new_v4() -> io::Result<TcpSocket> {
        TcpSocket::new(socket2::Domain::IPV4)
    }

    /// Creates a new socket configured for IPv6.
    ///
    /// Calls `socket(2)` with `AF_INET6` and `SOCK_STREAM`.
    ///
    /// # Returns
    ///
    /// On success, the newly created `TcpSocket` is returned. If an error is
    /// encountered, it is returned instead.
    ///
    /// # Examples
    ///
    /// Create a new IPv6 socket and start listening.
    ///
    /// ```no_run
    /// use tokio::net::TcpSocket;
    ///
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     let addr = "[::1]:8080".parse().unwrap();
    ///     let socket = TcpSocket::new_v6()?;
    ///     socket.bind(addr)?;
    ///
    ///     let listener = socket.listen(128)?;
    /// # drop(listener);
    ///     Ok(())
    /// }
    /// ```
    pub fn new_v6() -> io::Result<TcpSocket> {
        TcpSocket::new(socket2::Domain::IPV6)
    }

    fn new(domain: socket2::Domain) -> io::Result<TcpSocket> {
        let ty = socket2::Type::STREAM;
        #[cfg(any(
            target_os = "android",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "fuchsia",
            target_os = "illumos",
            target_os = "linux",
            target_os = "netbsd",
            target_os = "openbsd"
        ))]
        let ty = ty.nonblocking();
        let inner = socket2::Socket::new(domain, ty, Some(socket2::Protocol::TCP))?;
        #[cfg(not(any(
            target_os = "android",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "fuchsia",
            target_os = "illumos",
            target_os = "linux",
            target_os = "netbsd",
            target_os = "openbsd"
        )))]
        inner.set_nonblocking(true)?;
        Ok(TcpSocket { inner })
    }

    /// Sets value for the `SO_KEEPALIVE` option on this socket.
    pub fn set_keepalive(&self, keepalive: bool) -> io::Result<()> {
        self.inner.set_keepalive(keepalive)
    }

    /// Gets the value of the `SO_KEEPALIVE` option on this socket.
    pub fn keepalive(&self) -> io::Result<bool> {
        self.inner.keepalive()
    }

    /// Allows the socket to bind to an in-use address.
    ///
    /// Behavior is platform specific. Refer to the target platform's
    /// documentation for more details.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::TcpSocket;
    ///
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     let addr = "127.0.0.1:8080".parse().unwrap();
    ///
    ///     let socket = TcpSocket::new_v4()?;
    ///     socket.set_reuseaddr(true)?;
    ///     socket.bind(addr)?;
    ///
    ///     let listener = socket.listen(1024)?;
    /// # drop(listener);
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn set_reuseaddr(&self, reuseaddr: bool) -> io::Result<()> {
        self.inner.set_reuse_address(reuseaddr)
    }

    /// Retrieves the value set for `SO_REUSEADDR` on this socket.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::TcpSocket;
    ///
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     let addr = "127.0.0.1:8080".parse().unwrap();
    ///
    ///     let socket = TcpSocket::new_v4()?;
    ///     socket.set_reuseaddr(true)?;
    ///     assert!(socket.reuseaddr().unwrap());
    ///     socket.bind(addr)?;
    ///
    ///     let listener = socket.listen(1024)?;
    ///     Ok(())
    /// }
    /// ```
    pub fn reuseaddr(&self) -> io::Result<bool> {
        self.inner.reuse_address()
    }

    /// Allows the socket to bind to an in-use port. Only available for unix systems
    /// (excluding Solaris & Illumos).
    ///
    /// Behavior is platform specific. Refer to the target platform's
    /// documentation for more details.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::TcpSocket;
    ///
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     let addr = "127.0.0.1:8080".parse().unwrap();
    ///
    ///     let socket = TcpSocket::new_v4()?;
    ///     socket.set_reuseport(true)?;
    ///     socket.bind(addr)?;
    ///
    ///     let listener = socket.listen(1024)?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(all(
        unix,
        not(target_os = "solaris"),
        not(target_os = "illumos"),
        not(target_os = "cygwin"),
    ))]
    #[cfg_attr(
        docsrs,
        doc(cfg(all(
            unix,
            not(target_os = "solaris"),
            not(target_os = "illumos"),
            not(target_os = "cygwin"),
        )))
    )]
    pub fn set_reuseport(&self, reuseport: bool) -> io::Result<()> {
        self.inner.set_reuse_port(reuseport)
    }

    /// Allows the socket to bind to an in-use port. Only available for unix systems
    /// (excluding Solaris & Illumos).
    ///
    /// Behavior is platform specific. Refer to the target platform's
    /// documentation for more details.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::TcpSocket;
    ///
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     let addr = "127.0.0.1:8080".parse().unwrap();
    ///
    ///     let socket = TcpSocket::new_v4()?;
    ///     socket.set_reuseport(true)?;
    ///     assert!(socket.reuseport().unwrap());
    ///     socket.bind(addr)?;
    ///
    ///     let listener = socket.listen(1024)?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(all(
        unix,
        not(target_os = "solaris"),
        not(target_os = "illumos"),
        not(target_os = "cygwin"),
    ))]
    #[cfg_attr(
        docsrs,
        doc(cfg(all(
            unix,
            not(target_os = "solaris"),
            not(target_os = "illumos"),
            not(target_os = "cygwin"),
        )))
    )]
    pub fn reuseport(&self) -> io::Result<bool> {
        self.inner.reuse_port()
    }

    /// Sets the size of the TCP send buffer on this socket.
    ///
    /// On most operating systems, this sets the `SO_SNDBUF` socket option.
    pub fn set_send_buffer_size(&self, size: u32) -> io::Result<()> {
        self.inner.set_send_buffer_size(size as usize)
    }

    /// Returns the size of the TCP send buffer for this socket.
    ///
    /// On most operating systems, this is the value of the `SO_SNDBUF` socket
    /// option.
    ///
    /// Note that if [`set_send_buffer_size`] has been called on this socket
    /// previously, the value returned by this function may not be the same as
    /// the argument provided to `set_send_buffer_size`. This is for the
    /// following reasons:
    ///
    /// * Most operating systems have minimum and maximum allowed sizes for the
    ///   send buffer, and will clamp the provided value if it is below the
    ///   minimum or above the maximum. The minimum and maximum buffer sizes are
    ///   OS-dependent.
    /// * Linux will double the buffer size to account for internal bookkeeping
    ///   data, and returns the doubled value from `getsockopt(2)`. As per `man
    ///   7 socket`:
    ///   > Sets or gets the maximum socket send buffer in bytes. The
    ///   > kernel doubles this value (to allow space for bookkeeping
    ///   > overhead) when it is set using `setsockopt(2)`, and this doubled
    ///   > value is returned by `getsockopt(2)`.
    ///
    /// [`set_send_buffer_size`]: #method.set_send_buffer_size
    pub fn send_buffer_size(&self) -> io::Result<u32> {
        self.inner.send_buffer_size().map(|n| n as u32)
    }

    /// Sets the size of the TCP receive buffer on this socket.
    ///
    /// On most operating systems, this sets the `SO_RCVBUF` socket option.
    pub fn set_recv_buffer_size(&self, size: u32) -> io::Result<()> {
        self.inner.set_recv_buffer_size(size as usize)
    }

    /// Returns the size of the TCP receive buffer for this socket.
    ///
    /// On most operating systems, this is the value of the `SO_RCVBUF` socket
    /// option.
    ///
    /// Note that if [`set_recv_buffer_size`] has been called on this socket
    /// previously, the value returned by this function may not be the same as
    /// the argument provided to `set_recv_buffer_size`. This is for the
    /// following reasons:
    ///
    /// * Most operating systems have minimum and maximum allowed sizes for the
    ///   receive buffer, and will clamp the provided value if it is below the
    ///   minimum or above the maximum. The minimum and maximum buffer sizes are
    ///   OS-dependent.
    /// * Linux will double the buffer size to account for internal bookkeeping
    ///   data, and returns the doubled value from `getsockopt(2)`. As per `man
    ///   7 socket`:
    ///   > Sets or gets the maximum socket send buffer in bytes. The
    ///   > kernel doubles this value (to allow space for bookkeeping
    ///   > overhead) when it is set using `setsockopt(2)`, and this doubled
    ///   > value is returned by `getsockopt(2)`.
    ///
    /// [`set_recv_buffer_size`]: #method.set_recv_buffer_size
    pub fn recv_buffer_size(&self) -> io::Result<u32> {
        self.inner.recv_buffer_size().map(|n| n as u32)
    }

    /// Sets the linger duration of this socket by setting the `SO_LINGER` option.
    ///
    /// This option controls the action taken when a stream has unsent messages and the stream is
    /// closed. If `SO_LINGER` is set, the system shall block the process until it can transmit the
    /// data or until the time expires.
    ///
    /// If `SO_LINGER` is not specified, and the socket is closed, the system handles the call in a
    /// way that allows the process to continue as quickly as possible.
    pub fn set_linger(&self, dur: Option<Duration>) -> io::Result<()> {
        self.inner.set_linger(dur)
    }

    /// Reads the linger duration for this socket by getting the `SO_LINGER`
    /// option.
    ///
    /// For more information about this option, see [`set_linger`].
    ///
    /// [`set_linger`]: TcpSocket::set_linger
    pub fn linger(&self) -> io::Result<Option<Duration>> {
        self.inner.linger()
    }

    /// Sets the value of the `TCP_NODELAY` option on this socket.
    ///
    /// If set, this option disables the Nagle algorithm. This means that segments are always
    /// sent as soon as possible, even if there is only a small amount of data. When not set,
    /// data is buffered until there is a sufficient amount to send out, thereby avoiding
    /// the frequent sending of small packets.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::TcpSocket;
    ///
    /// # async fn dox() -> Result<(), Box<dyn std::error::Error>> {
    /// let socket = TcpSocket::new_v4()?;
    ///
    /// socket.set_nodelay(true)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.inner.set_tcp_nodelay(nodelay)
    }

    /// Gets the value of the `TCP_NODELAY` option on this socket.
    ///
    /// For more information about this option, see [`set_nodelay`].
    ///
    /// [`set_nodelay`]: TcpSocket::set_nodelay
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::TcpSocket;
    ///
    /// # async fn dox() -> Result<(), Box<dyn std::error::Error>> {
    /// let socket = TcpSocket::new_v4()?;
    ///
    /// println!("{:?}", socket.nodelay()?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn nodelay(&self) -> io::Result<bool> {
        self.inner.tcp_nodelay()
    }

    /// Gets the value of the `IP_TOS` option for this socket.
    ///
    /// For more information about this option, see [`set_tos`].
    ///
    /// **NOTE:** On Windows, `IP_TOS` is only supported on [Windows 8+ or
    /// Windows Server 2012+.](https://docs.microsoft.com/en-us/windows/win32/winsock/ipproto-ip-socket-options)
    ///
    /// [`set_tos`]: Self::set_tos
    // https://docs.rs/socket2/0.5.3/src/socket2/socket.rs.html#1464
    #[cfg(not(any(
        target_os = "fuchsia",
        target_os = "redox",
        target_os = "solaris",
        target_os = "illumos",
        target_os = "haiku"
    )))]
    #[cfg_attr(
        docsrs,
        doc(cfg(not(any(
            target_os = "fuchsia",
            target_os = "redox",
            target_os = "solaris",
            target_os = "illumos",
            target_os = "haiku"
        ))))
    )]
    pub fn tos(&self) -> io::Result<u32> {
        self.inner.tos_v4()
    }

    /// Sets the value for the `IP_TOS` option on this socket.
    ///
    /// This value sets the type-of-service field that is used in every packet
    /// sent from this socket.
    ///
    /// **NOTE:** On Windows, `IP_TOS` is only supported on [Windows 8+ or
    /// Windows Server 2012+.](https://docs.microsoft.com/en-us/windows/win32/winsock/ipproto-ip-socket-options)
    // https://docs.rs/socket2/0.5.3/src/socket2/socket.rs.html#1446
    #[cfg(not(any(
        target_os = "fuchsia",
        target_os = "redox",
        target_os = "solaris",
        target_os = "illumos",
        target_os = "haiku"
    )))]
    #[cfg_attr(
        docsrs,
        doc(cfg(not(any(
            target_os = "fuchsia",
            target_os = "redox",
            target_os = "solaris",
            target_os = "illumos",
            target_os = "haiku"
        ))))
    )]
    pub fn set_tos(&self, tos: u32) -> io::Result<()> {
        self.inner.set_tos_v4(tos)
    }

    /// Gets the value for the `SO_BINDTODEVICE` option on this socket
    ///
    /// This value gets the socket binded device's interface name.
    #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux",))]
    #[cfg_attr(
        docsrs,
        doc(cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux",)))
    )]
    pub fn device(&self) -> io::Result<Option<Vec<u8>>> {
        self.inner.device()
    }

    /// Sets the value for the `SO_BINDTODEVICE` option on this socket
    ///
    /// If a socket is bound to an interface, only packets received from that
    /// particular interface are processed by the socket. Note that this only
    /// works for some socket types, particularly `AF_INET` sockets.
    ///
    /// If `interface` is `None` or an empty string it removes the binding.
    #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
    #[cfg_attr(
        docsrs,
        doc(cfg(all(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))))
    )]
    pub fn bind_device(&self, interface: Option<&[u8]>) -> io::Result<()> {
        self.inner.bind_device(interface)
    }

    /// Gets the local address of this socket.
    ///
    /// Will fail on windows if called before `bind`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::TcpSocket;
    ///
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     let addr = "127.0.0.1:8080".parse().unwrap();
    ///
    ///     let socket = TcpSocket::new_v4()?;
    ///     socket.bind(addr)?;
    ///     assert_eq!(socket.local_addr().unwrap().to_string(), "127.0.0.1:8080");
    ///     let listener = socket.listen(1024)?;
    ///     Ok(())
    /// }
    /// ```
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr().and_then(convert_address)
    }

    /// Returns the value of the `SO_ERROR` option.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.inner.take_error()
    }

    /// Binds the socket to the given address.
    ///
    /// This calls the `bind(2)` operating-system function. Behavior is
    /// platform specific. Refer to the target platform's documentation for more
    /// details.
    ///
    /// # Examples
    ///
    /// Bind a socket before listening.
    ///
    /// ```no_run
    /// use tokio::net::TcpSocket;
    ///
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     let addr = "127.0.0.1:8080".parse().unwrap();
    ///
    ///     let socket = TcpSocket::new_v4()?;
    ///     socket.bind(addr)?;
    ///
    ///     let listener = socket.listen(1024)?;
    /// # drop(listener);
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn bind(&self, addr: SocketAddr) -> io::Result<()> {
        self.inner.bind(&addr.into())
    }

    /// Establishes a TCP connection with a peer at the specified socket address.
    ///
    /// The `TcpSocket` is consumed. Once the connection is established, a
    /// connected [`TcpStream`] is returned. If the connection fails, the
    /// encountered error is returned.
    ///
    /// [`TcpStream`]: TcpStream
    ///
    /// This calls the `connect(2)` operating-system function. Behavior is
    /// platform specific. Refer to the target platform's documentation for more
    /// details.
    ///
    /// # Examples
    ///
    /// Connecting to a peer.
    ///
    /// ```no_run
    /// use tokio::net::TcpSocket;
    ///
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     let addr = "127.0.0.1:8080".parse().unwrap();
    ///
    ///     let socket = TcpSocket::new_v4()?;
    ///     let stream = socket.connect(addr).await?;
    /// # drop(stream);
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn connect(self, addr: SocketAddr) -> io::Result<TcpStream> {
        if let Err(err) = self.inner.connect(&addr.into()) {
            #[cfg(unix)]
            if err.raw_os_error() != Some(libc::EINPROGRESS) {
                return Err(err);
            }
            #[cfg(windows)]
            if err.kind() != io::ErrorKind::WouldBlock {
                return Err(err);
            }
        }
        #[cfg(unix)]
        let mio = {
            use std::os::unix::io::{FromRawFd, IntoRawFd};

            let raw_fd = self.inner.into_raw_fd();
            unsafe { mio::net::TcpStream::from_raw_fd(raw_fd) }
        };

        #[cfg(windows)]
        let mio = {
            use std::os::windows::io::{FromRawSocket, IntoRawSocket};

            let raw_socket = self.inner.into_raw_socket();
            unsafe { mio::net::TcpStream::from_raw_socket(raw_socket) }
        };

        TcpStream::connect_mio(mio).await
    }

    /// Converts the socket into a `TcpListener`.
    ///
    /// `backlog` defines the maximum number of pending connections are queued
    /// by the operating system at any given time. Connection are removed from
    /// the queue with [`TcpListener::accept`]. When the queue is full, the
    /// operating-system will start rejecting connections.
    ///
    /// [`TcpListener::accept`]: TcpListener::accept
    ///
    /// This calls the `listen(2)` operating-system function, marking the socket
    /// as a passive socket. Behavior is platform specific. Refer to the target
    /// platform's documentation for more details.
    ///
    /// # Examples
    ///
    /// Create a `TcpListener`.
    ///
    /// ```no_run
    /// use tokio::net::TcpSocket;
    ///
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     let addr = "127.0.0.1:8080".parse().unwrap();
    ///
    ///     let socket = TcpSocket::new_v4()?;
    ///     socket.bind(addr)?;
    ///
    ///     let listener = socket.listen(1024)?;
    /// # drop(listener);
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn listen(self, backlog: u32) -> io::Result<TcpListener> {
        self.inner.listen(backlog as i32)?;
        #[cfg(unix)]
        let mio = {
            use std::os::unix::io::{FromRawFd, IntoRawFd};

            let raw_fd = self.inner.into_raw_fd();
            unsafe { mio::net::TcpListener::from_raw_fd(raw_fd) }
        };

        #[cfg(windows)]
        let mio = {
            use std::os::windows::io::{FromRawSocket, IntoRawSocket};

            let raw_socket = self.inner.into_raw_socket();
            unsafe { mio::net::TcpListener::from_raw_socket(raw_socket) }
        };

        TcpListener::new(mio)
    }

    /// Converts a [`std::net::TcpStream`] into a `TcpSocket`. The provided
    /// socket must not have been connected prior to calling this function. This
    /// function is typically used together with crates such as [`socket2`] to
    /// configure socket options that are not available on `TcpSocket`.
    ///
    /// [`std::net::TcpStream`]: struct@std::net::TcpStream
    /// [`socket2`]: https://docs.rs/socket2/
    ///
    /// # Notes
    ///
    /// The caller is responsible for ensuring that the socket is in
    /// non-blocking mode. Otherwise all I/O operations on the socket
    /// will block the thread, which will cause unexpected behavior.
    /// Non-blocking mode can be set using [`set_nonblocking`].
    ///
    /// [`set_nonblocking`]: std::net::TcpStream::set_nonblocking
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::net::TcpSocket;
    /// use socket2::{Domain, Socket, Type};
    ///
    /// #[tokio::main]
    /// async fn main() -> std::io::Result<()> {
    /// #   if cfg!(miri) { return Ok(()); } // No `socket` in miri.
    ///     let socket2_socket = Socket::new(Domain::IPV4, Type::STREAM, None)?;
    ///     socket2_socket.set_nonblocking(true)?;
    ///
    ///     let socket = TcpSocket::from_std_stream(socket2_socket.into());
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn from_std_stream(std_stream: std::net::TcpStream) -> TcpSocket {
        #[cfg(unix)]
        {
            use std::os::unix::io::{FromRawFd, IntoRawFd};

            let raw_fd = std_stream.into_raw_fd();
            unsafe { TcpSocket::from_raw_fd(raw_fd) }
        }

        #[cfg(windows)]
        {
            use std::os::windows::io::{FromRawSocket, IntoRawSocket};

            let raw_socket = std_stream.into_raw_socket();
            unsafe { TcpSocket::from_raw_socket(raw_socket) }
        }
    }
}

fn convert_address(address: socket2::SockAddr) -> io::Result<SocketAddr> {
    match address.as_socket() {
        Some(address) => Ok(address),
        None => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid address family (not IPv4 or IPv6)",
        )),
    }
}

impl fmt::Debug for TcpSocket {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(fmt)
    }
}

// These trait implementations can't be build on Windows, so we completely
// ignore them, even when building documentation.
#[cfg(unix)]
cfg_unix! {
    impl AsRawFd for TcpSocket {
        fn as_raw_fd(&self) -> RawFd {
            self.inner.as_raw_fd()
        }
    }

    impl AsFd for TcpSocket {
        fn as_fd(&self) -> BorrowedFd<'_> {
            unsafe { BorrowedFd::borrow_raw(self.as_raw_fd()) }
        }
    }

    impl FromRawFd for TcpSocket {
        /// Converts a `RawFd` to a `TcpSocket`.
        ///
        /// # Notes
        ///
        /// The caller is responsible for ensuring that the socket is in
        /// non-blocking mode.
        unsafe fn from_raw_fd(fd: RawFd) -> TcpSocket {
            let inner = socket2::Socket::from_raw_fd(fd);
            TcpSocket { inner }
        }
    }

    impl IntoRawFd for TcpSocket {
        fn into_raw_fd(self) -> RawFd {
            self.inner.into_raw_fd()
        }
    }
}

cfg_windows! {
    impl IntoRawSocket for TcpSocket {
        fn into_raw_socket(self) -> RawSocket {
            self.inner.into_raw_socket()
        }
    }

    impl AsRawSocket for TcpSocket {
        fn as_raw_socket(&self) -> RawSocket {
            self.inner.as_raw_socket()
        }
    }

    impl AsSocket for TcpSocket {
        fn as_socket(&self) -> BorrowedSocket<'_> {
            unsafe { BorrowedSocket::borrow_raw(self.as_raw_socket()) }
        }
    }

    impl FromRawSocket for TcpSocket {
        /// Converts a `RawSocket` to a `TcpStream`.
        ///
        /// # Notes
        ///
        /// The caller is responsible for ensuring that the socket is in
        /// non-blocking mode.
        unsafe fn from_raw_socket(socket: RawSocket) -> TcpSocket {
            let inner = socket2::Socket::from_raw_socket(socket);
            TcpSocket { inner }
        }
    }
}
