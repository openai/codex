//! Memory management declarations.

use crate::errno::Errno;
#[cfg(not(target_os = "android"))]
use crate::NixPath;
use crate::Result;
#[cfg(not(target_os = "android"))]
#[cfg(feature = "fs")]
use crate::{fcntl::OFlag, sys::stat::Mode};
use libc::{self, c_int, c_void, off_t, size_t};
use std::ptr::NonNull;
use std::{
    num::NonZeroUsize,
    os::unix::io::{AsFd, AsRawFd},
};

libc_bitflags! {
    /// Desired memory protection of a memory mapping.
    pub struct ProtFlags: c_int {
        /// Pages cannot be accessed.
        PROT_NONE;
        /// Pages can be read.
        PROT_READ;
        /// Pages can be written.
        PROT_WRITE;
        /// Pages can be executed
        PROT_EXEC;
        /// Apply protection up to the end of a mapping that grows upwards.
        #[cfg(linux_android)]
        PROT_GROWSDOWN;
        /// Apply protection down to the beginning of a mapping that grows downwards.
        #[cfg(linux_android)]
        PROT_GROWSUP;
    }
}

libc_bitflags! {
    /// Additional parameters for [`mmap`].
    pub struct MapFlags: c_int {
        /// Compatibility flag. Ignored.
        MAP_FILE;
        /// Share this mapping. Mutually exclusive with `MAP_PRIVATE`.
        MAP_SHARED;
        /// Create a private copy-on-write mapping. Mutually exclusive with `MAP_SHARED`.
        MAP_PRIVATE;
        /// Place the mapping at exactly the address specified in `addr`.
        MAP_FIXED;
        /// Place the mapping at exactly the address specified in `addr`, but never clobber an existing range.
        #[cfg(target_os = "linux")]
        MAP_FIXED_NOREPLACE;
        /// To be used with `MAP_FIXED`, to forbid the system
        /// to select a different address than the one specified.
        #[cfg(target_os = "freebsd")]
        MAP_EXCL;
        /// Synonym for `MAP_ANONYMOUS`.
        MAP_ANON;
        /// The mapping is not backed by any file.
        MAP_ANONYMOUS;
        /// Put the mapping into the first 2GB of the process address space.
        #[cfg(any(all(linux_android,
                      any(target_arch = "x86", target_arch = "x86_64")),
                  all(target_os = "linux", target_env = "musl", any(target_arch = "x86", target_arch = "x86_64")),
                  all(target_os = "freebsd", target_pointer_width = "64")))]
        MAP_32BIT;
        /// Used for stacks; indicates to the kernel that the mapping should extend downward in memory.
        #[cfg(linux_android)]
        MAP_GROWSDOWN;
        /// Compatibility flag. Ignored.
        #[cfg(linux_android)]
        MAP_DENYWRITE;
        /// Compatibility flag. Ignored.
        #[cfg(linux_android)]
        MAP_EXECUTABLE;
        /// Mark the mmaped region to be locked in the same way as `mlock(2)`.
        #[cfg(linux_android)]
        MAP_LOCKED;
        /// Do not reserve swap space for this mapping.
        ///
        /// This was removed in FreeBSD 11 and is unused in DragonFlyBSD.
        #[cfg(not(any(freebsdlike, target_os = "aix", target_os = "hurd")))]
        MAP_NORESERVE;
        /// Populate page tables for a mapping.
        #[cfg(linux_android)]
        MAP_POPULATE;
        /// Only meaningful when used with `MAP_POPULATE`. Don't perform read-ahead.
        #[cfg(linux_android)]
        MAP_NONBLOCK;
        /// Allocate the mapping using "huge pages."
        #[cfg(linux_android)]
        MAP_HUGETLB;
        /// Make use of 64KB huge page (must be supported by the system)
        #[cfg(target_os = "linux")]
        MAP_HUGE_64KB;
        /// Make use of 512KB huge page (must be supported by the system)
        #[cfg(target_os = "linux")]
        MAP_HUGE_512KB;
        /// Make use of 1MB huge page (must be supported by the system)
        #[cfg(target_os = "linux")]
        MAP_HUGE_1MB;
        /// Make use of 2MB huge page (must be supported by the system)
        #[cfg(target_os = "linux")]
        MAP_HUGE_2MB;
        /// Make use of 8MB huge page (must be supported by the system)
        #[cfg(target_os = "linux")]
        MAP_HUGE_8MB;
        /// Make use of 16MB huge page (must be supported by the system)
        #[cfg(target_os = "linux")]
        MAP_HUGE_16MB;
        /// Make use of 32MB huge page (must be supported by the system)
        #[cfg(target_os = "linux")]
        MAP_HUGE_32MB;
        /// Make use of 256MB huge page (must be supported by the system)
        #[cfg(target_os = "linux")]
        MAP_HUGE_256MB;
        /// Make use of 512MB huge page (must be supported by the system)
        #[cfg(target_os = "linux")]
        MAP_HUGE_512MB;
        /// Make use of 1GB huge page (must be supported by the system)
        #[cfg(target_os = "linux")]
        MAP_HUGE_1GB;
        /// Make use of 2GB huge page (must be supported by the system)
        #[cfg(target_os = "linux")]
        MAP_HUGE_2GB;
        /// Make use of 16GB huge page (must be supported by the system)
        #[cfg(target_os = "linux")]
        MAP_HUGE_16GB;

        /// Lock the mapped region into memory as with `mlock(2)`.
        #[cfg(target_os = "netbsd")]
        MAP_WIRED;
        /// Causes dirtied data in the specified range to be flushed to disk only when necessary.
        #[cfg(freebsdlike)]
        MAP_NOSYNC;
        /// Rename private pages to a file.
        ///
        /// This was removed in FreeBSD 11 and is unused in DragonFlyBSD.
        #[cfg(netbsdlike)]
        MAP_RENAME;
        /// Region may contain semaphores.
        #[cfg(any(freebsdlike, netbsdlike))]
        MAP_HASSEMAPHORE;
        /// Region grows down, like a stack.
        #[cfg(any(linux_android, freebsdlike, target_os = "openbsd"))]
        MAP_STACK;
        /// Pages in this mapping are not retained in the kernel's memory cache.
        #[cfg(apple_targets)]
        MAP_NOCACHE;
        /// Allows the W/X bit on the page, it's necessary on aarch64 architecture.
        #[cfg(apple_targets)]
        MAP_JIT;
        /// Allows to use large pages, underlying alignment based on size.
        #[cfg(target_os = "freebsd")]
        MAP_ALIGNED_SUPER;
        /// Pages will be discarded in the core dumps.
        #[cfg(target_os = "openbsd")]
        MAP_CONCEAL;
        /// Attempt to place the mapping at exactly the address specified in `addr`.
        /// it's a default behavior on OpenBSD.
        #[cfg(netbsdlike)]
        MAP_TRYFIXED;
    }
}

