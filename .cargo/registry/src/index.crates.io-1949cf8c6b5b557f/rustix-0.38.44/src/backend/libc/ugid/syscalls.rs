use crate::backend::c;
use crate::ugid::{Gid, Uid};

#[cfg(not(target_os = "wasi"))]
#[inline]
#[must_use]
pub(crate) fn getuid() -> Uid {
    unsafe {
        let uid = c::getuid();
        Uid::from_raw(uid)
    }
}

#[cfg(not(target_os = "wasi"))]
#[inline]
#[must_use]
pub(crate) fn geteuid() -> Uid {
    unsafe {
        let uid = c::geteuid();
        Uid::from_raw(uid)
    }
}

#[cfg(not(target_os = "wasi"))]
#[inline]
#[must_use]
pub(crate) fn getgid() -> Gid {
    unsafe {
        let gid = c::getgid();
        Gid::from_raw(gid)
    }
}

#[cfg(not(target_os = "wasi"))]
#[inline]
#[must_use]
pub(crate) fn getegid() -> Gid {
    unsafe {
        let gid = c::getegid();
        Gid::from_raw(gid)
    }
}
