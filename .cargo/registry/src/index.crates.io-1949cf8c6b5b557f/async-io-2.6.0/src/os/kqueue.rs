//! Functionality that is only available for `kqueue`-based platforms.

use __private::QueueableSealed;

use crate::reactor::{Reactor, Readable, Registration};
use crate::Async;

use std::future::Future;
use std::io::{Error, Result};
use std::num::NonZeroI32;
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd};
use std::pin::Pin;
use std::process::Child;
use std::task::{Context, Poll};

/// A wrapper around a queueable object that waits until it is ready.
///
/// The underlying `kqueue` implementation can be used to poll for events besides file descriptor
/// read/write readiness. This API makes these faculties available to the user.
///
/// See the [`Queueable`] trait and its implementors for objects that currently support being registered
/// into the reactor.
#[derive(Debug)]
pub struct Filter<T>(Async<T>);

impl<T> AsRef<T> for Filter<T> {
    fn as_ref(&self) -> &T {
        self.0.as_ref()
    }
}

impl<T> AsMut<T> for Filter<T> {
    fn as_mut(&mut self) -> &mut T {
        self.get_mut()
    }
}

impl<T: Queueable> Filter<T> {
    /// Create a new [`Filter`] around a [`Queueable`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::process::Command;
    /// use async_io::os::kqueue::{Exit, Filter};
    ///
    /// // Create a new process to wait for.
    /// let mut child = Command::new("sleep").arg("5").spawn().unwrap();
    ///
    /// // Wrap the process in an `Async` object that waits for it to exit.
    /// let process = Filter::new(Exit::new(child)).unwrap();
    ///
    /// // Wait for the process to exit.
    /// # async_io::block_on(async {
    /// process.ready().await.unwrap();
    /// # });
    /// ```
    pub fn new(mut filter: T) -> Result<Self> {
        Ok(Self(Async {
            source: Reactor::get().insert_io(filter.registration())?,
            io: Some(filter),
        }))
    }
}

impl<T: AsRawFd> AsRawFd for Filter<T> {
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

impl<T: AsFd> AsFd for Filter<T> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl<T: AsFd + From<OwnedFd>> TryFrom<OwnedFd> for Filter<T> {
    type Error = Error;

    fn try_from(fd: OwnedFd) -> Result<Self> {
        Ok(Self(Async::try_from(fd)?))
    }
}

impl<T: Into<OwnedFd>> TryFrom<Filter<T>> for OwnedFd {
    type Error = Error;

    fn try_from(filter: Filter<T>) -> Result<Self> {
        filter.0.try_into()
    }
}

impl<T> Filter<T> {
    /// Gets a reference to the underlying [`Queueable`] object.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_io::os::kqueue::{Exit, Filter};
    ///
    /// # futures_lite::future::block_on(async {
    /// let child = std::process::Command::new("sleep").arg("5").spawn().unwrap();
    /// let process = Filter::new(Exit::new(child)).unwrap();
    /// let inner = process.get_ref();
    /// # });
    /// ```
    pub fn get_ref(&self) -> &T {
        self.0.get_ref()
    }

    /// Gets a mutable reference to the underlying [`Queueable`] object.
    ///
    /// Unlike in [`Async`], this method is safe to call, since dropping the [`Filter`] will
    /// not cause any undefined behavior.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_io::os::kqueue::{Exit, Filter};
    ///
    /// # futures_lite::future::block_on(async {
    /// let child = std::process::Command::new("sleep").arg("5").spawn().unwrap();
    /// let mut process = Filter::new(Exit::new(child)).unwrap();
    /// let inner = process.get_mut();
    /// # });
    /// ```
    pub fn get_mut(&mut self) -> &mut T {
        unsafe { self.0.get_mut() }
    }

    /// Unwraps the inner [`Queueable`] object.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_io::os::kqueue::{Exit, Filter};
    ///
    /// # futures_lite::future::block_on(async {
    /// let child = std::process::Command::new("sleep").arg("5").spawn().unwrap();
    /// let process = Filter::new(Exit::new(child)).unwrap();
    /// let inner = process.into_inner().unwrap();
    /// # });
    /// ```
    pub fn into_inner(self) -> Result<T> {
        self.0.into_inner()
    }

