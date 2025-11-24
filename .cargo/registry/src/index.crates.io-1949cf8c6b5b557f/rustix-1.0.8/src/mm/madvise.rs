//! The `madvise` function.
//!
//! # Safety
//!
//! `madvise` operates on a raw pointer. Some forms of `madvise` may
//! mutate the memory or have other side effects.
#![allow(unsafe_code)]

use crate::{backend, io};
use core::ffi::c_void;

pub use backend::mm::types::Advice;

/// `posix_madvise(addr, len, advice)`—Declares an expected access pattern
/// for a memory-mapped file.
///
/// # Safety
///
/// `addr` must be a valid pointer to memory that is appropriate to call
/// `posix_madvise` on. Some forms of `advice` may mutate the memory or evoke a
/// variety of side-effects on the mapping and/or the file.
///
/// # References
///  - [POSIX]
///  - [Linux `madvise`]
///  - [Linux `posix_madvise`]
///  - [Apple]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///  - [glibc]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/posix_madvise.html
/// [Linux `madvise`]: https://man7.org/linux/man-pages/man2/madvise.2.html
/// [Linux `posix_madvise`]: https://man7.org/linux/man-pages/man3/posix_madvise.3.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/madvise.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=madvise&sektion=2
/// [NetBSD]: https://man.netbsd.org/madvise.2
/// [OpenBSD]: https://man.openbsd.org/madvise.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=madvise&section=2
/// [illumos]: https://illumos.org/man/3C/madvise
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Memory_002dmapped-I_002fO.html#index-madvise
#[inline]
#[doc(alias = "posix_madvise")]
pub unsafe fn madvise(addr: *mut c_void, len: usize, advice: Advice) -> io::Result<()> {
    backend::mm::syscalls::madvise(addr, len, advice)
}
