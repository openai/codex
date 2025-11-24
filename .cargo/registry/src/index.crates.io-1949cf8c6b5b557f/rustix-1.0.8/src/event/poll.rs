use crate::event::Timespec;
use crate::{backend, io};

pub use backend::event::poll_fd::{PollFd, PollFlags};

/// `poll(self.fds, timeout)`—Wait for events on lists of file descriptors.
///
/// Some platforms (those that don't support `ppoll`) don't support timeouts
/// greater than `c_int::MAX` milliseconds; if an unsupported timeout is
/// passed, this function fails with [`io::Errno::INVAL`].
///
/// On macOS, `poll` doesn't work on fds for /dev/tty or /dev/null, however
/// [`select`] is available and does work on these fds.
///
/// [`select`]: crate::event::select()
///
/// This function does not use the [`Buffer`] trait because the `fds` list is
/// both an input and output buffer.
///
/// [`Buffer`]: crate::buffer::Buffer
///
/// # References
///  - [Beej's Guide to Network Programming]
///  - [POSIX]
///  - [Linux]
///  - [Apple]
///  - [Winsock]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///
/// [Beej's Guide to Network Programming]: https://beej.us/guide/bgnet/html/split/slightly-advanced-techniques.html#poll
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/poll.html
/// [Linux]: https://man7.org/linux/man-pages/man2/poll.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/poll.2.html
/// [Winsock]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-wsapoll
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=poll&sektion=2
/// [NetBSD]: https://man.netbsd.org/poll.2
/// [OpenBSD]: https://man.openbsd.org/poll.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=poll&section=2
/// [illumos]: https://illumos.org/man/2/poll
#[inline]
pub fn poll(fds: &mut [PollFd<'_>], timeout: Option<&Timespec>) -> io::Result<usize> {
    backend::event::syscalls::poll(fds, timeout)
}
