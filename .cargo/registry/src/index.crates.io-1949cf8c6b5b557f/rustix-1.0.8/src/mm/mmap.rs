//! The `mmap` API.
//!
//! # Safety
//!
//! `mmap` and related functions manipulate raw pointers and have special
//! semantics.
#![allow(unsafe_code)]

use crate::{backend, io};
use backend::fd::AsFd;
use core::ffi::c_void;

#[cfg(any(linux_kernel, freebsdlike, netbsdlike))]
pub use backend::mm::types::MlockAllFlags;
#[cfg(linux_kernel)]
pub use backend::mm::types::MlockFlags;
#[cfg(any(target_os = "emscripten", target_os = "linux"))]
pub use backend::mm::types::MremapFlags;
pub use backend::mm::types::{MapFlags, MprotectFlags, ProtFlags};

impl MapFlags {
    /// Create `MAP_HUGETLB` with provided size of huge page.
    ///
    /// Under the hood it computes
    /// `MAP_HUGETLB | (huge_page_size_log2 << MAP_HUGE_SHIFT)`.
    /// `huge_page_size_log2` denotes logarithm of huge page size to use and
    /// should be between 16 and 63 (inclusive).
    ///
    /// ```
    /// use rustix::mm::MapFlags;
    ///
    /// let f = MapFlags::hugetlb_with_size_log2(30).unwrap();
    /// assert_eq!(f, MapFlags::HUGETLB | MapFlags::HUGE_1GB);
    /// ```
    #[cfg(linux_kernel)]
    pub const fn hugetlb_with_size_log2(huge_page_size_log2: u32) -> Option<Self> {
        use crate::backend::c;
        if 16 <= huge_page_size_log2 && huge_page_size_log2 <= 63 {
            let bits = bitcast!(c::MAP_HUGETLB) | (huge_page_size_log2 << c::MAP_HUGE_SHIFT);
            Self::from_bits(bits)
        } else {
            None
        }
    }
}

/// `mmap(ptr, len, prot, flags, fd, offset)`—Create a file-backed memory
/// mapping.
///
/// For anonymous mappings (`MAP_ANON`/`MAP_ANONYMOUS`), see
/// [`mmap_anonymous`].
///
/// # Safety
///
/// If `ptr` is not null, it must be aligned to the applicable page size, and
/// the range of memory starting at `ptr` and extending for `len` bytes,
/// rounded up to the applicable page size, must be valid to mutate using
/// `ptr`'s provenance.
///
/// If there exist any Rust references referring to the memory region, or if
/// you subsequently create a Rust reference referring to the resulting region,
/// it is your responsibility to ensure that the Rust reference invariants are
/// preserved, including ensuring that the memory is not mutated in a way that
/// a Rust reference would not expect.
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
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/mmap.html
/// [Linux]: https://man7.org/linux/man-pages/man2/mmap.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/mmap.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=mmap&sektion=2
/// [NetBSD]: https://man.netbsd.org/mmap.2
/// [OpenBSD]: https://man.openbsd.org/mmap.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=mmap&section=2
/// [illumos]: https://illumos.org/man/2/mmap
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Memory_002dmapped-I_002fO.html#index-mmap
#[inline]
pub unsafe fn mmap<Fd: AsFd>(
    ptr: *mut c_void,
    len: usize,
    prot: ProtFlags,
    flags: MapFlags,
    fd: Fd,
    offset: u64,
) -> io::Result<*mut c_void> {
    backend::mm::syscalls::mmap(ptr, len, prot, flags, fd.as_fd(), offset)
}

/// `mmap(ptr, len, prot, MAP_ANONYMOUS | flags, -1, 0)`—Create an anonymous
/// memory mapping.
///
/// For file-backed mappings, see [`mmap`].
///
/// # Safety
///
/// If `ptr` is not null, it must be aligned to the applicable page size, and
/// the range of memory starting at `ptr` and extending for `len` bytes,
/// rounded up to the applicable page size, must be valid to mutate with
/// `ptr`'s provenance.
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
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/mmap.html
/// [Linux]: https://man7.org/linux/man-pages/man2/mmap.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/mmap.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=mmap&sektion=2
/// [NetBSD]: https://man.netbsd.org/mmap.2
/// [OpenBSD]: https://man.openbsd.org/mmap.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=mmap&section=2
/// [illumos]: https://illumos.org/man/2/mmap
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Memory_002dmapped-I_002fO.html#index-mmap
#[inline]
#[doc(alias = "mmap")]
pub unsafe fn mmap_anonymous(
    ptr: *mut c_void,
    len: usize,
    prot: ProtFlags,
    flags: MapFlags,
) -> io::Result<*mut c_void> {
    backend::mm::syscalls::mmap_anonymous(ptr, len, prot, flags)
}

