use crate::ffi::CStr;

use crate::backend::c;
use crate::backend::conv::{c_str, ret, ret_owned_fd};
use crate::fd::OwnedFd;
use crate::fs::Mode;
use crate::io;
use crate::shm::ShmOFlags;

pub(crate) fn shm_open(name: &CStr, oflags: ShmOFlags, mode: Mode) -> io::Result<OwnedFd> {
    // On this platforms, `mode_t` is `u16` and can't be passed directly to a
    // variadic function.
    #[cfg(apple)]
    let mode: c::c_uint = mode.bits().into();

    // Otherwise, cast to `mode_t` as that's what `open` is documented to take.
    #[cfg(not(apple))]
    let mode: c::mode_t = mode.bits() as _;

    unsafe { ret_owned_fd(c::shm_open(c_str(name), bitflags_bits!(oflags), mode)) }
}

pub(crate) fn shm_unlink(name: &CStr) -> io::Result<()> {
    unsafe { ret(c::shm_unlink(c_str(name))) }
}
