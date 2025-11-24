use crate::io::{Interest, Ready};
use crate::runtime::io::{ReadyEvent, Registration};
use crate::runtime::scheduler;

use mio::unix::SourceFd;
use std::error::Error;
use std::fmt;
use std::io;
use std::os::unix::io::{AsRawFd, RawFd};
use std::task::{ready, Context, Poll};

/// Associates an IO object backed by a Unix file descriptor with the tokio
/// reactor, allowing for readiness to be polled. The file descriptor must be of
/// a type that can be used with the OS polling facilities (ie, `poll`, `epoll`,
/// `kqueue`, etc), such as a network socket or pipe, and the file descriptor
/// must have the nonblocking mode set to true.
///
/// Creating an [`AsyncFd`] registers the file descriptor with the current tokio
/// Reactor, allowing you to directly await the file descriptor being readable
/// or writable. Once registered, the file descriptor remains registered until
/// the [`AsyncFd`] is dropped.
///
/// The [`AsyncFd`] takes ownership of an arbitrary object to represent the IO
/// object. It is intended that the inner object will handle closing the file
/// descriptor when it is dropped, avoiding resource leaks and ensuring that the
/// [`AsyncFd`] can clean up the registration before closing the file descriptor.
/// The [`AsyncFd::into_inner`] function can be used to extract the inner object
/// to retake control from the tokio IO reactor. The [`OwnedFd`] type is often
/// used as the inner object, as it is the simplest type that closes the fd on
/// drop.
///
/// The inner object is required to implement [`AsRawFd`]. This file descriptor
/// must not change while [`AsyncFd`] owns the inner object, i.e. the
/// [`AsRawFd::as_raw_fd`] method on the inner type must always return the same
/// file descriptor when called multiple times. Failure to uphold this results
/// in unspecified behavior in the IO driver, which may include breaking
/// notifications for other sockets/etc.
///
/// Polling for readiness is done by calling the async functions [`readable`]
/// and [`writable`]. These functions complete when the associated readiness
/// condition is observed. Any number of tasks can query the same `AsyncFd` in
/// parallel, on the same or different conditions.
///
/// On some platforms, the readiness detecting mechanism relies on
/// edge-triggered notifications. This means that the OS will only notify Tokio
/// when the file descriptor transitions from not-ready to ready. For this to
/// work you should first try to read or write and only poll for readiness
/// if that fails with an error of [`std::io::ErrorKind::WouldBlock`].
///
/// Tokio internally tracks when it has received a ready notification, and when
/// readiness checking functions like [`readable`] and [`writable`] are called,
/// if the readiness flag is set, these async functions will complete
/// immediately. This however does mean that it is critical to ensure that this
/// ready flag is cleared when (and only when) the file descriptor ceases to be
/// ready. The [`AsyncFdReadyGuard`] returned from readiness checking functions
/// serves this function; after calling a readiness-checking async function,
/// you must use this [`AsyncFdReadyGuard`] to signal to tokio whether the file
/// descriptor is no longer in a ready state.
///
/// ## Use with to a poll-based API
///
/// In some cases it may be desirable to use `AsyncFd` from APIs similar to
/// [`TcpStream::poll_read_ready`]. The [`AsyncFd::poll_read_ready`] and
/// [`AsyncFd::poll_write_ready`] functions are provided for this purpose.
/// Because these functions don't create a future to hold their state, they have
/// the limitation that only one task can wait on each direction (read or write)
/// at a time.
///
/// # Examples
///
/// This example shows how to turn [`std::net::TcpStream`] asynchronous using
/// `AsyncFd`.  It implements the read/write operations both as an `async fn`
/// and using the IO traits [`AsyncRead`] and [`AsyncWrite`].
///
/// ```no_run
/// use std::io::{self, Read, Write};
/// use std::net::TcpStream;
/// use std::pin::Pin;
/// use std::task::{ready, Context, Poll};
/// use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
/// use tokio::io::unix::AsyncFd;
///
/// pub struct AsyncTcpStream {
///     inner: AsyncFd<TcpStream>,
/// }
///
/// impl AsyncTcpStream {
///     pub fn new(tcp: TcpStream) -> io::Result<Self> {
///         tcp.set_nonblocking(true)?;
///         Ok(Self {
///             inner: AsyncFd::new(tcp)?,
///         })
///     }
///
///     pub async fn read(&self, out: &mut [u8]) -> io::Result<usize> {
///         loop {
///             let mut guard = self.inner.readable().await?;
///
///             match guard.try_io(|inner| inner.get_ref().read(out)) {
///                 Ok(result) => return result,
///                 Err(_would_block) => continue,
///             }
///         }
///     }
///
///     pub async fn write(&self, buf: &[u8]) -> io::Result<usize> {
///         loop {
///             let mut guard = self.inner.writable().await?;
///
///             match guard.try_io(|inner| inner.get_ref().write(buf)) {
///                 Ok(result) => return result,
///                 Err(_would_block) => continue,
///             }
///         }
///     }
/// }
///
/// impl AsyncRead for AsyncTcpStream {
///     fn poll_read(
///         self: Pin<&mut Self>,
///         cx: &mut Context<'_>,
///         buf: &mut ReadBuf<'_>
///     ) -> Poll<io::Result<()>> {
///         loop {
///             let mut guard = ready!(self.inner.poll_read_ready(cx))?;
///
///             let unfilled = buf.initialize_unfilled();
///             match guard.try_io(|inner| inner.get_ref().read(unfilled)) {
///                 Ok(Ok(len)) => {
///                     buf.advance(len);
///                     return Poll::Ready(Ok(()));
///                 },
///                 Ok(Err(err)) => return Poll::Ready(Err(err)),
///                 Err(_would_block) => continue,
///             }
///         }
///     }
/// }
///
/// impl AsyncWrite for AsyncTcpStream {
///     fn poll_write(
///         self: Pin<&mut Self>,
///         cx: &mut Context<'_>,
///         buf: &[u8]
///     ) -> Poll<io::Result<usize>> {
///         loop {
///             let mut guard = ready!(self.inner.poll_write_ready(cx))?;
///
///             match guard.try_io(|inner| inner.get_ref().write(buf)) {
///                 Ok(result) => return Poll::Ready(result),
///                 Err(_would_block) => continue,
///             }
///         }
///     }
///
///     fn poll_flush(
///         self: Pin<&mut Self>,
///         cx: &mut Context<'_>,
///     ) -> Poll<io::Result<()>> {
///         // tcp flush is a no-op
///         Poll::Ready(Ok(()))
///     }
///
///     fn poll_shutdown(
///         self: Pin<&mut Self>,
///         cx: &mut Context<'_>,
///     ) -> Poll<io::Result<()>> {
///         self.inner.get_ref().shutdown(std::net::Shutdown::Write)?;
///         Poll::Ready(Ok(()))
///     }
/// }
/// ```
///
/// [`readable`]: method@Self::readable
/// [`writable`]: method@Self::writable
/// [`AsyncFdReadyGuard`]: struct@self::AsyncFdReadyGuard
/// [`TcpStream::poll_read_ready`]: struct@crate::net::TcpStream
/// [`AsyncRead`]: trait@crate::io::AsyncRead
/// [`AsyncWrite`]: trait@crate::io::AsyncWrite
/// [`OwnedFd`]: struct@std::os::fd::OwnedFd
pub struct AsyncFd<T: AsRawFd> {
    registration: Registration,
    // The inner value is always present. the Option is required for `drop` and `into_inner`.
    // In all other methods `unwrap` is valid, and will never panic.
    inner: Option<T>,
}

