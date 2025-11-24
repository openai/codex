use crate::backend::c;
use bitflags::bitflags;

bitflags! {
    /// `PROT_*` flags for use with [`mmap`].
    ///
    /// For `PROT_NONE`, use `ProtFlags::empty()`.
    ///
    /// [`mmap`]: crate::mm::mmap
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct ProtFlags: u32 {
        /// `PROT_READ`
        const READ = linux_raw_sys::general::PROT_READ;
        /// `PROT_WRITE`
        const WRITE = linux_raw_sys::general::PROT_WRITE;
        /// `PROT_EXEC`
        const EXEC = linux_raw_sys::general::PROT_EXEC;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags! {
    /// `PROT_*` flags for use with [`mprotect`].
    ///
    /// For `PROT_NONE`, use `MprotectFlags::empty()`.
    ///
    /// [`mprotect`]: crate::mm::mprotect
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct MprotectFlags: u32 {
        /// `PROT_READ`
        const READ = linux_raw_sys::general::PROT_READ;
        /// `PROT_WRITE`
        const WRITE = linux_raw_sys::general::PROT_WRITE;
        /// `PROT_EXEC`
        const EXEC = linux_raw_sys::general::PROT_EXEC;
        /// `PROT_GROWSUP`
        const GROWSUP = linux_raw_sys::general::PROT_GROWSUP;
        /// `PROT_GROWSDOWN`
        const GROWSDOWN = linux_raw_sys::general::PROT_GROWSDOWN;
        /// `PROT_SEM`
        const SEM = linux_raw_sys::general::PROT_SEM;
        /// `PROT_BTI`
        #[cfg(target_arch = "aarch64")]
        const BTI = linux_raw_sys::general::PROT_BTI;
        /// `PROT_MTE`
        #[cfg(target_arch = "aarch64")]
        const MTE = linux_raw_sys::general::PROT_MTE;
        /// `PROT_SAO`
        #[cfg(any(target_arch = "powerpc", target_arch = "powerpc64"))]
        const SAO = linux_raw_sys::general::PROT_SAO;
        /// `PROT_ADI`
        #[cfg(any(target_arch = "sparc", target_arch = "sparc64"))]
        const ADI = linux_raw_sys::general::PROT_ADI;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags! {
    /// `MAP_*` flags for use with [`mmap`].
    ///
    /// For `MAP_ANONYMOUS` (aka `MAP_ANON`), see [`mmap_anonymous`].
    ///
    /// [`mmap`]: crate::mm::mmap
    /// [`mmap_anonymous`]: crates::mm::mmap_anonymous
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct MapFlags: u32 {
        /// `MAP_SHARED`
        const SHARED = linux_raw_sys::general::MAP_SHARED;
        /// `MAP_SHARED_VALIDATE` (since Linux 4.15)
        const SHARED_VALIDATE = linux_raw_sys::general::MAP_SHARED_VALIDATE;
        /// `MAP_PRIVATE`
        const PRIVATE = linux_raw_sys::general::MAP_PRIVATE;
        /// `MAP_DENYWRITE`
        const DENYWRITE = linux_raw_sys::general::MAP_DENYWRITE;
        /// `MAP_FIXED`
        const FIXED = linux_raw_sys::general::MAP_FIXED;
        /// `MAP_FIXED_NOREPLACE` (since Linux 4.17)
        const FIXED_NOREPLACE = linux_raw_sys::general::MAP_FIXED_NOREPLACE;
        /// `MAP_GROWSDOWN`
        const GROWSDOWN = linux_raw_sys::general::MAP_GROWSDOWN;
        /// `MAP_HUGETLB`
        const HUGETLB = linux_raw_sys::general::MAP_HUGETLB;
        /// `MAP_HUGE_2MB` (since Linux 3.8)
        const HUGE_2MB = linux_raw_sys::general::MAP_HUGE_2MB;
        /// `MAP_HUGE_1GB` (since Linux 3.8)
        const HUGE_1GB = linux_raw_sys::general::MAP_HUGE_1GB;
        /// `MAP_LOCKED`
        const LOCKED = linux_raw_sys::general::MAP_LOCKED;
        /// `MAP_NORESERVE`
        const NORESERVE = linux_raw_sys::general::MAP_NORESERVE;
        /// `MAP_POPULATE`
        const POPULATE = linux_raw_sys::general::MAP_POPULATE;
        /// `MAP_STACK`
        const STACK = linux_raw_sys::general::MAP_STACK;
        /// `MAP_SYNC` (since Linux 4.15)
        #[cfg(not(any(target_arch = "mips", target_arch = "mips32r6", target_arch = "mips64", target_arch = "mips64r6")))]
        const SYNC = linux_raw_sys::general::MAP_SYNC;
        /// `MAP_UNINITIALIZED`
        #[cfg(not(any(target_arch = "mips", target_arch = "mips32r6", target_arch = "mips64", target_arch = "mips64r6")))]
        const UNINITIALIZED = linux_raw_sys::general::MAP_UNINITIALIZED;
        /// `MAP_DROPPABLE`
        const DROPPABLE = c::MAP_DROPPABLE;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags! {
    /// `MREMAP_*` flags for use with [`mremap`].
    ///
    /// For `MREMAP_FIXED`, see [`mremap_fixed`].
    ///
    /// [`mremap`]: crate::mm::mremap
    /// [`mremap_fixed`]: crate::mm::mremap_fixed
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct MremapFlags: u32 {
        /// `MREMAP_MAYMOVE`
        const MAYMOVE = linux_raw_sys::general::MREMAP_MAYMOVE;
        /// `MREMAP_DONTUNMAP` (since Linux 5.7)
        const DONTUNMAP = linux_raw_sys::general::MREMAP_DONTUNMAP;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags! {
    /// `MS_*` flags for use with [`msync`].
    ///
    /// [`msync`]: crate::mm::msync
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct MsyncFlags: u32 {
        /// `MS_SYNC`—Requests an update and waits for it to complete.
        const SYNC = linux_raw_sys::general::MS_SYNC;
        /// `MS_ASYNC`—Specifies that an update be scheduled, but the call
        /// returns immediately.
        const ASYNC = linux_raw_sys::general::MS_ASYNC;
        /// `MS_INVALIDATE`—Asks to invalidate other mappings of the same
        /// file (so that they can be updated with the fresh values just
        /// written).
        const INVALIDATE = linux_raw_sys::general::MS_INVALIDATE;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags! {
    /// `MLOCK_*` flags for use with [`mlock_with`].
    ///
    /// [`mlock_with`]: crate::mm::mlock_with
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct MlockFlags: u32 {
        /// `MLOCK_ONFAULT`
        const ONFAULT = linux_raw_sys::general::MLOCK_ONFAULT;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

/// `POSIX_MADV_*` constants for use with [`madvise`].
///
/// [`madvise`]: crate::mm::madvise
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
#[non_exhaustive]
pub enum Advice {
    /// `POSIX_MADV_NORMAL`
    Normal = linux_raw_sys::general::MADV_NORMAL,

    /// `POSIX_MADV_SEQUENTIAL`
    Sequential = linux_raw_sys::general::MADV_SEQUENTIAL,

    /// `POSIX_MADV_RANDOM`
    Random = linux_raw_sys::general::MADV_RANDOM,

    /// `POSIX_MADV_WILLNEED`
    WillNeed = linux_raw_sys::general::MADV_WILLNEED,

    /// `MADV_DONTNEED`
    LinuxDontNeed = linux_raw_sys::general::MADV_DONTNEED,

    /// `MADV_FREE` (since Linux 4.5)
    LinuxFree = linux_raw_sys::general::MADV_FREE,
    /// `MADV_REMOVE`
    LinuxRemove = linux_raw_sys::general::MADV_REMOVE,
    /// `MADV_DONTFORK`
    LinuxDontFork = linux_raw_sys::general::MADV_DONTFORK,
    /// `MADV_DOFORK`
    LinuxDoFork = linux_raw_sys::general::MADV_DOFORK,
    /// `MADV_HWPOISON`
    LinuxHwPoison = linux_raw_sys::general::MADV_HWPOISON,
    /// `MADV_SOFT_OFFLINE`
    #[cfg(not(any(
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6"
    )))]
    LinuxSoftOffline = linux_raw_sys::general::MADV_SOFT_OFFLINE,
    /// `MADV_MERGEABLE`
    LinuxMergeable = linux_raw_sys::general::MADV_MERGEABLE,
    /// `MADV_UNMERGEABLE`
    LinuxUnmergeable = linux_raw_sys::general::MADV_UNMERGEABLE,
    /// `MADV_HUGEPAGE`
    LinuxHugepage = linux_raw_sys::general::MADV_HUGEPAGE,
    /// `MADV_NOHUGEPAGE`
    LinuxNoHugepage = linux_raw_sys::general::MADV_NOHUGEPAGE,
    /// `MADV_DONTDUMP` (since Linux 3.4)
    LinuxDontDump = linux_raw_sys::general::MADV_DONTDUMP,
    /// `MADV_DODUMP` (since Linux 3.4)
    LinuxDoDump = linux_raw_sys::general::MADV_DODUMP,
    /// `MADV_WIPEONFORK` (since Linux 4.14)
    LinuxWipeOnFork = linux_raw_sys::general::MADV_WIPEONFORK,
    /// `MADV_KEEPONFORK` (since Linux 4.14)
    LinuxKeepOnFork = linux_raw_sys::general::MADV_KEEPONFORK,
    /// `MADV_COLD` (since Linux 5.4)
    LinuxCold = linux_raw_sys::general::MADV_COLD,
    /// `MADV_PAGEOUT` (since Linux 5.4)
    LinuxPageOut = linux_raw_sys::general::MADV_PAGEOUT,
    /// `MADV_POPULATE_READ` (since Linux 5.14)
    LinuxPopulateRead = linux_raw_sys::general::MADV_POPULATE_READ,
    /// `MADV_POPULATE_WRITE` (since Linux 5.14)
    LinuxPopulateWrite = linux_raw_sys::general::MADV_POPULATE_WRITE,
    /// `MADV_DONTNEED_LOCKED` (since Linux 5.18)
    LinuxDontneedLocked = linux_raw_sys::general::MADV_DONTNEED_LOCKED,
}

#[allow(non_upper_case_globals)]
impl Advice {
    /// `POSIX_MADV_DONTNEED`
    ///
    /// On Linux, this is mapped to `POSIX_MADV_NORMAL` because Linux's
    /// `MADV_DONTNEED` differs from `POSIX_MADV_DONTNEED`. See `LinuxDontNeed`
    /// for the Linux behavior.
    pub const DontNeed: Self = Self::Normal;
}

bitflags! {
    /// `O_*` flags for use with [`userfaultfd`].
    ///
    /// [`userfaultfd`]: crate::mm::userfaultfd
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct UserfaultfdFlags: c::c_uint {
        /// `O_CLOEXEC`
        const CLOEXEC = linux_raw_sys::general::O_CLOEXEC;
        /// `O_NONBLOCK`
        const NONBLOCK = linux_raw_sys::general::O_NONBLOCK;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags! {
    /// `MCL_*` flags for use with [`mlockall`].
    ///
    /// [`mlockall`]: crate::mm::mlockall
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct MlockAllFlags: u32 {
        /// Used together with `MCL_CURRENT`, `MCL_FUTURE`, or both. Mark all
        /// current (with `MCL_CURRENT`) or future (with `MCL_FUTURE`) mappings
        /// to lock pages when they are faulted in. When used with
        /// `MCL_CURRENT`, all present pages are locked, but `mlockall` will
        /// not fault in non-present pages. When used with `MCL_FUTURE`, all
        /// future mappings will be marked to lock pages when they are faulted
        /// in, but they will not be populated by the lock when the mapping is
        /// created. `MCL_ONFAULT` must be used with either `MCL_CURRENT` or
        /// `MCL_FUTURE` or both.
        const ONFAULT = linux_raw_sys::general::MCL_ONFAULT;
        /// Lock all pages which will become mapped into the address space of
        /// the process in the future. These could be, for instance, new pages
        /// required by a growing heap and stack as well as new memory-mapped
        /// files or shared memory regions.
        const FUTURE = linux_raw_sys::general::MCL_FUTURE;
        /// Lock all pages which are currently mapped into the address space of
        /// the process.
        const CURRENT = linux_raw_sys::general::MCL_CURRENT;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}
