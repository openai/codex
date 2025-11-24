use crate::io::{Interest, PollEvented, ReadBuf, Ready};
use crate::net::unix::SocketAddr;
use crate::util::check_socket_for_blocking;

use std::fmt;
use std::io;
use std::net::Shutdown;
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, RawFd};
use std::os::unix::net;
use std::path::Path;
use std::task::{ready, Context, Poll};

cfg_io_util! {
    use bytes::BufMut;
}

cfg_net_unix! {
    /// An I/O object representing a Unix datagram socket.
    ///
    /// A socket can be either named (associated with a filesystem path) or
    /// unnamed.
    ///
    /// This type does not provide a `split` method, because this functionality
    /// can be achieved by wrapping the socket in an [`Arc`]. Note that you do
    /// not need a `Mutex` to share the `UnixDatagram` — an `Arc<UnixDatagram>`
    /// is enough. This is because all of the methods take `&self` instead of
    /// `&mut self`.
    ///
    /// **Note:** named sockets are persisted even after the object is dropped
    /// and the program has exited, and cannot be reconnected. It is advised
    /// that you either check for and unlink the existing socket if it exists,
    /// or use a temporary file that is guaranteed to not already exist.
    ///
    /// [`Arc`]: std::sync::Arc
    ///
    /// # Examples
    /// Using named sockets, associated with a filesystem path:
    /// ```
    /// # use std::error::Error;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn Error>> {
    /// # if cfg!(miri) { return Ok(()); } // No `socket` in miri.
    /// use tokio::net::UnixDatagram;
    /// use tempfile::tempdir;
    ///
    /// // We use a temporary directory so that the socket
    /// // files left by the bound sockets will get cleaned up.
    /// let tmp = tempdir()?;
    ///
    /// // Bind each socket to a filesystem path
    /// let tx_path = tmp.path().join("tx");
    /// let tx = UnixDatagram::bind(&tx_path)?;
    /// let rx_path = tmp.path().join("rx");
    /// let rx = UnixDatagram::bind(&rx_path)?;
    ///
    /// let bytes = b"hello world";
    /// tx.send_to(bytes, &rx_path).await?;
    ///
    /// let mut buf = vec![0u8; 24];
    /// let (size, addr) = rx.recv_from(&mut buf).await?;
    ///
    /// let dgram = &buf[..size];
    /// assert_eq!(dgram, bytes);
    /// assert_eq!(addr.as_pathname().unwrap(), &tx_path);
    ///
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Using unnamed sockets, created as a pair
    /// ```
    /// # use std::error::Error;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn Error>> {
    /// # if cfg!(miri) { return Ok(()); } // No SOCK_DGRAM for `socketpair` in miri.
    /// use tokio::net::UnixDatagram;
    ///
    /// // Create the pair of sockets
    /// let (sock1, sock2) = UnixDatagram::pair()?;
    ///
    /// // Since the sockets are paired, the paired send/recv
    /// // functions can be used
    /// let bytes = b"hello world";
    /// sock1.send(bytes).await?;
    ///
    /// let mut buff = vec![0u8; 24];
    /// let size = sock2.recv(&mut buff).await?;
    ///
    /// let dgram = &buff[..size];
    /// assert_eq!(dgram, bytes);
    ///
    /// # Ok(())
    /// # }
    /// ```
    #[cfg_attr(docsrs, doc(alias = "uds"))]
    pub struct UnixDatagram {
        io: PollEvented<mio::net::UnixDatagram>,
    }
}

impl UnixDatagram {
    pub(crate) fn from_mio(sys: mio::net::UnixDatagram) -> io::Result<UnixDatagram> {
        let datagram = UnixDatagram::new(sys)?;

        if let Some(e) = datagram.io.take_error()? {
            return Err(e);
        }

        Ok(datagram)
    }

    /// Waits for any of the requested ready states.
    ///
    /// This function is usually paired with `try_recv()` or `try_send()`. It
    /// can be used to concurrently `recv` / `send` to the same socket on a single
    /// task without splitting the socket.
    ///
    /// The function may complete without the socket being ready. This is a
    /// false-positive and attempting an operation will return with
    /// `io::ErrorKind::WouldBlock`. The function can also return with an empty
    /// [`Ready`] set, so you should always check the returned value and possibly
    /// wait again if the requested states are not set.
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. Once a readiness event occurs, the method
    /// will continue to return immediately until the readiness event is
    /// consumed by an attempt to read or write that fails with `WouldBlock` or
    /// `Poll::Pending`.
    ///
    /// # Examples
    ///
    /// Concurrently receive from and send to the socket on the same task
    /// without splitting.
    ///
    /// ```no_run
    /// use tokio::io::Interest;
    /// use tokio::net::UnixDatagram;
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     let dir = tempfile::tempdir().unwrap();
    ///     let client_path = dir.path().join("client.sock");
    ///     let server_path = dir.path().join("server.sock");
    ///     let socket = UnixDatagram::bind(&client_path)?;
    ///     socket.connect(&server_path)?;
    ///
    ///     loop {
    ///         let ready = socket.ready(Interest::READABLE | Interest::WRITABLE).await?;
    ///
    ///         if ready.is_readable() {
    ///             let mut data = [0; 1024];
    ///             match socket.try_recv(&mut data[..]) {
    ///                 Ok(n) => {
    ///                     println!("received {:?}", &data[..n]);
    ///                 }
    ///                 // False-positive, continue
    ///                 Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {}
    ///                 Err(e) => {
    ///                     return Err(e);
    ///                 }
    ///             }
    ///         }
    ///
    ///         if ready.is_writable() {
    ///             // Write some data
    ///             match socket.try_send(b"hello world") {
    ///                 Ok(n) => {
    ///                     println!("sent {} bytes", n);
    ///                 }
    ///                 // False-positive, continue
    ///                 Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {}
    ///                 Err(e) => {
    ///                     return Err(e);
    ///                 }
    ///             }
    ///         }
    ///     }
    /// }
    /// ```
    pub async fn ready(&self, interest: Interest) -> io::Result<Ready> {
        let event = self.io.registration().readiness(interest).await?;
        Ok(event.ready)
    }

