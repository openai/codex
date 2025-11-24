#[cfg(all(
    unix,
    not(mio_unsupported_force_poll_poll),
    not(any(
        target_os = "aix",
        target_os = "espidf",
        target_os = "fuchsia",
        target_os = "haiku",
        target_os = "hermit",
        target_os = "hurd",
        target_os = "nto",
        target_os = "solaris",
        target_os = "vita"
    )),
))]
use std::os::fd::{AsRawFd, RawFd};
#[cfg(all(debug_assertions, not(target_os = "wasi")))]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(all(debug_assertions, not(target_os = "wasi")))]
use std::sync::Arc;
use std::time::Duration;
use std::{fmt, io};

use crate::{event, sys, Events, Interest, Token};

/// Polls for readiness events on all registered values.
///
/// `Poll` allows a program to monitor a large number of [`event::Source`]s,
/// waiting until one or more become "ready" for some class of operations; e.g.
/// reading and writing. An event source is considered ready if it is possible
/// to immediately perform a corresponding operation; e.g. [`read`] or
/// [`write`].
///
/// To use `Poll`, an `event::Source` must first be registered with the `Poll`
/// instance using the [`register`] method on its associated `Register`,
/// supplying readiness interest. The readiness interest tells `Poll` which
/// specific operations on the handle to monitor for readiness. A `Token` is
/// also passed to the [`register`] function. When `Poll` returns a readiness
/// event, it will include this token.  This associates the event with the
/// event source that generated the event.
///
/// [`event::Source`]: ./event/trait.Source.html
/// [`read`]: ./net/struct.TcpStream.html#method.read
/// [`write`]: ./net/struct.TcpStream.html#method.write
/// [`register`]: struct.Registry.html#method.register
///
/// # Examples
///
/// A basic example -- establishing a `TcpStream` connection.
///
#[cfg_attr(all(feature = "os-poll", feature = "net"), doc = "```")]
#[cfg_attr(not(all(feature = "os-poll", feature = "net")), doc = "```ignore")]
/// # use std::error::Error;
/// # fn main() -> Result<(), Box<dyn Error>> {
/// use mio::{Events, Poll, Interest, Token};
/// use mio::net::TcpStream;
///
/// use std::net::{self, SocketAddr};
///
/// // Bind a server socket to connect to.
/// let addr: SocketAddr = "127.0.0.1:0".parse()?;
/// let server = net::TcpListener::bind(addr)?;
///
/// // Construct a new `Poll` handle as well as the `Events` we'll store into
/// let mut poll = Poll::new()?;
/// let mut events = Events::with_capacity(1024);
///
/// // Connect the stream
/// let mut stream = TcpStream::connect(server.local_addr()?)?;
///
/// // Register the stream with `Poll`
/// poll.registry().register(&mut stream, Token(0), Interest::READABLE | Interest::WRITABLE)?;
///
/// // Wait for the socket to become ready. This has to happens in a loop to
/// // handle spurious wakeups.
/// loop {
///     poll.poll(&mut events, None)?;
///
///     for event in &events {
///         if event.token() == Token(0) && event.is_writable() {
///             // The socket connected (probably, it could still be a spurious
///             // wakeup)
///             return Ok(());
///         }
///     }
/// }
/// # }
/// ```
///
/// # Portability
///
/// Using `Poll` provides a portable interface across supported platforms as
/// long as the caller takes the following into consideration:
///
/// ### Spurious events
///
/// [`Poll::poll`] may return readiness events even if the associated
/// event source is not actually ready. Given the same code, this may
/// happen more on some platforms than others. It is important to never assume
/// that, just because a readiness event was received, that the associated
/// operation will succeed as well.
///
/// If operation fails with [`WouldBlock`], then the caller should not treat
/// this as an error, but instead should wait until another readiness event is
/// received.
///
/// ### Draining readiness
///
/// Once a readiness event is received, the corresponding operation must be
/// performed repeatedly until it returns [`WouldBlock`]. Unless this is done,
/// there is no guarantee that another readiness event will be delivered, even
/// if further data is received for the event source.
///
/// [`WouldBlock`]: std::io::ErrorKind::WouldBlock
///
/// ### Readiness operations
///
/// The only readiness operations that are guaranteed to be present on all
/// supported platforms are [`readable`] and [`writable`]. All other readiness
/// operations may have false negatives and as such should be considered
/// **hints**. This means that if a socket is registered with [`readable`]
/// interest and either an error or close is received, a readiness event will
/// be generated for the socket, but it **may** only include `readable`
/// readiness. Also note that, given the potential for spurious events,
/// receiving a readiness event with `read_closed`, `write_closed`, or `error`
/// doesn't actually mean that a `read` on the socket will return a result
/// matching the readiness event.
///
/// In other words, portable programs that explicitly check for [`read_closed`],
/// [`write_closed`], or [`error`] readiness should be doing so as an
/// **optimization** and always be able to handle an error or close situation
/// when performing the actual read operation.
///
/// [`readable`]: ./event/struct.Event.html#method.is_readable
/// [`writable`]: ./event/struct.Event.html#method.is_writable
/// [`error`]: ./event/struct.Event.html#method.is_error
/// [`read_closed`]: ./event/struct.Event.html#method.is_read_closed
/// [`write_closed`]: ./event/struct.Event.html#method.is_write_closed
///
/// ### Registering handles
///
/// Unless otherwise noted, it should be assumed that types implementing
/// [`event::Source`] will never become ready unless they are registered with
/// `Poll`.
///
/// For example:
///
#[cfg_attr(all(feature = "os-poll", feature = "net"), doc = "```")]
#[cfg_attr(not(all(feature = "os-poll", feature = "net")), doc = "```ignore")]
/// # use std::error::Error;
/// # use std::net;
/// # fn main() -> Result<(), Box<dyn Error>> {
/// use mio::{Poll, Interest, Token};
/// use mio::net::TcpStream;
/// use std::net::SocketAddr;
/// use std::time::Duration;
/// use std::thread;
///
/// let address: SocketAddr = "127.0.0.1:0".parse()?;
/// let listener = net::TcpListener::bind(address)?;
/// let mut sock = TcpStream::connect(listener.local_addr()?)?;
///
/// thread::sleep(Duration::from_secs(1));
///
/// let poll = Poll::new()?;
///
/// // The connect is not guaranteed to have started until it is registered at
/// // this point
/// poll.registry().register(&mut sock, Token(0), Interest::READABLE | Interest::WRITABLE)?;
/// #     Ok(())
/// # }
/// ```
///
/// ### Dropping `Poll`
///
/// When the `Poll` instance is dropped it may cancel in-flight operations for
/// the registered [event sources], meaning that no further events for them may
/// be received. It also means operations on the registered event sources may no
/// longer work. It is up to the user to keep the `Poll` instance alive while
/// registered event sources are being used.
///
/// [event sources]: ./event/trait.Source.html
///
/// ### Accessing raw fd/socket/handle
///
/// Mio makes it possible for many types to be converted into a raw file
/// descriptor (fd, Unix), socket (Windows) or handle (Windows). This makes it
/// possible to support more operations on the type than Mio supports, for
/// example it makes [mio-aio] possible. However accessing the raw fd is not
/// without it's pitfalls.
///
/// Specifically performing I/O operations outside of Mio on these types (via
/// the raw fd) has unspecified behaviour. It could cause no more events to be
/// generated for the type even though it returned `WouldBlock` (in an operation
/// directly accessing the fd). The behaviour is OS specific and Mio can only
/// guarantee cross-platform behaviour if it can control the I/O.
///
/// [mio-aio]: https://github.com/asomers/mio-aio
///
/// *The following is **not** guaranteed, just a description of the current
/// situation!* Mio is allowed to change the following without it being considered
/// a breaking change, don't depend on this, it's just here to inform the user.
/// Currently the kqueue and epoll implementation support direct I/O operations
/// on the fd without Mio's knowledge. Windows however needs **all** I/O
/// operations to go through Mio otherwise it is not able to update it's
/// internal state properly and won't generate events.
///
/// ### Polling without registering event sources
///
///
/// *The following is **not** guaranteed, just a description of the current
/// situation!* Mio is allowed to change the following without it being
/// considered a breaking change, don't depend on this, it's just here to inform
/// the user. On platforms that use epoll, kqueue or IOCP (see implementation
/// notes below) polling without previously registering [event sources] will
/// result in sleeping forever, only a process signal will be able to wake up
/// the thread.
///
/// On WASM/WASI this is different as it doesn't support process signals,
/// furthermore the WASI specification doesn't specify a behaviour in this
/// situation, thus it's up to the implementation what to do here. As an
/// example, the wasmtime runtime will return `EINVAL` in this situation, but
/// different runtimes may return different results. If you have further
/// insights or thoughts about this situation (and/or how Mio should handle it)
/// please add you comment to [pull request#1580].
///
/// [event sources]: crate::event::Source
/// [pull request#1580]: https://github.com/tokio-rs/mio/pull/1580
///
/// # Implementation notes
///
/// `Poll` is backed by the selector provided by the operating system.
///
/// |      OS       |  Selector |
/// |---------------|-----------|
/// | Android       | [epoll]   |
/// | DragonFly BSD | [kqueue]  |
/// | FreeBSD       | [kqueue]  |
/// | iOS           | [kqueue]  |
/// | illumos       | [epoll]   |
/// | Linux         | [epoll]   |
/// | NetBSD        | [kqueue]  |
/// | OpenBSD       | [kqueue]  |
/// | Windows       | [IOCP]    |
/// | macOS         | [kqueue]  |
///
/// On all supported platforms, socket operations are handled by using the
/// system selector. Platform specific extensions (e.g. [`SourceFd`]) allow
/// accessing other features provided by individual system selectors. For
/// example, Linux's [`signalfd`] feature can be used by registering the FD with
/// `Poll` via [`SourceFd`].
///
/// On all platforms except windows, a call to [`Poll::poll`] is mostly just a
/// direct call to the system selector. However, [IOCP] uses a completion model
/// instead of a readiness model. In this case, `Poll` must adapt the completion
/// model Mio's API. While non-trivial, the bridge layer is still quite
/// efficient. The most expensive part being calls to `read` and `write` require
/// data to be copied into an intermediate buffer before it is passed to the
/// kernel.
///
/// [epoll]: https://man7.org/linux/man-pages/man7/epoll.7.html
/// [kqueue]: https://www.freebsd.org/cgi/man.cgi?query=kqueue&sektion=2
/// [IOCP]: https://docs.microsoft.com/en-us/windows/win32/fileio/i-o-completion-ports
/// [`signalfd`]: https://man7.org/linux/man-pages/man2/signalfd.2.html
/// [`SourceFd`]: unix/struct.SourceFd.html
/// [`Poll::poll`]: struct.Poll.html#method.poll
pub struct Poll {
    registry: Registry,
}

