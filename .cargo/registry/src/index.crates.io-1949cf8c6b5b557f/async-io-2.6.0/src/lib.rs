//! Async I/O and timers.
//!
//! This crate provides two tools:
//!
//! * [`Async`], an adapter for standard networking types (and [many other] types) to use in
//!   async programs.
//! * [`Timer`], a future or stream that emits timed events.
//!
//! For concrete async networking types built on top of this crate, see [`async-net`].
//!
//! [many other]: https://github.com/smol-rs/async-io/tree/master/examples
//! [`async-net`]: https://docs.rs/async-net
//!
//! # Implementation
//!
//! The first time [`Async`] or [`Timer`] is used, a thread named "async-io" will be spawned.
//! The purpose of this thread is to wait for I/O events reported by the operating system, and then
//! wake appropriate futures blocked on I/O or timers when they can be resumed.
//!
//! To wait for the next I/O event, the "async-io" thread uses [epoll] on Linux/Android/illumos,
//! [kqueue] on macOS/iOS/BSD, [event ports] on illumos/Solaris, and [IOCP] on Windows. That
//! functionality is provided by the [`polling`] crate.
//!
//! However, note that you can also process I/O events and wake futures on any thread using the
//! [`block_on()`] function. The "async-io" thread is therefore just a fallback mechanism
//! processing I/O events in case no other threads are.
//!
//! [epoll]: https://en.wikipedia.org/wiki/Epoll
//! [kqueue]: https://en.wikipedia.org/wiki/Kqueue
//! [event ports]: https://illumos.org/man/port_create
//! [IOCP]: https://learn.microsoft.com/en-us/windows/win32/fileio/i-o-completion-ports
//! [`polling`]: https://docs.rs/polling
//!
//! # Examples
//!
//! Connect to `example.com:80`, or time out after 10 seconds.
//!
//! ```
//! use async_io::{Async, Timer};
//! use futures_lite::{future::FutureExt, io};
//!
//! use std::net::{TcpStream, ToSocketAddrs};
//! use std::time::Duration;
//!
//! # futures_lite::future::block_on(async {
//! let addr = "example.com:80".to_socket_addrs()?.next().unwrap();
//!
//! let stream = Async::<TcpStream>::connect(addr).or(async {
//!     Timer::after(Duration::from_secs(10)).await;
//!     Err(io::ErrorKind::TimedOut.into())
//! })
//! .await?;
//! # std::io::Result::Ok(()) });
//! ```

#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms)]
#![doc(
    html_favicon_url = "https://raw.githubusercontent.com/smol-rs/smol/master/assets/images/logo_fullsize_transparent.png"
)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/smol-rs/smol/master/assets/images/logo_fullsize_transparent.png"
)]

use std::future::{poll_fn, Future};
use std::io::{self, IoSlice, IoSliceMut, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::pin::{pin, Pin};
use std::sync::Arc;
use std::task::{ready, Context, Poll, Waker};
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::{
    os::unix::io::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd},
    os::unix::net::{SocketAddr as UnixSocketAddr, UnixDatagram, UnixListener, UnixStream},
    path::Path,
};

#[cfg(windows)]
use std::os::windows::io::{AsRawSocket, AsSocket, BorrowedSocket, OwnedSocket, RawSocket};

use futures_io::{AsyncRead, AsyncWrite};
use futures_lite::stream::{self, Stream};

use rustix::io as rio;
use rustix::net as rn;
use rustix::net::addr::SocketAddrArg;

use crate::reactor::{Reactor, Registration, Source};

mod driver;
mod reactor;

pub mod os;

pub use driver::block_on;
pub use reactor::{Readable, ReadableOwned, Writable, WritableOwned};

/// A future or stream that emits timed events.
///
/// Timers are futures that output a single [`Instant`] when they fire.
///
/// Timers are also streams that can output [`Instant`]s periodically.
///
/// # Precision
///
/// There is a limit on the maximum precision that a `Timer` can provide. This limit is
/// dependent on the current platform; for instance, on Windows, the maximum precision is
/// about 16 milliseconds. Because of this limit, the timer may sleep for longer than the
/// requested duration. It will never sleep for less.
///
/// # Examples
///
/// Sleep for 1 second:
///
/// ```
/// use async_io::Timer;
/// use std::time::Duration;
///
/// # futures_lite::future::block_on(async {
/// Timer::after(Duration::from_secs(1)).await;
/// # });
/// ```
///
/// Timeout after 1 second:
///
/// ```
/// use async_io::Timer;
/// use futures_lite::FutureExt;
/// use std::time::Duration;
///
/// # futures_lite::future::block_on(async {
/// let addrs = async_net::resolve("google.com:80")
///     .or(async {
///         Timer::after(Duration::from_secs(1)).await;
///         Err(std::io::ErrorKind::TimedOut.into())
///     })
///     .await?;
/// # std::io::Result::Ok(()) });
/// ```
#[doc(alias = "sleep")]
#[doc(alias = "timeout")]
#[derive(Debug)]
pub struct Timer {
    /// This timer's ID and last waker that polled it.
    ///
    /// When this field is set to `None`, this timer is not registered in the reactor.
    id_and_waker: Option<(usize, Waker)>,

    /// The next instant at which this timer fires.
    ///
    /// If this timer is a blank timer, this value is None. If the timer
    /// must be set, this value contains the next instant at which the
    /// timer must fire.
    when: Option<Instant>,

    /// The period.
    period: Duration,
}

impl Timer {
    /// Creates a timer that will never fire.
    ///
    /// # Examples
    ///
    /// This function may also be useful for creating a function with an optional timeout.
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_io::Timer;
    /// use futures_lite::prelude::*;
    /// use std::time::Duration;
    ///
    /// async fn run_with_timeout(timeout: Option<Duration>) {
    ///     let timer = timeout
    ///         .map(|timeout| Timer::after(timeout))
    ///         .unwrap_or_else(Timer::never);
    ///
    ///     run_lengthy_operation().or(timer).await;
    /// }
    /// # // Note that since a Timer as a Future returns an Instant,
    /// # // this function needs to return an Instant to be used
    /// # // in "or".
    /// # async fn run_lengthy_operation() -> std::time::Instant {
    /// #    std::time::Instant::now()
    /// # }
    ///
    /// // Times out after 5 seconds.
    /// run_with_timeout(Some(Duration::from_secs(5))).await;
    /// // Does not time out.
    /// run_with_timeout(None).await;
    /// # });
    /// ```
    pub fn never() -> Timer {
        Timer {
            id_and_waker: None,
            when: None,
            period: Duration::MAX,
        }
    }

    /// Creates a timer that emits an event once after the given duration of time.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_io::Timer;
    /// use std::time::Duration;
    ///
    /// # futures_lite::future::block_on(async {
    /// Timer::after(Duration::from_secs(1)).await;
    /// # });
    /// ```
    pub fn after(duration: Duration) -> Timer {
        Instant::now()
            .checked_add(duration)
            .map_or_else(Timer::never, Timer::at)
    }

    /// Creates a timer that emits an event once at the given time instant.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_io::Timer;
    /// use std::time::{Duration, Instant};
    ///
    /// # futures_lite::future::block_on(async {
    /// let now = Instant::now();
    /// let when = now + Duration::from_secs(1);
    /// Timer::at(when).await;
    /// # });
    /// ```
    pub fn at(instant: Instant) -> Timer {
        Timer::interval_at(instant, Duration::MAX)
    }

    /// Creates a timer that emits events periodically.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_io::Timer;
    /// use futures_lite::StreamExt;
    /// use std::time::{Duration, Instant};
    ///
    /// # futures_lite::future::block_on(async {
    /// let period = Duration::from_secs(1);
    /// Timer::interval(period).next().await;
    /// # });
    /// ```
    pub fn interval(period: Duration) -> Timer {
        Instant::now()
            .checked_add(period)
            .map_or_else(Timer::never, |at| Timer::interval_at(at, period))
    }

    /// Creates a timer that emits events periodically, starting at `start`.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_io::Timer;
    /// use futures_lite::StreamExt;
    /// use std::time::{Duration, Instant};
    ///
    /// # futures_lite::future::block_on(async {
    /// let start = Instant::now();
    /// let period = Duration::from_secs(1);
    /// Timer::interval_at(start, period).next().await;
    /// # });
    /// ```
    pub fn interval_at(start: Instant, period: Duration) -> Timer {
        Timer {
            id_and_waker: None,
            when: Some(start),
            period,
        }
    }

