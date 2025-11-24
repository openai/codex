use crate::io::{Interest, PollEvented};
use crate::net::unix::{SocketAddr, UnixStream};
use crate::util::check_socket_for_blocking;

use std::fmt;
use std::io;
#[cfg(target_os = "android")]
use std::os::android::net::SocketAddrExt;
#[cfg(target_os = "linux")]
use std::os::linux::net::SocketAddrExt;
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, RawFd};
use std::os::unix::net::{self, SocketAddr as StdSocketAddr};
use std::path::Path;
use std::task::{ready, Context, Poll};

cfg_net_unix! {
    /// A Unix socket which can accept connections from other Unix sockets.
    ///
    /// You can accept a new connection by using the [`accept`](`UnixListener::accept`) method.
    ///
    /// A `UnixListener` can be turned into a `Stream` with [`UnixListenerStream`].
    ///
    /// [`UnixListenerStream`]: https://docs.rs/tokio-stream/0.1/tokio_stream/wrappers/struct.UnixListenerStream.html
    ///
    /// # Errors
    ///
    /// Note that accepting a connection can lead to various errors and not all
    /// of them are necessarily fatal ‒ for example having too many open file
    /// descriptors or the other side closing the connection while it waits in
    /// an accept queue. These would terminate the stream if not handled in any
    /// way.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::UnixListener;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let listener = UnixListener::bind("/path/to/the/socket").unwrap();
    ///     loop {
    ///         match listener.accept().await {
    ///             Ok((stream, _addr)) => {
    ///                 println!("new client!");
    ///             }
    ///             Err(e) => { /* connection failed */ }
    ///         }
    ///     }
    /// }
    /// ```
    #[cfg_attr(docsrs, doc(alias = "uds"))]
    pub struct UnixListener {
        io: PollEvented<mio::net::UnixListener>,
    }
}

impl UnixListener {
    pub(crate) fn new(listener: mio::net::UnixListener) -> io::Result<UnixListener> {
        let io = PollEvented::new(listener)?;
        Ok(UnixListener { io })
    }

    /// Creates a new `UnixListener` bound to the specified path.
    ///
    /// # Panics
    ///
    /// This function panics if it is not called from within a runtime with
    /// IO enabled.
    ///
    /// The runtime is usually set implicitly when this function is called
    /// from a future driven by a tokio runtime, otherwise runtime can be set
    /// explicitly with [`Runtime::enter`](crate::runtime::Runtime::enter) function.
    #[track_caller]
    pub fn bind<P>(path: P) -> io::Result<UnixListener>
    where
        P: AsRef<Path>,
    {
        // For now, we handle abstract socket paths on linux here.
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let addr = {
            let os_str_bytes = path.as_ref().as_os_str().as_bytes();
            if os_str_bytes.starts_with(b"\0") {
                StdSocketAddr::from_abstract_name(&os_str_bytes[1..])?
            } else {
                StdSocketAddr::from_pathname(path)?
            }
        };
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        let addr = StdSocketAddr::from_pathname(path)?;

        let listener = mio::net::UnixListener::bind_addr(&addr)?;
        let io = PollEvented::new(listener)?;
        Ok(UnixListener { io })
    }

    /// Creates new [`UnixListener`] from a [`std::os::unix::net::UnixListener`].
    ///
    /// This function is intended to be used to wrap a `UnixListener` from the
    /// standard library in the Tokio equivalent.
    ///
    /// # Notes
    ///
    /// The caller is responsible for ensuring that the listener is in
    /// non-blocking mode. Otherwise all I/O operations on the listener
    /// will block the thread, which will cause unexpected behavior.
    /// Non-blocking mode can be set using [`set_nonblocking`].
    ///
    /// Passing a listener in blocking mode is always erroneous,
    /// and the behavior in that case may change in the future.
    /// For example, it could panic.
    ///
    /// [`set_nonblocking`]: std::os::unix::net::UnixListener::set_nonblocking
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::UnixListener;
    /// use std::os::unix::net::UnixListener as StdUnixListener;
    /// # use std::error::Error;
    ///
    /// # async fn dox() -> Result<(), Box<dyn Error>> {
    /// let std_listener = StdUnixListener::bind("/path/to/the/socket")?;
    /// std_listener.set_nonblocking(true)?;
    /// let listener = UnixListener::from_std(std_listener)?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if it is not called from within a runtime with
    /// IO enabled.
    ///
    /// The runtime is usually set implicitly when this function is called
    /// from a future driven by a tokio runtime, otherwise runtime can be set
    /// explicitly with [`Runtime::enter`](crate::runtime::Runtime::enter) function.
    #[track_caller]
    pub fn from_std(listener: net::UnixListener) -> io::Result<UnixListener> {
        check_socket_for_blocking(&listener)?;

        let listener = mio::net::UnixListener::from_std(listener);
        let io = PollEvented::new(listener)?;
        Ok(UnixListener { io })
    }

