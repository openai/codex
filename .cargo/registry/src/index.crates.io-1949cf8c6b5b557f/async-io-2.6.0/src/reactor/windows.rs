// SPDX-License-Identifier: MIT OR Apache-2.0

use polling::os::iocp::PollerIocpExt;
use polling::{Event, PollMode, Poller};
use std::fmt;
use std::io::Result;
use std::os::windows::io::{
    AsRawHandle, AsRawSocket, BorrowedHandle, BorrowedSocket, RawHandle, RawSocket,
};

/// The raw registration into the reactor.
#[doc(hidden)]
pub enum Registration {
    /// Raw socket handle on Windows.
    ///
    /// # Invariant
    ///
    /// This describes a valid socket that has not been `close`d. It will not be
    /// closed while this object is alive.
    Socket(RawSocket),

    /// Waitable handle for Windows.
    ///
    /// # Invariant
    ///
    /// This describes a valid waitable handle that has not been `close`d. It will not be
    /// closed while this object is alive.
    Handle(RawHandle),
}

unsafe impl Send for Registration {}
unsafe impl Sync for Registration {}

impl fmt::Debug for Registration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Socket(raw) => fmt::Debug::fmt(raw, f),
            Self::Handle(handle) => fmt::Debug::fmt(handle, f),
        }
    }
}

impl Registration {
    /// Add this file descriptor into the reactor.
    ///
    /// # Safety
    ///
    /// The provided file descriptor must be valid and not be closed while this object is alive.
    pub(crate) unsafe fn new(f: BorrowedSocket<'_>) -> Self {
        Self::Socket(f.as_raw_socket())
    }

    /// Create a new [`Registration`] around a waitable handle.
    ///
    /// # Safety
    ///
    /// The provided handle must be valid and not be closed while this object is alive.
    pub(crate) unsafe fn new_waitable(f: BorrowedHandle<'_>) -> Self {
        Self::Handle(f.as_raw_handle())
    }

    /// Registers the object into the reactor.
    #[inline]
    pub(crate) fn add(&self, poller: &Poller, token: usize) -> Result<()> {
        // SAFETY: This object's existence validates the invariants of Poller::add
        unsafe {
            match self {
                Self::Socket(raw) => poller.add(*raw, Event::none(token)),
                Self::Handle(handle) => {
                    poller.add_waitable(*handle, Event::none(token), PollMode::Oneshot)
                }
            }
        }
    }

    /// Re-registers the object into the reactor.
    #[inline]
    pub(crate) fn modify(&self, poller: &Poller, interest: Event) -> Result<()> {
        // SAFETY: self.raw is a valid file descriptor
        match self {
            Self::Socket(raw) => {
                poller.modify(unsafe { BorrowedSocket::borrow_raw(*raw) }, interest)
            }
            Self::Handle(handle) => poller.modify_waitable(
                unsafe { BorrowedHandle::borrow_raw(*handle) },
                interest,
                PollMode::Oneshot,
            ),
        }
    }

    /// Deregisters the object from the reactor.
    #[inline]
    pub(crate) fn delete(&self, poller: &Poller) -> Result<()> {
        // SAFETY: self.raw is a valid file descriptor
        match self {
            Self::Socket(raw) => poller.delete(unsafe { BorrowedSocket::borrow_raw(*raw) }),
            Self::Handle(handle) => {
                poller.remove_waitable(unsafe { BorrowedHandle::borrow_raw(*handle) })
            }
        }
    }
}
