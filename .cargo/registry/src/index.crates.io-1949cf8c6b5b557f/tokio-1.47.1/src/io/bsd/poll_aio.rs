//! Use POSIX AIO futures with Tokio.

use crate::io::interest::Interest;
use crate::runtime::io::{ReadyEvent, Registration};
use crate::runtime::scheduler;
use mio::event::Source;
use mio::Registry;
use mio::Token;
use std::fmt;
use std::io;
use std::ops::{Deref, DerefMut};
use std::os::unix::io::AsRawFd;
use std::os::unix::prelude::RawFd;
use std::task::{ready, Context, Poll};

/// Like [`mio::event::Source`], but for POSIX AIO only.
///
/// Tokio's consumer must pass an implementor of this trait to create a
/// [`Aio`] object.
pub trait AioSource {
    /// Registers this AIO event source with Tokio's reactor.
    fn register(&mut self, kq: RawFd, token: usize);

    /// Deregisters this AIO event source with Tokio's reactor.
    fn deregister(&mut self);
}

/// Wraps the user's AioSource in order to implement mio::event::Source, which
/// is what the rest of the crate wants.
struct MioSource<T>(T);

impl<T: AioSource> Source for MioSource<T> {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: mio::Interest,
    ) -> io::Result<()> {
        assert!(interests.is_aio() || interests.is_lio());
        self.0.register(registry.as_raw_fd(), usize::from(token));
        Ok(())
    }

    fn deregister(&mut self, _registry: &Registry) -> io::Result<()> {
        self.0.deregister();
        Ok(())
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: mio::Interest,
    ) -> io::Result<()> {
        assert!(interests.is_aio() || interests.is_lio());
        self.0.register(registry.as_raw_fd(), usize::from(token));
        Ok(())
    }
}

/// Associates a POSIX AIO control block with the reactor that drives it.
///
/// `Aio`'s wrapped type must implement [`AioSource`] to be driven
/// by the reactor.
///
/// The wrapped source may be accessed through the `Aio` via the `Deref` and
/// `DerefMut` traits.
///
/// ## Clearing readiness
///
/// If [`Aio::poll_ready`] returns ready, but the consumer determines that the
/// Source is not completely ready and must return to the Pending state,
/// [`Aio::clear_ready`] may be used.  This can be useful with
/// [`lio_listio`], which may generate a kevent when only a portion of the
/// operations have completed.
///
/// ## Platforms
///
/// Only FreeBSD implements POSIX AIO with kqueue notification, so
/// `Aio` is only available for that operating system.
///
/// [`lio_listio`]: https://pubs.opengroup.org/onlinepubs/9699919799/functions/lio_listio.html
// Note: Unlike every other kqueue event source, POSIX AIO registers events not
// via kevent(2) but when the aiocb is submitted to the kernel via aio_read,
// aio_write, etc.  It needs the kqueue's file descriptor to do that.  So
// AsyncFd can't be used for POSIX AIO.
//
// Note that Aio doesn't implement Drop.  There's no need.  Unlike other
// kqueue sources, simply dropping the object effectively deregisters it.
pub struct Aio<E> {
    io: MioSource<E>,
    registration: Registration,
}

// ===== impl Aio =====

impl<E: AioSource> Aio<E> {
    /// Creates a new `Aio` suitable for use with POSIX AIO functions.
    ///
    /// It will be associated with the default reactor.  The runtime is usually
    /// set implicitly when this function is called from a future driven by a
    /// Tokio runtime, otherwise runtime can be set explicitly with
    /// [`Runtime::enter`](crate::runtime::Runtime::enter) function.
    pub fn new_for_aio(io: E) -> io::Result<Self> {
        Self::new_with_interest(io, Interest::AIO)
    }

    /// Creates a new `Aio` suitable for use with [`lio_listio`].
    ///
    /// It will be associated with the default reactor.  The runtime is usually
    /// set implicitly when this function is called from a future driven by a
    /// Tokio runtime, otherwise runtime can be set explicitly with
    /// [`Runtime::enter`](crate::runtime::Runtime::enter) function.
    ///
    /// [`lio_listio`]: https://pubs.opengroup.org/onlinepubs/9699919799/functions/lio_listio.html
    pub fn new_for_lio(io: E) -> io::Result<Self> {
        Self::new_with_interest(io, Interest::LIO)
    }

    fn new_with_interest(io: E, interest: Interest) -> io::Result<Self> {
        let mut io = MioSource(io);
        let handle = scheduler::Handle::current();
        let registration = Registration::new_with_interest_and_handle(&mut io, interest, handle)?;
        Ok(Self { io, registration })
    }

    /// Indicates to Tokio that the source is no longer ready.  The internal
    /// readiness flag will be cleared, and tokio will wait for the next
    /// edge-triggered readiness notification from the OS.
    ///
    /// It is critical that this method not be called unless your code
    /// _actually observes_ that the source is _not_ ready.  The OS must
    /// deliver a subsequent notification, or this source will block
    /// forever.  It is equally critical that you `do` call this method if you
    /// resubmit the same structure to the kernel and poll it again.
    ///
    /// This method is not very useful with AIO readiness, since each `aiocb`
    /// structure is typically only used once.  It's main use with
    /// [`lio_listio`], which will sometimes send notification when only a
    /// portion of its elements are complete.  In that case, the caller must
    /// call `clear_ready` before resubmitting it.
    ///
    /// [`lio_listio`]: https://pubs.opengroup.org/onlinepubs/9699919799/functions/lio_listio.html
    pub fn clear_ready(&self, ev: AioEvent) {
        self.registration.clear_readiness(ev.0)
    }

    /// Destroy the [`Aio`] and return its inner source.
    pub fn into_inner(self) -> E {
        self.io.0
    }

    /// Polls for readiness.  Either AIO or LIO counts.
    ///
    /// This method returns:
    ///  * `Poll::Pending` if the underlying operation is not complete, whether
    ///     or not it completed successfully.  This will be true if the OS is
    ///     still processing it, or if it has not yet been submitted to the OS.
    ///  * `Poll::Ready(Ok(_))` if the underlying operation is complete.
    ///  * `Poll::Ready(Err(_))` if the reactor has been shutdown.  This does
    ///     _not_ indicate that the underlying operation encountered an error.
    ///
    /// When the method returns `Poll::Pending`, the `Waker` in the provided `Context`
    /// is scheduled to receive a wakeup when the underlying operation
    /// completes. Note that on multiple calls to `poll_ready`, only the `Waker` from the
    /// `Context` passed to the most recent call is scheduled to receive a wakeup.
    pub fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<io::Result<AioEvent>> {
        let ev = ready!(self.registration.poll_read_ready(cx))?;
        Poll::Ready(Ok(AioEvent(ev)))
    }
}

impl<E: AioSource> Deref for Aio<E> {
    type Target = E;

    fn deref(&self) -> &E {
        &self.io.0
    }
}

impl<E: AioSource> DerefMut for Aio<E> {
    fn deref_mut(&mut self) -> &mut E {
        &mut self.io.0
    }
}

impl<E: AioSource + fmt::Debug> fmt::Debug for Aio<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Aio").field("io", &self.io.0).finish()
    }
}

/// Opaque data returned by [`Aio::poll_ready`].
///
/// It can be fed back to [`Aio::clear_ready`].
#[derive(Debug)]
pub struct AioEvent(ReadyEvent);