/// Represents an IO-ready event detected on a particular file descriptor that
/// has not yet been acknowledged. This is a `must_use` structure to help ensure
/// that you do not forget to explicitly clear (or not clear) the event.
///
/// This type exposes an immutable reference to the underlying IO object.
#[must_use = "You must explicitly choose whether to clear the readiness state by calling a method on ReadyGuard"]
pub struct AsyncFdReadyGuard<'a, T: AsRawFd> {
    async_fd: &'a AsyncFd<T>,
    event: Option<ReadyEvent>,
}

/// Represents an IO-ready event detected on a particular file descriptor that
/// has not yet been acknowledged. This is a `must_use` structure to help ensure
/// that you do not forget to explicitly clear (or not clear) the event.
///
/// This type exposes a mutable reference to the underlying IO object.
#[must_use = "You must explicitly choose whether to clear the readiness state by calling a method on ReadyGuard"]
pub struct AsyncFdReadyMutGuard<'a, T: AsRawFd> {
    async_fd: &'a mut AsyncFd<T>,
    event: Option<ReadyEvent>,
}

impl<T: AsRawFd> AsyncFd<T> {
    /// Creates an [`AsyncFd`] backed by (and taking ownership of) an object
    /// implementing [`AsRawFd`]. The backing file descriptor is cached at the
    /// time of creation.
    ///
    /// Only configures the [`Interest::READABLE`] and [`Interest::WRITABLE`] interests. For more
    /// control, use [`AsyncFd::with_interest`].
    ///
    /// This method must be called in the context of a tokio runtime.
    ///
    /// # Panics
    ///
    /// This function panics if there is no current reactor set, or if the `rt`
    /// feature flag is not enabled.
    #[inline]
    #[track_caller]
    pub fn new(inner: T) -> io::Result<Self>
    where
        T: AsRawFd,
    {
        Self::with_interest(inner, Interest::READABLE | Interest::WRITABLE)
    }

    /// Creates an [`AsyncFd`] backed by (and taking ownership of) an object
    /// implementing [`AsRawFd`], with a specific [`Interest`]. The backing
    /// file descriptor is cached at the time of creation.
    ///
    /// # Panics
    ///
    /// This function panics if there is no current reactor set, or if the `rt`
    /// feature flag is not enabled.
    #[inline]
    #[track_caller]
    pub fn with_interest(inner: T, interest: Interest) -> io::Result<Self>
    where
        T: AsRawFd,
    {
        Self::new_with_handle_and_interest(inner, scheduler::Handle::current(), interest)
    }

    #[track_caller]
    pub(crate) fn new_with_handle_and_interest(
        inner: T,
        handle: scheduler::Handle,
        interest: Interest,
    ) -> io::Result<Self> {
        Self::try_new_with_handle_and_interest(inner, handle, interest).map_err(Into::into)
    }

    /// Creates an [`AsyncFd`] backed by (and taking ownership of) an object
    /// implementing [`AsRawFd`]. The backing file descriptor is cached at the
    /// time of creation.
    ///
    /// Only configures the [`Interest::READABLE`] and [`Interest::WRITABLE`] interests. For more
    /// control, use [`AsyncFd::try_with_interest`].
    ///
    /// This method must be called in the context of a tokio runtime.
    ///
    /// In the case of failure, it returns [`AsyncFdTryNewError`] that contains the original object
    /// passed to this function.
    ///
    /// # Panics
    ///
    /// This function panics if there is no current reactor set, or if the `rt`
    /// feature flag is not enabled.
    #[inline]
    #[track_caller]
    pub fn try_new(inner: T) -> Result<Self, AsyncFdTryNewError<T>>
    where
        T: AsRawFd,
    {
        Self::try_with_interest(inner, Interest::READABLE | Interest::WRITABLE)
    }

    /// Creates an [`AsyncFd`] backed by (and taking ownership of) an object
    /// implementing [`AsRawFd`], with a specific [`Interest`]. The backing
    /// file descriptor is cached at the time of creation.
    ///
    /// In the case of failure, it returns [`AsyncFdTryNewError`] that contains the original object
    /// passed to this function.
    ///
    /// # Panics
    ///
    /// This function panics if there is no current reactor set, or if the `rt`
    /// feature flag is not enabled.
    #[inline]
    #[track_caller]
    pub fn try_with_interest(inner: T, interest: Interest) -> Result<Self, AsyncFdTryNewError<T>>
    where
        T: AsRawFd,
    {
        Self::try_new_with_handle_and_interest(inner, scheduler::Handle::current(), interest)
    }

    #[track_caller]
    pub(crate) fn try_new_with_handle_and_interest(
        inner: T,
        handle: scheduler::Handle,
        interest: Interest,
    ) -> Result<Self, AsyncFdTryNewError<T>> {
        let fd = inner.as_raw_fd();

        match Registration::new_with_interest_and_handle(&mut SourceFd(&fd), interest, handle) {
            Ok(registration) => Ok(AsyncFd {
                registration,
                inner: Some(inner),
            }),
            Err(cause) => Err(AsyncFdTryNewError { inner, cause }),
        }
    }

    /// Returns a shared reference to the backing object of this [`AsyncFd`].
    #[inline]
    pub fn get_ref(&self) -> &T {
        self.inner.as_ref().unwrap()
    }