/// Registers I/O resources.
pub struct Registry {
    selector: sys::Selector,
    /// Whether this selector currently has an associated waker.
    #[cfg(all(debug_assertions, not(target_os = "wasi")))]
    has_waker: Arc<AtomicBool>,
}

impl Poll {
    cfg_os_poll! {
        /// Return a new `Poll` handle.
        ///
        /// This function will make a syscall to the operating system to create
        /// the system selector. If this syscall fails, `Poll::new` will return
        /// with the error.
        ///
        /// close-on-exec flag is set on the file descriptors used by the selector to prevent
        /// leaking it to executed processes. However, on some systems such as
        /// old Linux systems that don't support `epoll_create1` syscall it is done
        /// non-atomically, so a separate thread executing in parallel to this
        /// function may accidentally leak the file descriptor if it executes a
        /// new process before this function returns.
        ///
        /// See [struct] level docs for more details.
        ///
        /// [struct]: struct.Poll.html
        ///
        /// # Examples
        ///
        /// ```
        /// # use std::error::Error;
        /// # fn main() -> Result<(), Box<dyn Error>> {
        /// use mio::{Poll, Events};
        /// use std::time::Duration;
        ///
        /// let mut poll = match Poll::new() {
        ///     Ok(poll) => poll,
        ///     Err(e) => panic!("failed to create Poll instance; err={:?}", e),
        /// };
        ///
        /// // Create a structure to receive polled events
        /// let mut events = Events::with_capacity(1024);
        ///
        /// // Wait for events, but none will be received because no
        /// // `event::Source`s have been registered with this `Poll` instance.
        /// poll.poll(&mut events, Some(Duration::from_millis(500)))?;
        /// assert!(events.is_empty());
        /// #     Ok(())
        /// # }
        /// ```
        pub fn new() -> io::Result<Poll> {
            sys::Selector::new().map(|selector| Poll {
                registry: Registry {
                    selector,
                    #[cfg(all(debug_assertions, not(target_os = "wasi")))]
                    has_waker: Arc::new(AtomicBool::new(false)),
                },
            })
        }
    }