    /// Waits for the socket to become writable.
    ///
    /// This function is equivalent to `ready(Interest::WRITABLE)` and is
    /// usually paired with `try_send()` or `try_send_to()`.
    ///
    /// The function may complete without the socket being writable. This is a
    /// false-positive and attempting a `try_send()` will return with
    /// `io::ErrorKind::WouldBlock`.
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. Once a readiness event occurs, the method
    /// will continue to return immediately until the readiness event is
    /// consumed by an attempt to write that fails with `WouldBlock` or
    /// `Poll::Pending`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::UnixDatagram;
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     let dir = tempfile::tempdir().unwrap();
    ///     let client_path = dir.path().join("client.sock");
    ///     let server_path = dir.path().join("server.sock");
    ///     let socket = UnixDatagram::bind(&client_path)?;
    ///     socket.connect(&server_path)?;
    ///
    ///     loop {
    ///         // Wait for the socket to be writable
    ///         socket.writable().await?;
    ///
    ///         // Try to send data, this may still fail with `WouldBlock`
    ///         // if the readiness event is a false positive.
    ///         match socket.try_send(b"hello world") {
    ///             Ok(n) => {
    ///                 break;
    ///             }
    ///             Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                 continue;
    ///             }
    ///             Err(e) => {
    ///                 return Err(e);
    ///             }
    ///         }
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn writable(&self) -> io::Result<()> {
        self.ready(Interest::WRITABLE).await?;
        Ok(())
    }

    /// Polls for write/send readiness.
    ///
    /// If the socket is not currently ready for sending, this method will
    /// store a clone of the `Waker` from the provided `Context`. When the socket
    /// becomes ready for sending, `Waker::wake` will be called on the
    /// waker.
    ///
    /// Note that on multiple calls to `poll_send_ready` or `poll_send`, only
    /// the `Waker` from the `Context` passed to the most recent call is
    /// scheduled to receive a wakeup. (However, `poll_recv_ready` retains a
    /// second, independent waker.)
    ///
    /// This function is intended for cases where creating and pinning a future
    /// via [`writable`] is not feasible. Where possible, using [`writable`] is
    /// preferred, as this supports polling from multiple tasks at once.
    ///
    /// # Return value
    ///
    /// The function returns:
    ///
    /// * `Poll::Pending` if the socket is not ready for writing.
    /// * `Poll::Ready(Ok(()))` if the socket is ready for writing.
    /// * `Poll::Ready(Err(e))` if an error is encountered.
    ///
    /// # Errors
    ///
    /// This function may encounter any standard I/O error except `WouldBlock`.
    ///
    /// [`writable`]: method@Self::writable
    pub fn poll_send_ready(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.io.registration().poll_write_ready(cx).map_ok(|_| ())
    }

    /// Waits for the socket to become readable.
    ///
    /// This function is equivalent to `ready(Interest::READABLE)` and is usually
    /// paired with `try_recv()`.
    ///
    /// The function may complete without the socket being readable. This is a
    /// false-positive and attempting a `try_recv()` will return with
    /// `io::ErrorKind::WouldBlock`.
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. Once a readiness event occurs, the method
    /// will continue to return immediately until the readiness event is
    /// consumed by an attempt to read that fails with `WouldBlock` or
    /// `Poll::Pending`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::UnixDatagram;
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     // Connect to a peer
    ///     let dir = tempfile::tempdir().unwrap();
    ///     let client_path = dir.path().join("client.sock");
    ///     let server_path = dir.path().join("server.sock");
    ///     let socket = UnixDatagram::bind(&client_path)?;
    ///     socket.connect(&server_path)?;
    ///
    ///     loop {
    ///         // Wait for the socket to be readable
    ///         socket.readable().await?;
    ///
    ///         // The buffer is **not** included in the async task and will
    ///         // only exist on the stack.
    ///         let mut buf = [0; 1024];
    ///
    ///         // Try to recv data, this may still fail with `WouldBlock`
    ///         // if the readiness event is a false positive.
    ///         match socket.try_recv(&mut buf) {
    ///             Ok(n) => {
    ///                 println!("GOT {:?}", &buf[..n]);
    ///                 break;
    ///             }
    ///             Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                 continue;
    ///             }
    ///             Err(e) => {
    ///                 return Err(e);
    ///             }
    ///         }
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn readable(&self) -> io::Result<()> {
        self.ready(Interest::READABLE).await?;
        Ok(())
    }

    /// Polls for read/receive readiness.
    ///
    /// If the socket is not currently ready for receiving, this method will
    /// store a clone of the `Waker` from the provided `Context`. When the
    /// socket becomes ready for reading, `Waker::wake` will be called on the
    /// waker.
    ///
    /// Note that on multiple calls to `poll_recv_ready`, `poll_recv` or
    /// `poll_peek`, only the `Waker` from the `Context` passed to the most
    /// recent call is scheduled to receive a wakeup. (However,
    /// `poll_send_ready` retains a second, independent waker.)
    ///
    /// This function is intended for cases where creating and pinning a future
    /// via [`readable`] is not feasible. Where possible, using [`readable`] is
    /// preferred, as this supports polling from multiple tasks at once.
    ///
    /// # Return value
    ///
    /// The function returns:
    ///
    /// * `Poll::Pending` if the socket is not ready for reading.
    /// * `Poll::Ready(Ok(()))` if the socket is ready for reading.
    /// * `Poll::Ready(Err(e))` if an error is encountered.
    ///
    /// # Errors
    ///
    /// This function may encounter any standard I/O error except `WouldBlock`.
    ///
    /// [`readable`]: method@Self::readable
    pub fn poll_recv_ready(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.io.registration().poll_read_ready(cx).map_ok(|_| ())
    }

    /// Creates a new `UnixDatagram` bound to the specified path.
    ///
    /// # Examples
    /// ```
    /// # use std::error::Error;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn Error>> {
    /// # if cfg!(miri) { return Ok(()); } // No `socket` in miri.
    /// use tokio::net::UnixDatagram;
    /// use tempfile::tempdir;
    ///
    /// // We use a temporary directory so that the socket
    /// // files left by the bound sockets will get cleaned up.
    /// let tmp = tempdir()?;
    ///
    /// // Bind the socket to a filesystem path
    /// let socket_path = tmp.path().join("socket");
    /// let socket = UnixDatagram::bind(&socket_path)?;
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn bind<P>(path: P) -> io::Result<UnixDatagram>
    where
        P: AsRef<Path>,
    {
        let socket = mio::net::UnixDatagram::bind(path)?;
        UnixDatagram::new(socket)
    }