    /// Returns a mutable reference to the backing object of this [`AsyncFd`].
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.as_mut().unwrap()
    }

    fn take_inner(&mut self) -> Option<T> {
        let inner = self.inner.take()?;
        let fd = inner.as_raw_fd();

        let _ = self.registration.deregister(&mut SourceFd(&fd));

        Some(inner)
    }

    /// Deregisters this file descriptor and returns ownership of the backing
    /// object.
    pub fn into_inner(mut self) -> T {
        self.take_inner().unwrap()
    }

    /// Polls for read readiness.
    ///
    /// If the file descriptor is not currently ready for reading, this method
    /// will store a clone of the [`Waker`] from the provided [`Context`]. When the
    /// file descriptor becomes ready for reading, [`Waker::wake`] will be called.
    ///
    /// Note that on multiple calls to [`poll_read_ready`] or
    /// [`poll_read_ready_mut`], only the `Waker` from the `Context` passed to the
    /// most recent call is scheduled to receive a wakeup. (However,
    /// [`poll_write_ready`] retains a second, independent waker).
    ///
    /// This method is intended for cases where creating and pinning a future
    /// via [`readable`] is not feasible. Where possible, using [`readable`] is
    /// preferred, as this supports polling from multiple tasks at once.
    ///
    /// This method takes `&self`, so it is possible to call this method
    /// concurrently with other methods on this struct. This method only
    /// provides shared access to the inner IO resource when handling the
    /// [`AsyncFdReadyGuard`].
    ///
    /// [`poll_read_ready`]: method@Self::poll_read_ready
    /// [`poll_read_ready_mut`]: method@Self::poll_read_ready_mut
    /// [`poll_write_ready`]: method@Self::poll_write_ready
    /// [`readable`]: method@Self::readable
    /// [`Context`]: struct@std::task::Context
    /// [`Waker`]: struct@std::task::Waker
    /// [`Waker::wake`]: method@std::task::Waker::wake
    pub fn poll_read_ready<'a>(
        &'a self,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<AsyncFdReadyGuard<'a, T>>> {
        let event = ready!(self.registration.poll_read_ready(cx))?;

        Poll::Ready(Ok(AsyncFdReadyGuard {
            async_fd: self,
            event: Some(event),
        }))
    }

    /// Polls for read readiness.
    ///
    /// If the file descriptor is not currently ready for reading, this method
    /// will store a clone of the [`Waker`] from the provided [`Context`]. When the
    /// file descriptor becomes ready for reading, [`Waker::wake`] will be called.
    ///
    /// Note that on multiple calls to [`poll_read_ready`] or
    /// [`poll_read_ready_mut`], only the `Waker` from the `Context` passed to the
    /// most recent call is scheduled to receive a wakeup. (However,
    /// [`poll_write_ready`] retains a second, independent waker).
    ///
    /// This method is intended for cases where creating and pinning a future
    /// via [`readable`] is not feasible. Where possible, using [`readable`] is
    /// preferred, as this supports polling from multiple tasks at once.
    ///
    /// This method takes `&mut self`, so it is possible to access the inner IO
    /// resource mutably when handling the [`AsyncFdReadyMutGuard`].
    ///
    /// [`poll_read_ready`]: method@Self::poll_read_ready
    /// [`poll_read_ready_mut`]: method@Self::poll_read_ready_mut
    /// [`poll_write_ready`]: method@Self::poll_write_ready
    /// [`readable`]: method@Self::readable
    /// [`Context`]: struct@std::task::Context
    /// [`Waker`]: struct@std::task::Waker
    /// [`Waker::wake`]: method@std::task::Waker::wake
    pub fn poll_read_ready_mut<'a>(
        &'a mut self,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<AsyncFdReadyMutGuard<'a, T>>> {
        let event = ready!(self.registration.poll_read_ready(cx))?;

        Poll::Ready(Ok(AsyncFdReadyMutGuard {
            async_fd: self,
            event: Some(event),
        }))
    }

    /// Polls for write readiness.
    ///
    /// If the file descriptor is not currently ready for writing, this method
    /// will store a clone of the [`Waker`] from the provided [`Context`]. When the
    /// file descriptor becomes ready for writing, [`Waker::wake`] will be called.
    ///
    /// Note that on multiple calls to [`poll_write_ready`] or
    /// [`poll_write_ready_mut`], only the `Waker` from the `Context` passed to the
    /// most recent call is scheduled to receive a wakeup. (However,
    /// [`poll_read_ready`] retains a second, independent waker).
    ///
    /// This method is intended for cases where creating and pinning a future
    /// via [`writable`] is not feasible. Where possible, using [`writable`] is
    /// preferred, as this supports polling from multiple tasks at once.
    ///
    /// This method takes `&self`, so it is possible to call this method
    /// concurrently with other methods on this struct. This method only
    /// provides shared access to the inner IO resource when handling the
    /// [`AsyncFdReadyGuard`].
    ///
    /// [`poll_read_ready`]: method@Self::poll_read_ready
    /// [`poll_write_ready`]: method@Self::poll_write_ready
    /// [`poll_write_ready_mut`]: method@Self::poll_write_ready_mut
    /// [`writable`]: method@Self::readable
    /// [`Context`]: struct@std::task::Context
    /// [`Waker`]: struct@std::task::Waker
    /// [`Waker::wake`]: method@std::task::Waker::wake
    pub fn poll_write_ready<'a>(
        &'a self,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<AsyncFdReadyGuard<'a, T>>> {
        let event = ready!(self.registration.poll_write_ready(cx))?;

        Poll::Ready(Ok(AsyncFdReadyGuard {
            async_fd: self,
            event: Some(event),
        }))
    }

    /// Polls for write readiness.
    ///
    /// If the file descriptor is not currently ready for writing, this method
    /// will store a clone of the [`Waker`] from the provided [`Context`]. When the
    /// file descriptor becomes ready for writing, [`Waker::wake`] will be called.
    ///
    /// Note that on multiple calls to [`poll_write_ready`] or
    /// [`poll_write_ready_mut`], only the `Waker` from the `Context` passed to the
    /// most recent call is scheduled to receive a wakeup. (However,
    /// [`poll_read_ready`] retains a second, independent waker).
    ///
    /// This method is intended for cases where creating and pinning a future
    /// via [`writable`] is not feasible. Where possible, using [`writable`] is
    /// preferred, as this supports polling from multiple tasks at once.
    ///
    /// This method takes `&mut self`, so it is possible to access the inner IO
    /// resource mutably when handling the [`AsyncFdReadyMutGuard`].
    ///
    /// [`poll_read_ready`]: method@Self::poll_read_ready
    /// [`poll_write_ready`]: method@Self::poll_write_ready
    /// [`poll_write_ready_mut`]: method@Self::poll_write_ready_mut
    /// [`writable`]: method@Self::readable
    /// [`Context`]: struct@std::task::Context
    /// [`Waker`]: struct@std::task::Waker
    /// [`Waker::wake`]: method@std::task::Waker::wake
    pub fn poll_write_ready_mut<'a>(
        &'a mut self,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<AsyncFdReadyMutGuard<'a, T>>> {
        let event = ready!(self.registration.poll_write_ready(cx))?;

        Poll::Ready(Ok(AsyncFdReadyMutGuard {
            async_fd: self,
            event: Some(event),
        }))
    }

    /// Waits for any of the requested ready states, returning a
    /// [`AsyncFdReadyGuard`] that must be dropped to resume
    /// polling for the requested ready states.
    ///
    /// The function may complete without the file descriptor being ready. This is a
    /// false-positive and attempting an operation will return with
    /// `io::ErrorKind::WouldBlock`. The function can also return with an empty
    /// [`Ready`] set, so you should always check the returned value and possibly
    /// wait again if the requested states are not set.
    ///
    /// When an IO operation does return `io::ErrorKind::WouldBlock`, the readiness must be cleared.
    /// When a combined interest is used, it is important to clear only the readiness
    /// that is actually observed to block. For instance when the combined
    /// interest `Interest::READABLE | Interest::WRITABLE` is used, and a read blocks, only
    /// read readiness should be cleared using the [`AsyncFdReadyGuard::clear_ready_matching`] method:
    /// `guard.clear_ready_matching(Ready::READABLE)`.
    /// Also clearing the write readiness in this case would be incorrect. The [`AsyncFdReadyGuard::clear_ready`]
    /// method clears all readiness flags.
    ///
    /// This method takes `&self`, so it is possible to call this method
    /// concurrently with other methods on this struct. This method only
    /// provides shared access to the inner IO resource when handling the
    /// [`AsyncFdReadyGuard`].
    ///
    /// # Examples
    ///
    /// Concurrently read and write to a [`std::net::TcpStream`] on the same task without
    /// splitting.
    ///
    /// ```no_run
    /// use std::error::Error;
    /// use std::io;
    /// use std::io::{Read, Write};
    /// use std::net::TcpStream;
    /// use tokio::io::unix::AsyncFd;
    /// use tokio::io::{Interest, Ready};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let stream = TcpStream::connect("127.0.0.1:8080")?;
    ///     stream.set_nonblocking(true)?;
    ///     let stream = AsyncFd::new(stream)?;
    ///
    ///     loop {
    ///         let mut guard = stream
    ///             .ready(Interest::READABLE | Interest::WRITABLE)
    ///             .await?;
    ///
    ///         if guard.ready().is_readable() {
    ///             let mut data = vec![0; 1024];
    ///             // Try to read data, this may still fail with `WouldBlock`
    ///             // if the readiness event is a false positive.
    ///             match stream.get_ref().read(&mut data) {
    ///                 Ok(n) => {
    ///                     println!("read {} bytes", n);
    ///                 }
    ///                 Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                     // a read has blocked, but a write might still succeed.
    ///                     // clear only the read readiness.
    ///                     guard.clear_ready_matching(Ready::READABLE);
    ///                     continue;
    ///                 }
    ///                 Err(e) => {
    ///                     return Err(e.into());
    ///                 }
    ///             }
    ///         }
    ///
    ///         if guard.ready().is_writable() {
    ///             // Try to write data, this may still fail with `WouldBlock`
    ///             // if the readiness event is a false positive.
    ///             match stream.get_ref().write(b"hello world") {
    ///                 Ok(n) => {
    ///                     println!("write {} bytes", n);
    ///                 }
    ///                 Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                     // a write has blocked, but a read might still succeed.
    ///                     // clear only the write readiness.
    ///                     guard.clear_ready_matching(Ready::WRITABLE);
    ///                     continue;
    ///                 }
    ///                 Err(e) => {
    ///                     return Err(e.into());
    ///                 }
    ///             }
    ///         }
    ///     }
    /// }
    /// ```
    pub async fn ready(&self, interest: Interest) -> io::Result<AsyncFdReadyGuard<'_, T>> {
        let event = self.registration.readiness(interest).await?;

        Ok(AsyncFdReadyGuard {
            async_fd: self,
            event: Some(event),
        })
    }

    /// Waits for any of the requested ready states, returning a
    /// [`AsyncFdReadyMutGuard`] that must be dropped to resume
    /// polling for the requested ready states.
    ///
    /// The function may complete without the file descriptor being ready. This is a
    /// false-positive and attempting an operation will return with
    /// `io::ErrorKind::WouldBlock`. The function can also return with an empty
    /// [`Ready`] set, so you should always check the returned value and possibly
    /// wait again if the requested states are not set.
    ///
    /// When an IO operation does return `io::ErrorKind::WouldBlock`, the readiness must be cleared.
    /// When a combined interest is used, it is important to clear only the readiness
    /// that is actually observed to block. For instance when the combined
    /// interest `Interest::READABLE | Interest::WRITABLE` is used, and a read blocks, only
    /// read readiness should be cleared using the [`AsyncFdReadyMutGuard::clear_ready_matching`] method:
    /// `guard.clear_ready_matching(Ready::READABLE)`.
    /// Also clearing the write readiness in this case would be incorrect.
    /// The [`AsyncFdReadyMutGuard::clear_ready`] method clears all readiness flags.
    ///
    /// This method takes `&mut self`, so it is possible to access the inner IO
    /// resource mutably when handling the [`AsyncFdReadyMutGuard`].
    ///
    /// # Examples
    ///
    /// Concurrently read and write to a [`std::net::TcpStream`] on the same task without
    /// splitting.
    ///
    /// ```no_run
    /// use std::error::Error;
    /// use std::io;
    /// use std::io::{Read, Write};
    /// use std::net::TcpStream;
    /// use tokio::io::unix::AsyncFd;
    /// use tokio::io::{Interest, Ready};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let stream = TcpStream::connect("127.0.0.1:8080")?;
    ///     stream.set_nonblocking(true)?;
    ///     let mut stream = AsyncFd::new(stream)?;
    ///
    ///     loop {
    ///         let mut guard = stream
    ///             .ready_mut(Interest::READABLE | Interest::WRITABLE)
    ///             .await?;
    ///
    ///         if guard.ready().is_readable() {
    ///             let mut data = vec![0; 1024];
    ///             // Try to read data, this may still fail with `WouldBlock`
    ///             // if the readiness event is a false positive.
    ///             match guard.get_inner_mut().read(&mut data) {
    ///                 Ok(n) => {
    ///                     println!("read {} bytes", n);
    ///                 }
    ///                 Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                     // a read has blocked, but a write might still succeed.
    ///                     // clear only the read readiness.
    ///                     guard.clear_ready_matching(Ready::READABLE);
    ///                     continue;
    ///                 }
    ///                 Err(e) => {
    ///                     return Err(e.into());
    ///                 }
    ///             }
    ///         }
    ///
    ///         if guard.ready().is_writable() {
    ///             // Try to write data, this may still fail with `WouldBlock`
    ///             // if the readiness event is a false positive.
    ///             match guard.get_inner_mut().write(b"hello world") {
    ///                 Ok(n) => {
    ///                     println!("write {} bytes", n);
    ///                 }
    ///                 Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                     // a write has blocked, but a read might still succeed.
    ///                     // clear only the write readiness.
    ///                     guard.clear_ready_matching(Ready::WRITABLE);
    ///                     continue;
    ///                 }
    ///                 Err(e) => {
    ///                     return Err(e.into());
    ///                 }
    ///             }
    ///         }
    ///     }
    /// }
    /// ```
    pub async fn ready_mut(
        &mut self,
        interest: Interest,
    ) -> io::Result<AsyncFdReadyMutGuard<'_, T>> {
        let event = self.registration.readiness(interest).await?;

        Ok(AsyncFdReadyMutGuard {
            async_fd: self,
            event: Some(event),
        })
    }

    /// Waits for the file descriptor to become readable, returning a
    /// [`AsyncFdReadyGuard`] that must be dropped to resume read-readiness
    /// polling.
    ///
    /// This method takes `&self`, so it is possible to call this method
    /// concurrently with other methods on this struct. This method only
    /// provides shared access to the inner IO resource when handling the
    /// [`AsyncFdReadyGuard`].
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. Once a readiness event occurs, the method
    /// will continue to return immediately until the readiness event is
    /// consumed by an attempt to read or write that fails with `WouldBlock` or
    /// `Poll::Pending`.
    #[allow(clippy::needless_lifetimes)] // The lifetime improves rustdoc rendering.
    pub async fn readable<'a>(&'a self) -> io::Result<AsyncFdReadyGuard<'a, T>> {
        self.ready(Interest::READABLE).await
    }

    /// Waits for the file descriptor to become readable, returning a
    /// [`AsyncFdReadyMutGuard`] that must be dropped to resume read-readiness
    /// polling.
    ///
    /// This method takes `&mut self`, so it is possible to access the inner IO
    /// resource mutably when handling the [`AsyncFdReadyMutGuard`].
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. Once a readiness event occurs, the method
    /// will continue to return immediately until the readiness event is
    /// consumed by an attempt to read or write that fails with `WouldBlock` or
    /// `Poll::Pending`.
    #[allow(clippy::needless_lifetimes)] // The lifetime improves rustdoc rendering.
    pub async fn readable_mut<'a>(&'a mut self) -> io::Result<AsyncFdReadyMutGuard<'a, T>> {
        self.ready_mut(Interest::READABLE).await
    }

    /// Waits for the file descriptor to become writable, returning a
    /// [`AsyncFdReadyGuard`] that must be dropped to resume write-readiness
    /// polling.
    ///
    /// This method takes `&self`, so it is possible to call this method
    /// concurrently with other methods on this struct. This method only
    /// provides shared access to the inner IO resource when handling the
    /// [`AsyncFdReadyGuard`].
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. Once a readiness event occurs, the method
    /// will continue to return immediately until the readiness event is
    /// consumed by an attempt to read or write that fails with `WouldBlock` or
    /// `Poll::Pending`.
    #[allow(clippy::needless_lifetimes)] // The lifetime improves rustdoc rendering.
    pub async fn writable<'a>(&'a self) -> io::Result<AsyncFdReadyGuard<'a, T>> {
        self.ready(Interest::WRITABLE).await
    }

    /// Waits for the file descriptor to become writable, returning a
    /// [`AsyncFdReadyMutGuard`] that must be dropped to resume write-readiness
    /// polling.
    ///
    /// This method takes `&mut self`, so it is possible to access the inner IO
    /// resource mutably when handling the [`AsyncFdReadyMutGuard`].
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. Once a readiness event occurs, the method
    /// will continue to return immediately until the readiness event is
    /// consumed by an attempt to read or write that fails with `WouldBlock` or
    /// `Poll::Pending`.
    #[allow(clippy::needless_lifetimes)] // The lifetime improves rustdoc rendering.
    pub async fn writable_mut<'a>(&'a mut self) -> io::Result<AsyncFdReadyMutGuard<'a, T>> {
        self.ready_mut(Interest::WRITABLE).await
    }

    /// Reads or writes from the file descriptor using a user-provided IO operation.
    ///
    /// The `async_io` method is a convenience utility that waits for the file
    /// descriptor to become ready, and then executes the provided IO operation.
    /// Since file descriptors may be marked ready spuriously, the closure will
    /// be called repeatedly until it returns something other than a
    /// [`WouldBlock`] error. This is done using the following loop:
    ///
    /// ```no_run
    /// # use std::io::{self, Result};
    /// # struct Dox<T> { inner: T }
    /// # impl<T> Dox<T> {
    /// #     async fn writable(&self) -> Result<&Self> {
    /// #         Ok(self)
    /// #     }
    /// #     fn try_io<R>(&self, _: impl FnMut(&T) -> Result<R>) -> Result<Result<R>> {
    /// #         panic!()
    /// #     }
    /// async fn async_io<R>(&self, mut f: impl FnMut(&T) -> io::Result<R>) -> io::Result<R> {
    ///     loop {
    ///         // or `readable` if called with the read interest.
    ///         let guard = self.writable().await?;
    ///
    ///         match guard.try_io(&mut f) {
    ///             Ok(result) => return result,
    ///             Err(_would_block) => continue,
    ///         }
    ///     }
    /// }
    /// # }
    /// ```
    ///
    /// The closure should only return a [`WouldBlock`] error if it has performed
    /// an IO operation on the file descriptor that failed due to the file descriptor not being
    /// ready. Returning a [`WouldBlock`] error in any other situation will
    /// incorrectly clear the readiness flag, which can cause the file descriptor to
    /// behave incorrectly.
    ///
    /// The closure should not perform the IO operation using any of the methods
    /// defined on the Tokio [`AsyncFd`] type, as this will mess with the
    /// readiness flag and can cause the file descriptor to behave incorrectly.
    ///
    /// This method is not intended to be used with combined interests.
    /// The closure should perform only one type of IO operation, so it should not
    /// require more than one ready state. This method may panic or sleep forever
    /// if it is called with a combined interest.
    ///
    /// # Examples
    ///
    /// This example sends some bytes on the inner [`std::net::UdpSocket`]. The `async_io`
    /// method waits for readiness, and retries if the send operation does block. This example
    /// is equivalent to the one given for [`try_io`].
    ///
    /// ```no_run
    /// use tokio::io::{Interest, unix::AsyncFd};
    ///
    /// use std::io;
    /// use std::net::UdpSocket;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     let socket = UdpSocket::bind("0.0.0.0:8080")?;
    ///     socket.set_nonblocking(true)?;
    ///     let async_fd = AsyncFd::new(socket)?;
    ///
    ///     let written = async_fd
    ///         .async_io(Interest::WRITABLE, |inner| inner.send(&[1, 2]))
    ///         .await?;
    ///
    ///     println!("wrote {written} bytes");
    ///
    ///     Ok(())
    /// }
    /// ```
    ///
    /// [`try_io`]: AsyncFdReadyGuard::try_io
    /// [`WouldBlock`]: std::io::ErrorKind::WouldBlock
    pub async fn async_io<R>(
        &self,
        interest: Interest,
        mut f: impl FnMut(&T) -> io::Result<R>,
    ) -> io::Result<R> {
        self.registration
            .async_io(interest, || f(self.get_ref()))
            .await
    }

    /// Reads or writes from the file descriptor using a user-provided IO operation.
    ///
    /// The behavior is the same as [`async_io`], except that the closure can mutate the inner
    /// value of the [`AsyncFd`].
    ///
    /// [`async_io`]: AsyncFd::async_io
    pub async fn async_io_mut<R>(
        &mut self,
        interest: Interest,
        mut f: impl FnMut(&mut T) -> io::Result<R>,
    ) -> io::Result<R> {
        self.registration
            .async_io(interest, || f(self.inner.as_mut().unwrap()))
            .await
    }

    /// Tries to read or write from the file descriptor using a user-provided IO operation.
    ///
    /// If the file descriptor is ready, the provided closure is called. The closure
    /// should attempt to perform IO operation on the file descriptor by manually
    /// calling the appropriate syscall. If the operation fails because the
    /// file descriptor is not actually ready, then the closure should return a
    /// `WouldBlock` error and the readiness flag is cleared. The return value
    /// of the closure is then returned by `try_io`.
    ///
    /// If the file descriptor is not ready, then the closure is not called
    /// and a `WouldBlock` error is returned.
    ///
    /// The closure should only return a `WouldBlock` error if it has performed
    /// an IO operation on the file descriptor that failed due to the file descriptor not being
    /// ready. Returning a `WouldBlock` error in any other situation will
    /// incorrectly clear the readiness flag, which can cause the file descriptor to
    /// behave incorrectly.
    ///
    /// The closure should not perform the IO operation using any of the methods
    /// defined on the Tokio `AsyncFd` type, as this will mess with the
    /// readiness flag and can cause the file descriptor to behave incorrectly.
    ///
    /// This method is not intended to be used with combined interests.
    /// The closure should perform only one type of IO operation, so it should not
    /// require more than one ready state. This method may panic or sleep forever
    /// if it is called with a combined interest.
    pub fn try_io<R>(
        &self,
        interest: Interest,
        f: impl FnOnce(&T) -> io::Result<R>,
    ) -> io::Result<R> {
        self.registration
            .try_io(interest, || f(self.inner.as_ref().unwrap()))
    }

    /// Tries to read or write from the file descriptor using a user-provided IO operation.
    ///
    /// The behavior is the same as [`try_io`], except that the closure can mutate the inner
    /// value of the [`AsyncFd`].
    ///
    /// [`try_io`]: AsyncFd::try_io
    pub fn try_io_mut<R>(
        &mut self,
        interest: Interest,
        f: impl FnOnce(&mut T) -> io::Result<R>,
    ) -> io::Result<R> {
        self.registration
            .try_io(interest, || f(self.inner.as_mut().unwrap()))
    }
}