    /// Indicates whether or not this timer will ever fire.
    ///
    /// [`never()`] will never fire, and timers created with [`after()`] or [`at()`] will fire
    /// if the duration is not too large.
    ///
    /// [`never()`]: Timer::never()
    /// [`after()`]: Timer::after()
    /// [`at()`]: Timer::at()
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_io::Timer;
    /// use futures_lite::prelude::*;
    /// use std::time::Duration;
    ///
    /// // `never` will never fire.
    /// assert!(!Timer::never().will_fire());
    ///
    /// // `after` will fire if the duration is not too large.
    /// assert!(Timer::after(Duration::from_secs(1)).will_fire());
    /// assert!(!Timer::after(Duration::MAX).will_fire());
    ///
    /// // However, once an `after` timer has fired, it will never fire again.
    /// let mut t = Timer::after(Duration::from_secs(1));
    /// assert!(t.will_fire());
    /// (&mut t).await;
    /// assert!(!t.will_fire());
    ///
    /// // Interval timers will fire periodically.
    /// let mut t = Timer::interval(Duration::from_secs(1));
    /// assert!(t.will_fire());
    /// t.next().await;
    /// assert!(t.will_fire());
    /// # });
    /// ```
    #[inline]
    pub fn will_fire(&self) -> bool {
        self.when.is_some()
    }

    /// Sets the timer to emit an event once after the given duration of time.
    ///
    /// Note that resetting a timer is different from creating a new timer because
    /// [`set_after()`][`Timer::set_after()`] does not remove the waker associated with the task
    /// that is polling the timer.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_io::Timer;
    /// use std::time::Duration;
    ///
    /// # futures_lite::future::block_on(async {
    /// let mut t = Timer::after(Duration::from_secs(1));
    /// t.set_after(Duration::from_millis(100));
    /// # });
    /// ```
    pub fn set_after(&mut self, duration: Duration) {
        match Instant::now().checked_add(duration) {
            Some(instant) => self.set_at(instant),
            None => {
                // Overflow to never going off.
                self.clear();
            }
        }
    }

    /// Sets the timer to emit an event once at the given time instant.
    ///
    /// Note that resetting a timer is different from creating a new timer because
    /// [`set_at()`][`Timer::set_at()`] does not remove the waker associated with the task
    /// that is polling the timer.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_io::Timer;
    /// use std::time::{Duration, Instant};
    ///
    /// # futures_lite::future::block_on(async {
    /// let mut t = Timer::after(Duration::from_secs(1));
    ///
    /// let now = Instant::now();
    /// let when = now + Duration::from_secs(1);
    /// t.set_at(when);
    /// # });
    /// ```
    pub fn set_at(&mut self, instant: Instant) {
        self.clear();

        // Update the timeout.
        self.when = Some(instant);

        if let Some((id, waker)) = self.id_and_waker.as_mut() {
            // Re-register the timer with the new timeout.
            *id = Reactor::get().insert_timer(instant, waker);
        }
    }

    /// Sets the timer to emit events periodically.
    ///
    /// Note that resetting a timer is different from creating a new timer because
    /// [`set_interval()`][`Timer::set_interval()`] does not remove the waker associated with the
    /// task that is polling the timer.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_io::Timer;
    /// use futures_lite::StreamExt;
    /// use std::time::{Duration, Instant};
    ///
    /// # futures_lite::future::block_on(async {
    /// let mut t = Timer::after(Duration::from_secs(1));
    ///
    /// let period = Duration::from_secs(2);
    /// t.set_interval(period);
    /// # });
    /// ```
    pub fn set_interval(&mut self, period: Duration) {
        match Instant::now().checked_add(period) {
            Some(instant) => self.set_interval_at(instant, period),
            None => {
                // Overflow to never going off.
                self.clear();
            }
        }
    }

    /// Sets the timer to emit events periodically, starting at `start`.
    ///
    /// Note that resetting a timer is different from creating a new timer because
    /// [`set_interval_at()`][`Timer::set_interval_at()`] does not remove the waker associated with
    /// the task that is polling the timer.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_io::Timer;
    /// use futures_lite::StreamExt;
    /// use std::time::{Duration, Instant};
    ///
    /// # futures_lite::future::block_on(async {
    /// let mut t = Timer::after(Duration::from_secs(1));
    ///
    /// let start = Instant::now();
    /// let period = Duration::from_secs(2);
    /// t.set_interval_at(start, period);
    /// # });
    /// ```
    pub fn set_interval_at(&mut self, start: Instant, period: Duration) {
        self.clear();

        self.when = Some(start);
        self.period = period;

        if let Some((id, waker)) = self.id_and_waker.as_mut() {
            // Re-register the timer with the new timeout.
            *id = Reactor::get().insert_timer(start, waker);
        }
    }

    /// Clear any timeouts set on this timer. It will never fire again until a new interval or instant is set.
    pub fn clear(&mut self) {
        if let (Some(when), Some((id, _))) = (self.when, self.id_and_waker.as_ref()) {
            // Deregister the timer from the reactor.
            Reactor::get().remove_timer(when, *id);
        }
        self.when = None;
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        if let (Some(when), Some((id, _))) = (self.when, self.id_and_waker.take()) {
            // Deregister the timer from the reactor.
            Reactor::get().remove_timer(when, id);
        }
    }
}

impl Future for Timer {
    type Output = Instant;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.poll_next(cx) {
            Poll::Ready(Some(when)) => Poll::Ready(when),
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => unreachable!(),
        }
    }
}

impl Stream for Timer {
    type Item = Instant;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if let Some(ref mut when) = this.when {
            // Check if the timer has already fired.
            if Instant::now() >= *when {
                if let Some((id, _)) = this.id_and_waker.take() {
                    // Deregister the timer from the reactor.
                    Reactor::get().remove_timer(*when, id);
                }
                let result_time = *when;
                if let Some(next) = (*when).checked_add(this.period) {
                    *when = next;
                    // Register the timer in the reactor.
                    let id = Reactor::get().insert_timer(next, cx.waker());
                    this.id_and_waker = Some((id, cx.waker().clone()));
                } else {
                    this.when = None;
                }
                return Poll::Ready(Some(result_time));
            } else {
                match &this.id_and_waker {
                    None => {
                        // Register the timer in the reactor.
                        let id = Reactor::get().insert_timer(*when, cx.waker());
                        this.id_and_waker = Some((id, cx.waker().clone()));
                    }
                    Some((id, w)) if !w.will_wake(cx.waker()) => {
                        // Deregister the timer from the reactor to remove the old waker.
                        Reactor::get().remove_timer(*when, *id);

                        // Register the timer in the reactor with the new waker.
                        let id = Reactor::get().insert_timer(*when, cx.waker());
                        this.id_and_waker = Some((id, cx.waker().clone()));
                    }
                    Some(_) => {}
                }
            }
        }

        Poll::Pending
    }
}

