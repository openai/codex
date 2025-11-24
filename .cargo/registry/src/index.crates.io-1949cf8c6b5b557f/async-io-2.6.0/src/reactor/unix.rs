// SPDX-License-Identifier: MIT OR Apache-2.0

use polling::{Event, Poller};

use std::fmt;
use std::io::Result;
use std::os::unix::io::{AsRawFd, BorrowedFd, RawFd};

/// The raw registration into the reactor.
#[doc(hidden)]
pub struct Registration {
    /// Raw file descriptor on Unix.
    ///
    /// # Invariant
    ///
    /// This describes a valid file descriptor that has not been `close`d. It will not be
    /// closed while this object is alive.
    raw: RawFd,
}

impl fmt::Debug for Registration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.raw, f)
    }
}

impl Registration {
    /// Add this file descriptor into the reactor.
    ///
    /// # Safety
    ///
    /// The provided file descriptor must be valid and not be closed while this object is alive.
    pub(crate) unsafe fn new(f: BorrowedFd<'_>) -> Self {
        Self { raw: f.as_raw_fd() }
    }

    /// Registers the object into the reactor.
    #[inline]
    pub(crate) fn add(&self, poller: &Poller, token: usize) -> Result<()> {
        // SAFETY: This object's existence validates the invariants of Poller::add
        unsafe { poller.add(self.raw, Event::none(token)) }
    }

    /// Re-registers the object into the reactor.
    #[inline]
    pub(crate) fn modify(&self, poller: &Poller, interest: Event) -> Result<()> {
        // SAFETY: self.raw is a valid file descriptor
        let fd = unsafe { BorrowedFd::borrow_raw(self.raw) };
        poller.modify(fd, interest)
    }

    /// Deregisters the object from the reactor.
    #[inline]
    pub(crate) fn delete(&self, poller: &Poller) -> Result<()> {
        // SAFETY: self.raw is a valid file descriptor
        let fd = unsafe { BorrowedFd::borrow_raw(self.raw) };
        poller.delete(fd)
    }
}
