#[cfg(not(target_os = "fuchsia"))]
use crate::backend::fd::AsFd;
#[cfg(feature = "fs")]
use crate::path;
#[cfg(any(feature = "fs", not(target_os = "fuchsia")))]
use crate::{backend, io};
#[cfg(all(feature = "alloc", feature = "fs"))]
use {
    crate::ffi::{CStr, CString},
    crate::path::SMALL_PATH_BUFFER_SIZE,
    alloc::vec::Vec,
};

/// `chdir(path)`—Change the current working directory.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/chdir.html
/// [Linux]: https://man7.org/linux/man-pages/man2/chdir.2.html
#[inline]
#[cfg(feature = "fs")]
#[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
pub fn chdir<P: path::Arg>(path: P) -> io::Result<()> {
    path.into_with_c_str(backend::process::syscalls::chdir)
}

/// `fchdir(fd)`—Change the current working directory.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/fchdir.html
/// [Linux]: https://man7.org/linux/man-pages/man2/fchdir.2.html
#[cfg(not(target_os = "fuchsia"))]
#[inline]
pub fn fchdir<Fd: AsFd>(fd: Fd) -> io::Result<()> {
    backend::process::syscalls::fchdir(fd.as_fd())
}

/// `getcwd`—Return the current working directory.
///
/// If `reuse` already has available capacity, reuse it if possible.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/getcwd.html
/// [Linux]: https://man7.org/linux/man-pages/man3/getcwd.3.html
#[cfg(all(feature = "alloc", feature = "fs"))]
#[cfg(not(target_os = "wasi"))]
#[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
#[inline]
pub fn getcwd<B: Into<Vec<u8>>>(reuse: B) -> io::Result<CString> {
    _getcwd(reuse.into())
}

#[cfg(all(feature = "alloc", feature = "fs"))]
#[allow(unsafe_code)]
fn _getcwd(mut buffer: Vec<u8>) -> io::Result<CString> {
    buffer.clear();
    buffer.reserve(SMALL_PATH_BUFFER_SIZE);

    loop {
        match backend::process::syscalls::getcwd(buffer.spare_capacity_mut()) {
            Err(io::Errno::RANGE) => {
                // Use `Vec` reallocation strategy to grow capacity
                // exponentially.
                buffer.reserve(buffer.capacity() + 1);
            }
            Ok(_) => {
                // SAFETY:
                // - “These functions return a null-terminated string”
                // - [POSIX definition 3.375: String]: “A contiguous sequence
                //   of bytes terminated by and including the first null byte.”
                //
                // Thus, there will be a single NUL byte at the end of the
                // string.
                //
                // [POSIX definition 3.375: String]: https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/V1_chap03.html#tag_03_375
                unsafe {
                    buffer.set_len(
                        CStr::from_ptr(buffer.as_ptr().cast())
                            .to_bytes_with_nul()
                            .len(),
                    );

                    return Ok(CString::from_vec_with_nul_unchecked(buffer));
                }
            }
            Err(errno) => return Err(errno),
        }
    }
}
