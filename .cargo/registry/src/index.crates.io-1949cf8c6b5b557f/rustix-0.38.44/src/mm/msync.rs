//! The `msync` function.
//!
//! # Safety
//!
//! `msync` operates on a raw pointer. Some forms of `msync` may mutate the
//! memory or have other side effects.
#![allow(unsafe_code)]

use crate::{backend, io};
use core::ffi::c_void;

pub use backend::mm::types::MsyncFlags;

/// `msync(addr, len, flags)`—Synchronizes a memory-mapping with its backing
/// storage.
///
/// # Safety
///
/// `addr` must be a valid pointer to memory that is appropriate to call
/// `msync` on. Some forms of `msync` may mutate the memory or evoke a variety
/// of side-effects on the mapping and/or the file.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [Apple]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///  - [glibc]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/msync.html
/// [Linux]: https://man7.org/linux/man-pages/man2/msync.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/msync.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=msync&sektion=2
/// [NetBSD]: https://man.netbsd.org/msync.2
/// [OpenBSD]: https://man.openbsd.org/msync.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=msync&section=2
/// [illumos]: https://illumos.org/man/3C/msync
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Memory_002dmapped-I_002fO.html#index-msync
#[inline]
pub unsafe fn msync(addr: *mut c_void, len: usize, flags: MsyncFlags) -> io::Result<()> {
    backend::mm::syscalls::msync(addr, len, flags)
}
