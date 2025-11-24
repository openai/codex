use std::ops::{Deref, DerefMut};
#[cfg(any(unix, target_os = "wasi"))]
use std::os::fd::AsRawFd;
// TODO: once <https://github.com/rust-lang/rust/issues/126198> is fixed this
// can use `std::os::fd` and be merged with the above.
#[cfg(target_os = "hermit")]
use std::os::hermit::io::AsRawFd;
#[cfg(windows)]
use std::os::windows::io::AsRawSocket;
#[cfg(debug_assertions)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{fmt, io};

use crate::sys::IoSourceState;
use crate::{event, Interest, Registry, Token};

/// Adapter for a [`RawFd`] or [`RawSocket`] providing an [`event::Source`]
/// implementation.
///
/// `IoSource` enables registering any FD or socket wrapper with [`Poll`].
///
/// While only implementations for TCP, UDP, and UDS (Unix only) are provided,
/// Mio supports registering any FD or socket that can be registered with the
/// underlying OS selector. `IoSource` provides the necessary bridge.
///
/// [`RawFd`]: std::os::fd::RawFd
/// [`RawSocket`]: std::os::windows::io::RawSocket
///
/// # Notes
///
/// To handle the registrations and events properly **all** I/O operations (such
/// as `read`, `write`, etc.) must go through the [`do_io`] method to ensure the
/// internal state is updated accordingly.
///
/// [`Poll`]: crate::Poll
/// [`do_io`]: IoSource::do_io
pub struct IoSource<T> {
    state: IoSourceState,
    inner: T,
    #[cfg(debug_assertions)]
    selector_id: SelectorId,
}

impl<T> IoSource<T> {
    /// Create a new `IoSource`.
    pub fn new(io: T) -> IoSource<T> {
        IoSource {
            state: IoSourceState::new(),
            inner: io,
            #[cfg(debug_assertions)]
            selector_id: SelectorId::new(),
        }
    }

    /// Execute an I/O operations ensuring that the socket receives more events
    /// if it hits a [`WouldBlock`] error.
    ///
    /// # Notes
    ///
    /// This method is required to be called for **all** I/O operations to
    /// ensure the user will receive events once the socket is ready again after
    /// returning a [`WouldBlock`] error.
    ///
    /// [`WouldBlock`]: io::ErrorKind::WouldBlock
    pub fn do_io<F, R>(&self, f: F) -> io::Result<R>
    where
        F: FnOnce(&T) -> io::Result<R>,
    {
        self.state.do_io(f, &self.inner)
    }

    /// Returns the I/O source, dropping the state.
    ///
    /// # Notes
    ///
    /// To ensure no more events are to be received for this I/O source first
    /// [`deregister`] it.
    ///
    /// [`deregister`]: Registry::deregister
    pub fn into_inner(self) -> T {
        self.inner
    }
}

/// Be careful when using this method. All I/O operations that may block must go
/// through the [`do_io`] method.
///
/// [`do_io`]: IoSource::do_io
impl<T> Deref for IoSource<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Be careful when using this method. All I/O operations that may block must go
/// through the [`do_io`] method.
///
/// [`do_io`]: IoSource::do_io
impl<T> DerefMut for IoSource<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[cfg(any(unix, target_os = "hermit"))]
impl<T> event::Source for IoSource<T>
where
    T: AsRawFd,
{
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        #[cfg(debug_assertions)]
        self.selector_id.associate(registry)?;
        self.state
            .register(registry, token, interests, self.inner.as_raw_fd())
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        #[cfg(debug_assertions)]
        self.selector_id.check_association(registry)?;
        self.state
            .reregister(registry, token, interests, self.inner.as_raw_fd())
    }

    fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
        #[cfg(debug_assertions)]
        self.selector_id.remove_association(registry)?;
        self.state.deregister(registry, self.inner.as_raw_fd())
    }
}

#[cfg(windows)]
impl<T> event::Source for IoSource<T>
where
    T: AsRawSocket,
{
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        #[cfg(debug_assertions)]
        self.selector_id.associate(registry)?;
        self.state
            .register(registry, token, interests, self.inner.as_raw_socket())
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        #[cfg(debug_assertions)]
        self.selector_id.check_association(registry)?;
        self.state.reregister(registry, token, interests)
    }

    fn deregister(&mut self, _registry: &Registry) -> io::Result<()> {
        #[cfg(debug_assertions)]
        self.selector_id.remove_association(_registry)?;
        self.state.deregister()
    }
}

#[cfg(target_os = "wasi")]
impl<T> event::Source for IoSource<T>
where
    T: AsRawFd,
{
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        #[cfg(debug_assertions)]
        self.selector_id.associate(registry)?;
        registry
            .selector()
            .register(self.inner.as_raw_fd() as _, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        #[cfg(debug_assertions)]
        self.selector_id.check_association(registry)?;
        registry
            .selector()
            .reregister(self.inner.as_raw_fd() as _, token, interests)
    }

    fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
        #[cfg(debug_assertions)]
        self.selector_id.remove_association(registry)?;
        registry.selector().deregister(self.inner.as_raw_fd() as _)
    }
}

impl<T> fmt::Debug for IoSource<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

/// Used to associate an `IoSource` with a `sys::Selector`.
#[cfg(debug_assertions)]
#[derive(Debug)]
struct SelectorId {
    id: AtomicUsize,
}

#[cfg(debug_assertions)]
impl SelectorId {
    /// Value of `id` if `SelectorId` is not associated with any
    /// `sys::Selector`. Valid selector ids start at 1.
    const UNASSOCIATED: usize = 0;

    /// Create a new `SelectorId`.
    const fn new() -> SelectorId {
        SelectorId {
            id: AtomicUsize::new(Self::UNASSOCIATED),
        }
    }

    /// Associate an I/O source with `registry`, returning an error if its
    /// already registered.
    fn associate(&self, registry: &Registry) -> io::Result<()> {
        let registry_id = registry.selector().id();
        let previous_id = self.id.swap(registry_id, Ordering::AcqRel);

        if previous_id == Self::UNASSOCIATED {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "I/O source already registered with a `Registry`",
            ))
        }
    }

    /// Check the association of an I/O source with `registry`, returning an
    /// error if its registered with a different `Registry` or not registered at
    /// all.
    fn check_association(&self, registry: &Registry) -> io::Result<()> {
        let registry_id = registry.selector().id();
        let id = self.id.load(Ordering::Acquire);

        if id == registry_id {
            Ok(())
        } else if id == Self::UNASSOCIATED {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "I/O source not registered with `Registry`",
            ))
        } else {
            Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "I/O source already registered with a different `Registry`",
            ))
        }
    }

    /// Remove a previously made association from `registry`, returns an error
    /// if it was not previously associated with `registry`.
    fn remove_association(&self, registry: &Registry) -> io::Result<()> {
        let registry_id = registry.selector().id();
        let previous_id = self.id.swap(Self::UNASSOCIATED, Ordering::AcqRel);

        if previous_id == registry_id {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "I/O source not registered with `Registry`",
            ))
        }
    }
}

#[cfg(debug_assertions)]
impl Clone for SelectorId {
    fn clone(&self) -> SelectorId {
        SelectorId {
            id: AtomicUsize::new(self.id.load(Ordering::Acquire)),
        }
    }
}