impl<T: AsRawFd> AsRawFd for AsyncFd<T> {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_ref().unwrap().as_raw_fd()
    }
}

impl<T: AsRawFd> std::os::unix::io::AsFd for AsyncFd<T> {
    fn as_fd(&self) -> std::os::unix::io::BorrowedFd<'_> {
        unsafe { std::os::unix::io::BorrowedFd::borrow_raw(self.as_raw_fd()) }
    }
}

impl<T: std::fmt::Debug + AsRawFd> std::fmt::Debug for AsyncFd<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncFd")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<T: AsRawFd> Drop for AsyncFd<T> {
    fn drop(&mut self) {
        let _ = self.take_inner();
    }
}

impl<'a, Inner: AsRawFd> AsyncFdReadyGuard<'a, Inner> {
    /// Indicates to tokio that the file descriptor is no longer ready. All
    /// internal readiness flags will be cleared, and tokio will wait for the
    /// next edge-triggered readiness notification from the OS.
    ///
    /// This function is commonly used with guards returned by [`AsyncFd::readable`] and
    /// [`AsyncFd::writable`].
    ///
    /// It is critical that this function not be called unless your code
    /// _actually observes_ that the file descriptor is _not_ ready. Do not call
    /// it simply because, for example, a read succeeded; it should be called
    /// when a read is observed to block.
    ///
    /// This method only clears readiness events that happened before the creation of this guard.
    /// In other words, if the IO resource becomes ready between the creation of the guard and
    /// this call to `clear_ready`, then the readiness is not actually cleared.
    pub fn clear_ready(&mut self) {
        if let Some(event) = self.event.take() {
            self.async_fd.registration.clear_readiness(event);
        }
    }

