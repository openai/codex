use crate::process::Pid;
use crate::{backend, io};

pub use crate::signal::Signal;

/// `kill(pid, sig)`—Sends a signal to a process.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/kill.html
/// [Linux]: https://man7.org/linux/man-pages/man2/kill.2.html
#[inline]
#[doc(alias = "kill")]
pub fn kill_process(pid: Pid, sig: Signal) -> io::Result<()> {
    backend::process::syscalls::kill_process(pid, sig)
}

/// `kill(-pid, sig)`—Sends a signal to all processes in a process group.
///
/// If `pid` is 1, this sends a signal to all processes the current process has
/// permission to send signals to, except process `1`, possibly other
/// system-specific processes, and on some systems, the current process.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/kill.html
/// [Linux]: https://man7.org/linux/man-pages/man2/kill.2.html
#[inline]
#[doc(alias = "kill")]
pub fn kill_process_group(pid: Pid, sig: Signal) -> io::Result<()> {
    backend::process::syscalls::kill_process_group(pid, sig)
}

/// `kill(0, sig)`—Sends a signal to all processes in the current process
/// group.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/kill.html
/// [Linux]: https://man7.org/linux/man-pages/man2/kill.2.html
#[inline]
#[doc(alias = "kill")]
pub fn kill_current_process_group(sig: Signal) -> io::Result<()> {
    backend::process::syscalls::kill_current_process_group(sig)
}

/// `kill(pid, 0)`—Check validity of pid and permissions to send signals to
/// the process, without actually sending any signals.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/kill.html
/// [Linux]: https://man7.org/linux/man-pages/man2/kill.2.html
#[inline]
#[doc(alias = "kill")]
pub fn test_kill_process(pid: Pid) -> io::Result<()> {
    backend::process::syscalls::test_kill_process(pid)
}

/// `kill(-pid, 0)`—Check validity of pid and permissions to send signals to
/// all processes in the process group, without actually sending any signals.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/kill.html
/// [Linux]: https://man7.org/linux/man-pages/man2/kill.2.html
#[inline]
#[doc(alias = "kill")]
pub fn test_kill_process_group(pid: Pid) -> io::Result<()> {
    backend::process::syscalls::test_kill_process_group(pid)
}

/// `kill(0, 0)`—Check validity of pid and permissions to send signals to the
/// all processes in the current process group, without actually sending any
/// signals.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/kill.html
/// [Linux]: https://man7.org/linux/man-pages/man2/kill.2.html
#[inline]
#[doc(alias = "kill")]
pub fn test_kill_current_process_group() -> io::Result<()> {
    backend::process::syscalls::test_kill_current_process_group()
}