/// Async adapter for I/O types.
///
/// This type puts an I/O handle into non-blocking mode, registers it in
/// [epoll]/[kqueue]/[event ports]/[IOCP], and then provides an async interface for it.
///
/// [epoll]: https://en.wikipedia.org/wiki/Epoll
/// [kqueue]: https://en.wikipedia.org/wiki/Kqueue
/// [event ports]: https://illumos.org/man/port_create
/// [IOCP]: https://learn.microsoft.com/en-us/windows/win32/fileio/i-o-completion-ports
///
/// # Caveats
///
/// [`Async`] is a low-level primitive, and as such it comes with some caveats.
///
/// For higher-level primitives built on top of [`Async`], look into [`async-net`] or
/// [`async-process`] (on Unix).
///
/// The most notable caveat is that it is unsafe to access the inner I/O source mutably
/// using this primitive. Traits likes [`AsyncRead`] and [`AsyncWrite`] are not implemented by
/// default unless it is guaranteed that the resource won't be invalidated by reading or writing.
/// See the [`IoSafe`] trait for more information.
///
/// [`async-net`]: https://github.com/smol-rs/async-net
/// [`async-process`]: https://github.com/smol-rs/async-process
/// [`AsyncRead`]: https://docs.rs/futures-io/latest/futures_io/trait.AsyncRead.html
/// [`AsyncWrite`]: https://docs.rs/futures-io/latest/futures_io/trait.AsyncWrite.html
///
/// ### Supported types
///
/// [`Async`] supports all networking types, as well as some OS-specific file descriptors like
/// [timerfd] and [inotify].
///
/// However, do not use [`Async`] with types like [`File`][`std::fs::File`],
/// [`Stdin`][`std::io::Stdin`], [`Stdout`][`std::io::Stdout`], or [`Stderr`][`std::io::Stderr`]
/// because all operating systems have issues with them when put in non-blocking mode.
///
/// [timerfd]: https://github.com/smol-rs/async-io/blob/master/examples/linux-timerfd.rs
/// [inotify]: https://github.com/smol-rs/async-io/blob/master/examples/linux-inotify.rs
///
/// ### Concurrent I/O
///
/// Note that [`&Async<T>`][`Async`] implements [`AsyncRead`] and [`AsyncWrite`] if `&T`
/// implements those traits, which means tasks can concurrently read and write using shared
/// references.
///
/// But there is a catch: only one task can read a time, and only one task can write at a time. It
/// is okay to have two tasks where one is reading and the other is writing at the same time, but
/// it is not okay to have two tasks reading at the same time or writing at the same time. If you
/// try to do that, conflicting tasks will just keep waking each other in turn, thus wasting CPU
/// time.
///
/// Besides [`AsyncRead`] and [`AsyncWrite`], this caveat also applies to
/// [`poll_readable()`][`Async::poll_readable()`] and
/// [`poll_writable()`][`Async::poll_writable()`].
///
/// However, any number of tasks can be concurrently calling other methods like
/// [`readable()`][`Async::readable()`] or [`read_with()`][`Async::read_with()`].
///
/// ### Closing
///
/// Closing the write side of [`Async`] with [`close()`][`futures_lite::AsyncWriteExt::close()`]
/// simply flushes. If you want to shutdown a TCP or Unix socket, use
/// [`Shutdown`][`std::net::Shutdown`].
///
/// # Examples
///
/// Connect to a server and echo incoming messages back to the server:
///
/// ```no_run
/// use async_io::Async;
/// use futures_lite::io;
/// use std::net::TcpStream;
///
/// # futures_lite::future::block_on(async {
/// // Connect to a local server.
/// let stream = Async::<TcpStream>::connect(([127, 0, 0, 1], 8000)).await?;
///
/// // Echo all messages from the read side of the stream into the write side.
/// io::copy(&stream, &stream).await?;
/// # std::io::Result::Ok(()) });
/// ```
///
/// You can use either predefined async methods or wrap blocking I/O operations in
/// [`Async::read_with()`], [`Async::read_with_mut()`], [`Async::write_with()`], and
/// [`Async::write_with_mut()`]:
///
/// ```no_run
/// use async_io::Async;
/// use std::net::TcpListener;
///
/// # futures_lite::future::block_on(async {
/// let listener = Async::<TcpListener>::bind(([127, 0, 0, 1], 0))?;
///
/// // These two lines are equivalent:
/// let (stream, addr) = listener.accept().await?;
/// let (stream, addr) = listener.read_with(|inner| inner.accept()).await?;
/// # std::io::Result::Ok(()) });
/// ```
#[derive(Debug)]
pub struct Async<T> {
    /// A source registered in the reactor.
    source: Arc<Source>,

    /// The inner I/O handle.
    io: Option<T>,
}

impl<T> Unpin for Async<T> {}

#[cfg(unix)]
impl<T: AsFd> Async<T> {
    /// Creates an async I/O handle.
    ///
    /// This method will put the handle in non-blocking mode and register it in
    /// [epoll]/[kqueue]/[event ports]/[IOCP].
    ///
    /// On Unix systems, the handle must implement `AsFd`, while on Windows it must implement
    /// `AsSocket`.
    ///
    /// [epoll]: https://en.wikipedia.org/wiki/Epoll
    /// [kqueue]: https://en.wikipedia.org/wiki/Kqueue
    /// [event ports]: https://illumos.org/man/port_create
    /// [IOCP]: https://learn.microsoft.com/en-us/windows/win32/fileio/i-o-completion-ports
    ///
    /// # Examples
    ///
    /// ```
    /// use async_io::Async;
    /// use std::net::{SocketAddr, TcpListener};
    ///
    /// # futures_lite::future::block_on(async {
    /// let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))?;
    /// let listener = Async::new(listener)?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn new(io: T) -> io::Result<Async<T>> {
        // Put the file descriptor in non-blocking mode.
        set_nonblocking(io.as_fd())?;

        Self::new_nonblocking(io)
    }

    /// Creates an async I/O handle without setting it to non-blocking mode.
    ///
    /// This method will register the handle in [epoll]/[kqueue]/[event ports]/[IOCP].
    ///
    /// On Unix systems, the handle must implement `AsFd`, while on Windows it must implement
    /// `AsSocket`.
    ///
    /// [epoll]: https://en.wikipedia.org/wiki/Epoll
    /// [kqueue]: https://en.wikipedia.org/wiki/Kqueue
    /// [event ports]: https://illumos.org/man/port_create
    /// [IOCP]: https://learn.microsoft.com/en-us/windows/win32/fileio/i-o-completion-ports
    ///
    /// # Caveats
    ///
    /// The caller should ensure that the handle is set to non-blocking mode or that it is okay if
    /// it is not set. If not set to non-blocking mode, I/O operations may block the current thread
    /// and cause a deadlock in an asynchronous context.
    pub fn new_nonblocking(io: T) -> io::Result<Async<T>> {
        // SAFETY: It is impossible to drop the I/O source while it is registered through
        // this type.
        let registration = unsafe { Registration::new(io.as_fd()) };

        Ok(Async {
            source: Reactor::get().insert_io(registration)?,
            io: Some(io),
        })
    }
}

#[cfg(unix)]
impl<T: AsRawFd> AsRawFd for Async<T> {
    fn as_raw_fd(&self) -> RawFd {
        self.get_ref().as_raw_fd()
    }
}

#[cfg(unix)]
impl<T: AsFd> AsFd for Async<T> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.get_ref().as_fd()
    }
}

#[cfg(unix)]
impl<T: AsFd + From<OwnedFd>> TryFrom<OwnedFd> for Async<T> {
    type Error = io::Error;

    fn try_from(value: OwnedFd) -> Result<Self, Self::Error> {
        Async::new(value.into())
    }
}

#[cfg(unix)]
impl<T: Into<OwnedFd>> TryFrom<Async<T>> for OwnedFd {
    type Error = io::Error;

    fn try_from(value: Async<T>) -> Result<Self, Self::Error> {
        value.into_inner().map(Into::into)
    }
}

#[cfg(windows)]
impl<T: AsSocket> Async<T> {
    /// Creates an async I/O handle.
    ///
    /// This method will put the handle in non-blocking mode and register it in
    /// [epoll]/[kqueue]/[event ports]/[IOCP].
    ///
    /// On Unix systems, the handle must implement `AsFd`, while on Windows it must implement
    /// `AsSocket`.
    ///
    /// [epoll]: https://en.wikipedia.org/wiki/Epoll
    /// [kqueue]: https://en.wikipedia.org/wiki/Kqueue
    /// [event ports]: https://illumos.org/man/port_create
    /// [IOCP]: https://learn.microsoft.com/en-us/windows/win32/fileio/i-o-completion-ports
    ///
    /// # Examples
    ///
    /// ```
    /// use async_io::Async;
    /// use std::net::{SocketAddr, TcpListener};
    ///
    /// # futures_lite::future::block_on(async {
    /// let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))?;
    /// let listener = Async::new(listener)?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn new(io: T) -> io::Result<Async<T>> {
        // Put the socket in non-blocking mode.
        set_nonblocking(io.as_socket())?;