    /// Turns a [`tokio::net::UnixListener`] into a [`std::os::unix::net::UnixListener`].
    ///
    /// The returned [`std::os::unix::net::UnixListener`] will have nonblocking mode
    /// set as `true`. Use [`set_nonblocking`] to change the blocking mode if needed.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use std::error::Error;
    /// # async fn dox() -> Result<(), Box<dyn Error>> {
    /// let tokio_listener = tokio::net::UnixListener::bind("/path/to/the/socket")?;
    /// let std_listener = tokio_listener.into_std()?;
    /// std_listener.set_nonblocking(false)?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`tokio::net::UnixListener`]: UnixListener
    /// [`std::os::unix::net::UnixListener`]: std::os::unix::net::UnixListener
    /// [`set_nonblocking`]: fn@std::os::unix::net::UnixListener::set_nonblocking
    pub fn into_std(self) -> io::Result<std::os::unix::net::UnixListener> {
        self.io
            .into_inner()
            .map(IntoRawFd::into_raw_fd)
            .map(|raw_fd| unsafe { net::UnixListener::from_raw_fd(raw_fd) })
    }

    /// Returns the local socket address of this listener.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.io.local_addr().map(SocketAddr)
    }

    /// Returns the value of the `SO_ERROR` option.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.io.take_error()
    }

    /// Accepts a new incoming connection to this listener.
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. If the method is used as the event in a
    /// [`tokio::select!`](crate::select) statement and some other branch
    /// completes first, then it is guaranteed that no new connections were
    /// accepted by this method.
    pub async fn accept(&self) -> io::Result<(UnixStream, SocketAddr)> {
        let (mio, addr) = self
            .io
            .registration()
            .async_io(Interest::READABLE, || self.io.accept())
            .await?;

        let addr = SocketAddr(addr);
        let stream = UnixStream::new(mio)?;
        Ok((stream, addr))
    }

    /// Polls to accept a new incoming connection to this listener.
    ///
    /// If there is no connection to accept, `Poll::Pending` is returned and the
    /// current task will be notified by a waker.  Note that on multiple calls
    /// to `poll_accept`, only the `Waker` from the `Context` passed to the most
    /// recent call is scheduled to receive a wakeup.
    pub fn poll_accept(&self, cx: &mut Context<'_>) -> Poll<io::Result<(UnixStream, SocketAddr)>> {
        let (sock, addr) = ready!(self.io.registration().poll_read_io(cx, || self.io.accept()))?;
        let addr = SocketAddr(addr);
        let sock = UnixStream::new(sock)?;
        Poll::Ready(Ok((sock, addr)))
    }
}

impl TryFrom<std::os::unix::net::UnixListener> for UnixListener {
    type Error = io::Error;

    /// Consumes stream, returning the tokio I/O object.
    ///
    /// This is equivalent to
    /// [`UnixListener::from_std(stream)`](UnixListener::from_std).
    fn try_from(stream: std::os::unix::net::UnixListener) -> io::Result<Self> {
        Self::from_std(stream)
    }
}

impl fmt::Debug for UnixListener {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.io.fmt(f)
    }
}

impl AsRawFd for UnixListener {
    fn as_raw_fd(&self) -> RawFd {
        self.io.as_raw_fd()
    }
}

impl AsFd for UnixListener {
    fn as_fd(&self) -> BorrowedFd<'_> {
        unsafe { BorrowedFd::borrow_raw(self.as_raw_fd()) }
    }
}