impl MapFlags {
    /// Create `MAP_HUGETLB` with provided size of huge page.
    ///
    /// Under the hood it computes `MAP_HUGETLB | (huge_page_size_log2 << libc::MAP_HUGE_SHIFT`).
    /// `huge_page_size_log2` denotes logarithm of huge page size to use and should be
    /// between 16 and 63 (inclusively).
    ///
    /// ```
    /// # use nix::sys::mman::MapFlags;
    /// let f = MapFlags::map_hugetlb_with_size_log2(30).unwrap();
    /// assert_eq!(f, MapFlags::MAP_HUGETLB | MapFlags::MAP_HUGE_1GB);
    /// ```
    #[cfg(any(linux_android, target_os = "fuchsia"))]
    pub fn map_hugetlb_with_size_log2(
        huge_page_size_log2: u32,
    ) -> Option<Self> {
        if (16..=63).contains(&huge_page_size_log2) {
            let flag = libc::MAP_HUGETLB
                | (huge_page_size_log2 << libc::MAP_HUGE_SHIFT) as i32;
            Some(Self(flag.into()))
        } else {
            None
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "netbsd"))]
libc_bitflags! {
    /// Options for [`mremap`].
    pub struct MRemapFlags: c_int {
        /// Permit the kernel to relocate the mapping to a new virtual address, if necessary.
        #[cfg(target_os = "linux")]
        MREMAP_MAYMOVE;
        /// Place the mapping at exactly the address specified in `new_address`.
        #[cfg(target_os = "linux")]
        MREMAP_FIXED;
        /// Place the mapping at exactly the address specified in `new_address`.
        #[cfg(target_os = "netbsd")]
        MAP_FIXED;
        /// Allows to duplicate the mapping to be able to apply different flags on the copy.
        #[cfg(target_os = "netbsd")]
        MAP_REMAPDUP;
    }
}