        Self::new_nonblocking(io)
    }

    /// Creates an async I/O handle without setting it to non-blocking mode.
    ///
    /// This method will register the handle in [epoll]/[kqueue]/[event ports]/[IOCP].
    ///
    /// On Unix systems, the handle must implement `AsFd`, while on Windows it must implement
    /// `AsSocket`.
    ///
    /// [epoll]: https://en.wikipedia.org/wiki/Epoll
    /// [kqueue]: https://en.wikipedia.org/wiki/Kqueue
    /// [event ports]: https://illumos.org/man/port_create
    /// [IOCP]: https://learn.microsoft.com/en-us/windows/win32/fileio/i-o-completion-ports
    ///
    /// # Caveats
    ///
    /// The caller should ensure that the handle is set to non-blocking mode or that it is okay if
    /// it is not set. If not set to non-blocking mode, I/O operations may block the current thread
    /// and cause a deadlock in an asynchronous context.
    pub fn new_nonblocking(io: T) -> io::Result<Async<T>> {
        // Create the registration.
        //
        // SAFETY: It is impossible to drop the I/O source while it is registered through
        // this type.
        let registration = unsafe { Registration::new(io.as_socket()) };

        Ok(Async {
            source: Reactor::get().insert_io(registration)?,
            io: Some(io),
        })
    }
}

#[cfg(windows)]
impl<T: AsRawSocket> AsRawSocket for Async<T> {
    fn as_raw_socket(&self) -> RawSocket {
        self.get_ref().as_raw_socket()
    }
}

#[cfg(windows)]
impl<T: AsSocket> AsSocket for Async<T> {
    fn as_socket(&self) -> BorrowedSocket<'_> {
        self.get_ref().as_socket()
    }
}

#[cfg(windows)]
impl<T: AsSocket + From<OwnedSocket>> TryFrom<OwnedSocket> for Async<T> {
    type Error = io::Error;

    fn try_from(value: OwnedSocket) -> Result<Self, Self::Error> {
        Async::new(value.into())
    }
}

#[cfg(windows)]
impl<T: Into<OwnedSocket>> TryFrom<Async<T>> for OwnedSocket {
    type Error = io::Error;

    fn try_from(value: Async<T>) -> Result<Self, Self::Error> {
        value.into_inner().map(Into::into)
    }
}

impl<T> Async<T> {
    /// Gets a reference to the inner I/O handle.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_io::Async;
    /// use std::net::TcpListener;
    ///
    /// # futures_lite::future::block_on(async {
    /// let listener = Async::<TcpListener>::bind(([127, 0, 0, 1], 0))?;
    /// let inner = listener.get_ref();
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn get_ref(&self) -> &T {
        self.io.as_ref().unwrap()
    }

    /// Gets a mutable reference to the inner I/O handle.
    ///
    /// # Safety
    ///
    /// The underlying I/O source must not be dropped using this function.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_io::Async;
    /// use std::net::TcpListener;
    ///
    /// # futures_lite::future::block_on(async {
    /// let mut listener = Async::<TcpListener>::bind(([127, 0, 0, 1], 0))?;
    /// let inner = unsafe { listener.get_mut() };
    /// # std::io::Result::Ok(()) });
    /// ```
    pub unsafe fn get_mut(&mut self) -> &mut T {
        self.io.as_mut().unwrap()
    }

    /// Unwraps the inner I/O handle.
    ///
    /// This method will **not** put the I/O handle back into blocking mode.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_io::Async;
    /// use std::net::TcpListener;
    ///
    /// # futures_lite::future::block_on(async {
    /// let listener = Async::<TcpListener>::bind(([127, 0, 0, 1], 0))?;
    /// let inner = listener.into_inner()?;
    ///
    /// // Put the listener back into blocking mode.
    /// inner.set_nonblocking(false)?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn into_inner(mut self) -> io::Result<T> {
        let io = self.io.take().unwrap();
        Reactor::get().remove_io(&self.source)?;
        Ok(io)
    }

    /// Waits until the I/O handle is readable.
    ///
    /// This method completes when a read operation on this I/O handle wouldn't block.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::Async;
    /// use std::net::TcpListener;
    ///
    /// # futures_lite::future::block_on(async {
    /// let mut listener = Async::<TcpListener>::bind(([127, 0, 0, 1], 0))?;
    ///
    /// // Wait until a client can be accepted.
    /// listener.readable().await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn readable(&self) -> Readable<'_, T> {
        Source::readable(self)
    }

    /// Waits until the I/O handle is readable.
    ///
    /// This method completes when a read operation on this I/O handle wouldn't block.
    pub fn readable_owned(self: Arc<Self>) -> ReadableOwned<T> {
        Source::readable_owned(self)
    }

    /// Waits until the I/O handle is writable.
    ///
    /// This method completes when a write operation on this I/O handle wouldn't block.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_io::Async;
    /// use std::net::{TcpStream, ToSocketAddrs};
    ///
    /// # futures_lite::future::block_on(async {
    /// let addr = "example.com:80".to_socket_addrs()?.next().unwrap();
    /// let stream = Async::<TcpStream>::connect(addr).await?;
    ///
    /// // Wait until the stream is writable.
    /// stream.writable().await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn writable(&self) -> Writable<'_, T> {
        Source::writable(self)
    }

    /// Waits until the I/O handle is writable.
    ///
    /// This method completes when a write operation on this I/O handle wouldn't block.
    pub fn writable_owned(self: Arc<Self>) -> WritableOwned<T> {
        Source::writable_owned(self)
    }

    /// Polls the I/O handle for readability.
    ///
    /// When this method returns [`Poll::Ready`], that means the OS has delivered an event
    /// indicating readability since the last time this task has called the method and received
    /// [`Poll::Pending`].
    ///
    /// # Caveats
    ///
    /// Two different tasks should not call this method concurrently. Otherwise, conflicting tasks
    /// will just keep waking each other in turn, thus wasting CPU time.
    ///
    /// Note that the [`AsyncRead`] implementation for [`Async`] also uses this method.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::Async;
    /// use std::future::poll_fn;
    /// use std::net::TcpListener;
    ///
    /// # futures_lite::future::block_on(async {
    /// let mut listener = Async::<TcpListener>::bind(([127, 0, 0, 1], 0))?;
    ///
    /// // Wait until a client can be accepted.
    /// poll_fn(|cx| listener.poll_readable(cx)).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn poll_readable(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.source.poll_readable(cx)
    }

    /// Polls the I/O handle for writability.
    ///
    /// When this method returns [`Poll::Ready`], that means the OS has delivered an event
    /// indicating writability since the last time this task has called the method and received
    /// [`Poll::Pending`].
    ///
    /// # Caveats
    ///
    /// Two different tasks should not call this method concurrently. Otherwise, conflicting tasks
    /// will just keep waking each other in turn, thus wasting CPU time.
    ///
    /// Note that the [`AsyncWrite`] implementation for [`Async`] also uses this method.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_io::Async;
    /// use std::future::poll_fn;
    /// use std::net::{TcpStream, ToSocketAddrs};
    ///
    /// # futures_lite::future::block_on(async {
    /// let addr = "example.com:80".to_socket_addrs()?.next().unwrap();
    /// let stream = Async::<TcpStream>::connect(addr).await?;
    ///
    /// // Wait until the stream is writable.
    /// poll_fn(|cx| stream.poll_writable(cx)).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn poll_writable(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.source.poll_writable(cx)
    }

    /// Performs a read operation asynchronously.
    ///
    /// The I/O handle is registered in the reactor and put in non-blocking mode. This method
    /// invokes the `op` closure in a loop until it succeeds or returns an error other than
    /// [`io::ErrorKind::WouldBlock`]. In between iterations of the loop, it waits until the OS
    /// sends a notification that the I/O handle is readable.
    ///
    /// The closure receives a shared reference to the I/O handle.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::Async;
    /// use std::net::TcpListener;
    ///
    /// # futures_lite::future::block_on(async {
    /// let listener = Async::<TcpListener>::bind(([127, 0, 0, 1], 0))?;
    ///
    /// // Accept a new client asynchronously.
    /// let (stream, addr) = listener.read_with(|l| l.accept()).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn read_with<R>(&self, op: impl FnMut(&T) -> io::Result<R>) -> io::Result<R> {
        let mut op = op;
        loop {
            match op(self.get_ref()) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return res,
            }
            optimistic(self.readable()).await?;
        }
    }

    /// Performs a read operation asynchronously.
    ///
    /// The I/O handle is registered in the reactor and put in non-blocking mode. This method
    /// invokes the `op` closure in a loop until it succeeds or returns an error other than
    /// [`io::ErrorKind::WouldBlock`]. In between iterations of the loop, it waits until the OS
    /// sends a notification that the I/O handle is readable.
    ///
    /// The closure receives a mutable reference to the I/O handle.
    ///
    /// # Safety
    ///
    /// In the closure, the underlying I/O source must not be dropped.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::Async;
    /// use std::net::TcpListener;
    ///
    /// # futures_lite::future::block_on(async {
    /// let mut listener = Async::<TcpListener>::bind(([127, 0, 0, 1], 0))?;
    ///
    /// // Accept a new client asynchronously.
    /// let (stream, addr) = unsafe { listener.read_with_mut(|l| l.accept()).await? };
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async unsafe fn read_with_mut<R>(
        &mut self,
        op: impl FnMut(&mut T) -> io::Result<R>,
    ) -> io::Result<R> {
        let mut op = op;
        loop {
            match op(self.get_mut()) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return res,
            }
            optimistic(self.readable()).await?;
        }
    }

    /// Performs a write operation asynchronously.
    ///
    /// The I/O handle is registered in the reactor and put in non-blocking mode. This method
    /// invokes the `op` closure in a loop until it succeeds or returns an error other than
    /// [`io::ErrorKind::WouldBlock`]. In between iterations of the loop, it waits until the OS
    /// sends a notification that the I/O handle is writable.
    ///
    /// The closure receives a shared reference to the I/O handle.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::Async;
    /// use std::net::UdpSocket;
    ///
    /// # futures_lite::future::block_on(async {
    /// let socket = Async::<UdpSocket>::bind(([127, 0, 0, 1], 8000))?;
    /// socket.get_ref().connect("127.0.0.1:9000")?;
    ///
    /// let msg = b"hello";
    /// let len = socket.write_with(|s| s.send(msg)).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn write_with<R>(&self, op: impl FnMut(&T) -> io::Result<R>) -> io::Result<R> {
        let mut op = op;
        loop {
            match op(self.get_ref()) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return res,
            }
            optimistic(self.writable()).await?;
        }
    }

    /// Performs a write operation asynchronously.
    ///
    /// The I/O handle is registered in the reactor and put in non-blocking mode. This method
    /// invokes the `op` closure in a loop until it succeeds or returns an error other than
    /// [`io::ErrorKind::WouldBlock`]. In between iterations of the loop, it waits until the OS
    /// sends a notification that the I/O handle is writable.
    ///
    /// # Safety
    ///
    /// The closure receives a mutable reference to the I/O handle. In the closure, the underlying
    /// I/O source must not be dropped.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::Async;
    /// use std::net::UdpSocket;
    ///
    /// # futures_lite::future::block_on(async {
    /// let mut socket = Async::<UdpSocket>::bind(([127, 0, 0, 1], 8000))?;
    /// socket.get_ref().connect("127.0.0.1:9000")?;
    ///
    /// let msg = b"hello";
    /// let len = unsafe { socket.write_with_mut(|s| s.send(msg)).await? };
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async unsafe fn write_with_mut<R>(
        &mut self,
        op: impl FnMut(&mut T) -> io::Result<R>,
    ) -> io::Result<R> {
        let mut op = op;
        loop {
            match op(self.get_mut()) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return res,
            }
            optimistic(self.writable()).await?;
        }
    }
}

