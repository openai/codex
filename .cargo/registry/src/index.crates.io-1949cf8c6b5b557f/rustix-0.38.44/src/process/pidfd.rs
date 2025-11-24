use crate::fd::OwnedFd;
use crate::process::{Pid, Signal};
use crate::{backend, io};
use backend::fd::AsFd;

bitflags::bitflags! {
    /// `PIDFD_*` flags for use with [`pidfd_open`].
    ///
    /// [`pidfd_open`]: crate::process::pidfd_open
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct PidfdFlags: backend::c::c_uint {
        /// `PIDFD_NONBLOCK`.
        const NONBLOCK = backend::c::PIDFD_NONBLOCK;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

/// `syscall(SYS_pidfd_open, pid, flags)`—Creates a file descriptor for a
/// process.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/pidfd_open.2.html
#[inline]
pub fn pidfd_open(pid: Pid, flags: PidfdFlags) -> io::Result<OwnedFd> {
    backend::process::syscalls::pidfd_open(pid, flags)
}

/// `syscall(SYS_pidfd_send_signal, pidfd, sig, NULL, 0)`—Send a signal to a
/// process specified by a file descriptor.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/pidfd_send_signal.2.html
#[inline]
pub fn pidfd_send_signal<Fd: AsFd>(pidfd: Fd, sig: Signal) -> io::Result<()> {
    backend::process::syscalls::pidfd_send_signal(pidfd.as_fd(), sig)
}