    /// Returns a `Registry` which can be used to register
    /// `event::Source`s.
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Wait for readiness events
    ///
    /// Blocks the current thread and waits for readiness events for any of the
    /// [`event::Source`]s that have been registered with this `Poll` instance.
    /// The function will block until either at least one readiness event has
    /// been received or `timeout` has elapsed. A `timeout` of `None` means that
    /// `poll` will block until a readiness event has been received.
    ///
    /// The supplied `events` will be cleared and newly received readiness events
    /// will be pushed onto the end. At most `events.capacity()` events will be
    /// returned. If there are further pending readiness events, they will be
    /// returned on the next call to `poll`.
    ///
    /// A single call to `poll` may result in multiple readiness events being
    /// returned for a single event source. For example, if a TCP socket becomes
    /// both readable and writable, it may be possible for a single readiness
    /// event to be returned with both [`readable`] and [`writable`] readiness
    /// **OR** two separate events may be returned, one with [`readable`] set
    /// and one with [`writable`] set.
    ///
    /// Note that the `timeout` will be rounded up to the system clock
    /// granularity (usually 1ms), and kernel scheduling delays mean that
    /// the blocking interval may be overrun by a small amount.
    ///
    /// See the [struct] level documentation for a higher level discussion of
    /// polling.
    ///
    /// [`event::Source`]: ./event/trait.Source.html
    /// [`readable`]: struct.Interest.html#associatedconstant.READABLE
    /// [`writable`]: struct.Interest.html#associatedconstant.WRITABLE
    /// [struct]: struct.Poll.html
    /// [`iter`]: ./event/struct.Events.html#method.iter
    ///
    /// # Notes
    ///
    /// This returns any errors without attempting to retry, previous versions
    /// of Mio would automatically retry the poll call if it was interrupted
    /// (if `EINTR` was returned).
    ///
    /// Currently if the `timeout` elapses without any readiness events
    /// triggering this will return `Ok(())`. However we're not guaranteeing
    /// this behaviour as this depends on the OS.
    ///
    /// # Examples
    ///
    /// A basic example -- establishing a `TcpStream` connection.
    ///
    #[cfg_attr(all(feature = "os-poll", feature = "net"), doc = "```")]
    #[cfg_attr(not(all(feature = "os-poll", feature = "net")), doc = "```ignore")]
    /// # use std::error::Error;
    /// # fn main() -> Result<(), Box<dyn Error>> {
    /// use mio::{Events, Poll, Interest, Token};
    /// use mio::net::TcpStream;
    ///
    /// use std::net::{TcpListener, SocketAddr};
    /// use std::thread;
    ///
    /// // Bind a server socket to connect to.
    /// let addr: SocketAddr = "127.0.0.1:0".parse()?;
    /// let server = TcpListener::bind(addr)?;
    /// let addr = server.local_addr()?.clone();
    ///
    /// // Spawn a thread to accept the socket
    /// thread::spawn(move || {
    ///     let _ = server.accept();
    /// });
    ///
    /// // Construct a new `Poll` handle as well as the `Events` we'll store into
    /// let mut poll = Poll::new()?;
    /// let mut events = Events::with_capacity(1024);
    ///
    /// // Connect the stream
    /// let mut stream = TcpStream::connect(addr)?;
    ///
    /// // Register the stream with `Poll`
    /// poll.registry().register(
    ///     &mut stream,
    ///     Token(0),
    ///     Interest::READABLE | Interest::WRITABLE)?;
    ///
    /// // Wait for the socket to become ready. This has to happens in a loop to
    /// // handle spurious wakeups.
    /// loop {
    ///     poll.poll(&mut events, None)?;
    ///
    ///     for event in &events {
    ///         if event.token() == Token(0) && event.is_writable() {
    ///             // The socket connected (probably, it could still be a spurious
    ///             // wakeup)
    ///             return Ok(());
    ///         }
    ///     }
    /// }
    /// # }
    /// ```
    ///
    /// [struct]: #
    pub fn poll(&mut self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        self.registry.selector.select(events.sys(), timeout)
    }
}