libc_enum! {
    /// Usage information for a range of memory to allow for performance optimizations by the kernel.
    ///
    /// Used by [`madvise`].
    #[repr(i32)]
    #[non_exhaustive]
    pub enum MmapAdvise {
        /// No further special treatment. This is the default.
        MADV_NORMAL,
        /// Expect random page references.
        MADV_RANDOM,
        /// Expect sequential page references.
        MADV_SEQUENTIAL,
        /// Expect access in the near future.
        MADV_WILLNEED,
        /// Do not expect access in the near future.
        MADV_DONTNEED,
        /// Free up a given range of pages and its associated backing store.
        #[cfg(linux_android)]
        MADV_REMOVE,
        /// Do not make pages in this range available to the child after a `fork(2)`.
        #[cfg(linux_android)]
        MADV_DONTFORK,
        /// Undo the effect of `MADV_DONTFORK`.
        #[cfg(linux_android)]
        MADV_DOFORK,
        /// Poison the given pages.
        ///
        /// Subsequent references to those pages are treated like hardware memory corruption.
        #[cfg(linux_android)]
        MADV_HWPOISON,
        /// Enable Kernel Samepage Merging (KSM) for the given pages.
        #[cfg(linux_android)]
        MADV_MERGEABLE,
        /// Undo the effect of `MADV_MERGEABLE`
        #[cfg(linux_android)]
        MADV_UNMERGEABLE,
        /// Preserve the memory of each page but offline the original page.
        #[cfg(any(target_os = "android",
            all(target_os = "linux", any(
                target_arch = "aarch64",
                target_arch = "arm",
                target_arch = "powerpc",
                target_arch = "powerpc64",
                target_arch = "s390x",
                target_arch = "x86",
                target_arch = "x86_64",
                target_arch = "sparc64"))))]
        MADV_SOFT_OFFLINE,
        /// Enable Transparent Huge Pages (THP) for pages in the given range.
        #[cfg(linux_android)]
        MADV_HUGEPAGE,
        /// Undo the effect of `MADV_HUGEPAGE`.
        #[cfg(linux_android)]
        MADV_NOHUGEPAGE,
        /// Exclude the given range from a core dump.
        #[cfg(linux_android)]
        MADV_DONTDUMP,
        /// Undo the effect of an earlier `MADV_DONTDUMP`.
        #[cfg(linux_android)]
        MADV_DODUMP,
        /// Specify that the application no longer needs the pages in the given range.
        #[cfg(not(any(target_os = "aix", target_os = "hurd")))]
        MADV_FREE,
        /// Request that the system not flush the current range to disk unless it needs to.
        #[cfg(freebsdlike)]
        MADV_NOSYNC,
        /// Undoes the effects of `MADV_NOSYNC` for any future pages dirtied within the given range.
        #[cfg(freebsdlike)]
        MADV_AUTOSYNC,
        /// Region is not included in a core file.
        #[cfg(freebsdlike)]
        MADV_NOCORE,
        /// Include region in a core file
        #[cfg(freebsdlike)]
        MADV_CORE,
        /// This process should not be killed when swap space is exhausted.
        #[cfg(any(target_os = "freebsd"))]
        MADV_PROTECT,
        /// Invalidate the hardware page table for the given region.
        #[cfg(target_os = "dragonfly")]
        MADV_INVAL,
        /// Set the offset of the page directory page to `value` for the virtual page table.
        #[cfg(target_os = "dragonfly")]
        MADV_SETMAP,
        /// Indicates that the application will not need the data in the given range.
        #[cfg(apple_targets)]
        MADV_ZERO_WIRED_PAGES,
        /// Pages can be reused (by anyone).
        #[cfg(apple_targets)]
        MADV_FREE_REUSABLE,
        /// Caller wants to reuse those pages.
        #[cfg(apple_targets)]
        MADV_FREE_REUSE,
        // Darwin doesn't document this flag's behavior.
        #[cfg(apple_targets)]
        #[allow(missing_docs)]
        MADV_CAN_REUSE,
    }
}