/// `munmap(ptr, len)`—Remove a memory mapping.
///
/// # Safety
///
/// `ptr` must be aligned to the applicable page size, and the range of memory
/// starting at `ptr` and extending for `len` bytes, rounded up to the
/// applicable page size, must be valid to mutate with `ptr`'s provenance. And
/// there must be no Rust references referring to that memory.
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
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/munmap.html
/// [Linux]: https://man7.org/linux/man-pages/man2/munmap.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/munmap.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=munmap&sektion=2
/// [NetBSD]: https://man.netbsd.org/munmap.2
/// [OpenBSD]: https://man.openbsd.org/munmap.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=munmap&section=2
/// [illumos]: https://illumos.org/man/2/munmap
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Memory_002dmapped-I_002fO.html#index-munmap
#[inline]
pub unsafe fn munmap(ptr: *mut c_void, len: usize) -> io::Result<()> {
    backend::mm::syscalls::munmap(ptr, len)
}

/// `mremap(old_address, old_size, new_size, flags)`—Resize, modify, and/or
/// move a memory mapping.
///
/// For moving a mapping to a fixed address (`MREMAP_FIXED`), see
/// [`mremap_fixed`].
///
/// # Safety
///
/// `old_address` must be aligned to the applicable page size, and the range of
/// memory starting at `old_address` and extending for `old_size` bytes,
/// rounded up to the applicable page size, must be valid to mutate with
/// `old_address`'s provenance. If `MremapFlags::MAY_MOVE` is set in `flags`,
/// there must be no Rust references referring to that the memory.
///
/// If `new_size` is less than `old_size`, than there must be no Rust
/// references referring to the memory starting at offset `new_size` and ending
/// at `old_size`.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/mremap.2.html
#[cfg(any(target_os = "emscripten", target_os = "linux"))]
#[inline]
pub unsafe fn mremap(
    old_address: *mut c_void,
    old_size: usize,
    new_size: usize,
    flags: MremapFlags,
) -> io::Result<*mut c_void> {
    backend::mm::syscalls::mremap(old_address, old_size, new_size, flags)
}

/// `mremap(old_address, old_size, new_size, MREMAP_FIXED | flags)`—Resize,
/// modify, and/or move a memory mapping to a specific address.
///
/// For `mremap` without moving to a specific address, see [`mremap`].
/// [`mremap_fixed`].
///
/// # Safety
///
/// `old_address` and `new_address` must be aligned to the applicable page
/// size, the range of memory starting at `old_address` and extending for
/// `old_size` bytes, rounded up to the applicable page size, must be valid to
/// mutate with `old_address`'s provenance, and the range of memory starting at
/// `new_address` and extending for `new_size` bytes, rounded up to the
/// applicable page size, must be valid to mutate with `new_address`'s
/// provenance.
///
/// There must be no Rust references referring to either of those memory
/// regions.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/mremap.2.html
#[cfg(any(target_os = "emscripten", target_os = "linux"))]
#[inline]
#[doc(alias = "mremap")]
pub unsafe fn mremap_fixed(
    old_address: *mut c_void,
    old_size: usize,
    new_size: usize,
    flags: MremapFlags,
    new_address: *mut c_void,
) -> io::Result<*mut c_void> {
    backend::mm::syscalls::mremap_fixed(old_address, old_size, new_size, flags, new_address)
}

/// `mprotect(ptr, len, flags)`—Change the protection flags of a region of
/// memory.
///
/// # Safety
///
/// The range of memory starting at `ptr` and extending for `len` bytes,
/// rounded up to the applicable page size, must be valid to read with `ptr`'s
/// provenance.
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
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/mprotect.html
/// [Linux]: https://man7.org/linux/man-pages/man2/mprotect.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/mprotect.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=mprotect&sektion=2
/// [NetBSD]: https://man.netbsd.org/mprotect.2
/// [OpenBSD]: https://man.openbsd.org/mprotect.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=mprotect&section=2
/// [illumos]: https://illumos.org/man/2/mprotect
#[inline]
pub unsafe fn mprotect(ptr: *mut c_void, len: usize, flags: MprotectFlags) -> io::Result<()> {
    backend::mm::syscalls::mprotect(ptr, len, flags)
}

