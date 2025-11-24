// The functions replacing the C macros use the same names as in libc.
#![allow(non_snake_case, unsafe_code)]

use linux_raw_sys::ctypes::c_int;
pub(crate) use linux_raw_sys::general::{
    siginfo_t, WCONTINUED, WEXITED, WNOHANG, WNOWAIT, WSTOPPED, WUNTRACED,
};

#[inline]
pub(crate) fn WIFSTOPPED(status: u32) -> bool {
    (status & 0xff) == 0x7f
}

#[inline]
pub(crate) fn WSTOPSIG(status: u32) -> u32 {
    (status >> 8) & 0xff
}

#[inline]
pub(crate) fn WIFCONTINUED(status: u32) -> bool {
    status == 0xffff
}

#[inline]
pub(crate) fn WIFSIGNALED(status: u32) -> bool {
    ((status & 0x7f) + 1) as i8 >= 2
}

#[inline]
pub(crate) fn WTERMSIG(status: u32) -> u32 {
    status & 0x7f
}

#[inline]
pub(crate) fn WIFEXITED(status: u32) -> bool {
    (status & 0x7f) == 0
}

#[inline]
pub(crate) fn WEXITSTATUS(status: u32) -> u32 {
    (status >> 8) & 0xff
}

pub(crate) trait SiginfoExt {
    fn si_code(&self) -> c_int;
    unsafe fn si_status(&self) -> c_int;
}

impl SiginfoExt for siginfo_t {
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
    #[rustfmt::skip]
    unsafe fn si_status(&self) -> c_int {
        self.__bindgen_anon_1.__bindgen_anon_1._sifields._sigchld._status
    }
}