    /// Indicates to tokio that the file descriptor no longer has a specific readiness.
    /// The internal readiness flag will be cleared, and tokio will wait for the
    /// next edge-triggered readiness notification from the OS.
    ///
    /// This function is useful in combination with the [`AsyncFd::ready`] method when a
    /// combined interest like `Interest::READABLE | Interest::WRITABLE` is used.
    ///
    /// It is critical that this function not be called unless your code
    /// _actually observes_ that the file descriptor is _not_ ready for the provided `Ready`.
    /// Do not call it simply because, for example, a read succeeded; it should be called
    /// when a read is observed to block. Only clear the specific readiness that is observed to
    /// block. For example when a read blocks when using a combined interest,
    /// only clear `Ready::READABLE`.
    ///
    /// This method only clears readiness events that happened before the creation of this guard.
    /// In other words, if the IO resource becomes ready between the creation of the guard and
    /// this call to `clear_ready`, then the readiness is not actually cleared.
    ///
    /// # Examples
    ///
    /// Concurrently read and write to a [`std::net::TcpStream`] on the same task without
    /// splitting.
    ///
    /// ```no_run
    /// use std::error::Error;
    /// use std::io;
    /// use std::io::{Read, Write};
    /// use std::net::TcpStream;
    /// use tokio::io::unix::AsyncFd;
    /// use tokio::io::{Interest, Ready};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let stream = TcpStream::connect("127.0.0.1:8080")?;
    ///     stream.set_nonblocking(true)?;
    ///     let stream = AsyncFd::new(stream)?;
    ///
    ///     loop {
    ///         let mut guard = stream
    ///             .ready(Interest::READABLE | Interest::WRITABLE)
    ///             .await?;
    ///
    ///         if guard.ready().is_readable() {
    ///             let mut data = vec![0; 1024];
    ///             // Try to read data, this may still fail with `WouldBlock`
    ///             // if the readiness event is a false positive.
    ///             match stream.get_ref().read(&mut data) {
    ///                 Ok(n) => {
    ///                     println!("read {} bytes", n);
    ///                 }
    ///                 Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                     // a read has blocked, but a write might still succeed.
    ///                     // clear only the read readiness.
    ///                     guard.clear_ready_matching(Ready::READABLE);
    ///                     continue;
    ///                 }
    ///                 Err(e) => {
    ///                     return Err(e.into());
    ///                 }
    ///             }
    ///         }
    ///
    ///         if guard.ready().is_writable() {
    ///             // Try to write data, this may still fail with `WouldBlock`
    ///             // if the readiness event is a false positive.
    ///             match stream.get_ref().write(b"hello world") {
    ///                 Ok(n) => {
    ///                     println!("write {} bytes", n);
    ///                 }
    ///                 Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                     // a write has blocked, but a read might still succeed.
    ///                     // clear only the write readiness.
    ///                     guard.clear_ready_matching(Ready::WRITABLE);
    ///                     continue;
    ///                 }
    ///                 Err(e) => {
    ///                     return Err(e.into());
    ///                 }
    ///             }
    ///         }
    ///     }
    /// }
    /// ```
    pub fn clear_ready_matching(&mut self, ready: Ready) {
        if let Some(mut event) = self.event.take() {
            self.async_fd
                .registration
                .clear_readiness(event.with_ready(ready));

            // the event is no longer ready for the readiness that was just cleared
            event.ready = event.ready - ready;

            if !event.ready.is_empty() {
                self.event = Some(event);
            }
        }
    }

