use crate::fd::{AsFd, OwnedFd};
use crate::{backend, io};

pub use backend::time::types::{Itimerspec, TimerfdClockId, TimerfdFlags, TimerfdTimerFlags};

/// `timerfd_create(clockid, flags)`—Create a timer.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/timerfd_create.2.html
#[inline]
pub fn timerfd_create(clockid: TimerfdClockId, flags: TimerfdFlags) -> io::Result<OwnedFd> {
    backend::time::syscalls::timerfd_create(clockid, flags)
}

/// `timerfd_settime(clockid, flags, new_value)`—Set the time on a timer.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/timerfd_settime.2.html
#[inline]
pub fn timerfd_settime<Fd: AsFd>(
    fd: Fd,
    flags: TimerfdTimerFlags,
    new_value: &Itimerspec,
) -> io::Result<Itimerspec> {
    backend::time::syscalls::timerfd_settime(fd.as_fd(), flags, new_value)
}

/// `timerfd_gettime(clockid, flags)`—Query a timer.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/timerfd_gettime.2.html
#[inline]
pub fn timerfd_gettime<Fd: AsFd>(fd: Fd) -> io::Result<Itimerspec> {
    backend::time::syscalls::timerfd_gettime(fd.as_fd())
}
