//! Functionality that is only available on Windows.

use crate::reactor::{Reactor, Readable, Registration};
use crate::Async;

use std::future::Future;
use std::io::{self, Result};
use std::os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle, OwnedHandle, RawHandle};
use std::pin::Pin;
use std::task::{Context, Poll};

/// A waitable handle registered in the reactor.
///
/// Some handles in Windows are “waitable”, which means that they emit a “readiness” signal after some event occurs. This function can be used to wait for such events to occur on a handle. This function can be used in addition to regular socket polling.
///
/// Waitable objects include the following:
///
/// - Console inputs
/// - Waitable events
/// - Mutexes
/// - Processes
/// - Semaphores
/// - Threads
/// - Timer
///
/// This structure can be used to wait for any of these objects to become ready.
///
/// ## Implementation
///
/// The current implementation waits on the handle by registering it in the application-global
/// Win32 threadpool. However, in the future it may be possible to migrate to an implementation
/// on Windows 10 that uses a mechanism similar to [`MsgWaitForMultipleObjectsEx`].
///
/// [`MsgWaitForMultipleObjectsEx`]: https://docs.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-msgwaitformultipleobjectsex
///
/// ## Caveats
///
/// Read the documentation for the [`Async`](crate::Async) type for more information regarding the
/// abilities and caveats with using this type.
#[derive(Debug)]
pub struct Waitable<T>(Async<T>);

impl<T> AsRef<T> for Waitable<T> {
    fn as_ref(&self) -> &T {
        self.0.as_ref()
    }
}

impl<T: AsHandle> Waitable<T> {
    /// Create a new [`Waitable`] around a waitable handle.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::process::Command;
    /// use async_io::os::windows::Waitable;
    ///
    /// // Create a new process to wait for.
    /// let mut child = Command::new("sleep").arg("5").spawn().unwrap();
    ///
    /// // Wrap the process in an `Async` object that waits for it to exit.
    /// let process = Waitable::new(child).unwrap();
    ///
    /// // Wait for the process to exit.
    /// # async_io::block_on(async {
    /// process.ready().await.unwrap();
    /// # });
    /// ```
    pub fn new(handle: T) -> Result<Self> {
        Ok(Self(Async {
            source: Reactor::get()
                .insert_io(unsafe { Registration::new_waitable(handle.as_handle()) })?,
            io: Some(handle),
        }))
    }
}

impl<T: AsRawHandle> AsRawHandle for Waitable<T> {
    fn as_raw_handle(&self) -> RawHandle {
        self.get_ref().as_raw_handle()
    }
}

impl<T: AsHandle> AsHandle for Waitable<T> {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.get_ref().as_handle()
    }
}

impl<T: AsHandle + From<OwnedHandle>> TryFrom<OwnedHandle> for Waitable<T> {
    type Error = io::Error;

    fn try_from(handle: OwnedHandle) -> Result<Self> {
        Self::new(handle.into())
    }
}

impl<T: Into<OwnedHandle>> TryFrom<Waitable<T>> for OwnedHandle {
    type Error = io::Error;

    fn try_from(value: Waitable<T>) -> std::result::Result<Self, Self::Error> {
        value.into_inner().map(|handle| handle.into())
    }
}

impl<T> Waitable<T> {
    /// Get a reference to the inner handle.
    pub fn get_ref(&self) -> &T {
        self.0.get_ref()
    }

    /// Get a mutable reference to the inner handle.
    ///
    /// # Safety
    ///
    /// The underlying I/O source must not be dropped or moved out using this function.
    pub unsafe fn get_mut(&mut self) -> &mut T {
        self.0.get_mut()
    }

    /// Consumes the [`Waitable`], returning the inner handle.
    pub fn into_inner(self) -> Result<T> {
        self.0.into_inner()
    }

    /// Waits until the [`Waitable`] object is ready.
    ///
    /// This method completes when the underlying [`Waitable`] object has completed. See the documentation
    /// for the [`Waitable`] object for more information.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::process::Command;
    /// use async_io::os::windows::Waitable;
    ///
    /// # futures_lite::future::block_on(async {
    /// let child = Command::new("sleep").arg("5").spawn()?;
    /// let process = Waitable::new(child)?;
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
    /// that the underlying [`Waitable`] object is ready. See the documentation for the [`Waitable`]
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
    /// use async_io::os::windows::Waitable;
    /// use std::future::poll_fn;
    /// use std::process::Command;
    ///
    /// # futures_lite::future::block_on(async {
    /// let child = Command::new("sleep").arg("5").spawn()?;
    /// let process = Waitable::new(child)?;
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