#[cfg(all(
    unix,
    not(mio_unsupported_force_poll_poll),
    not(any(
        target_os = "aix",
        target_os = "espidf",
        target_os = "fuchsia",
        target_os = "haiku",
        target_os = "hermit",
        target_os = "hurd",
        target_os = "nto",
        target_os = "solaris",
        target_os = "vita"
    )),
))]
impl AsRawFd for Poll {
    fn as_raw_fd(&self) -> RawFd {
        self.registry.as_raw_fd()
    }
}

impl fmt::Debug for Poll {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Poll").finish()
    }
}

impl Registry {
    /// Register an [`event::Source`] with the `Poll` instance.
    ///
    /// Once registered, the `Poll` instance will monitor the event source for
    /// readiness state changes. When it notices a state change, it will return
    /// a readiness event for the handle the next time [`poll`] is called.
    ///
    /// See [`Poll`] docs for a high level overview.
    ///
    /// # Arguments
    ///
    /// `source: &mut S: event::Source`: This is the source of events that the
    /// `Poll` instance should monitor for readiness state changes.
    ///
    /// `token: Token`: The caller picks a token to associate with the socket.
    /// When [`poll`] returns an event for the handle, this token is included.
    /// This allows the caller to map the event to its source. The token
    /// associated with the `event::Source` can be changed at any time by
    /// calling [`reregister`].
    ///
    /// See documentation on [`Token`] for an example showing how to pick
    /// [`Token`] values.
    ///
    /// `interest: Interest`: Specifies which operations `Poll` should monitor
    /// for readiness. `Poll` will only return readiness events for operations
    /// specified by this argument.
    ///
    /// If a socket is registered with readable interest and the socket becomes
    /// writable, no event will be returned from [`poll`].
    ///
    /// The readiness interest for an `event::Source` can be changed at any time
    /// by calling [`reregister`].
    ///
    /// # Notes
    ///
    /// Callers must ensure that if a source being registered with a `Poll`
    /// instance was previously registered with that `Poll` instance, then a
    /// call to [`deregister`] has already occurred. Consecutive calls to
    /// `register` is unspecified behavior.
    ///
    /// Unless otherwise specified, the caller should assume that once an event
    /// source is registered with a `Poll` instance, it is bound to that `Poll`
    /// instance for the lifetime of the event source. This remains true even
    /// if the event source is deregistered from the poll instance using
    /// [`deregister`].
    ///
    /// [`event::Source`]: ./event/trait.Source.html
    /// [`poll`]: struct.Poll.html#method.poll
    /// [`reregister`]: struct.Registry.html#method.reregister
    /// [`deregister`]: struct.Registry.html#method.deregister
    /// [`Token`]: struct.Token.html
    ///
    /// # Examples
    ///
    #[cfg_attr(all(feature = "os-poll", feature = "net"), doc = "```")]
    #[cfg_attr(not(all(feature = "os-poll", feature = "net")), doc = "```ignore")]
    /// # use std::error::Error;
    /// # use std::net;
    /// # fn main() -> Result<(), Box<dyn Error>> {
    /// use mio::{Events, Poll, Interest, Token};
    /// use mio::net::TcpStream;
    /// use std::net::SocketAddr;
    /// use std::time::{Duration, Instant};
    ///
    /// let mut poll = Poll::new()?;
    ///
    /// let address: SocketAddr = "127.0.0.1:0".parse()?;
    /// let listener = net::TcpListener::bind(address)?;
    /// let mut socket = TcpStream::connect(listener.local_addr()?)?;
    ///
    /// // Register the socket with `poll`
    /// poll.registry().register(
    ///     &mut socket,
    ///     Token(0),
    ///     Interest::READABLE | Interest::WRITABLE)?;
    ///
    /// let mut events = Events::with_capacity(1024);
    /// let start = Instant::now();
    /// let timeout = Duration::from_millis(500);
    ///
    /// loop {
    ///     let elapsed = start.elapsed();
    ///
    ///     if elapsed >= timeout {
    ///         // Connection timed out
    ///         return Ok(());
    ///     }
    ///
    ///     let remaining = timeout - elapsed;
    ///     poll.poll(&mut events, Some(remaining))?;
    ///
    ///     for event in &events {
    ///         if event.token() == Token(0) {
    ///             // Something (probably) happened on the socket.
    ///             return Ok(());
    ///         }
    ///     }
    /// }
    /// # }
    /// ```
    pub fn register<S>(&self, source: &mut S, token: Token, interests: Interest) -> io::Result<()>
    where
        S: event::Source + ?Sized,
    {
        trace!(
            "registering event source with poller: token={:?}, interests={:?}",
            token,
            interests
        );
        source.register(self, token, interests)
    }