    /// Creates an unnamed pair of connected sockets.
    ///
    /// This function will create a pair of interconnected Unix sockets for
    /// communicating back and forth between one another.
    ///
    /// # Examples
    /// ```
    /// # use std::error::Error;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn Error>> {
    /// # if cfg!(miri) { return Ok(()); } // No SOCK_DGRAM for `socketpair` in miri.
    /// use tokio::net::UnixDatagram;
    ///
    /// // Create the pair of sockets
    /// let (sock1, sock2) = UnixDatagram::pair()?;
    ///
    /// // Since the sockets are paired, the paired send/recv
    /// // functions can be used
    /// let bytes = b"hail eris";
    /// sock1.send(bytes).await?;
    ///
    /// let mut buff = vec![0u8; 24];
    /// let size = sock2.recv(&mut buff).await?;
    ///
    /// let dgram = &buff[..size];
    /// assert_eq!(dgram, bytes);
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn pair() -> io::Result<(UnixDatagram, UnixDatagram)> {
        let (a, b) = mio::net::UnixDatagram::pair()?;
        let a = UnixDatagram::new(a)?;
        let b = UnixDatagram::new(b)?;

        Ok((a, b))
    }

    /// Creates new [`UnixDatagram`] from a [`std::os::unix::net::UnixDatagram`].
    ///
    /// This function is intended to be used to wrap a `UnixDatagram` from the
    /// standard library in the Tokio equivalent.
    ///
    /// # Notes
    ///
    /// The caller is responsible for ensuring that the socket is in
    /// non-blocking mode. Otherwise all I/O operations on the socket
    /// will block the thread, which will cause unexpected behavior.
    /// Non-blocking mode can be set using [`set_nonblocking`].
    ///
    /// Passing a listener in blocking mode is always erroneous,
    /// and the behavior in that case may change in the future.
    /// For example, it could panic.
    ///
    /// [`set_nonblocking`]: std::os::unix::net::UnixDatagram::set_nonblocking
    ///
    /// # Panics
    ///
    /// This function panics if it is not called from within a runtime with
    /// IO enabled.
    ///
    /// The runtime is usually set implicitly when this function is called
    /// from a future driven by a Tokio runtime, otherwise runtime can be set
    /// explicitly with [`Runtime::enter`](crate::runtime::Runtime::enter) function.
    /// # Examples
    /// ```
    /// # use std::error::Error;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn Error>> {
    /// # if cfg!(miri) { return Ok(()); } // No `socket` in miri.
    /// use tokio::net::UnixDatagram;
    /// use std::os::unix::net::UnixDatagram as StdUDS;
    /// use tempfile::tempdir;
    ///
    /// // We use a temporary directory so that the socket
    /// // files left by the bound sockets will get cleaned up.
    /// let tmp = tempdir()?;
    ///
    /// // Bind the socket to a filesystem path
    /// let socket_path = tmp.path().join("socket");
    /// let std_socket = StdUDS::bind(&socket_path)?;
    /// std_socket.set_nonblocking(true)?;
    /// let tokio_socket = UnixDatagram::from_std(std_socket)?;
    ///
    /// # Ok(())
    /// # }
    /// ```
    #[track_caller]
    pub fn from_std(datagram: net::UnixDatagram) -> io::Result<UnixDatagram> {
        check_socket_for_blocking(&datagram)?;

        let socket = mio::net::UnixDatagram::from_std(datagram);
        let io = PollEvented::new(socket)?;
        Ok(UnixDatagram { io })
    }

    /// Turns a [`tokio::net::UnixDatagram`] into a [`std::os::unix::net::UnixDatagram`].
    ///
    /// The returned [`std::os::unix::net::UnixDatagram`] will have nonblocking
    /// mode set as `true`. Use [`set_nonblocking`] to change the blocking mode
    /// if needed.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use std::error::Error;
    /// # async fn dox() -> Result<(), Box<dyn Error>> {
    /// let tokio_socket = tokio::net::UnixDatagram::bind("/path/to/the/socket")?;
    /// let std_socket = tokio_socket.into_std()?;
    /// std_socket.set_nonblocking(false)?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`tokio::net::UnixDatagram`]: UnixDatagram
    /// [`std::os::unix::net::UnixDatagram`]: std::os::unix::net::UnixDatagram
    /// [`set_nonblocking`]: fn@std::os::unix::net::UnixDatagram::set_nonblocking
    pub fn into_std(self) -> io::Result<std::os::unix::net::UnixDatagram> {
        self.io
            .into_inner()
            .map(IntoRawFd::into_raw_fd)
            .map(|raw_fd| unsafe { std::os::unix::net::UnixDatagram::from_raw_fd(raw_fd) })
    }

    fn new(socket: mio::net::UnixDatagram) -> io::Result<UnixDatagram> {
        let io = PollEvented::new(socket)?;
        Ok(UnixDatagram { io })
    }

    /// Creates a new `UnixDatagram` which is not bound to any address.
    ///
    /// # Examples
    /// ```
    /// # use std::error::Error;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn Error>> {
    /// # if cfg!(miri) { return Ok(()); } // No `socket` in miri.
    /// use tokio::net::UnixDatagram;
    /// use tempfile::tempdir;
    ///
    /// // Create an unbound socket
    /// let tx = UnixDatagram::unbound()?;
    ///
    /// // Create another, bound socket
    /// let tmp = tempdir()?;
    /// let rx_path = tmp.path().join("rx");
    /// let rx = UnixDatagram::bind(&rx_path)?;
    ///
    /// // Send to the bound socket
    /// let bytes = b"hello world";
    /// tx.send_to(bytes, &rx_path).await?;
    ///
    /// let mut buf = vec![0u8; 24];
    /// let (size, addr) = rx.recv_from(&mut buf).await?;
    ///
    /// let dgram = &buf[..size];
    /// assert_eq!(dgram, bytes);
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn unbound() -> io::Result<UnixDatagram> {
        let socket = mio::net::UnixDatagram::unbound()?;
        UnixDatagram::new(socket)
    }

