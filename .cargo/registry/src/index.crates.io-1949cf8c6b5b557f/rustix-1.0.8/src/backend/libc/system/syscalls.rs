//! libc syscalls supporting `rustix::process`.

use super::types::RawUname;
use crate::backend::c;
#[cfg(not(target_os = "wasi"))]
use crate::backend::conv::ret_infallible;
#[cfg(target_os = "linux")]
use crate::system::RebootCommand;
use core::mem::MaybeUninit;
#[cfg(linux_kernel)]
use {
    crate::backend::conv::c_str, crate::fd::BorrowedFd, crate::ffi::CStr, crate::system::Sysinfo,
};
#[cfg(not(any(
    target_os = "emscripten",
    target_os = "espidf",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
use {crate::backend::conv::ret, crate::io};

#[cfg(not(target_os = "wasi"))]
#[inline]
pub(crate) fn uname() -> RawUname {
    let mut uname = MaybeUninit::<RawUname>::uninit();
    unsafe {
        let r = c::uname(uname.as_mut_ptr());

        // On POSIX, `uname` is documented to return non-negative on success
        // instead of the usual 0, though some specific systems do document
        // that they always use zero allowing us to skip this check.
        #[cfg(not(any(apple, freebsdlike, linux_like, target_os = "netbsd")))]
        let r = core::cmp::min(r, 0);

        ret_infallible(r);
        uname.assume_init()
    }
}

#[cfg(linux_kernel)]
pub(crate) fn sysinfo() -> Sysinfo {
    let mut info = MaybeUninit::<Sysinfo>::uninit();
    unsafe {
        ret_infallible(c::sysinfo(info.as_mut_ptr()));
        info.assume_init()
    }
}

#[cfg(not(any(
    target_os = "emscripten",
    target_os = "espidf",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
pub(crate) fn sethostname(name: &[u8]) -> io::Result<()> {
    unsafe {
        ret(c::sethostname(
            name.as_ptr().cast(),
            name.len().try_into().map_err(|_| io::Errno::INVAL)?,
        ))
    }
}

#[cfg(not(any(
    target_os = "android",
    target_os = "cygwin",
    target_os = "emscripten",
    target_os = "espidf",
    target_os = "illumos",
    target_os = "haiku",
    target_os = "horizon",
    target_os = "redox",
    target_os = "solaris",
    target_os = "vita",
    target_os = "wasi",
)))]
pub(crate) fn setdomainname(name: &[u8]) -> io::Result<()> {
    unsafe {
        ret(c::setdomainname(
            name.as_ptr().cast(),
            name.len().try_into().map_err(|_| io::Errno::INVAL)?,
        ))
    }
}

// <https://github.com/rust-lang/libc/pull/4212>
#[cfg(target_os = "android")]
pub(crate) fn setdomainname(name: &[u8]) -> io::Result<()> {
    syscall! {
        fn setdomainname(
            name: *const c::c_char,
            len: c::size_t
        ) via SYS_setdomainname -> c::c_int
    }

    unsafe {
        ret(setdomainname(
            name.as_ptr().cast(),
            name.len().try_into().map_err(|_| io::Errno::INVAL)?,
        ))
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn reboot(cmd: RebootCommand) -> io::Result<()> {
    unsafe { ret(c::reboot(cmd as i32)) }
}

#[cfg(linux_kernel)]
#[inline]
pub(crate) fn init_module(image: &[u8], param_values: &CStr) -> io::Result<()> {
    syscall! {
        fn init_module(
            module_image: *const c::c_void,
            len: c::c_ulong,
            param_values: *const c::c_char
        ) via SYS_init_module -> c::c_int
    }

    unsafe {
        ret(init_module(
            image.as_ptr().cast(),
            image.len() as _,
            c_str(param_values),
        ))
    }
}

#[cfg(linux_kernel)]
#[inline]
pub(crate) fn finit_module(
    fd: BorrowedFd<'_>,
    param_values: &CStr,
    flags: c::c_int,
) -> io::Result<()> {
    use crate::fd::AsRawFd as _;

    syscall! {
        fn finit_module(
            fd: c::c_int,
            param_values: *const c::c_char,
            flags: c::c_int
        ) via SYS_finit_module -> c::c_int
    }

    unsafe { ret(finit_module(fd.as_raw_fd(), c_str(param_values), flags)) }
}

#[cfg(linux_kernel)]
#[inline]
pub(crate) fn delete_module(name: &CStr, flags: c::c_int) -> io::Result<()> {
    syscall! {
        fn delete_module(
            name: *const c::c_char,
            flags: c::c_int
        ) via SYS_delete_module -> c::c_int
    }
    unsafe { ret(delete_module(c_str(name), flags)) }
}