impl<T> AsRef<T> for Async<T> {
    fn as_ref(&self) -> &T {
        self.get_ref()
    }
}

impl<T> Drop for Async<T> {
    fn drop(&mut self) {
        if self.io.is_some() {
            // Deregister and ignore errors because destructors should not panic.
            Reactor::get().remove_io(&self.source).ok();

            // Drop the I/O handle to close it.
            self.io.take();
        }
    }
}

/// Types whose I/O trait implementations do not drop the underlying I/O source.
///
/// The resource contained inside of the [`Async`] cannot be invalidated. This invalidation can
/// happen if the inner resource (the [`TcpStream`], [`UnixListener`] or other `T`) is moved out
/// and dropped before the [`Async`]. Because of this, functions that grant mutable access to
/// the inner type are unsafe, as there is no way to guarantee that the source won't be dropped
/// and a dangling handle won't be left behind.
///
/// Unfortunately this extends to implementations of [`Read`] and [`Write`]. Since methods on those
/// traits take `&mut`, there is no guarantee that the implementor of those traits won't move the
/// source out while the method is being run.
///
/// This trait is an antidote to this predicament. By implementing this trait, the user pledges
/// that using any I/O traits won't destroy the source. This way, [`Async`] can implement the
/// `async` version of these I/O traits, like [`AsyncRead`] and [`AsyncWrite`].
///
/// # Safety
///
/// Any I/O trait implementations for this type must not drop the underlying I/O source. Traits
/// affected by this trait include [`Read`], [`Write`], [`Seek`] and [`BufRead`].
///
/// This trait is implemented by default on top of `libstd` types. In addition, it is implemented
/// for immutable reference types, as it is impossible to invalidate any outstanding references
/// while holding an immutable reference, even with interior mutability. As Rust's current pinning
/// system relies on similar guarantees, I believe that this approach is robust.
///
/// [`BufRead`]: https://doc.rust-lang.org/std/io/trait.BufRead.html
/// [`Read`]: https://doc.rust-lang.org/std/io/trait.Read.html
/// [`Seek`]: https://doc.rust-lang.org/std/io/trait.Seek.html
/// [`Write`]: https://doc.rust-lang.org/std/io/trait.Write.html
///
/// [`AsyncRead`]: https://docs.rs/futures-io/latest/futures_io/trait.AsyncRead.html
/// [`AsyncWrite`]: https://docs.rs/futures-io/latest/futures_io/trait.AsyncWrite.html
pub unsafe trait IoSafe {}

/// Reference types can't be mutated.
///
/// The worst thing that can happen is that external state is used to change what kind of pointer
/// `as_fd()` returns. For instance:
///
/// ```
/// # #[cfg(unix)] {
/// use std::cell::Cell;
/// use std::net::TcpStream;
/// use std::os::unix::io::{AsFd, BorrowedFd};
///
/// struct Bar {
///     flag: Cell<bool>,
///     a: TcpStream,
///     b: TcpStream
/// }
///
/// impl AsFd for Bar {
///     fn as_fd(&self) -> BorrowedFd<'_> {
///         if self.flag.replace(!self.flag.get()) {
///             self.a.as_fd()
///         } else {
///             self.b.as_fd()
///         }
///     }
/// }
/// # }
/// ```
///
/// We solve this problem by only calling `as_fd()` once to get the original source. Implementations
/// like this are considered buggy (but not unsound) and are thus not really supported by `async-io`.
unsafe impl<T: ?Sized> IoSafe for &T {}

// Can be implemented on top of libstd types.
unsafe impl IoSafe for std::fs::File {}
unsafe impl IoSafe for std::io::Stderr {}
unsafe impl IoSafe for std::io::Stdin {}
unsafe impl IoSafe for std::io::Stdout {}
unsafe impl IoSafe for std::io::StderrLock<'_> {}
unsafe impl IoSafe for std::io::StdinLock<'_> {}
unsafe impl IoSafe for std::io::StdoutLock<'_> {}
unsafe impl IoSafe for std::net::TcpStream {}
unsafe impl IoSafe for std::process::ChildStdin {}
unsafe impl IoSafe for std::process::ChildStdout {}
unsafe impl IoSafe for std::process::ChildStderr {}

#[cfg(unix)]
unsafe impl IoSafe for std::os::unix::net::UnixStream {}

// PipeReader & PipeWriter require std >= 1.87, our MSRV is 1.71, hence
// conditional on cfg()s, generated from build.rs
#[cfg(not(async_io_no_pipe))]
unsafe impl IoSafe for std::io::PipeReader {}
#[cfg(not(async_io_no_pipe))]
unsafe impl IoSafe for std::io::PipeWriter {}

unsafe impl<T: IoSafe + Read> IoSafe for std::io::BufReader<T> {}
unsafe impl<T: IoSafe + Write> IoSafe for std::io::BufWriter<T> {}
unsafe impl<T: IoSafe + Write> IoSafe for std::io::LineWriter<T> {}
unsafe impl<T: IoSafe + ?Sized> IoSafe for &mut T {}
unsafe impl<T: IoSafe + ?Sized> IoSafe for Box<T> {}
unsafe impl<T: Clone + IoSafe> IoSafe for std::borrow::Cow<'_, T> {}

