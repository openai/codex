//! libc syscalls supporting `rustix::pty`.

use crate::backend::c;
use crate::backend::conv::{borrowed_fd, ret};
use crate::fd::BorrowedFd;
use crate::io;
#[cfg(all(
    feature = "alloc",
    any(
        apple,
        linux_like,
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "illumos"
    )
))]
use {
    crate::ffi::{CStr, CString},
    crate::path::SMALL_PATH_BUFFER_SIZE,
    alloc::borrow::ToOwned,
    alloc::vec::Vec,
};

#[cfg(not(linux_kernel))]
use crate::{backend::conv::ret_owned_fd, fd::OwnedFd, pty::OpenptFlags};

#[cfg(not(linux_kernel))]
#[inline]
pub(crate) fn openpt(flags: OpenptFlags) -> io::Result<OwnedFd> {
    unsafe { ret_owned_fd(c::posix_openpt(flags.bits() as _)) }
}

#[cfg(all(
    feature = "alloc",
    any(
        apple,
        linux_like,
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "illumos"
    )
))]
#[inline]
pub(crate) fn ptsname(fd: BorrowedFd<'_>, mut buffer: Vec<u8>) -> io::Result<CString> {
    // This code would benefit from having a better way to read into
    // uninitialized memory, but that requires `unsafe`.
    buffer.clear();
    buffer.reserve(SMALL_PATH_BUFFER_SIZE);
    buffer.resize(buffer.capacity(), 0_u8);

    loop {
        // On platforms with `ptsname_r`, use it.
        #[cfg(any(linux_like, target_os = "fuchsia", target_os = "illumos"))]
        let r = unsafe { c::ptsname_r(borrowed_fd(fd), buffer.as_mut_ptr().cast(), buffer.len()) };

        // FreeBSD 12 doesn't have `ptsname_r`.
        #[cfg(target_os = "freebsd")]
        let r = unsafe {
            weak! {
                fn ptsname_r(
                     c::c_int,
                     *mut c::c_char,
                     c::size_t
                ) -> c::c_int
            }
            if let Some(func) = ptsname_r.get() {
                func(borrowed_fd(fd), buffer.as_mut_ptr().cast(), buffer.len())
            } else {
                libc::ENOSYS
            }
        };

        // macOS 10.13.4 has `ptsname_r`; use it if we have it, otherwise fall
        // back to calling the underlying ioctl directly.
        #[cfg(apple)]
        let r = unsafe {
            weak! { fn ptsname_r(c::c_int, *mut c::c_char, c::size_t) -> c::c_int }

            if let Some(libc_ptsname_r) = ptsname_r.get() {
                libc_ptsname_r(borrowed_fd(fd), buffer.as_mut_ptr().cast(), buffer.len())
            } else {
                // The size declared in the `TIOCPTYGNAME` macro in
                // sys/ttycom.h is 128.
                let mut name: [u8; 128] = [0_u8; 128];
                match c::ioctl(borrowed_fd(fd), c::TIOCPTYGNAME as _, &mut name) {
                    0 => {
                        let len = CStr::from_ptr(name.as_ptr().cast()).to_bytes().len();
                        std::ptr::copy_nonoverlapping(name.as_ptr(), buffer.as_mut_ptr(), len + 1);
                        0
                    }
                    _ => libc_errno::errno().0,
                }
            }
        };

        if r == 0 {
            return Ok(unsafe { CStr::from_ptr(buffer.as_ptr().cast()).to_owned() });
        }
        if r != c::ERANGE {
            return Err(io::Errno::from_raw_os_error(r));
        }

        // Use `Vec` reallocation strategy to grow capacity exponentially.
        buffer.reserve(1);
        buffer.resize(buffer.capacity(), 0_u8);
    }
}

#[inline]
pub(crate) fn unlockpt(fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe { ret(c::unlockpt(borrowed_fd(fd))) }
}

#[cfg(not(linux_kernel))]
#[inline]
pub(crate) fn grantpt(fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe { ret(c::grantpt(borrowed_fd(fd))) }
}