    /// This method should be invoked when you intentionally want to keep the
    /// ready flag asserted.
    ///
    /// While this function is itself a no-op, it satisfies the `#[must_use]`
    /// constraint on the [`AsyncFdReadyGuard`] type.
    pub fn retain_ready(&mut self) {
        // no-op
    }

    /// Get the [`Ready`] value associated with this guard.
    ///
    /// This method will return the empty readiness state if
    /// [`AsyncFdReadyGuard::clear_ready`] has been called on
    /// the guard.
    ///
    /// [`Ready`]: crate::io::Ready
    pub fn ready(&self) -> Ready {
        match &self.event {
            Some(event) => event.ready,
            None => Ready::EMPTY,
        }
    }

    /// Performs the provided IO operation.
    ///
    /// If `f` returns a [`WouldBlock`] error, the readiness state associated
    /// with this file descriptor is cleared, and the method returns
    /// `Err(TryIoError::WouldBlock)`. You will typically need to poll the
    /// `AsyncFd` again when this happens.
    ///
    /// This method helps ensure that the readiness state of the underlying file
    /// descriptor remains in sync with the tokio-side readiness state, by
    /// clearing the tokio-side state only when a [`WouldBlock`] condition
    /// occurs. It is the responsibility of the caller to ensure that `f`
    /// returns [`WouldBlock`] only if the file descriptor that originated this
    /// `AsyncFdReadyGuard` no longer expresses the readiness state that was queried to
    /// create this `AsyncFdReadyGuard`.
    ///
    /// # Examples
    ///
    /// This example sends some bytes to the inner [`std::net::UdpSocket`]. Waiting
    /// for write-readiness and retrying when the send operation does block are explicit.
    /// This example can be written more succinctly using [`AsyncFd::async_io`].
    ///
    /// ```no_run
    /// use tokio::io::unix::AsyncFd;
    ///
    /// use std::io;
    /// use std::net::UdpSocket;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     let socket = UdpSocket::bind("0.0.0.0:8080")?;
    ///     socket.set_nonblocking(true)?;
    ///     let async_fd = AsyncFd::new(socket)?;
    ///
    ///     let written = loop {
    ///         let mut guard = async_fd.writable().await?;
    ///         match guard.try_io(|inner| inner.get_ref().send(&[1, 2])) {
    ///             Ok(result) => {
    ///                 break result?;
    ///             }
    ///             Err(_would_block) => {
    ///                 // try_io already cleared the file descriptor's readiness state
    ///                 continue;
    ///             }
    ///         }
    ///     };
    ///
    ///     println!("wrote {written} bytes");
    ///
    ///     Ok(())
    /// }
    /// ```
    ///
    /// [`WouldBlock`]: std::io::ErrorKind::WouldBlock
    // Alias for old name in 0.x
    #[cfg_attr(docsrs, doc(alias = "with_io"))]
    pub fn try_io<R>(
        &mut self,
        f: impl FnOnce(&'a AsyncFd<Inner>) -> io::Result<R>,
    ) -> Result<io::Result<R>, TryIoError> {
        let result = f(self.async_fd);

        match result {
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                self.clear_ready();
                Err(TryIoError(()))
            }
            result => Ok(result),
        }
    }

