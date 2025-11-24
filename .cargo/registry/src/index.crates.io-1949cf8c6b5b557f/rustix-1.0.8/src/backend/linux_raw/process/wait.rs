// The functions replacing the C macros use the same names as in libc.
#![allow(non_snake_case, unsafe_code)]

use crate::ffi::c_int;
pub(crate) use linux_raw_sys::general::{
    siginfo_t, WCONTINUED, WEXITED, WNOHANG, WNOWAIT, WSTOPPED, WUNTRACED,
};

#[inline]
pub(crate) fn WIFSTOPPED(status: i32) -> bool {
    (status & 0xff) == 0x7f
}

#[inline]
pub(crate) fn WSTOPSIG(status: i32) -> i32 {
    (status >> 8) & 0xff
}

#[inline]
pub(crate) fn WIFCONTINUED(status: i32) -> bool {
    status == 0xffff
}

#[inline]
pub(crate) fn WIFSIGNALED(status: i32) -> bool {
    ((status & 0x7f) + 1) as i8 >= 2
}

#[inline]
pub(crate) fn WTERMSIG(status: i32) -> i32 {
    status & 0x7f
}

#[inline]
pub(crate) fn WIFEXITED(status: i32) -> bool {
    (status & 0x7f) == 0
}

#[inline]
pub(crate) fn WEXITSTATUS(status: i32) -> i32 {
    (status >> 8) & 0xff
}

pub(crate) trait SiginfoExt {
    fn si_signo(&self) -> c_int;
    fn si_errno(&self) -> c_int;
    fn si_code(&self) -> c_int;
    unsafe fn si_status(&self) -> c_int;
}

impl SiginfoExt for siginfo_t {
    #[inline]
    fn si_signo(&self) -> c_int {
        // SAFETY: This is technically a union access, but it's only a union
        // with padding.
        unsafe { self.__bindgen_anon_1.__bindgen_anon_1.si_signo }
    }

    #[inline]
    fn si_errno(&self) -> c_int {
        // SAFETY: This is technically a union access, but it's only a union
        // with padding.
        unsafe { self.__bindgen_anon_1.__bindgen_anon_1.si_errno }
    }

    #[inline]
    fn si_code(&self) -> c_int {
        // SAFETY: This is technically a union access, but it's only a union
        // with padding.
        unsafe { self.__bindgen_anon_1.__bindgen_anon_1.si_code }
    }

    /// Return the exit status or signal number recorded in a `siginfo_t`.
    ///
    /// # Safety
    ///
    /// `si_signo` must equal `SIGCHLD` (as it is guaranteed to do after a
    /// `waitid` call).
    #[inline]
    unsafe fn si_status(&self) -> c_int {
        self.__bindgen_anon_1
            .__bindgen_anon_1
            ._sifields
            ._sigchld
            ._status
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_libc_correspondence() {
        for status in [
            0,
            1,
            63,
            64,
            65,
            127,
            128,
            129,
            255,
            256,
            257,
            4095,
            4096,
            4097,
            i32::MAX,
            i32::MIN,
            u32::MAX as i32,
        ] {
            assert_eq!(WIFSTOPPED(status), libc::WIFSTOPPED(status));
            assert_eq!(WSTOPSIG(status), libc::WSTOPSIG(status));
            assert_eq!(WIFCONTINUED(status), libc::WIFCONTINUED(status));
            assert_eq!(WIFSIGNALED(status), libc::WIFSIGNALED(status));
            assert_eq!(WTERMSIG(status), libc::WTERMSIG(status));
            assert_eq!(WIFEXITED(status), libc::WIFEXITED(status));
            assert_eq!(WEXITSTATUS(status), libc::WEXITSTATUS(status));
        }
    }
}
