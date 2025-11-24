use crate::fd::{AsFd, BorrowedFd};
use bitflags::bitflags;

bitflags! {
    /// `POLL*` flags for use with [`poll`].
    ///
    /// [`poll`]: crate::event::poll
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct PollFlags: u16 {
        /// `POLLIN`
        const IN = linux_raw_sys::general::POLLIN as u16;
        /// `POLLPRI`
        const PRI = linux_raw_sys::general::POLLPRI as u16;
        /// `POLLOUT`
        const OUT = linux_raw_sys::general::POLLOUT as u16;
        /// `POLLRDNORM`
        const RDNORM = linux_raw_sys::general::POLLRDNORM as u16;
        /// `POLLWRNORM`
        const WRNORM = linux_raw_sys::general::POLLWRNORM as u16;
        /// `POLLRDBAND`
        const RDBAND = linux_raw_sys::general::POLLRDBAND as u16;
        /// `POLLWRBAND`
        const WRBAND = linux_raw_sys::general::POLLWRBAND as u16;
        /// `POLLERR`
        const ERR = linux_raw_sys::general::POLLERR as u16;
        /// `POLLHUP`
        const HUP = linux_raw_sys::general::POLLHUP as u16;
        /// `POLLNVAL`
        const NVAL = linux_raw_sys::general::POLLNVAL as u16;
        /// `POLLRDHUP`
        const RDHUP = linux_raw_sys::general::POLLRDHUP as u16;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

/// `struct pollfd`—File descriptor and flags for use with [`poll`].
///
/// [`poll`]: crate::event::poll
#[doc(alias = "pollfd")]
#[repr(C)]
#[derive(Debug, Clone)]
pub struct PollFd<'fd> {
    pub(crate) fd: BorrowedFd<'fd>,
    pub(crate) events: u16,
    pub(crate) revents: u16,
}

impl<'fd> PollFd<'fd> {
    /// Constructs a new `PollFd` holding `fd` and `events`.
    #[inline]
    pub fn new<Fd: AsFd>(fd: &'fd Fd, events: PollFlags) -> Self {
        Self::from_borrowed_fd(fd.as_fd(), events)
    }

    /// Sets the contained file descriptor to `fd`.
    #[inline]
    pub fn set_fd<Fd: AsFd>(&mut self, fd: &'fd Fd) {
        self.fd = fd.as_fd();
    }

    /// Clears the ready events.
    #[inline]
    pub fn clear_revents(&mut self) {
        self.revents = 0;
    }

    /// Constructs a new `PollFd` holding `fd` and `events`.
    ///
    /// This is the same as `new`, but can be used to avoid borrowing the
    /// `BorrowedFd`, which can be tricky in situations where the `BorrowedFd`
    /// is a temporary.
    #[inline]
    pub fn from_borrowed_fd(fd: BorrowedFd<'fd>, events: PollFlags) -> Self {
        Self {
            fd,
            events: events.bits(),
            revents: 0,
        }
    }

    /// Returns the ready events.
    #[inline]
    pub fn revents(&self) -> PollFlags {
        // Use `.unwrap()` here because in theory we know we know all the bits
        // the OS might set here, but OS's have added extensions in the past.
        PollFlags::from_bits(self.revents).unwrap()
    }
}

impl<'fd> AsFd for PollFd<'fd> {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}
