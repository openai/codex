use crate::fd::AsFd;
#[cfg(not(target_os = "espidf"))]
use crate::termios::{Action, OptionalActions, QueueSelector, Termios, Winsize};
use crate::{backend, io};

pub use crate::pid::Pid;

/// `tcgetattr(fd)`—Get terminal attributes.
///
/// Also known as the `TCGETS` (or `TCGETS2` on Linux) operation with `ioctl`.
///
/// On Linux, this uses `TCGETS2`. If that fails in a way that indicates that
/// the host doesn't support it, this falls back to the old `TCGETS`, manually
/// initializes the fields that `TCGETS` doesn't initialize, and fails with
/// `io::Errno::RANGE` if the input or output speeds cannot be supported.
///
/// # References
///  - [POSIX `tcgetattr`]
///  - [Linux `ioctl_tty`]
///  - [Linux `termios`]
///
/// [POSIX `tcgetattr`]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/tcgetattr.html
/// [Linux `ioctl_tty`]: https://man7.org/linux/man-pages/man4/tty_ioctl.4.html
/// [Linux `termios`]: https://man7.org/linux/man-pages/man3/termios.3.html
#[cfg(not(any(windows, target_os = "espidf", target_os = "wasi")))]
#[inline]
#[doc(alias = "TCGETS")]
#[doc(alias = "TCGETS2")]
#[doc(alias = "tcgetattr2")]
pub fn tcgetattr<Fd: AsFd>(fd: Fd) -> io::Result<Termios> {
    backend::termios::syscalls::tcgetattr(fd.as_fd())
}

/// `tcgetwinsize(fd)`—Get the current terminal window size.
///
/// Also known as the `TIOCGWINSZ` operation with `ioctl`.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man4/tty_ioctl.4.html
#[cfg(not(any(
    windows,
    target_os = "horizon",
    target_os = "espidf",
    target_os = "wasi"
)))]
#[inline]
#[doc(alias = "TIOCGWINSZ")]
pub fn tcgetwinsize<Fd: AsFd>(fd: Fd) -> io::Result<Winsize> {
    backend::termios::syscalls::tcgetwinsize(fd.as_fd())
}

/// `tcgetpgrp(fd)`—Get the terminal foreground process group.
///
/// Also known as the `TIOCGPGRP` operation with `ioctl`.
///
/// On Linux, if `fd` is a pseudo-terminal, the underlying system call here can
/// return a pid of 0, which rustix's `Pid` type doesn't support. So rustix
/// instead handles this case by failing with [`io::Errno::OPNOTSUPP`] if the
/// pid is 0.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/tcgetpgrp.html
/// [Linux]: https://man7.org/linux/man-pages/man3/tcgetpgrp.3.html
#[cfg(not(any(windows, target_os = "wasi")))]
#[inline]
#[doc(alias = "TIOCGPGRP")]
pub fn tcgetpgrp<Fd: AsFd>(fd: Fd) -> io::Result<Pid> {
    backend::termios::syscalls::tcgetpgrp(fd.as_fd())
}

/// `tcsetpgrp(fd, pid)`—Set the terminal foreground process group.
///
/// Also known as the `TIOCSPGRP` operation with `ioctl`.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/tcsetpgrp.html
/// [Linux]: https://man7.org/linux/man-pages/man3/tcsetpgrp.3.html
#[cfg(not(any(windows, target_os = "wasi")))]
#[inline]
#[doc(alias = "TIOCSPGRP")]
pub fn tcsetpgrp<Fd: AsFd>(fd: Fd, pid: Pid) -> io::Result<()> {
    backend::termios::syscalls::tcsetpgrp(fd.as_fd(), pid)
}

/// `tcsetattr(fd)`—Set terminal attributes.
///
/// Also known as the `TCSETS` (or `TCSETS2` on Linux) operation with `ioctl`.
///
/// On Linux, this uses `TCSETS2`. If that fails in a way that indicates that
/// the host doesn't support it, this falls back to the old `TCSETS`, and fails
/// with `io::Errno::RANGE` if the input or output speeds cannot be supported.
///
/// # References
///  - [POSIX `tcsetattr`]
///  - [Linux `ioctl_tty`]
///  - [Linux `termios`]
///
/// [POSIX `tcsetattr`]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/tcsetattr.html
/// [Linux `ioctl_tty`]: https://man7.org/linux/man-pages/man4/tty_ioctl.4.html
/// [Linux `termios`]: https://man7.org/linux/man-pages/man3/termios.3.html
#[cfg(not(target_os = "espidf"))]
#[inline]
#[doc(alias = "TCSETS")]
#[doc(alias = "TCSETS2")]
#[doc(alias = "tcsetattr2")]
pub fn tcsetattr<Fd: AsFd>(
    fd: Fd,
    optional_actions: OptionalActions,
    termios: &Termios,
) -> io::Result<()> {
    backend::termios::syscalls::tcsetattr(fd.as_fd(), optional_actions, termios)
}