    /// Re-register an [`event::Source`] with the `Poll` instance.
    ///
    /// Re-registering an event source allows changing the details of the
    /// registration. Specifically, it allows updating the associated `token`
    /// and `interests` specified in previous `register` and `reregister` calls.
    ///
    /// The `reregister` arguments fully override the previous values. In other
    /// words, if a socket is registered with [`readable`] interest and the call
    /// to `reregister` specifies [`writable`], then read interest is no longer
    /// requested for the handle.
    ///
    /// The event source must have previously been registered with this instance
    /// of `Poll`, otherwise the behavior is unspecified.
    ///
    /// See the [`register`] documentation for details about the function
    /// arguments and see the [`struct`] docs for a high level overview of
    /// polling.
    ///
    /// # Examples
    ///
    #[cfg_attr(all(feature = "os-poll", feature = "net"), doc = "```")]
    #[cfg_attr(not(all(feature = "os-poll", feature = "net")), doc = "```ignore")]
    /// # use std::error::Error;
    /// # use std::net;
    /// # fn main() -> Result<(), Box<dyn Error>> {
    /// use mio::{Poll, Interest, Token};
    /// use mio::net::TcpStream;
    /// use std::net::SocketAddr;
    ///
    /// let poll = Poll::new()?;
    ///
    /// let address: SocketAddr = "127.0.0.1:0".parse()?;
    /// let listener = net::TcpListener::bind(address)?;
    /// let mut socket = TcpStream::connect(listener.local_addr()?)?;
    ///
    /// // Register the socket with `poll`, requesting readable
    /// poll.registry().register(
    ///     &mut socket,
    ///     Token(0),
    ///     Interest::READABLE)?;
    ///
    /// // Reregister the socket specifying write interest instead. Even though
    /// // the token is the same it must be specified.
    /// poll.registry().reregister(
    ///     &mut socket,
    ///     Token(0),
    ///     Interest::WRITABLE)?;
    /// #     Ok(())
    /// # }
    /// ```
    ///
    /// [`event::Source`]: ./event/trait.Source.html
    /// [`struct`]: struct.Poll.html
    /// [`register`]: struct.Registry.html#method.register
    /// [`readable`]: ./event/struct.Event.html#is_readable
    /// [`writable`]: ./event/struct.Event.html#is_writable
    pub fn reregister<S>(&self, source: &mut S, token: Token, interests: Interest) -> io::Result<()>
    where
        S: event::Source + ?Sized,
    {
        trace!(
            "reregistering event source with poller: token={:?}, interests={:?}",
            token,
            interests
        );
        source.reregister(self, token, interests)
    }