/// `mlock(ptr, len)`—Lock memory into RAM.
///
/// Some implementations implicitly round the memory region out to the nearest
/// page boundaries, so this function may lock more memory than explicitly
/// requested if the memory isn't page-aligned. Other implementations fail if
/// the memory isn't page-aligned.
///
/// See [`mlock_with`] to pass additional flags.
///
/// # Safety
///
/// The range of memory starting at `ptr`, rounded down to the applicable page
/// boundary, and extending for `len` bytes, rounded up to the applicable page
/// size, must be valid to read with `ptr`'s provenance.
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
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/mlock.html
/// [Linux]: https://man7.org/linux/man-pages/man2/mlock.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/mlock.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=mlock&sektion=2
/// [NetBSD]: https://man.netbsd.org/mlock.2
/// [OpenBSD]: https://man.openbsd.org/mlock.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=mlock&section=2
/// [illumos]: https://illumos.org/man/3C/mlock
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Page-Lock-Functions.html#index-mlock
#[inline]
pub unsafe fn mlock(ptr: *mut c_void, len: usize) -> io::Result<()> {
    backend::mm::syscalls::mlock(ptr, len)
}

/// `mlock2(ptr, len, flags)`—Lock memory into RAM, with flags.
///
/// `mlock_with` is the same as [`mlock`] but adds an additional flags operand.
///
/// Some implementations implicitly round the memory region out to the nearest
/// page boundaries, so this function may lock more memory than explicitly
/// requested if the memory isn't page-aligned.
///
/// # Safety
///
/// The range of memory starting at `ptr`, rounded down to the applicable page
/// boundary, and extending for `len` bytes, rounded up to the applicable page
/// size, must be valid to read with `ptr`'s provenance.
///
/// # References
///  - [Linux]
///  - [glibc]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/mlock2.2.html
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Page-Lock-Functions.html#index-mlock2
#[cfg(linux_kernel)]
#[inline]
#[doc(alias = "mlock2")]
pub unsafe fn mlock_with(ptr: *mut c_void, len: usize, flags: MlockFlags) -> io::Result<()> {
    backend::mm::syscalls::mlock_with(ptr, len, flags)
}

/// `munlock(ptr, len)`—Unlock memory.
///
/// Some implementations implicitly round the memory region out to the nearest
/// page boundaries, so this function may unlock more memory than explicitly
/// requested if the memory isn't page-aligned.
///
/// # Safety
///
/// The range of memory starting at `ptr`, rounded down to the applicable page
/// boundary, and extending for `len` bytes, rounded up to the applicable page
/// size, must be valid to read with `ptr`'s provenance.
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
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/munlock.html
/// [Linux]: https://man7.org/linux/man-pages/man2/munlock.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/munlock.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=munlock&sektion=2
/// [NetBSD]: https://man.netbsd.org/munlock.2
/// [OpenBSD]: https://man.openbsd.org/munlock.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=munlock&section=2
/// [illumos]: https://illumos.org/man/3C/munlock
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Page-Lock-Functions.html#index-munlock
#[inline]
pub unsafe fn munlock(ptr: *mut c_void, len: usize) -> io::Result<()> {
    backend::mm::syscalls::munlock(ptr, len)
}

/// Locks all pages mapped into the address space of the calling process.
///
/// This includes the pages of the code, data, and stack segment, as well as
/// shared libraries, user space kernel data, shared memory, and memory-mapped
/// files. All mapped pages are guaranteed to be resident in RAM when the call
/// returns successfully; the pages are guaranteed to stay in RAM until later
/// unlocked.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///  - [glibc]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/mlockall.html
/// [Linux]: https://man7.org/linux/man-pages/man2/mlockall.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=mlockall&sektion=2
/// [NetBSD]: https://man.netbsd.org/mlockall.2
/// [OpenBSD]: https://man.openbsd.org/mlockall.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=mlockall&section=2
/// [illumos]: https://illumos.org/man/3C/mlockall
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Page-Lock-Functions.html#index-mlockall
#[cfg(any(linux_kernel, freebsdlike, netbsdlike))]
#[inline]
pub fn mlockall(flags: MlockAllFlags) -> io::Result<()> {
    backend::mm::syscalls::mlockall(flags)
}

/// Unlocks all pages mapped into the address space of the calling process.
///
/// # Warning
///
/// This function is aware of all the memory pages in the process, as if it
/// were a debugger. It unlocks all the pages, which could potentially
/// compromise security assumptions made by code about memory it has
/// encapsulated.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///  - [glibc]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/munlockall.html
/// [Linux]: https://man7.org/linux/man-pages/man2/munlockall.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=munlockall&sektion=2
/// [NetBSD]: https://man.netbsd.org/munlockall.2
/// [OpenBSD]: https://man.openbsd.org/munlockall.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=munlockall&section=2
/// [illumos]: https://illumos.org/man/3C/munlockall
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Page-Lock-Functions.html#index-munlockall
#[cfg(any(linux_kernel, freebsdlike, netbsdlike))]
#[inline]
pub fn munlockall() -> io::Result<()> {
    backend::mm::syscalls::munlockall()
}