    /// Waits until the [`Queueable`] object is ready.
    ///
    /// This method completes when the underlying [`Queueable`] object has completed. See the documentation
    /// for the [`Queueable`] object for more information.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::process::Command;
    /// use async_io::os::kqueue::{Exit, Filter};
    ///
    /// # futures_lite::future::block_on(async {
    /// let child = Command::new("sleep").arg("5").spawn()?;
    /// let process = Filter::new(Exit::new(child))?;
    ///
    /// // Wait for the process to exit.
    /// process.ready().await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn ready(&self) -> Ready<'_, T> {
        Ready(self.0.readable())
    }

    /// Polls the I/O handle for readiness.
    ///
    /// When this method returns [`Poll::Ready`], that means that the OS has delivered a notification
    /// that the underlying [`Queueable`] object is ready. See the documentation for the [`Queueable`]
    /// object for more information.
    ///
    /// # Caveats
    ///
    /// Two different tasks should not call this method concurrently. Otherwise, conflicting tasks
    /// will just keep waking each other in turn, thus wasting CPU time.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_io::os::kqueue::{Exit, Filter};
    /// use std::future::poll_fn;
    /// use std::process::Command;
    ///
    /// # futures_lite::future::block_on(async {
    /// let child = Command::new("sleep").arg("5").spawn()?;
    /// let process = Filter::new(Exit::new(child))?;
    ///
    /// // Wait for the process to exit.
    /// poll_fn(|cx| process.poll_ready(cx)).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<()>> {
        self.0.poll_readable(cx)
    }
}

/// Future for [`Filter::ready`].
#[must_use = "futures do nothing unless you `.await` or poll them"]
#[derive(Debug)]
pub struct Ready<'a, T>(Readable<'a, T>);

impl<T> Future for Ready<'_, T> {
    type Output = Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx)
    }
}

/// Objects that can be registered into the reactor via a [`Async`](crate::Async).
///
/// These objects represent other filters associated with the `kqueue` runtime aside from readability
/// and writability. Rather than waiting on readable/writable, they wait on "readiness". This is
/// typically used for signals and child process exits.
pub trait Queueable: QueueableSealed {}

/// An object representing a signal.
///
/// When registered into [`Async`](crate::Async) via [`with_filter`](AsyncKqueueExt::with_filter),
/// it will return a [`readable`](crate::Async::readable) event when the signal is received.
#[derive(Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct Signal(pub i32);

impl QueueableSealed for Signal {
    fn registration(&mut self) -> Registration {
        Registration::Signal(*self)
    }
}
impl Queueable for Signal {}

/// Wait for a child process to exit.
///
/// When registered into [`Async`](crate::Async) via [`with_filter`](AsyncKqueueExt::with_filter),
/// it will return a [`readable`](crate::Async::readable) event when the child process exits.
#[derive(Debug)]
pub struct Exit(NonZeroI32);

impl Exit {
    /// Create a new `Exit` object.
    pub fn new(child: Child) -> Self {
        Self(
            NonZeroI32::new(child.id().try_into().expect("unable to parse pid"))
                .expect("cannot register pid with zero value"),
        )
    }

    /// Create a new `Exit` object from a PID.
    ///
    /// # Safety
    ///
    /// The PID must be tied to an actual child process.
    pub unsafe fn from_pid(pid: NonZeroI32) -> Self {
        Self(pid)
    }
}

impl QueueableSealed for Exit {
    fn registration(&mut self) -> Registration {
        Registration::Process(self.0)
    }
}
impl Queueable for Exit {}

mod __private {
    use crate::reactor::Registration;

    #[doc(hidden)]
    pub trait QueueableSealed {
        /// Get a registration object for this filter.
        fn registration(&mut self) -> Registration;
    }
}
