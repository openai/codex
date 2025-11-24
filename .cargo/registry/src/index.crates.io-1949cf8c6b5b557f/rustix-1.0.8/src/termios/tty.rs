//! Functions which operate on file descriptors which might be terminals.

use crate::backend;
#[cfg(feature = "alloc")]
#[cfg(feature = "fs")]
#[cfg(not(any(target_os = "fuchsia", target_os = "wasi")))]
use crate::path::SMALL_PATH_BUFFER_SIZE;
use backend::fd::AsFd;
#[cfg(feature = "alloc")]
#[cfg(not(any(target_os = "fuchsia", target_os = "wasi")))]
use {crate::ffi::CString, crate::io, alloc::vec::Vec, backend::fd::BorrowedFd};

/// `isatty(fd)`—Tests whether a file descriptor refers to a terminal.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/isatty.html
/// [Linux]: https://man7.org/linux/man-pages/man3/isatty.3.html
#[inline]
pub fn isatty<Fd: AsFd>(fd: Fd) -> bool {
    backend::termios::syscalls::isatty(fd.as_fd())
}

/// `ttyname_r(fd)`—Returns the name of the tty open on `fd`.
///
/// If `reuse` already has available capacity, reuse it if possible.
///
/// On Linux, this function depends on procfs being mounted on /proc.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/ttyname.html
/// [Linux]: https://man7.org/linux/man-pages/man3/ttyname.3.html
#[cfg(not(any(target_os = "fuchsia", target_os = "wasi")))]
#[cfg(feature = "alloc")]
#[cfg(feature = "fs")]
#[doc(alias = "ttyname_r")]
#[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
#[inline]
pub fn ttyname<Fd: AsFd, B: Into<Vec<u8>>>(fd: Fd, reuse: B) -> io::Result<CString> {
    _ttyname(fd.as_fd(), reuse.into())
}

#[cfg(not(any(target_os = "fuchsia", target_os = "wasi")))]
#[cfg(feature = "alloc")]
#[cfg(feature = "fs")]
#[allow(unsafe_code)]
fn _ttyname(fd: BorrowedFd<'_>, mut buffer: Vec<u8>) -> io::Result<CString> {
    buffer.clear();
    buffer.reserve(SMALL_PATH_BUFFER_SIZE);

    loop {
        match backend::termios::syscalls::ttyname(fd, buffer.spare_capacity_mut()) {
            Err(io::Errno::RANGE) => {
                // Use `Vec` reallocation strategy to grow capacity
                // exponentially.
                buffer.reserve(buffer.capacity() + 1);
            }
            Ok(len) => {
                // SAFETY: Assume the backend returns the length of the string
                // excluding the NUL.
                unsafe {
                    buffer.set_len(len + 1);
                }

                // SAFETY:
                // - “ttyname_r stores this pathname in the buffer buf”
                // - [POSIX definition 3.271: Pathname]: “A string that is
                //   used to identify a file.”
                // - [POSIX definition 3.375: String]: “A contiguous sequence
                //   of bytes terminated by and including the first null byte.”
                //
                // Thus, there will be a single NUL byte at the end of the
                // string.
                //
                // [POSIX definition 3.271: Pathname]: https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/V1_chap03.html#tag_03_271
                // [POSIX definition 3.375: String]: https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/V1_chap03.html#tag_03_375
                unsafe {
                    return Ok(CString::from_vec_with_nul_unchecked(buffer));
                }
            }
            Err(errno) => return Err(errno),
        }
    }
}