impl<T: IoSafe + Read> AsyncRead for Async<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        loop {
            match unsafe { (*self).get_mut() }.read(buf) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return Poll::Ready(res),
            }
            ready!(self.poll_readable(cx))?;
        }
    }

    fn poll_read_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<io::Result<usize>> {
        loop {
            match unsafe { (*self).get_mut() }.read_vectored(bufs) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return Poll::Ready(res),
            }
            ready!(self.poll_readable(cx))?;
        }
    }
}

// Since this is through a reference, we can't mutate the inner I/O source.
// Therefore this is safe!
impl<T> AsyncRead for &Async<T>
where
    for<'a> &'a T: Read,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        loop {
            match (*self).get_ref().read(buf) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return Poll::Ready(res),
            }
            ready!(self.poll_readable(cx))?;
        }
    }

    fn poll_read_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<io::Result<usize>> {
        loop {
            match (*self).get_ref().read_vectored(bufs) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return Poll::Ready(res),
            }
            ready!(self.poll_readable(cx))?;
        }
    }
}

impl<T: IoSafe + Write> AsyncWrite for Async<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        loop {
            match unsafe { (*self).get_mut() }.write(buf) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return Poll::Ready(res),
            }
            ready!(self.poll_writable(cx))?;
        }
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        loop {
            match unsafe { (*self).get_mut() }.write_vectored(bufs) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return Poll::Ready(res),
            }
            ready!(self.poll_writable(cx))?;
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        loop {
            match unsafe { (*self).get_mut() }.flush() {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return Poll::Ready(res),
            }
            ready!(self.poll_writable(cx))?;
        }
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_flush(cx)
    }
}

impl<T> AsyncWrite for &Async<T>
where
    for<'a> &'a T: Write,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        loop {
            match (*self).get_ref().write(buf) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return Poll::Ready(res),
            }
            ready!(self.poll_writable(cx))?;
        }
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        loop {
            match (*self).get_ref().write_vectored(bufs) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return Poll::Ready(res),
            }
            ready!(self.poll_writable(cx))?;
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        loop {
            match (*self).get_ref().flush() {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return Poll::Ready(res),
            }
            ready!(self.poll_writable(cx))?;
        }
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_flush(cx)
    }
}

impl Async<TcpListener> {
    /// Creates a TCP listener bound to the specified address.
    ///
    /// Binding with port number 0 will request an available port from the OS.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_io::Async;
    /// use std::net::TcpListener;
    ///
    /// # futures_lite::future::block_on(async {
    /// let listener = Async::<TcpListener>::bind(([127, 0, 0, 1], 0))?;
    /// println!("Listening on {}", listener.get_ref().local_addr()?);
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn bind<A: Into<SocketAddr>>(addr: A) -> io::Result<Async<TcpListener>> {
        let addr = addr.into();
        Async::new(TcpListener::bind(addr)?)
    }

    /// Accepts a new incoming TCP connection.
    ///
    /// When a connection is established, it will be returned as a TCP stream together with its
    /// remote address.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::Async;
    /// use std::net::TcpListener;
    ///
    /// # futures_lite::future::block_on(async {
    /// let listener = Async::<TcpListener>::bind(([127, 0, 0, 1], 8000))?;
    /// let (stream, addr) = listener.accept().await?;
    /// println!("Accepted client: {}", addr);
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn accept(&self) -> io::Result<(Async<TcpStream>, SocketAddr)> {
        let (stream, addr) = self.read_with(|io| io.accept()).await?;
        Ok((Async::new(stream)?, addr))
    }

    /// Returns a stream of incoming TCP connections.
    ///
    /// The stream is infinite, i.e. it never stops with a [`None`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::Async;
    /// use futures_lite::{stream::StreamExt};
    /// use std::net::TcpListener;
    /// use std::pin::pin;
    ///
    /// # futures_lite::future::block_on(async {
    /// let listener = Async::<TcpListener>::bind(([127, 0, 0, 1], 8000))?;
    /// let incoming = listener.incoming();
    /// let mut incoming = pin!(incoming);
    ///
    /// while let Some(stream) = incoming.next().await {
    ///     let stream = stream?;
    ///     println!("Accepted client: {}", stream.get_ref().peer_addr()?);
    /// }
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn incoming(&self) -> impl Stream<Item = io::Result<Async<TcpStream>>> + Send + '_ {
        stream::unfold(self, |listener| async move {
            let res = listener.accept().await.map(|(stream, _)| stream);
            Some((res, listener))
        })
    }
}

impl TryFrom<std::net::TcpListener> for Async<std::net::TcpListener> {
    type Error = io::Error;

    fn try_from(listener: std::net::TcpListener) -> io::Result<Self> {
        Async::new(listener)
    }
}

impl Async<TcpStream> {
    /// Creates a TCP connection to the specified address.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_io::Async;
    /// use std::net::{TcpStream, ToSocketAddrs};
    ///
    /// # futures_lite::future::block_on(async {
    /// let addr = "example.com:80".to_socket_addrs()?.next().unwrap();
    /// let stream = Async::<TcpStream>::connect(addr).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn connect<A: Into<SocketAddr>>(addr: A) -> io::Result<Async<TcpStream>> {
        // Figure out how to handle this address.
        let addr = addr.into();
        let (domain, sock_addr) = match addr {
            SocketAddr::V4(v4) => (rn::AddressFamily::INET, v4.as_any()),
            SocketAddr::V6(v6) => (rn::AddressFamily::INET6, v6.as_any()),
        };

        // Begin async connect.
        let socket = connect(sock_addr, domain, Some(rn::ipproto::TCP))?;
        // Use new_nonblocking because connect already sets socket to non-blocking mode.
        let stream = Async::new_nonblocking(TcpStream::from(socket))?;

        // The stream becomes writable when connected.
        stream.writable().await?;

        // Check if there was an error while connecting.
        match stream.get_ref().take_error()? {
            None => Ok(stream),
            Some(err) => Err(err),
        }
    }

    /// Reads data from the stream without removing it from the buffer.
    ///
    /// Returns the number of bytes read. Successive calls of this method read the same data.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_io::Async;
    /// use futures_lite::{io::AsyncWriteExt, stream::StreamExt};
    /// use std::net::{TcpStream, ToSocketAddrs};
    ///
    /// # futures_lite::future::block_on(async {
    /// let addr = "example.com:80".to_socket_addrs()?.next().unwrap();
    /// let mut stream = Async::<TcpStream>::connect(addr).await?;
    ///
    /// stream
    ///     .write_all(b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n")
    ///     .await?;
    ///
    /// let mut buf = [0u8; 1024];
    /// let len = stream.peek(&mut buf).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.read_with(|io| io.peek(buf)).await
    }
}

impl TryFrom<std::net::TcpStream> for Async<std::net::TcpStream> {
    type Error = io::Error;

    fn try_from(stream: std::net::TcpStream) -> io::Result<Self> {
        Async::new(stream)
    }
}

impl Async<UdpSocket> {
    /// Creates a UDP socket bound to the specified address.
    ///
    /// Binding with port number 0 will request an available port from the OS.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_io::Async;
    /// use std::net::UdpSocket;
    ///
    /// # futures_lite::future::block_on(async {
    /// let socket = Async::<UdpSocket>::bind(([127, 0, 0, 1], 0))?;
    /// println!("Bound to {}", socket.get_ref().local_addr()?);
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn bind<A: Into<SocketAddr>>(addr: A) -> io::Result<Async<UdpSocket>> {
        let addr = addr.into();
        Async::new(UdpSocket::bind(addr)?)
    }