libc_bitflags! {
    /// Configuration flags for [`msync`].
    pub struct MsFlags: c_int {
        /// Schedule an update but return immediately.
        MS_ASYNC;
        /// Invalidate all cached data.
        MS_INVALIDATE;
        /// Invalidate pages, but leave them mapped.
        #[cfg(apple_targets)]
        MS_KILLPAGES;
        /// Deactivate pages, but leave them mapped.
        #[cfg(apple_targets)]
        MS_DEACTIVATE;
        /// Perform an update and wait for it to complete.
        MS_SYNC;
    }
}

#[cfg(not(target_os = "haiku"))]
libc_bitflags! {
    /// Flags for [`mlockall`].
    pub struct MlockAllFlags: c_int {
        /// Lock pages that are currently mapped into the address space of the process.
        MCL_CURRENT;
        /// Lock pages which will become mapped into the address space of the process in the future.
        MCL_FUTURE;
    }
}

/// Locks all memory pages that contain part of the address range with `length`
/// bytes starting at `addr`.
///
/// Locked pages never move to the swap area.
///
/// # Safety
///
/// `addr` must meet all the requirements described in the [`mlock(2)`] man page.
///
/// [`mlock(2)`]: https://man7.org/linux/man-pages/man2/mlock.2.html
pub unsafe fn mlock(addr: NonNull<c_void>, length: size_t) -> Result<()> {
    unsafe { Errno::result(libc::mlock(addr.as_ptr(), length)).map(drop) }
}

/// Unlocks all memory pages that contain part of the address range with
/// `length` bytes starting at `addr`.
///
/// # Safety
///
/// `addr` must meet all the requirements described in the [`munlock(2)`] man
/// page.
///
/// [`munlock(2)`]: https://man7.org/linux/man-pages/man2/munlock.2.html
pub unsafe fn munlock(addr: NonNull<c_void>, length: size_t) -> Result<()> {
    unsafe { Errno::result(libc::munlock(addr.as_ptr(), length)).map(drop) }
}

/// Locks all memory pages mapped into this process' address space.
///
/// Locked pages never move to the swap area. For more information, see [`mlockall(2)`].
///
/// [`mlockall(2)`]: https://man7.org/linux/man-pages/man2/mlockall.2.html
#[cfg(not(target_os = "haiku"))]
pub fn mlockall(flags: MlockAllFlags) -> Result<()> {
    unsafe { Errno::result(libc::mlockall(flags.bits())) }.map(drop)
}

/// Unlocks all memory pages mapped into this process' address space.
///
/// For more information, see [`munlockall(2)`].
///
/// [`munlockall(2)`]: https://man7.org/linux/man-pages/man2/munlockall.2.html
#[cfg(not(target_os = "haiku"))]
pub fn munlockall() -> Result<()> {
    unsafe { Errno::result(libc::munlockall()) }.map(drop)
}

/// Allocate memory, or map files or devices into memory
///
/// For anonymous mappings (`MAP_ANON`/`MAP_ANONYMOUS`), see [mmap_anonymous].
///
/// # Safety
///
/// See the [`mmap(2)`] man page for detailed requirements.
///
/// [`mmap(2)`]: https://man7.org/linux/man-pages/man2/mmap.2.html
pub unsafe fn mmap<F: AsFd>(
    addr: Option<NonZeroUsize>,
    length: NonZeroUsize,
    prot: ProtFlags,
    flags: MapFlags,
    f: F,
    offset: off_t,
) -> Result<NonNull<c_void>> {
    let ptr = addr.map_or(std::ptr::null_mut(), |a| a.get() as *mut c_void);

    let fd = f.as_fd().as_raw_fd();
    let ret = unsafe {
        libc::mmap(ptr, length.into(), prot.bits(), flags.bits(), fd, offset)
    };

    if ret == libc::MAP_FAILED {
        Err(Errno::last())
    } else {
        // SAFETY: `libc::mmap` returns a valid non-null pointer or `libc::MAP_FAILED`, thus `ret`
        // will be non-null here.
        Ok(unsafe { NonNull::new_unchecked(ret) })
    }
}

