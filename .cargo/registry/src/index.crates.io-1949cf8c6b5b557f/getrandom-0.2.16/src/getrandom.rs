//! Implementation using getrandom(2).
//!
//! Available since:
//!   - Linux Kernel 3.17, Glibc 2.25, Musl 1.1.20
//!   - Android API level 23 (Marshmallow)
//!   - NetBSD 10.0
//!   - FreeBSD 12.0
//!   - illumos since Dec 2018
//!   - DragonFly 5.7
//!   - Hurd Glibc 2.31
//!   - shim-3ds since Feb 2022
//!
//! For these platforms, we always use the default pool and never set the
//! GRND_RANDOM flag to use the /dev/random pool. On Linux/Android/Hurd, using
//! GRND_RANDOM is not recommended. On NetBSD/FreeBSD/Dragonfly/3ds, it does
//! nothing. On illumos, the default pool is used to implement getentropy(2),
//! so we assume it is acceptable here.
use crate::{util_libc::sys_fill_exact, Error};
use core::mem::MaybeUninit;

pub fn getrandom_inner(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    sys_fill_exact(dest, |buf| unsafe {
        libc::getrandom(buf.as_mut_ptr() as *mut libc::c_void, buf.len(), 0)
    })
}