/// `tcsendbreak(fd, 0)`—Transmit zero-valued bits.
///
/// This transmits zero-valued bits for at least 0.25 seconds.
///
/// This function does not have a `duration` parameter, and always uses the
/// implementation-defined value, which transmits for at least 0.25 seconds.
///
/// Also known as the `TCSBRK` operation with `ioctl`, with a duration
/// parameter of 0.
///
/// # References
///  - [POSIX `tcsendbreak`]
///  - [Linux `ioctl_tty`]
///  - [Linux `termios`]
///
/// [POSIX `tcsendbreak`]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/tcsendbreak.html
/// [Linux `ioctl_tty`]: https://man7.org/linux/man-pages/man4/tty_ioctl.4.html
/// [Linux `termios`]: https://man7.org/linux/man-pages/man3/termios.3.html
#[inline]
#[doc(alias = "TCSBRK")]
pub fn tcsendbreak<Fd: AsFd>(fd: Fd) -> io::Result<()> {
    backend::termios::syscalls::tcsendbreak(fd.as_fd())
}

/// `tcdrain(fd, duration)`—Wait until all pending output has been written.
///
/// # References
///  - [POSIX `tcdrain`]
///  - [Linux `ioctl_tty`]
///  - [Linux `termios`]
///
/// [POSIX `tcsetattr`]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/tcdrain.html
/// [Linux `ioctl_tty`]: https://man7.org/linux/man-pages/man4/tty_ioctl.4.html
/// [Linux `termios`]: https://man7.org/linux/man-pages/man3/termios.3.html
#[cfg(not(target_os = "espidf"))]
#[inline]
pub fn tcdrain<Fd: AsFd>(fd: Fd) -> io::Result<()> {
    backend::termios::syscalls::tcdrain(fd.as_fd())
}

/// `tcflush(fd, queue_selector)`—Wait until all pending output has been
/// written.
///
/// # References
///  - [POSIX `tcflush`]
///  - [Linux `ioctl_tty`]
///  - [Linux `termios`]
///
/// [POSIX `tcflush`]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/tcflush.html
/// [Linux `ioctl_tty`]: https://man7.org/linux/man-pages/man4/tty_ioctl.4.html
/// [Linux `termios`]: https://man7.org/linux/man-pages/man3/termios.3.html
#[cfg(not(target_os = "espidf"))]
#[inline]
#[doc(alias = "TCFLSH")]
pub fn tcflush<Fd: AsFd>(fd: Fd, queue_selector: QueueSelector) -> io::Result<()> {
    backend::termios::syscalls::tcflush(fd.as_fd(), queue_selector)
}

/// `tcflow(fd, action)`—Suspend or resume transmission or reception.
///
/// # References
///  - [POSIX `tcflow`]
///  - [Linux `ioctl_tty`]
///  - [Linux `termios`]
///
/// [POSIX `tcflow`]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/tcflow.html
/// [Linux `ioctl_tty`]: https://man7.org/linux/man-pages/man4/tty_ioctl.4.html
/// [Linux `termios`]: https://man7.org/linux/man-pages/man3/termios.3.html
#[cfg(not(target_os = "espidf"))]
#[inline]
#[doc(alias = "TCXONC")]
pub fn tcflow<Fd: AsFd>(fd: Fd, action: Action) -> io::Result<()> {
    backend::termios::syscalls::tcflow(fd.as_fd(), action)
}

/// `tcgetsid(fd)`—Return the session ID of the current session with `fd` as
/// its controlling terminal.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/tcgetsid.html
/// [Linux]: https://man7.org/linux/man-pages/man3/tcgetsid.3.html
#[inline]
#[doc(alias = "TIOCGSID")]
pub fn tcgetsid<Fd: AsFd>(fd: Fd) -> io::Result<Pid> {
    backend::termios::syscalls::tcgetsid(fd.as_fd())
}

/// `tcsetwinsize(fd)`—Set the current terminal window size.
///
/// Also known as the `TIOCSWINSZ` operation with `ioctl`.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man4/tty_ioctl.4.html
#[cfg(not(any(target_os = "espidf", target_os = "horizon")))]
#[inline]
#[doc(alias = "TIOCSWINSZ")]
pub fn tcsetwinsize<Fd: AsFd>(fd: Fd, winsize: Winsize) -> io::Result<()> {
    backend::termios::syscalls::tcsetwinsize(fd.as_fd(), winsize)
}