/// Create an anonymous memory mapping.
///
/// This function is a wrapper around [`mmap`]:
/// `mmap(ptr, len, prot, MAP_ANONYMOUS | flags, -1, 0)`.
///
/// # Safety
///
/// See the [`mmap(2)`] man page for detailed requirements.
///
/// [`mmap(2)`]: https://man7.org/linux/man-pages/man2/mmap.2.html
pub unsafe fn mmap_anonymous(
    addr: Option<NonZeroUsize>,
    length: NonZeroUsize,
    prot: ProtFlags,
    flags: MapFlags,
) -> Result<NonNull<c_void>> {
    let ptr = addr.map_or(std::ptr::null_mut(), |a| a.get() as *mut c_void);

    let flags = MapFlags::MAP_ANONYMOUS | flags;
    let ret = unsafe {
        libc::mmap(ptr, length.into(), prot.bits(), flags.bits(), -1, 0)
    };

    if ret == libc::MAP_FAILED {
        Err(Errno::last())
    } else {
        // SAFETY: `libc::mmap` returns a valid non-null pointer or `libc::MAP_FAILED`, thus `ret`
        // will be non-null here.
        Ok(unsafe { NonNull::new_unchecked(ret) })
    }
}

/// Expands (or shrinks) an existing memory mapping, potentially moving it at
/// the same time.
///
/// # Safety
///
/// See the `mremap(2)` [man page](https://man7.org/linux/man-pages/man2/mremap.2.html) for
/// detailed requirements.
#[cfg(any(target_os = "linux", target_os = "netbsd"))]
pub unsafe fn mremap(
    addr: NonNull<c_void>,
    old_size: size_t,
    new_size: size_t,
    flags: MRemapFlags,
    new_address: Option<NonNull<c_void>>,
) -> Result<NonNull<c_void>> {
    #[cfg(target_os = "linux")]
    let ret = unsafe {
        libc::mremap(
            addr.as_ptr(),
            old_size,
            new_size,
            flags.bits(),
            new_address
                .map(NonNull::as_ptr)
                .unwrap_or(std::ptr::null_mut()),
        )
    };
    #[cfg(target_os = "netbsd")]
    let ret = unsafe {
        libc::mremap(
            addr.as_ptr(),
            old_size,
            new_address
                .map(NonNull::as_ptr)
                .unwrap_or(std::ptr::null_mut()),
            new_size,
            flags.bits(),
        )
    };

    if ret == libc::MAP_FAILED {
        Err(Errno::last())
    } else {
        // SAFETY: `libc::mremap` returns a valid non-null pointer or `libc::MAP_FAILED`, thus `ret`
        // will be non-null here.
        Ok(unsafe { NonNull::new_unchecked(ret) })
    }
}

/// remove a mapping
///
/// # Safety
///
/// `addr` must meet all the requirements described in the [`munmap(2)`] man
/// page.
///
/// [`munmap(2)`]: https://man7.org/linux/man-pages/man2/munmap.2.html
pub unsafe fn munmap(addr: NonNull<c_void>, len: size_t) -> Result<()> {
    unsafe { Errno::result(libc::munmap(addr.as_ptr(), len)).map(drop) }
}

/// give advice about use of memory
///
/// # Safety
///
/// See the [`madvise(2)`] man page.  Take special care when using
/// [`MmapAdvise::MADV_FREE`].
///
/// [`madvise(2)`]: https://man7.org/linux/man-pages/man2/madvise.2.html
#[allow(rustdoc::broken_intra_doc_links)] // For Hurd as `MADV_FREE` is not available on it
pub unsafe fn madvise(
    addr: NonNull<c_void>,
    length: size_t,
    advise: MmapAdvise,
) -> Result<()> {
    unsafe {
        Errno::result(libc::madvise(addr.as_ptr(), length, advise as i32))
            .map(drop)
    }
}

