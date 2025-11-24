//! Interface for the `signalfd` syscall.
//!
//! # Signal discarding
//! When a signal can't be delivered to a process (or thread), it will become a pending signal.
//! Failure to deliver could happen if the signal is blocked by every thread in the process or if
//! the signal handler is still handling a previous signal.
//!
//! If a signal is sent to a process (or thread) that already has a pending signal of the same
//! type, it will be discarded. This means that if signals of the same type are received faster than
//! they are processed, some of those signals will be dropped. Because of this limitation,
//! `signalfd` in itself cannot be used for reliable communication between processes or threads.
//!
//! Once the signal is unblocked, or the signal handler is finished, and a signal is still pending
//! (ie. not consumed from a signalfd) it will be delivered to the signal handler.
//!
//! Please note that signal discarding is not specific to `signalfd`, but also happens with regular
//! signal handlers.
use crate::errno::Errno;
pub use crate::sys::signal::{self, SigSet};
use crate::Result;
pub use libc::signalfd_siginfo as siginfo;

use std::mem;
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};

libc_bitflags! {
    pub struct SfdFlags: libc::c_int {
        SFD_NONBLOCK;
        SFD_CLOEXEC;
    }
}

#[deprecated(since = "0.23.0", note = "use mem::size_of::<siginfo>() instead")]
pub const SIGNALFD_SIGINFO_SIZE: usize = mem::size_of::<siginfo>();

/// Creates a new file descriptor for reading signals.
///
/// **Important:** please read the module level documentation about signal discarding before using
/// this function!
///
/// The `mask` parameter specifies the set of signals that can be accepted via this file descriptor.
///
/// A signal must be blocked on every thread in a process, otherwise it won't be visible from
/// signalfd (the default handler will be invoked instead).
///
/// See [the signalfd man page for more information](https://man7.org/linux/man-pages/man2/signalfd.2.html)
#[deprecated(since = "0.27.0", note = "Use SignalFd instead")]
pub fn signalfd<F: AsFd>(
    fd: Option<F>,
    mask: &SigSet,
    flags: SfdFlags,
) -> Result<OwnedFd> {
    _signalfd(fd, mask, flags)
}

fn _signalfd<F: AsFd>(
    fd: Option<F>,
    mask: &SigSet,
    flags: SfdFlags,
) -> Result<OwnedFd> {
    let raw_fd = fd.map_or(-1, |x| x.as_fd().as_raw_fd());
    unsafe {
        Errno::result(libc::signalfd(raw_fd, mask.as_ref(), flags.bits()))
            .map(|raw_fd| FromRawFd::from_raw_fd(raw_fd))
    }
}

/// A helper struct for creating, reading and closing a `signalfd` instance.
///
/// **Important:** please read the module level documentation about signal discarding before using
/// this struct!
///
/// # Examples
///
/// ```
/// # use nix::sys::signalfd::*;
/// // Set the thread to block the SIGUSR1 signal, otherwise the default handler will be used
/// let mut mask = SigSet::empty();
/// mask.add(signal::SIGUSR1);
/// mask.thread_block().unwrap();
///
/// // Signals are queued up on the file descriptor
/// let mut sfd = SignalFd::with_flags(&mask, SfdFlags::SFD_NONBLOCK).unwrap();
///
/// match sfd.read_signal() {
///     // we caught a signal
///     Ok(Some(sig)) => (),
///     // there were no signals waiting (only happens when the SFD_NONBLOCK flag is set,
///     // otherwise the read_signal call blocks)
///     Ok(None) => (),
///     Err(err) => (), // some error happend
/// }
/// ```
#[derive(Debug)]
pub struct SignalFd(OwnedFd);

impl SignalFd {
    pub fn new(mask: &SigSet) -> Result<SignalFd> {
        Self::with_flags(mask, SfdFlags::empty())
    }

    pub fn with_flags(mask: &SigSet, flags: SfdFlags) -> Result<SignalFd> {
        let fd = _signalfd(None::<OwnedFd>, mask, flags)?;

        Ok(SignalFd(fd))
    }

    pub fn set_mask(&mut self, mask: &SigSet) -> Result<()> {
        self.update(mask, SfdFlags::empty())
    }

    pub fn read_signal(&mut self) -> Result<Option<siginfo>> {
        let mut buffer = mem::MaybeUninit::<siginfo>::uninit();

        let size = mem::size_of_val(&buffer);
        let res = Errno::result(unsafe {
            libc::read(self.0.as_raw_fd(), buffer.as_mut_ptr().cast(), size)
        })
        .map(|r| r as usize);
        match res {
            Ok(x) if x == size => Ok(Some(unsafe { buffer.assume_init() })),
            Ok(_) => unreachable!("partial read on signalfd"),
            Err(Errno::EAGAIN) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn update(&self, mask: &SigSet, flags: SfdFlags) -> Result<()> {
        let raw_fd = self.0.as_raw_fd();
        unsafe {
            Errno::result(libc::signalfd(raw_fd, mask.as_ref(), flags.bits()))
                .map(drop)
        }
    }
}

impl AsFd for SignalFd {
    fn as_fd(&self) -> BorrowedFd {
        self.0.as_fd()
    }
}
impl AsRawFd for SignalFd {
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

impl Iterator for SignalFd {
    type Item = siginfo;

    fn next(&mut self) -> Option<Self::Item> {
        match self.read_signal() {
            Ok(Some(sig)) => Some(sig),
            Ok(None) | Err(_) => None,
        }
    }
}
