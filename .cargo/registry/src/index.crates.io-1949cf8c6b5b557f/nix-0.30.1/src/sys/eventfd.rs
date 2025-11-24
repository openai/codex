use crate::errno::Errno;
use crate::{unistd, Result};
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};

libc_bitflags! {
    /// Eventfd flags.
    pub struct EfdFlags: libc::c_int {
        /// Set the close-on-exec (`FD_CLOEXEC`) flag on the new event file descriptor.
        EFD_CLOEXEC; // Since Linux 2.6.27/FreeBSD 13.0
        /// Set the `O_NONBLOCK` file status flag on the new event file description.
        EFD_NONBLOCK; // Since Linux 2.6.27/FreeBSD 13.0
        /// Provide semaphore-like semantics for reads from the new event file
        /// descriptor.
        EFD_SEMAPHORE; // Since Linux 2.6.30/FreeBSD 13.0
    }
}

#[deprecated(
    since = "0.28.0",
    note = "Use EventFd::from_value_and_flags() instead"
)]
#[allow(missing_docs)]
pub fn eventfd(initval: libc::c_uint, flags: EfdFlags) -> Result<OwnedFd> {
    let res = unsafe { libc::eventfd(initval, flags.bits()) };

    Errno::result(res).map(|r| unsafe { OwnedFd::from_raw_fd(r) })
}

/// An eventfd file descriptor.
#[derive(Debug)]
#[repr(transparent)]
pub struct EventFd(OwnedFd);

impl EventFd {
    /// [`EventFd::from_value_and_flags`] with `init_val = 0` and `flags = EfdFlags::empty()`.
    pub fn new() -> Result<Self> {
        Self::from_value_and_flags(0, EfdFlags::empty())
    }

    /// Constructs [`EventFd`] with the given `init_val` and `flags`.
    ///
    /// Wrapper around [`libc::eventfd`].
    pub fn from_value_and_flags(
        init_val: u32,
        flags: EfdFlags,
    ) -> Result<Self> {
        let res = unsafe { libc::eventfd(init_val, flags.bits()) };
        Errno::result(res).map(|r| Self(unsafe { OwnedFd::from_raw_fd(r) }))
    }

    /// [`EventFd::from_value_and_flags`] with `init_val = 0` and given `flags`.
    pub fn from_flags(flags: EfdFlags) -> Result<Self> {
        Self::from_value_and_flags(0, flags)
    }

    /// [`EventFd::from_value_and_flags`] with given `init_val` and `flags = EfdFlags::empty()`.
    pub fn from_value(init_val: u32) -> Result<Self> {
        Self::from_value_and_flags(init_val, EfdFlags::empty())
    }

    /// Constructs an `EventFd` wrapping an existing `OwnedFd`.
    ///
    /// # Safety
    ///
    /// `OwnedFd` is a valid eventfd.
    pub unsafe fn from_owned_fd(fd: OwnedFd) -> Self {
        Self(fd)
    }

    /// Enqueues `value` triggers, i.e., adds the integer value supplied in `value`
    /// to the counter.
    ///
    /// The next `value` calls to `poll`, `select` or `epoll` will return immediately.
    ///
    /// [`EventFd::write`] with `value`.
    pub fn write(&self, value: u64) -> Result<usize> {
        unistd::write(&self.0, &value.to_ne_bytes())
    }

    /// Reads the value from the file descriptor.
    ///
    /// * If [`EFD_SEMAPHORE`](EfdFlags::EFD_SEMAPHORE) was not specified and
    ///   the eventfd counter has a nonzero value, then this function returns
    ///   an `u64` containing that value, and the counter's value is reset to
    ///   zero.
    ///
    /// * If [`EFD_SEMAPHORE`](EfdFlags::EFD_SEMAPHORE) was specified and the
    ///   eventfd counter has a nonzero value, then this function returns an
    ///   `u64` containing the value 1, and the counter's value is decremented
    ///   by 1.
    ///
    /// * If the eventfd counter is zero at the time of this call, then the
    ///   call either blocks until the counter becomes nonzero (at which time,
    ///   this function proceeds as described above) or fails with the error
    ///   `EAGAIN` if the file descriptor has been made nonblocking with
    ///   [`EFD_NONBLOCK`](EfdFlags::EFD_NONBLOCK).
    pub fn read(&self) -> Result<u64> {
        let mut arr = [0; std::mem::size_of::<u64>()];
        unistd::read(&self.0, &mut arr)?;
        Ok(u64::from_ne_bytes(arr))
    }
}
impl AsFd for EventFd {
    fn as_fd(&self) -> BorrowedFd {
        self.0.as_fd()
    }
}
impl AsRawFd for EventFd {
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

impl From<EventFd> for OwnedFd {
    fn from(value: EventFd) -> Self {
        value.0
    }
}
