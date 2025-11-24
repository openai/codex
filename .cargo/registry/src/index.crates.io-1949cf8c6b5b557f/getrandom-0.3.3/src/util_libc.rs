use crate::Error;
use core::mem::MaybeUninit;

cfg_if! {
    if #[cfg(any(target_os = "netbsd", target_os = "openbsd", target_os = "android", target_os = "cygwin"))] {
        use libc::__errno as errno_location;
    } else if #[cfg(any(target_os = "linux", target_os = "emscripten", target_os = "hurd", target_os = "redox", target_os = "dragonfly"))] {
        use libc::__errno_location as errno_location;
    } else if #[cfg(any(target_os = "solaris", target_os = "illumos"))] {
        use libc::___errno as errno_location;
    } else if #[cfg(any(target_os = "macos", target_os = "freebsd"))] {
        use libc::__error as errno_location;
    } else if #[cfg(target_os = "haiku")] {
        use libc::_errnop as errno_location;
    } else if #[cfg(target_os = "nto")] {
        use libc::__get_errno_ptr as errno_location;
    } else if #[cfg(any(all(target_os = "horizon", target_arch = "arm"), target_os = "vita"))] {
        extern "C" {
            // Not provided by libc: https://github.com/rust-lang/libc/issues/1995
            fn __errno() -> *mut libc::c_int;
        }
        use __errno as errno_location;
    } else if #[cfg(target_os = "aix")] {
        use libc::_Errno as errno_location;
    }
}

cfg_if! {
    if #[cfg(target_os = "vxworks")] {
        use libc::errnoGet as get_errno;
    } else {
        unsafe fn get_errno() -> libc::c_int { *errno_location() }
    }
}

pub(crate) fn last_os_error() -> Error {
    // We assume that on all targets which use the `util_libc` module `c_int` is equal to `i32`
    let errno: i32 = unsafe { get_errno() };

    if errno > 0 {
        let code = errno
            .checked_neg()
            .expect("Positive number can be always negated");
        Error::from_neg_error_code(code)
    } else {
        Error::ERRNO_NOT_POSITIVE
    }
}

/// Fill a buffer by repeatedly invoking `sys_fill`.
///
/// The `sys_fill` function:
///   - should return -1 and set errno on failure
///   - should return the number of bytes written on success
#[allow(dead_code)]
pub(crate) fn sys_fill_exact(
    mut buf: &mut [MaybeUninit<u8>],
    sys_fill: impl Fn(&mut [MaybeUninit<u8>]) -> libc::ssize_t,
) -> Result<(), Error> {
    while !buf.is_empty() {
        let res = sys_fill(buf);
        match res {
            res if res > 0 => {
                let len = usize::try_from(res).map_err(|_| Error::UNEXPECTED)?;
                buf = buf.get_mut(len..).ok_or(Error::UNEXPECTED)?;
            }
            -1 => {
                let err = last_os_error();
                // We should try again if the call was interrupted.
                if err.raw_os_error() != Some(libc::EINTR) {
                    return Err(err);
                }
            }
            // Negative return codes not equal to -1 should be impossible.
            // EOF (ret = 0) should be impossible, as the data we are reading
            // should be an infinite stream of random bytes.
            _ => return Err(Error::UNEXPECTED),
        }
    }
    Ok(())
}