    /// Connects the socket to the specified address.
    ///
    /// The `send` method may be used to send data to the specified address.
    /// `recv` and `recv_from` will only receive data from that address.
    ///
    /// # Examples
    /// ```
    /// # use std::error::Error;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn Error>> {
    /// # if cfg!(miri) { return Ok(()); } // No `socket` in miri.
    /// use tokio::net::UnixDatagram;
    /// use tempfile::tempdir;
    ///
    /// // Create an unbound socket
    /// let tx = UnixDatagram::unbound()?;
    ///
    /// // Create another, bound socket
    /// let tmp = tempdir()?;
    /// let rx_path = tmp.path().join("rx");
    /// let rx = UnixDatagram::bind(&rx_path)?;
    ///
    /// // Connect to the bound socket
    /// tx.connect(&rx_path)?;
    ///
    /// // Send to the bound socket
    /// let bytes = b"hello world";
    /// tx.send(bytes).await?;
    ///
    /// let mut buf = vec![0u8; 24];
    /// let (size, addr) = rx.recv_from(&mut buf).await?;
    ///
    /// let dgram = &buf[..size];
    /// assert_eq!(dgram, bytes);
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn connect<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        self.io.connect(path)
    }

    /// Sends data on the socket to the socket's peer.
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. If `send` is used as the event in a
    /// [`tokio::select!`](crate::select) statement and some other branch
    /// completes first, then it is guaranteed that the message was not sent.
    ///
    /// # Examples
    /// ```
    /// # use std::error::Error;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn Error>> {
    /// # if cfg!(miri) { return Ok(()); } // No SOCK_DGRAM for `socketpair` in miri.
    /// use tokio::net::UnixDatagram;
    ///
    /// // Create the pair of sockets
    /// let (sock1, sock2) = UnixDatagram::pair()?;
    ///
    /// // Since the sockets are paired, the paired send/recv
    /// // functions can be used
    /// let bytes = b"hello world";
    /// sock1.send(bytes).await?;
    ///
    /// let mut buff = vec![0u8; 24];
    /// let size = sock2.recv(&mut buff).await?;
    ///
    /// let dgram = &buff[..size];
    /// assert_eq!(dgram, bytes);
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.io
            .registration()
            .async_io(Interest::WRITABLE, || self.io.send(buf))
            .await
    }

    /// Tries to send a datagram to the peer without waiting.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::UnixDatagram;
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     let dir = tempfile::tempdir().unwrap();
    ///     let client_path = dir.path().join("client.sock");
    ///     let server_path = dir.path().join("server.sock");
    ///     let socket = UnixDatagram::bind(&client_path)?;
    ///     socket.connect(&server_path)?;
    ///
    ///     loop {
    ///         // Wait for the socket to be writable
    ///         socket.writable().await?;
    ///
    ///         // Try to send data, this may still fail with `WouldBlock`
    ///         // if the readiness event is a false positive.
    ///         match socket.try_send(b"hello world") {
    ///             Ok(n) => {
    ///                 break;
    ///             }
    ///             Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                 continue;
    ///             }
    ///             Err(e) => {
    ///                 return Err(e);
    ///             }
    ///         }
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn try_send(&self, buf: &[u8]) -> io::Result<usize> {
        self.io
            .registration()
            .try_io(Interest::WRITABLE, || self.io.send(buf))
    }

    /// Tries to send a datagram to the peer without waiting.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::UnixDatagram;
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     let dir = tempfile::tempdir().unwrap();
    ///     let client_path = dir.path().join("client.sock");
    ///     let server_path = dir.path().join("server.sock");
    ///     let socket = UnixDatagram::bind(&client_path)?;
    ///
    ///     loop {
    ///         // Wait for the socket to be writable
    ///         socket.writable().await?;
    ///
    ///         // Try to send data, this may still fail with `WouldBlock`
    ///         // if the readiness event is a false positive.
    ///         match socket.try_send_to(b"hello world", &server_path) {
    ///             Ok(n) => {
    ///                 break;
    ///             }
    ///             Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                 continue;
    ///             }
    ///             Err(e) => {
    ///                 return Err(e);
    ///             }
    ///         }
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn try_send_to<P>(&self, buf: &[u8], target: P) -> io::Result<usize>
    where
        P: AsRef<Path>,
    {
        self.io
            .registration()
            .try_io(Interest::WRITABLE, || self.io.send_to(buf, target))
    }

    /// Receives data from the socket.
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. If `recv` is used as the event in a
    /// [`tokio::select!`](crate::select) statement and some other branch
    /// completes first, it is guaranteed that no messages were received on this
    /// socket.
    ///
    /// # Examples
    /// ```
    /// # use std::error::Error;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn Error>> {
    /// # if cfg!(miri) { return Ok(()); } // No SOCK_DGRAM for `socketpair` in miri.
    /// use tokio::net::UnixDatagram;
    ///
    /// // Create the pair of sockets
    /// let (sock1, sock2) = UnixDatagram::pair()?;
    ///
    /// // Since the sockets are paired, the paired send/recv
    /// // functions can be used
    /// let bytes = b"hello world";
    /// sock1.send(bytes).await?;
    ///
    /// let mut buff = vec![0u8; 24];
    /// let size = sock2.recv(&mut buff).await?;
    ///
    /// let dgram = &buff[..size];
    /// assert_eq!(dgram, bytes);
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub async fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.io
            .registration()
            .async_io(Interest::READABLE, || self.io.recv(buf))
            .await
    }

    /// Tries to receive a datagram from the peer without waiting.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::UnixDatagram;
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     // Connect to a peer
    ///     let dir = tempfile::tempdir().unwrap();
    ///     let client_path = dir.path().join("client.sock");
    ///     let server_path = dir.path().join("server.sock");
    ///     let socket = UnixDatagram::bind(&client_path)?;
    ///     socket.connect(&server_path)?;
    ///
    ///     loop {
    ///         // Wait for the socket to be readable
    ///         socket.readable().await?;
    ///
    ///         // The buffer is **not** included in the async task and will
    ///         // only exist on the stack.
    ///         let mut buf = [0; 1024];
    ///
    ///         // Try to recv data, this may still fail with `WouldBlock`
    ///         // if the readiness event is a false positive.
    ///         match socket.try_recv(&mut buf) {
    ///             Ok(n) => {
    ///                 println!("GOT {:?}", &buf[..n]);
    ///                 break;
    ///             }
    ///             Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                 continue;
    ///             }
    ///             Err(e) => {
    ///                 return Err(e);
    ///             }
    ///         }
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn try_recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.io
            .registration()
            .try_io(Interest::READABLE, || self.io.recv(buf))
    }

    cfg_io_util! {
        /// Tries to receive data from the socket without waiting.
        ///
        /// This method can be used even if `buf` is uninitialized.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use tokio::net::UnixDatagram;
        /// use std::io;
        ///
        /// #[tokio::main]
        /// async fn main() -> io::Result<()> {
        ///     // Connect to a peer
        ///     let dir = tempfile::tempdir().unwrap();
        ///     let client_path = dir.path().join("client.sock");
        ///     let server_path = dir.path().join("server.sock");
        ///     let socket = UnixDatagram::bind(&client_path)?;
        ///
        ///     loop {
        ///         // Wait for the socket to be readable
        ///         socket.readable().await?;
        ///
        ///         let mut buf = Vec::with_capacity(1024);
        ///
        ///         // Try to recv data, this may still fail with `WouldBlock`
        ///         // if the readiness event is a false positive.
        ///         match socket.try_recv_buf_from(&mut buf) {
        ///             Ok((n, _addr)) => {
        ///                 println!("GOT {:?}", &buf[..n]);
        ///                 break;
        ///             }
        ///             Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
        ///                 continue;
        ///             }
        ///             Err(e) => {
        ///                 return Err(e);
        ///             }
        ///         }
        ///     }
        ///
        ///     Ok(())
        /// }
        /// ```
        pub fn try_recv_buf_from<B: BufMut>(&self, buf: &mut B) -> io::Result<(usize, SocketAddr)> {
            let (n, addr) = self.io.registration().try_io(Interest::READABLE, || {
                let dst = buf.chunk_mut();
                let dst =
                    unsafe { &mut *(dst as *mut _ as *mut [std::mem::MaybeUninit<u8>] as *mut [u8]) };

                // Safety: We trust `UnixDatagram::recv_from` to have filled up `n` bytes in the
                // buffer.
                let (n, addr) = (*self.io).recv_from(dst)?;

                unsafe {
                    buf.advance_mut(n);
                }

                Ok((n, addr))
            })?;

            Ok((n, SocketAddr(addr)))
        }

        /// Receives from the socket, advances the
        /// buffer's internal cursor and returns how many bytes were read and the origin.
        ///
        /// This method can be used even if `buf` is uninitialized.
        ///
        /// # Examples
        /// ```
        /// # use std::error::Error;
        /// # #[tokio::main]
        /// # async fn main() -> Result<(), Box<dyn Error>> {
        /// # if cfg!(miri) { return Ok(()); } // No `socket` in miri.
        /// use tokio::net::UnixDatagram;
        /// use tempfile::tempdir;
        ///
        /// // We use a temporary directory so that the socket
        /// // files left by the bound sockets will get cleaned up.
        /// let tmp = tempdir()?;
        ///
        /// // Bind each socket to a filesystem path
        /// let tx_path = tmp.path().join("tx");
        /// let tx = UnixDatagram::bind(&tx_path)?;
        /// let rx_path = tmp.path().join("rx");
        /// let rx = UnixDatagram::bind(&rx_path)?;
        ///
        /// let bytes = b"hello world";
        /// tx.send_to(bytes, &rx_path).await?;
        ///
        /// let mut buf = Vec::with_capacity(24);
        /// let (size, addr) = rx.recv_buf_from(&mut buf).await?;
        ///
        /// let dgram = &buf[..size];
        /// assert_eq!(dgram, bytes);
        /// assert_eq!(addr.as_pathname().unwrap(), &tx_path);
        ///
        /// # Ok(())
        /// # }
        /// ```
        pub async fn recv_buf_from<B: BufMut>(&self, buf: &mut B) -> io::Result<(usize, SocketAddr)> {
            self.io.registration().async_io(Interest::READABLE, || {
                let dst = buf.chunk_mut();
                let dst =
                    unsafe { &mut *(dst as *mut _ as *mut [std::mem::MaybeUninit<u8>] as *mut [u8]) };

                // Safety: We trust `UnixDatagram::recv_from` to have filled up `n` bytes in the
                // buffer.
                let (n, addr) = (*self.io).recv_from(dst)?;

                unsafe {
                    buf.advance_mut(n);
                }
                Ok((n,SocketAddr(addr)))
            }).await
        }

        /// Tries to read data from the stream into the provided buffer, advancing the
        /// buffer's internal cursor, returning how many bytes were read.
        ///
        /// This method can be used even if `buf` is uninitialized.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use tokio::net::UnixDatagram;
        /// use std::io;
        ///
        /// #[tokio::main]
        /// async fn main() -> io::Result<()> {
        ///     // Connect to a peer
        ///     let dir = tempfile::tempdir().unwrap();
        ///     let client_path = dir.path().join("client.sock");
        ///     let server_path = dir.path().join("server.sock");
        ///     let socket = UnixDatagram::bind(&client_path)?;
        ///     socket.connect(&server_path)?;
        ///
        ///     loop {
        ///         // Wait for the socket to be readable
        ///         socket.readable().await?;
        ///
        ///         let mut buf = Vec::with_capacity(1024);
        ///
        ///         // Try to recv data, this may still fail with `WouldBlock`
        ///         // if the readiness event is a false positive.
        ///         match socket.try_recv_buf(&mut buf) {
        ///             Ok(n) => {
        ///                 println!("GOT {:?}", &buf[..n]);
        ///                 break;
        ///             }
        ///             Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
        ///                 continue;
        ///             }
        ///             Err(e) => {
        ///                 return Err(e);
        ///             }
        ///         }
        ///     }
        ///
        ///     Ok(())
        /// }
        /// ```
        pub fn try_recv_buf<B: BufMut>(&self, buf: &mut B) -> io::Result<usize> {
            self.io.registration().try_io(Interest::READABLE, || {
                let dst = buf.chunk_mut();
                let dst =
                    unsafe { &mut *(dst as *mut _ as *mut [std::mem::MaybeUninit<u8>] as *mut [u8]) };

                // Safety: We trust `UnixDatagram::recv` to have filled up `n` bytes in the
                // buffer.
                let n = (*self.io).recv(dst)?;

                unsafe {
                    buf.advance_mut(n);
                }

                Ok(n)
            })
        }

        /// Receives data from the socket from the address to which it is connected,
        /// advancing the buffer's internal cursor, returning how many bytes were read.
        ///
        /// This method can be used even if `buf` is uninitialized.
        ///
        /// # Examples
        /// ```
        /// # use std::error::Error;
        /// # #[tokio::main]
        /// # async fn main() -> Result<(), Box<dyn Error>> {
        /// # if cfg!(miri) { return Ok(()); } // No SOCK_DGRAM for `socketpair` in miri.
        /// use tokio::net::UnixDatagram;
        ///
        /// // Create the pair of sockets
        /// let (sock1, sock2) = UnixDatagram::pair()?;
        ///
        /// // Since the sockets are paired, the paired send/recv
        /// // functions can be used
        /// let bytes = b"hello world";
        /// sock1.send(bytes).await?;
        ///
        /// let mut buff = Vec::with_capacity(24);
        /// let size = sock2.recv_buf(&mut buff).await?;
        ///
        /// let dgram = &buff[..size];
        /// assert_eq!(dgram, bytes);
        ///
        /// # Ok(())
        /// # }
        /// ```
        pub async fn recv_buf<B: BufMut>(&self, buf: &mut B) -> io::Result<usize> {
            self.io.registration().async_io(Interest::READABLE, || {
                let dst = buf.chunk_mut();
                let dst =
                    unsafe { &mut *(dst as *mut _ as *mut [std::mem::MaybeUninit<u8>] as *mut [u8]) };

                // Safety: We trust `UnixDatagram::recv_from` to have filled up `n` bytes in the
                // buffer.
                let n = (*self.io).recv(dst)?;

                unsafe {
                    buf.advance_mut(n);
                }
                Ok(n)
            }).await
        }
    }

    /// Sends data on the socket to the specified address.
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. If `send_to` is used as the event in a
    /// [`tokio::select!`](crate::select) statement and some other branch
    /// completes first, then it is guaranteed that the message was not sent.
    ///
    /// # Examples
    /// ```
    /// # use std::error::Error;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn Error>> {
    /// # if cfg!(miri) { return Ok(()); } // No `socket` in miri.
    /// use tokio::net::UnixDatagram;
    /// use tempfile::tempdir;
    ///
    /// // We use a temporary directory so that the socket
    /// // files left by the bound sockets will get cleaned up.
    /// let tmp = tempdir()?;
    ///
    /// // Bind each socket to a filesystem path
    /// let tx_path = tmp.path().join("tx");
    /// let tx = UnixDatagram::bind(&tx_path)?;
    /// let rx_path = tmp.path().join("rx");
    /// let rx = UnixDatagram::bind(&rx_path)?;
    ///
    /// let bytes = b"hello world";
    /// tx.send_to(bytes, &rx_path).await?;
    ///
    /// let mut buf = vec![0u8; 24];
    /// let (size, addr) = rx.recv_from(&mut buf).await?;
    ///
    /// let dgram = &buf[..size];
    /// assert_eq!(dgram, bytes);
    /// assert_eq!(addr.as_pathname().unwrap(), &tx_path);
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send_to<P>(&self, buf: &[u8], target: P) -> io::Result<usize>
    where
        P: AsRef<Path>,
    {
        self.io
            .registration()
            .async_io(Interest::WRITABLE, || self.io.send_to(buf, target.as_ref()))
            .await
    }

    /// Receives data from the socket.
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. If `recv_from` is used as the event in a
    /// [`tokio::select!`](crate::select) statement and some other branch
    /// completes first, it is guaranteed that no messages were received on this
    /// socket.
    ///
    /// # Examples
    /// ```
    /// # use std::error::Error;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn Error>> {
    /// # if cfg!(miri) { return Ok(()); } // No `socket` in miri.
    /// use tokio::net::UnixDatagram;
    /// use tempfile::tempdir;
    ///
    /// // We use a temporary directory so that the socket
    /// // files left by the bound sockets will get cleaned up.
    /// let tmp = tempdir()?;
    ///
    /// // Bind each socket to a filesystem path
    /// let tx_path = tmp.path().join("tx");
    /// let tx = UnixDatagram::bind(&tx_path)?;
    /// let rx_path = tmp.path().join("rx");
    /// let rx = UnixDatagram::bind(&rx_path)?;
    ///
    /// let bytes = b"hello world";
    /// tx.send_to(bytes, &rx_path).await?;
    ///
    /// let mut buf = vec![0u8; 24];
    /// let (size, addr) = rx.recv_from(&mut buf).await?;
    ///
    /// let dgram = &buf[..size];
    /// assert_eq!(dgram, bytes);
    /// assert_eq!(addr.as_pathname().unwrap(), &tx_path);
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        let (n, addr) = self
            .io
            .registration()
            .async_io(Interest::READABLE, || self.io.recv_from(buf))
            .await?;

        Ok((n, SocketAddr(addr)))
    }

    /// Attempts to receive a single datagram on the specified address.
    ///
    /// Note that on multiple calls to a `poll_*` method in the `recv` direction, only the
    /// `Waker` from the `Context` passed to the most recent call will be scheduled to
    /// receive a wakeup.
    ///
    /// # Return value
    ///
    /// The function returns:
    ///
    /// * `Poll::Pending` if the socket is not ready to read
    /// * `Poll::Ready(Ok(addr))` reads data from `addr` into `ReadBuf` if the socket is ready
    /// * `Poll::Ready(Err(e))` if an error is encountered.
    ///
    /// # Errors
    ///
    /// This function may encounter any standard I/O error except `WouldBlock`.
    pub fn poll_recv_from(
        &self,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<SocketAddr>> {
        #[allow(clippy::blocks_in_conditions)]
        let (n, addr) = ready!(self.io.registration().poll_read_io(cx, || {
            // Safety: will not read the maybe uninitialized bytes.
            let b = unsafe {
                &mut *(buf.unfilled_mut() as *mut [std::mem::MaybeUninit<u8>] as *mut [u8])
            };

            self.io.recv_from(b)
        }))?;

        // Safety: We trust `recv` to have filled up `n` bytes in the buffer.
        unsafe {
            buf.assume_init(n);
        }
        buf.advance(n);
        Poll::Ready(Ok(SocketAddr(addr)))
    }

    /// Attempts to send data to the specified address.
    ///
    /// Note that on multiple calls to a `poll_*` method in the send direction, only the
    /// `Waker` from the `Context` passed to the most recent call will be scheduled to
    /// receive a wakeup.
    ///
    /// # Return value
    ///
    /// The function returns:
    ///
    /// * `Poll::Pending` if the socket is not ready to write
    /// * `Poll::Ready(Ok(n))` `n` is the number of bytes sent.
    /// * `Poll::Ready(Err(e))` if an error is encountered.
    ///
    /// # Errors
    ///
    /// This function may encounter any standard I/O error except `WouldBlock`.
    pub fn poll_send_to<P>(
        &self,
        cx: &mut Context<'_>,
        buf: &[u8],
        target: P,
    ) -> Poll<io::Result<usize>>
    where
        P: AsRef<Path>,
    {
        self.io
            .registration()
            .poll_write_io(cx, || self.io.send_to(buf, target.as_ref()))
    }

    /// Attempts to send data on the socket to the remote address to which it
    /// was previously `connect`ed.
    ///
    /// The [`connect`] method will connect this socket to a remote address.
    /// This method will fail if the socket is not connected.
    ///
    /// Note that on multiple calls to a `poll_*` method in the send direction,
    /// only the `Waker` from the `Context` passed to the most recent call will
    /// be scheduled to receive a wakeup.
    ///
    /// # Return value
    ///
    /// The function returns:
    ///
    /// * `Poll::Pending` if the socket is not available to write
    /// * `Poll::Ready(Ok(n))` `n` is the number of bytes sent
    /// * `Poll::Ready(Err(e))` if an error is encountered.
    ///
    /// # Errors
    ///
    /// This function may encounter any standard I/O error except `WouldBlock`.
    ///
    /// [`connect`]: method@Self::connect
    pub fn poll_send(&self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.io
            .registration()
            .poll_write_io(cx, || self.io.send(buf))
    }

    /// Attempts to receive a single datagram message on the socket from the remote
    /// address to which it is `connect`ed.
    ///
    /// The [`connect`] method will connect this socket to a remote address. This method
    /// resolves to an error if the socket is not connected.
    ///
    /// Note that on multiple calls to a `poll_*` method in the `recv` direction, only the
    /// `Waker` from the `Context` passed to the most recent call will be scheduled to
    /// receive a wakeup.
    ///
    /// # Return value
    ///
    /// The function returns:
    ///
    /// * `Poll::Pending` if the socket is not ready to read
    /// * `Poll::Ready(Ok(()))` reads data `ReadBuf` if the socket is ready
    /// * `Poll::Ready(Err(e))` if an error is encountered.
    ///
    /// # Errors
    ///
    /// This function may encounter any standard I/O error except `WouldBlock`.
    ///
    /// [`connect`]: method@Self::connect
    pub fn poll_recv(&self, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        #[allow(clippy::blocks_in_conditions)]
        let n = ready!(self.io.registration().poll_read_io(cx, || {
            // Safety: will not read the maybe uninitialized bytes.
            let b = unsafe {
                &mut *(buf.unfilled_mut() as *mut [std::mem::MaybeUninit<u8>] as *mut [u8])
            };

            self.io.recv(b)
        }))?;

        // Safety: We trust `recv` to have filled up `n` bytes in the buffer.
        unsafe {
            buf.assume_init(n);
        }
        buf.advance(n);
        Poll::Ready(Ok(()))
    }

    /// Tries to receive data from the socket without waiting.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::UnixDatagram;
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     // Connect to a peer
    ///     let dir = tempfile::tempdir().unwrap();
    ///     let client_path = dir.path().join("client.sock");
    ///     let server_path = dir.path().join("server.sock");
    ///     let socket = UnixDatagram::bind(&client_path)?;
    ///
    ///     loop {
    ///         // Wait for the socket to be readable
    ///         socket.readable().await?;
    ///
    ///         // The buffer is **not** included in the async task and will
    ///         // only exist on the stack.
    ///         let mut buf = [0; 1024];
    ///
    ///         // Try to recv data, this may still fail with `WouldBlock`
    ///         // if the readiness event is a false positive.
    ///         match socket.try_recv_from(&mut buf) {
    ///             Ok((n, _addr)) => {
    ///                 println!("GOT {:?}", &buf[..n]);
    ///                 break;
    ///             }
    ///             Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                 continue;
    ///             }
    ///             Err(e) => {
    ///                 return Err(e);
    ///             }
    ///         }
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn try_recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        let (n, addr) = self
            .io
            .registration()
            .try_io(Interest::READABLE, || self.io.recv_from(buf))?;

        Ok((n, SocketAddr(addr)))
    }

    /// Tries to read or write from the socket using a user-provided IO operation.
    ///
    /// If the socket is ready, the provided closure is called. The closure
    /// should attempt to perform IO operation on the socket by manually
    /// calling the appropriate syscall. If the operation fails because the
    /// socket is not actually ready, then the closure should return a
    /// `WouldBlock` error and the readiness flag is cleared. The return value
    /// of the closure is then returned by `try_io`.
    ///
    /// If the socket is not ready, then the closure is not called
    /// and a `WouldBlock` error is returned.
    ///
    /// The closure should only return a `WouldBlock` error if it has performed
    /// an IO operation on the socket that failed due to the socket not being
    /// ready. Returning a `WouldBlock` error in any other situation will
    /// incorrectly clear the readiness flag, which can cause the socket to
    /// behave incorrectly.
    ///
    /// The closure should not perform the IO operation using any of the methods
    /// defined on the Tokio `UnixDatagram` type, as this will mess with the
    /// readiness flag and can cause the socket to behave incorrectly.
    ///
    /// This method is not intended to be used with combined interests.
    /// The closure should perform only one type of IO operation, so it should not
    /// require more than one ready state. This method may panic or sleep forever
    /// if it is called with a combined interest.
    ///
    /// Usually, [`readable()`], [`writable()`] or [`ready()`] is used with this function.
    ///
    /// [`readable()`]: UnixDatagram::readable()
    /// [`writable()`]: UnixDatagram::writable()
    /// [`ready()`]: UnixDatagram::ready()
    pub fn try_io<R>(
        &self,
        interest: Interest,
        f: impl FnOnce() -> io::Result<R>,
    ) -> io::Result<R> {
        self.io
            .registration()
            .try_io(interest, || self.io.try_io(f))
    }

    /// Reads or writes from the socket using a user-provided IO operation.
    ///
    /// The readiness of the socket is awaited and when the socket is ready,
    /// the provided closure is called. The closure should attempt to perform
    /// IO operation on the socket by manually calling the appropriate syscall.
    /// If the operation fails because the socket is not actually ready,
    /// then the closure should return a `WouldBlock` error. In such case the
    /// readiness flag is cleared and the socket readiness is awaited again.
    /// This loop is repeated until the closure returns an `Ok` or an error
    /// other than `WouldBlock`.
    ///
    /// The closure should only return a `WouldBlock` error if it has performed
    /// an IO operation on the socket that failed due to the socket not being
    /// ready. Returning a `WouldBlock` error in any other situation will
    /// incorrectly clear the readiness flag, which can cause the socket to
    /// behave incorrectly.
    ///
    /// The closure should not perform the IO operation using any of the methods
    /// defined on the Tokio `UnixDatagram` type, as this will mess with the
    /// readiness flag and can cause the socket to behave incorrectly.
    ///
    /// This method is not intended to be used with combined interests.
    /// The closure should perform only one type of IO operation, so it should not
    /// require more than one ready state. This method may panic or sleep forever
    /// if it is called with a combined interest.
    pub async fn async_io<R>(
        &self,
        interest: Interest,
        mut f: impl FnMut() -> io::Result<R>,
    ) -> io::Result<R> {
        self.io
            .registration()
            .async_io(interest, || self.io.try_io(&mut f))
            .await
    }

    /// Returns the local address that this socket is bound to.
    ///
    /// # Examples
    /// For a socket bound to a local path
    /// ```
    /// # use std::error::Error;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn Error>> {
    /// # if cfg!(miri) { return Ok(()); } // No `socket` in miri.
    /// use tokio::net::UnixDatagram;
    /// use tempfile::tempdir;
    ///
    /// // We use a temporary directory so that the socket
    /// // files left by the bound sockets will get cleaned up.
    /// let tmp = tempdir()?;
    ///
    /// // Bind socket to a filesystem path
    /// let socket_path = tmp.path().join("socket");
    /// let socket = UnixDatagram::bind(&socket_path)?;
    ///
    /// assert_eq!(socket.local_addr()?.as_pathname().unwrap(), &socket_path);
    ///
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// For an unbound socket
    /// ```
    /// # use std::error::Error;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn Error>> {
    /// # if cfg!(miri) { return Ok(()); } // No `socket` in miri.
    /// use tokio::net::UnixDatagram;
    ///
    /// // Create an unbound socket
    /// let socket = UnixDatagram::unbound()?;
    ///
    /// assert!(socket.local_addr()?.is_unnamed());
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.io.local_addr().map(SocketAddr)
    }

    /// Returns the address of this socket's peer.
    ///
    /// The `connect` method will connect the socket to a peer.
    ///
    /// # Examples
    /// For a peer with a local path
    /// ```
    /// # use std::error::Error;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn Error>> {
    /// # if cfg!(miri) { return Ok(()); } // No `socket` in miri.
    /// use tokio::net::UnixDatagram;
    /// use tempfile::tempdir;
    ///
    /// // Create an unbound socket
    /// let tx = UnixDatagram::unbound()?;
    ///
    /// // Create another, bound socket
    /// let tmp = tempdir()?;
    /// let rx_path = tmp.path().join("rx");
    /// let rx = UnixDatagram::bind(&rx_path)?;
    ///
    /// // Connect to the bound socket
    /// tx.connect(&rx_path)?;
    ///
    /// assert_eq!(tx.peer_addr()?.as_pathname().unwrap(), &rx_path);
    ///
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// For an unbound peer
    /// ```
    /// # use std::error::Error;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn Error>> {
    /// # if cfg!(miri) { return Ok(()); } // No SOCK_DGRAM for `socketpair` in miri.
    /// use tokio::net::UnixDatagram;
    ///
    /// // Create the pair of sockets
    /// let (sock1, sock2) = UnixDatagram::pair()?;
    ///
    /// assert!(sock1.peer_addr()?.is_unnamed());
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.io.peer_addr().map(SocketAddr)
    }

    /// Returns the value of the `SO_ERROR` option.
    ///
    /// # Examples
    /// ```
    /// # use std::error::Error;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn Error>> {
    /// # if cfg!(miri) { return Ok(()); } // No `socket` in miri.
    /// use tokio::net::UnixDatagram;
    ///
    /// // Create an unbound socket
    /// let socket = UnixDatagram::unbound()?;
    ///
    /// if let Ok(Some(err)) = socket.take_error() {
    ///     println!("Got error: {:?}", err);
    /// }
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.io.take_error()
    }

    /// Shuts down the read, write, or both halves of this connection.
    ///
    /// This function will cause all pending and future I/O calls on the
    /// specified portions to immediately return with an appropriate value
    /// (see the documentation of `Shutdown`).
    ///
    /// # Examples
    /// ```
    /// # use std::error::Error;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn Error>> {
    /// # if cfg!(miri) { return Ok(()); } // No SOCK_DGRAM for `socketpair` in miri.
    /// use tokio::net::UnixDatagram;
    /// use std::net::Shutdown;
    ///
    /// // Create an unbound socket
    /// let (socket, other) = UnixDatagram::pair()?;
    ///
    /// socket.shutdown(Shutdown::Both)?;
    ///
    /// // NOTE: the following commented out code does NOT work as expected.
    /// // Due to an underlying issue, the recv call will block indefinitely.
    /// // See: https://github.com/tokio-rs/tokio/issues/1679
    /// //let mut buff = vec![0u8; 24];
    /// //let size = socket.recv(&mut buff).await?;
    /// //assert_eq!(size, 0);
    ///
    /// let send_result = socket.send(b"hello world").await;
    /// assert!(send_result.is_err());
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.io.shutdown(how)
    }
}

impl TryFrom<std::os::unix::net::UnixDatagram> for UnixDatagram {
    type Error = io::Error;

    /// Consumes stream, returning the Tokio I/O object.
    ///
    /// This is equivalent to
    /// [`UnixDatagram::from_std(stream)`](UnixDatagram::from_std).
    fn try_from(stream: std::os::unix::net::UnixDatagram) -> Result<Self, Self::Error> {
        Self::from_std(stream)
    }
}

impl fmt::Debug for UnixDatagram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.io.fmt(f)
    }
}

impl AsRawFd for UnixDatagram {
    fn as_raw_fd(&self) -> RawFd {
        self.io.as_raw_fd()
    }
}

impl AsFd for UnixDatagram {
    fn as_fd(&self) -> BorrowedFd<'_> {
        unsafe { BorrowedFd::borrow_raw(self.as_raw_fd()) }
    }
}