    /// Returns a shared reference to the inner [`AsyncFd`].
    pub fn get_ref(&self) -> &'a AsyncFd<Inner> {
        self.async_fd
    }

    /// Returns a shared reference to the backing object of the inner [`AsyncFd`].
    pub fn get_inner(&self) -> &'a Inner {
        self.get_ref().get_ref()
    }
}

impl<'a, Inner: AsRawFd> AsyncFdReadyMutGuard<'a, Inner> {
    /// Indicates to tokio that the file descriptor is no longer ready. All
    /// internal readiness flags will be cleared, and tokio will wait for the
    /// next edge-triggered readiness notification from the OS.
    ///
    /// This function is commonly used with guards returned by [`AsyncFd::readable_mut`] and
    /// [`AsyncFd::writable_mut`].
    ///
    /// It is critical that this function not be called unless your code
    /// _actually observes_ that the file descriptor is _not_ ready. Do not call
    /// it simply because, for example, a read succeeded; it should be called
    /// when a read is observed to block.
    ///
    /// This method only clears readiness events that happened before the creation of this guard.
    /// In other words, if the IO resource becomes ready between the creation of the guard and
    /// this call to `clear_ready`, then the readiness is not actually cleared.
    pub fn clear_ready(&mut self) {
        if let Some(event) = self.event.take() {
            self.async_fd.registration.clear_readiness(event);
        }
    }

