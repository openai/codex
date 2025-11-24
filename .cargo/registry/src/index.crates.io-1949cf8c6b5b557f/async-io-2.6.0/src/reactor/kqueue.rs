// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::os::kqueue::Signal;

use polling::os::kqueue::{PollerKqueueExt, Process, ProcessOps, Signal as PollSignal};
use polling::{Event, PollMode, Poller};

use std::fmt;
use std::io::Result;
use std::num::NonZeroI32;
use std::os::unix::io::{AsRawFd, BorrowedFd, RawFd};

/// The raw registration into the reactor.
///
/// This needs to be public, since it is technically exposed through the `QueueableSealed` trait.
#[doc(hidden)]
pub enum Registration {
    /// Raw file descriptor for readability/writability.
    ///
    ///
    /// # Invariant
    ///
    /// This describes a valid file descriptor that has not been `close`d. It will not be
    /// closed while this object is alive.
    Fd(RawFd),

    /// Raw signal number for signal delivery.
    Signal(Signal),

    /// Pid for process termination.
    Process(NonZeroI32),
}

impl fmt::Debug for Registration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fd(raw) => fmt::Debug::fmt(raw, f),
            Self::Signal(signal) => fmt::Debug::fmt(signal, f),
            Self::Process(process) => fmt::Debug::fmt(process, f),
        }
    }
}

impl Registration {
    /// Add this file descriptor into the reactor.
    ///
    /// # Safety
    ///
    /// The provided file descriptor must be valid and not be closed while this object is alive.
    pub(crate) unsafe fn new(f: BorrowedFd<'_>) -> Self {
        Self::Fd(f.as_raw_fd())
    }

    /// Registers the object into the reactor.
    #[inline]
    pub(crate) fn add(&self, poller: &Poller, token: usize) -> Result<()> {
        match self {
            Self::Fd(raw) => {
                // SAFETY: This object's existence validates the invariants of Poller::add
                unsafe { poller.add(*raw, Event::none(token)) }
            }
            Self::Signal(signal) => {
                poller.add_filter(PollSignal(signal.0), token, PollMode::Oneshot)
            }
            Self::Process(pid) => poller.add_filter(
                unsafe { Process::from_pid(*pid, ProcessOps::Exit) },
                token,
                PollMode::Oneshot,
            ),
        }
    }

    /// Re-registers the object into the reactor.
    #[inline]
    pub(crate) fn modify(&self, poller: &Poller, interest: Event) -> Result<()> {
        match self {
            Self::Fd(raw) => {
                // SAFETY: self.raw is a valid file descriptor
                let fd = unsafe { BorrowedFd::borrow_raw(*raw) };
                poller.modify(fd, interest)
            }
            Self::Signal(signal) => {
                poller.modify_filter(PollSignal(signal.0), interest.key, PollMode::Oneshot)
            }
            Self::Process(pid) => poller.modify_filter(
                unsafe { Process::from_pid(*pid, ProcessOps::Exit) },
                interest.key,
                PollMode::Oneshot,
            ),
        }
    }

    /// Deregisters the object from the reactor.
    #[inline]
    pub(crate) fn delete(&self, poller: &Poller) -> Result<()> {
        match self {
            Self::Fd(raw) => {
                // SAFETY: self.raw is a valid file descriptor
                let fd = unsafe { BorrowedFd::borrow_raw(*raw) };
                poller.delete(fd)
            }
            Self::Signal(signal) => poller.delete_filter(PollSignal(signal.0)),
            Self::Process(pid) => {
                poller.delete_filter(unsafe { Process::from_pid(*pid, ProcessOps::Exit) })
            }
        }
    }
}