    /// Deregister an [`event::Source`] with the `Poll` instance.
    ///
    /// When an event source is deregistered, the `Poll` instance will no longer
    /// monitor it for readiness state changes. Deregistering clears up any
    /// internal resources needed to track the handle.  After an explicit call
    /// to this method completes, it is guaranteed that the token previously
    /// registered to this handle will not be returned by a future poll, so long
    /// as a happens-before relationship is established between this call and
    /// the poll.
    ///
    /// The event source must have previously been registered with this instance
    /// of `Poll`, otherwise the behavior is unspecified.
    ///
    /// A handle can be passed back to `register` after it has been
    /// deregistered; however, it must be passed back to the **same** `Poll`
    /// instance, otherwise the behavior is unspecified.
    ///
    /// # Examples
    ///
    #[cfg_attr(all(feature = "os-poll", feature = "net"), doc = "```")]
    #[cfg_attr(not(all(feature = "os-poll", feature = "net")), doc = "```ignore")]
    /// # use std::error::Error;
    /// # use std::net;
    /// # fn main() -> Result<(), Box<dyn Error>> {
    /// use mio::{Events, Poll, Interest, Token};
    /// use mio::net::TcpStream;
    /// use std::net::SocketAddr;
    /// use std::time::Duration;
    ///
    /// let mut poll = Poll::new()?;
    ///
    /// let address: SocketAddr = "127.0.0.1:0".parse()?;
    /// let listener = net::TcpListener::bind(address)?;
    /// let mut socket = TcpStream::connect(listener.local_addr()?)?;
    ///
    /// // Register the socket with `poll`
    /// poll.registry().register(
    ///     &mut socket,
    ///     Token(0),
    ///     Interest::READABLE)?;
    ///
    /// poll.registry().deregister(&mut socket)?;
    ///
    /// let mut events = Events::with_capacity(1024);
    ///
    /// // Set a timeout because this poll should never receive any events.
    /// poll.poll(&mut events, Some(Duration::from_secs(1)))?;
    /// assert!(events.is_empty());
    /// #     Ok(())
    /// # }
    /// ```
    pub fn deregister<S>(&self, source: &mut S) -> io::Result<()>
    where
        S: event::Source + ?Sized,
    {
        trace!("deregistering event source from poller");
        source.deregister(self)
    }