    /// Receives a single datagram message.
    ///
    /// Returns the number of bytes read and the address the message came from.
    ///
    /// This method must be called with a valid byte slice of sufficient size to hold the message.
    /// If the message is too long to fit, excess bytes may get discarded.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::Async;
    /// use std::net::UdpSocket;
    ///
    /// # futures_lite::future::block_on(async {
    /// let socket = Async::<UdpSocket>::bind(([127, 0, 0, 1], 8000))?;
    ///
    /// let mut buf = [0u8; 1024];
    /// let (len, addr) = socket.recv_from(&mut buf).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.read_with(|io| io.recv_from(buf)).await
    }

    /// Receives a single datagram message without removing it from the queue.
    ///
    /// Returns the number of bytes read and the address the message came from.
    ///
    /// This method must be called with a valid byte slice of sufficient size to hold the message.
    /// If the message is too long to fit, excess bytes may get discarded.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::Async;
    /// use std::net::UdpSocket;
    ///
    /// # futures_lite::future::block_on(async {
    /// let socket = Async::<UdpSocket>::bind(([127, 0, 0, 1], 8000))?;
    ///
    /// let mut buf = [0u8; 1024];
    /// let (len, addr) = socket.peek_from(&mut buf).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn peek_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.read_with(|io| io.peek_from(buf)).await
    }

    /// Sends data to the specified address.
    ///
    /// Returns the number of bytes written.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::Async;
    /// use std::net::UdpSocket;
    ///
    /// # futures_lite::future::block_on(async {
    /// let socket = Async::<UdpSocket>::bind(([127, 0, 0, 1], 0))?;
    /// let addr = socket.get_ref().local_addr()?;
    ///
    /// let msg = b"hello";
    /// let len = socket.send_to(msg, addr).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn send_to<A: Into<SocketAddr>>(&self, buf: &[u8], addr: A) -> io::Result<usize> {
        let addr = addr.into();
        self.write_with(|io| io.send_to(buf, addr)).await
    }

    /// Receives a single datagram message from the connected peer.
    ///
    /// Returns the number of bytes read.
    ///
    /// This method must be called with a valid byte slice of sufficient size to hold the message.
    /// If the message is too long to fit, excess bytes may get discarded.
    ///
    /// The [`connect`][`UdpSocket::connect()`] method connects this socket to a remote address.
    /// This method will fail if the socket is not connected.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::Async;
    /// use std::net::UdpSocket;
    ///
    /// # futures_lite::future::block_on(async {
    /// let socket = Async::<UdpSocket>::bind(([127, 0, 0, 1], 8000))?;
    /// socket.get_ref().connect("127.0.0.1:9000")?;
    ///
    /// let mut buf = [0u8; 1024];
    /// let len = socket.recv(&mut buf).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.read_with(|io| io.recv(buf)).await
    }

    /// Receives a single datagram message from the connected peer without removing it from the
    /// queue.
    ///
    /// Returns the number of bytes read and the address the message came from.
    ///
    /// This method must be called with a valid byte slice of sufficient size to hold the message.
    /// If the message is too long to fit, excess bytes may get discarded.
    ///
    /// The [`connect`][`UdpSocket::connect()`] method connects this socket to a remote address.
    /// This method will fail if the socket is not connected.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::Async;
    /// use std::net::UdpSocket;
    ///
    /// # futures_lite::future::block_on(async {
    /// let socket = Async::<UdpSocket>::bind(([127, 0, 0, 1], 8000))?;
    /// socket.get_ref().connect("127.0.0.1:9000")?;
    ///
    /// let mut buf = [0u8; 1024];
    /// let len = socket.peek(&mut buf).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.read_with(|io| io.peek(buf)).await
    }

    /// Sends data to the connected peer.
    ///
    /// Returns the number of bytes written.
    ///
    /// The [`connect`][`UdpSocket::connect()`] method connects this socket to a remote address.
    /// This method will fail if the socket is not connected.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::Async;
    /// use std::net::UdpSocket;
    ///
    /// # futures_lite::future::block_on(async {
    /// let socket = Async::<UdpSocket>::bind(([127, 0, 0, 1], 8000))?;
    /// socket.get_ref().connect("127.0.0.1:9000")?;
    ///
    /// let msg = b"hello";
    /// let len = socket.send(msg).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.write_with(|io| io.send(buf)).await
    }
}

impl TryFrom<std::net::UdpSocket> for Async<std::net::UdpSocket> {
    type Error = io::Error;

    fn try_from(socket: std::net::UdpSocket) -> io::Result<Self> {
        Async::new(socket)
    }
}

#[cfg(unix)]
impl Async<UnixListener> {
    /// Creates a UDS listener bound to the specified path.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::Async;
    /// use std::os::unix::net::UnixListener;
    ///
    /// # futures_lite::future::block_on(async {
    /// let listener = Async::<UnixListener>::bind("/tmp/socket")?;
    /// println!("Listening on {:?}", listener.get_ref().local_addr()?);
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<Async<UnixListener>> {
        let path = path.as_ref().to_owned();
        Async::new(UnixListener::bind(path)?)
    }

    /// Accepts a new incoming UDS stream connection.
    ///
    /// When a connection is established, it will be returned as a stream together with its remote
    /// address.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::Async;
    /// use std::os::unix::net::UnixListener;
    ///
    /// # futures_lite::future::block_on(async {
    /// let listener = Async::<UnixListener>::bind("/tmp/socket")?;
    /// let (stream, addr) = listener.accept().await?;
    /// println!("Accepted client: {:?}", addr);
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn accept(&self) -> io::Result<(Async<UnixStream>, UnixSocketAddr)> {
        let (stream, addr) = self.read_with(|io| io.accept()).await?;
        Ok((Async::new(stream)?, addr))
    }

    /// Returns a stream of incoming UDS connections.
    ///
    /// The stream is infinite, i.e. it never stops with a [`None`] item.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::Async;
    /// use futures_lite::stream::StreamExt;
    /// use std::os::unix::net::UnixListener;
    /// use std::pin::pin;
    ///
    /// # futures_lite::future::block_on(async {
    /// let listener = Async::<UnixListener>::bind("/tmp/socket")?;
    /// let incoming = listener.incoming();
    /// let mut incoming = pin!(incoming);
    ///
    /// while let Some(stream) = incoming.next().await {
    ///     let stream = stream?;
    ///     println!("Accepted client: {:?}", stream.get_ref().peer_addr()?);
    /// }
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn incoming(&self) -> impl Stream<Item = io::Result<Async<UnixStream>>> + Send + '_ {
        stream::unfold(self, |listener| async move {
            let res = listener.accept().await.map(|(stream, _)| stream);
            Some((res, listener))
        })
    }
}

#[cfg(unix)]
impl TryFrom<std::os::unix::net::UnixListener> for Async<std::os::unix::net::UnixListener> {
    type Error = io::Error;

    fn try_from(listener: std::os::unix::net::UnixListener) -> io::Result<Self> {
        Async::new(listener)
    }
}

#[cfg(unix)]
impl Async<UnixStream> {
    /// Creates a UDS stream connected to the specified path.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::Async;
    /// use std::os::unix::net::UnixStream;
    ///
    /// # futures_lite::future::block_on(async {
    /// let stream = Async::<UnixStream>::connect("/tmp/socket").await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn connect<P: AsRef<Path>>(path: P) -> io::Result<Async<UnixStream>> {
        let address = convert_path_to_socket_address(path.as_ref())?;

        // Begin async connect.
        let socket = connect(address.into(), rn::AddressFamily::UNIX, None)?;
        // Use new_nonblocking because connect already sets socket to non-blocking mode.
        let stream = Async::new_nonblocking(UnixStream::from(socket))?;

        // The stream becomes writable when connected.
        stream.writable().await?;

        // On Linux, it appears the socket may become writable even when connecting fails, so we
        // must do an extra check here and see if the peer address is retrievable.
        stream.get_ref().peer_addr()?;
        Ok(stream)
    }

    /// Creates an unnamed pair of connected UDS stream sockets.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::Async;
    /// use std::os::unix::net::UnixStream;
    ///
    /// # futures_lite::future::block_on(async {
    /// let (stream1, stream2) = Async::<UnixStream>::pair()?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn pair() -> io::Result<(Async<UnixStream>, Async<UnixStream>)> {
        let (stream1, stream2) = UnixStream::pair()?;
        Ok((Async::new(stream1)?, Async::new(stream2)?))
    }
}

#[cfg(unix)]
impl TryFrom<std::os::unix::net::UnixStream> for Async<std::os::unix::net::UnixStream> {
    type Error = io::Error;

    fn try_from(stream: std::os::unix::net::UnixStream) -> io::Result<Self> {
        Async::new(stream)
    }
}