    /// Indicates to tokio that the file descriptor no longer has a specific readiness.
    /// The internal readiness flag will be cleared, and tokio will wait for the
    /// next edge-triggered readiness notification from the OS.
    ///
    /// This function is useful in combination with the [`AsyncFd::ready_mut`] method when a
    /// combined interest like `Interest::READABLE | Interest::WRITABLE` is used.
    ///
    /// It is critical that this function not be called unless your code
    /// _actually observes_ that the file descriptor is _not_ ready for the provided `Ready`.
    /// Do not call it simply because, for example, a read succeeded; it should be called
    /// when a read is observed to block. Only clear the specific readiness that is observed to
    /// block. For example when a read blocks when using a combined interest,
    /// only clear `Ready::READABLE`.
    ///
    /// This method only clears readiness events that happened before the creation of this guard.
    /// In other words, if the IO resource becomes ready between the creation of the guard and
    /// this call to `clear_ready`, then the readiness is not actually cleared.
    ///
    /// # Examples
    ///
    /// Concurrently read and write to a [`std::net::TcpStream`] on the same task without
    /// splitting.
    ///
    /// ```no_run
    /// use std::error::Error;
    /// use std::io;
    /// use std::io::{Read, Write};
    /// use std::net::TcpStream;
    /// use tokio::io::unix::AsyncFd;
    /// use tokio::io::{Interest, Ready};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let stream = TcpStream::connect("127.0.0.1:8080")?;
    ///     stream.set_nonblocking(true)?;
    ///     let mut stream = AsyncFd::new(stream)?;
    ///
    ///     loop {
    ///         let mut guard = stream
    ///             .ready_mut(Interest::READABLE | Interest::WRITABLE)
    ///             .await?;
    ///
    ///         if guard.ready().is_readable() {
    ///             let mut data = vec![0; 1024];
    ///             // Try to read data, this may still fail with `WouldBlock`
    ///             // if the readiness event is a false positive.
    ///             match guard.get_inner_mut().read(&mut data) {
    ///                 Ok(n) => {
    ///                     println!("read {} bytes", n);
    ///                 }
    ///                 Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                     // a read has blocked, but a write might still succeed.
    ///                     // clear only the read readiness.
    ///                     guard.clear_ready_matching(Ready::READABLE);
    ///                     continue;
    ///                 }
    ///                 Err(e) => {
    ///                     return Err(e.into());
    ///                 }
    ///             }
    ///         }
    ///
    ///         if guard.ready().is_writable() {
    ///             // Try to write data, this may still fail with `WouldBlock`
    ///             // if the readiness event is a false positive.
    ///             match guard.get_inner_mut().write(b"hello world") {
    ///                 Ok(n) => {
    ///                     println!("write {} bytes", n);
    ///                 }
    ///                 Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                     // a write has blocked, but a read might still succeed.
    ///                     // clear only the write readiness.
    ///                     guard.clear_ready_matching(Ready::WRITABLE);
    ///                     continue;
    ///                 }
    ///                 Err(e) => {
    ///                     return Err(e.into());
    ///                 }
    ///             }
    ///         }
    ///     }
    /// }
    /// ```
    pub fn clear_ready_matching(&mut self, ready: Ready) {
        if let Some(mut event) = self.event.take() {
            self.async_fd
                .registration
                .clear_readiness(event.with_ready(ready));

            // the event is no longer ready for the readiness that was just cleared
            event.ready = event.ready - ready;

            if !event.ready.is_empty() {
                self.event = Some(event);
            }
        }
    }

    /// This method should be invoked when you intentionally want to keep the
    /// ready flag asserted.
    ///
    /// While this function is itself a no-op, it satisfies the `#[must_use]`
    /// constraint on the [`AsyncFdReadyGuard`] type.
    pub fn retain_ready(&mut self) {
        // no-op
    }

    /// Get the [`Ready`] value associated with this guard.
    ///
    /// This method will return the empty readiness state if
    /// [`AsyncFdReadyGuard::clear_ready`] has been called on
    /// the guard.
    ///
    /// [`Ready`]: super::Ready
    pub fn ready(&self) -> Ready {
        match &self.event {
            Some(event) => event.ready,
            None => Ready::EMPTY,
        }
    }

    /// Performs the provided IO operation.
    ///
    /// If `f` returns a [`WouldBlock`] error, the readiness state associated
    /// with this file descriptor is cleared, and the method returns
    /// `Err(TryIoError::WouldBlock)`. You will typically need to poll the
    /// `AsyncFd` again when this happens.
    ///
    /// This method helps ensure that the readiness state of the underlying file
    /// descriptor remains in sync with the tokio-side readiness state, by
    /// clearing the tokio-side state only when a [`WouldBlock`] condition
    /// occurs. It is the responsibility of the caller to ensure that `f`
    /// returns [`WouldBlock`] only if the file descriptor that originated this
    /// `AsyncFdReadyGuard` no longer expresses the readiness state that was queried to
    /// create this `AsyncFdReadyGuard`.
    ///
    /// [`WouldBlock`]: std::io::ErrorKind::WouldBlock
    pub fn try_io<R>(
        &mut self,
        f: impl FnOnce(&mut AsyncFd<Inner>) -> io::Result<R>,
    ) -> Result<io::Result<R>, TryIoError> {
        let result = f(self.async_fd);

        match result {
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                self.clear_ready();
                Err(TryIoError(()))
            }
            result => Ok(result),
        }
    }

    /// Returns a shared reference to the inner [`AsyncFd`].
    pub fn get_ref(&self) -> &AsyncFd<Inner> {
        self.async_fd
    }

    /// Returns a mutable reference to the inner [`AsyncFd`].
    pub fn get_mut(&mut self) -> &mut AsyncFd<Inner> {
        self.async_fd
    }

    /// Returns a shared reference to the backing object of the inner [`AsyncFd`].
    pub fn get_inner(&self) -> &Inner {
        self.get_ref().get_ref()
    }

    /// Returns a mutable reference to the backing object of the inner [`AsyncFd`].
    pub fn get_inner_mut(&mut self) -> &mut Inner {
        self.get_mut().get_mut()
    }
}

impl<'a, T: std::fmt::Debug + AsRawFd> std::fmt::Debug for AsyncFdReadyGuard<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadyGuard")
            .field("async_fd", &self.async_fd)
            .finish()
    }
}

impl<'a, T: std::fmt::Debug + AsRawFd> std::fmt::Debug for AsyncFdReadyMutGuard<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MutReadyGuard")
            .field("async_fd", &self.async_fd)
            .finish()
    }
}

/// The error type returned by [`try_io`].
///
/// This error indicates that the IO resource returned a [`WouldBlock`] error.
///
/// [`WouldBlock`]: std::io::ErrorKind::WouldBlock
/// [`try_io`]: method@AsyncFdReadyGuard::try_io
#[derive(Debug)]
pub struct TryIoError(());

/// Error returned by [`try_new`] or [`try_with_interest`].
///
/// [`try_new`]: AsyncFd::try_new
/// [`try_with_interest`]: AsyncFd::try_with_interest
pub struct AsyncFdTryNewError<T> {
    inner: T,
    cause: io::Error,
}

impl<T> AsyncFdTryNewError<T> {
    /// Returns the original object passed to [`try_new`] or [`try_with_interest`]
    /// alongside the error that caused these functions to fail.
    ///
    /// [`try_new`]: AsyncFd::try_new
    /// [`try_with_interest`]: AsyncFd::try_with_interest
    pub fn into_parts(self) -> (T, io::Error) {
        (self.inner, self.cause)
    }
}

impl<T> fmt::Display for AsyncFdTryNewError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.cause, f)
    }
}

impl<T> fmt::Debug for AsyncFdTryNewError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.cause, f)
    }
}

impl<T> Error for AsyncFdTryNewError<T> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}

impl<T> From<AsyncFdTryNewError<T>> for io::Error {
    fn from(value: AsyncFdTryNewError<T>) -> Self {
        value.cause
    }
}