    /// Creates a new independently owned `Registry`.
    ///
    /// Event sources registered with this `Registry` will be registered with
    /// the original `Registry` and `Poll` instance.
    pub fn try_clone(&self) -> io::Result<Registry> {
        self.selector.try_clone().map(|selector| Registry {
            selector,
            #[cfg(all(debug_assertions, not(target_os = "wasi")))]
            has_waker: Arc::clone(&self.has_waker),
        })
    }

    /// Internal check to ensure only a single `Waker` is active per [`Poll`]
    /// instance.
    #[cfg(all(debug_assertions, not(target_os = "wasi")))]
    pub(crate) fn register_waker(&self) {
        assert!(
            !self.has_waker.swap(true, Ordering::AcqRel),
            "Only a single `Waker` can be active per `Poll` instance"
        );
    }

    /// Get access to the `sys::Selector`.
    #[cfg(any(not(target_os = "wasi"), feature = "net"))]
    pub(crate) fn selector(&self) -> &sys::Selector {
        &self.selector
    }
}

impl fmt::Debug for Registry {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Registry").finish()
    }
}

#[cfg(all(
    unix,
    not(mio_unsupported_force_poll_poll),
    not(any(
        target_os = "aix",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "fuchsia",
        target_os = "hermit",
        target_os = "hurd",
        target_os = "nto",
        target_os = "solaris",
        target_os = "vita"
    )),
))]
impl AsRawFd for Registry {
    fn as_raw_fd(&self) -> RawFd {
        self.selector.as_raw_fd()
    }
}

cfg_os_poll! {
    #[cfg(all(
        unix,
        not(mio_unsupported_force_poll_poll),
        not(any(
            target_os = "aix",
            target_os = "espidf",
            target_os = "hermit",
            target_os = "hurd",
            target_os = "nto",
            target_os = "solaris",
            target_os = "vita"
        )),
    ))]
    #[test]
    pub fn as_raw_fd() {
        let poll = Poll::new().unwrap();
        assert!(poll.as_raw_fd() > 0);
    }
}