#[cfg(unix)]
impl Async<UnixDatagram> {
    /// Creates a UDS datagram socket bound to the specified path.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::Async;
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// # futures_lite::future::block_on(async {
    /// let socket = Async::<UnixDatagram>::bind("/tmp/socket")?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<Async<UnixDatagram>> {
        let path = path.as_ref().to_owned();
        Async::new(UnixDatagram::bind(path)?)
    }

    /// Creates a UDS datagram socket not bound to any address.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::Async;
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// # futures_lite::future::block_on(async {
    /// let socket = Async::<UnixDatagram>::unbound()?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn unbound() -> io::Result<Async<UnixDatagram>> {
        Async::new(UnixDatagram::unbound()?)
    }

    /// Creates an unnamed pair of connected Unix datagram sockets.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::Async;
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// # futures_lite::future::block_on(async {
    /// let (socket1, socket2) = Async::<UnixDatagram>::pair()?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn pair() -> io::Result<(Async<UnixDatagram>, Async<UnixDatagram>)> {
        let (socket1, socket2) = UnixDatagram::pair()?;
        Ok((Async::new(socket1)?, Async::new(socket2)?))
    }

    /// Receives data from the socket.
    ///
    /// Returns the number of bytes read and the address the message came from.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::Async;
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// # futures_lite::future::block_on(async {
    /// let socket = Async::<UnixDatagram>::bind("/tmp/socket")?;
    ///
    /// let mut buf = [0u8; 1024];
    /// let (len, addr) = socket.recv_from(&mut buf).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, UnixSocketAddr)> {
        self.read_with(|io| io.recv_from(buf)).await
    }

    /// Sends data to the specified address.
    ///
    /// Returns the number of bytes written.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::Async;
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// # futures_lite::future::block_on(async {
    /// let socket = Async::<UnixDatagram>::unbound()?;
    ///
    /// let msg = b"hello";
    /// let addr = "/tmp/socket";
    /// let len = socket.send_to(msg, addr).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn send_to<P: AsRef<Path>>(&self, buf: &[u8], path: P) -> io::Result<usize> {
        self.write_with(|io| io.send_to(buf, &path)).await
    }

    /// Receives data from the connected peer.
    ///
    /// Returns the number of bytes read and the address the message came from.
    ///
    /// The [`connect`][`UnixDatagram::connect()`] method connects this socket to a remote address.
    /// This method will fail if the socket is not connected.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::Async;
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// # futures_lite::future::block_on(async {
    /// let socket = Async::<UnixDatagram>::bind("/tmp/socket1")?;
    /// socket.get_ref().connect("/tmp/socket2")?;
    ///
    /// let mut buf = [0u8; 1024];
    /// let len = socket.recv(&mut buf).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.read_with(|io| io.recv(buf)).await
    }

    /// Sends data to the connected peer.
    ///
    /// Returns the number of bytes written.
    ///
    /// The [`connect`][`UnixDatagram::connect()`] method connects this socket to a remote address.
    /// This method will fail if the socket is not connected.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::Async;
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// # futures_lite::future::block_on(async {
    /// let socket = Async::<UnixDatagram>::bind("/tmp/socket1")?;
    /// socket.get_ref().connect("/tmp/socket2")?;
    ///
    /// let msg = b"hello";
    /// let len = socket.send(msg).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.write_with(|io| io.send(buf)).await
    }
}

#[cfg(unix)]
impl TryFrom<std::os::unix::net::UnixDatagram> for Async<std::os::unix::net::UnixDatagram> {
    type Error = io::Error;

    fn try_from(socket: std::os::unix::net::UnixDatagram) -> io::Result<Self> {
        Async::new(socket)
    }
}

/// Polls a future once, waits for a wakeup, and then optimistically assumes the future is ready.
async fn optimistic(fut: impl Future<Output = io::Result<()>>) -> io::Result<()> {
    let mut polled = false;
    let mut fut = pin!(fut);

    poll_fn(|cx| {
        if !polled {
            polled = true;
            fut.as_mut().poll(cx)
        } else {
            Poll::Ready(Ok(()))
        }
    })
    .await
}

fn connect(
    addr: rn::SocketAddrAny,
    domain: rn::AddressFamily,
    protocol: Option<rn::Protocol>,
) -> io::Result<rustix::fd::OwnedFd> {
    #[cfg(windows)]
    use rustix::fd::AsFd;

    setup_networking();

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
    let socket = rn::socket_with(
        domain,
        rn::SocketType::STREAM,
        rn::SocketFlags::CLOEXEC | rn::SocketFlags::NONBLOCK,
        protocol,
    )?;

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
    let socket = {
        #[cfg(not(any(
            target_os = "aix",
            target_vendor = "apple",
            target_os = "espidf",
            target_os = "haiku",
            windows,
        )))]
        let flags = rn::SocketFlags::CLOEXEC;
        #[cfg(any(
            target_os = "aix",
            target_vendor = "apple",
            target_os = "espidf",
            target_os = "haiku",
            windows,
        ))]
        let flags = rn::SocketFlags::empty();

        // Create the socket.
        let socket = rn::socket_with(domain, rn::SocketType::STREAM, flags, protocol)?;

        // Set cloexec if necessary.
        #[cfg(any(target_os = "aix", target_vendor = "apple"))]
        rio::fcntl_setfd(&socket, rio::fcntl_getfd(&socket)? | rio::FdFlags::CLOEXEC)?;

        // Set non-blocking mode.
        set_nonblocking(socket.as_fd())?;

        socket
    };

    // Set nosigpipe if necessary.
    #[cfg(any(
        target_vendor = "apple",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "dragonfly",
    ))]
    rn::sockopt::set_socket_nosigpipe(&socket, true)?;

    // Set the handle information to HANDLE_FLAG_INHERIT.
    #[cfg(windows)]
    unsafe {
        if windows_sys::Win32::Foundation::SetHandleInformation(
            socket.as_raw_socket() as _,
            windows_sys::Win32::Foundation::HANDLE_FLAG_INHERIT,
            windows_sys::Win32::Foundation::HANDLE_FLAG_INHERIT,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
    }

    #[allow(unreachable_patterns)]
    match rn::connect(&socket, &addr) {
        Ok(_) => {}
        #[cfg(unix)]
        Err(rio::Errno::INPROGRESS) => {}
        Err(rio::Errno::AGAIN) | Err(rio::Errno::WOULDBLOCK) => {}
        Err(err) => return Err(err.into()),
    }
    Ok(socket)
}

#[inline]
fn setup_networking() {
    #[cfg(windows)]
    {
        // On Windows, we need to call WSAStartup before calling any networking code.
        // Make sure to call it at least once.
        static INIT: std::sync::Once = std::sync::Once::new();

        INIT.call_once(|| {
            let _ = rustix::net::wsa_startup();
        });
    }
}

#[inline]
fn set_nonblocking(
    #[cfg(unix)] fd: BorrowedFd<'_>,
    #[cfg(windows)] fd: BorrowedSocket<'_>,
) -> io::Result<()> {
    cfg_if::cfg_if! {
        // ioctl(FIONBIO) sets the flag atomically, but we use this only on Linux
        // for now, as with the standard library, because it seems to behave
        // differently depending on the platform.
        // https://github.com/rust-lang/rust/commit/efeb42be2837842d1beb47b51bb693c7474aba3d
        // https://github.com/libuv/libuv/blob/e9d91fccfc3e5ff772d5da90e1c4a24061198ca0/src/unix/poll.c#L78-L80
        // https://github.com/tokio-rs/mio/commit/0db49f6d5caf54b12176821363d154384357e70a
        if #[cfg(any(windows, target_os = "linux"))] {
            rustix::io::ioctl_fionbio(fd, true)?;
        } else {
            let previous = rustix::fs::fcntl_getfl(fd)?;
            let new = previous | rustix::fs::OFlags::NONBLOCK;
            if new != previous {
                rustix::fs::fcntl_setfl(fd, new)?;
            }
        }
    }

    Ok(())
}

/// Converts a `Path` to its socket address representation.
///
/// This function is abstract socket-aware.
#[cfg(unix)]
#[inline]
fn convert_path_to_socket_address(path: &Path) -> io::Result<rn::SocketAddrUnix> {
    // SocketAddrUnix::new() will throw EINVAL when a path with a zero in it is passed in.
    // However, some users expect to be able to pass in paths to abstract sockets, which
    // triggers this error as it has a zero in it. Therefore, if a path starts with a zero,
    // make it an abstract socket.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    let address = {
        use std::os::unix::ffi::OsStrExt;

        let path = path.as_os_str();
        match path.as_bytes().first() {
            Some(0) => rn::SocketAddrUnix::new_abstract_name(path.as_bytes().get(1..).unwrap())?,
            _ => rn::SocketAddrUnix::new(path)?,
        }
    };

    // Only Linux and Android support abstract sockets.
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    let address = rn::SocketAddrUnix::new(path)?;

    Ok(address)
}