/// Set protection of memory mapping.
///
/// See [`mprotect(3)`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/mprotect.html) for
/// details.
///
/// # Safety
///
/// Calls to `mprotect` are inherently unsafe, as changes to memory protections can lead to
/// SIGSEGVs.
///
/// ```
/// # use nix::libc::size_t;
/// # use nix::sys::mman::{mmap_anonymous, mprotect, MapFlags, ProtFlags};
/// # use std::ptr;
/// # use std::os::unix::io::BorrowedFd;
/// const ONE_K: size_t = 1024;
/// let one_k_non_zero = std::num::NonZeroUsize::new(ONE_K).unwrap();
/// let mut slice: &mut [u8] = unsafe {
///     let mem = mmap_anonymous(None, one_k_non_zero, ProtFlags::PROT_NONE, MapFlags::MAP_PRIVATE)
///         .unwrap();
///     mprotect(mem, ONE_K, ProtFlags::PROT_READ | ProtFlags::PROT_WRITE).unwrap();
///     std::slice::from_raw_parts_mut(mem.as_ptr().cast(), ONE_K)
/// };
/// assert_eq!(slice[0], 0x00);
/// slice[0] = 0xFF;
/// assert_eq!(slice[0], 0xFF);
/// ```
pub unsafe fn mprotect(
    addr: NonNull<c_void>,
    length: size_t,
    prot: ProtFlags,
) -> Result<()> {
    unsafe {
        Errno::result(libc::mprotect(addr.as_ptr(), length, prot.bits()))
            .map(drop)
    }
}

/// synchronize a mapped region
///
/// # Safety
///
/// `addr` must meet all the requirements described in the [`msync(2)`] man
/// page.
///
/// [`msync(2)`]: https://man7.org/linux/man-pages/man2/msync.2.html
pub unsafe fn msync(
    addr: NonNull<c_void>,
    length: size_t,
    flags: MsFlags,
) -> Result<()> {
    unsafe {
        Errno::result(libc::msync(addr.as_ptr(), length, flags.bits()))
            .map(drop)
    }
}

#[cfg(not(target_os = "android"))]
feature! {
#![feature = "fs"]
/// Creates and opens a new, or opens an existing, POSIX shared memory object.
///
/// For more information, see [`shm_open(3)`].
///
/// [`shm_open(3)`]: https://man7.org/linux/man-pages/man3/shm_open.3.html
pub fn shm_open<P>(
    name: &P,
    flag: OFlag,
    mode: Mode
    ) -> Result<std::os::unix::io::OwnedFd>
    where P: ?Sized + NixPath
{
    use std::os::unix::io::{FromRawFd, OwnedFd};

    let ret = name.with_nix_path(|cstr| {
        #[cfg(apple_targets)]
        unsafe {
            libc::shm_open(cstr.as_ptr(), flag.bits(), mode.bits() as libc::c_uint)
        }
        #[cfg(not(apple_targets))]
        unsafe {
            libc::shm_open(cstr.as_ptr(), flag.bits(), mode.bits() as libc::mode_t)
        }
    })?;

    match ret {
        -1 => Err(Errno::last()),
        fd => Ok(unsafe{ OwnedFd::from_raw_fd(fd) })
    }
}
}

/// Performs the converse of [`shm_open`], removing an object previously created.
///
/// For more information, see [`shm_unlink(3)`].
///
/// [`shm_unlink(3)`]: https://man7.org/linux/man-pages/man3/shm_unlink.3.html
#[cfg(not(target_os = "android"))]
pub fn shm_unlink<P: ?Sized + NixPath>(name: &P) -> Result<()> {
    let ret =
        name.with_nix_path(|cstr| unsafe { libc::shm_unlink(cstr.as_ptr()) })?;

    Errno::result(ret).map(drop)
}
