//! Libc and supplemental types and constants.

#![allow(unused_imports)]

// Import everything from libc, but we'll add some stuff and override some
// things below.
pub(crate) use libc::*;

/// `PROC_SUPER_MAGIC`—The magic number for the procfs filesystem.
#[cfg(all(linux_kernel, target_env = "musl"))]
pub(crate) const PROC_SUPER_MAGIC: u32 = 0x0000_9fa0;

/// `NFS_SUPER_MAGIC`—The magic number for the NFS filesystem.
#[cfg(all(linux_kernel, target_env = "musl"))]
pub(crate) const NFS_SUPER_MAGIC: u32 = 0x0000_6969;

#[cfg(feature = "process")]
#[cfg(not(any(target_os = "espidf", target_os = "wasi")))]
pub(crate) const EXIT_SIGNALED_SIGABRT: c_int = 128 + SIGABRT as c_int;

// TODO: Upstream these.
#[cfg(all(linux_kernel, feature = "net"))]
pub(crate) const ETH_P_TSN: c_int = linux_raw_sys::if_ether::ETH_P_TSN as _;
#[cfg(all(linux_kernel, feature = "net"))]
pub(crate) const ETH_P_ERSPAN2: c_int = linux_raw_sys::if_ether::ETH_P_ERSPAN2 as _;
#[cfg(all(linux_kernel, feature = "net"))]
pub(crate) const ETH_P_ERSPAN: c_int = linux_raw_sys::if_ether::ETH_P_ERSPAN as _;
#[cfg(all(linux_kernel, feature = "net"))]
pub(crate) const ETH_P_PROFINET: c_int = linux_raw_sys::if_ether::ETH_P_PROFINET as _;
#[cfg(all(linux_kernel, feature = "net"))]
pub(crate) const ETH_P_REALTEK: c_int = linux_raw_sys::if_ether::ETH_P_REALTEK as _;
#[cfg(all(linux_kernel, feature = "net"))]
pub(crate) const ETH_P_ETHERCAT: c_int = linux_raw_sys::if_ether::ETH_P_ETHERCAT as _;
#[cfg(all(linux_kernel, feature = "net"))]
pub(crate) const ETH_P_PREAUTH: c_int = linux_raw_sys::if_ether::ETH_P_PREAUTH as _;
#[cfg(all(linux_kernel, feature = "net"))]
pub(crate) const ETH_P_LLDP: c_int = linux_raw_sys::if_ether::ETH_P_LLDP as _;
#[cfg(all(linux_kernel, feature = "net"))]
pub(crate) const ETH_P_MRP: c_int = linux_raw_sys::if_ether::ETH_P_MRP as _;
#[cfg(all(linux_kernel, feature = "net"))]
pub(crate) const ETH_P_NCSI: c_int = linux_raw_sys::if_ether::ETH_P_NCSI as _;
#[cfg(all(linux_kernel, feature = "net"))]
pub(crate) const ETH_P_CFM: c_int = linux_raw_sys::if_ether::ETH_P_CFM as _;
#[cfg(all(linux_kernel, feature = "net"))]
pub(crate) const ETH_P_IBOE: c_int = linux_raw_sys::if_ether::ETH_P_IBOE as _;
#[cfg(all(linux_kernel, feature = "net"))]
pub(crate) const ETH_P_HSR: c_int = linux_raw_sys::if_ether::ETH_P_HSR as _;
#[cfg(all(linux_kernel, feature = "net"))]
pub(crate) const ETH_P_NSH: c_int = linux_raw_sys::if_ether::ETH_P_NSH as _;
#[cfg(all(linux_kernel, feature = "net"))]
pub(crate) const ETH_P_DSA_8021Q: c_int = linux_raw_sys::if_ether::ETH_P_DSA_8021Q as _;
#[cfg(all(linux_kernel, feature = "net"))]
pub(crate) const ETH_P_DSA_A5PSW: c_int = linux_raw_sys::if_ether::ETH_P_DSA_A5PSW as _;
#[cfg(all(linux_kernel, feature = "net"))]
pub(crate) const ETH_P_IFE: c_int = linux_raw_sys::if_ether::ETH_P_IFE as _;
#[cfg(all(linux_kernel, feature = "net"))]
pub(crate) const ETH_P_CAN: c_int = linux_raw_sys::if_ether::ETH_P_CAN as _;
#[cfg(all(linux_kernel, feature = "net"))]
pub(crate) const ETH_P_CANXL: c_int = linux_raw_sys::if_ether::ETH_P_CANXL as _;
#[cfg(all(linux_kernel, feature = "net"))]
pub(crate) const ETH_P_XDSA: c_int = linux_raw_sys::if_ether::ETH_P_XDSA as _;
#[cfg(all(linux_kernel, feature = "net"))]
pub(crate) const ETH_P_MAP: c_int = linux_raw_sys::if_ether::ETH_P_MAP as _;
#[cfg(all(linux_kernel, feature = "net"))]
pub(crate) const ETH_P_MCTP: c_int = linux_raw_sys::if_ether::ETH_P_MCTP as _;

#[cfg(all(
    linux_kernel,
    any(
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6",
        target_arch = "sparc",
        target_arch = "sparc64"
    )
))]
pub(crate) const SIGEMT: c_int = linux_raw_sys::general::SIGEMT as _;

// TODO: Upstream these.
#[cfg(all(linux_kernel, feature = "termios"))]
pub(crate) const IUCLC: tcflag_t = linux_raw_sys::general::IUCLC as _;
#[cfg(all(linux_kernel, feature = "termios"))]
pub(crate) const XCASE: tcflag_t = linux_raw_sys::general::XCASE as _;

#[cfg(target_os = "aix")]
pub(crate) const MSG_DONTWAIT: c_int = MSG_NONBLOCK;

// `O_LARGEFILE` can be automatically set by the kernel on Linux:
// <https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/fs/open.c?h=v6.13#n1423>
// so libc implementations may leave it undefined or defined to zero.
#[cfg(linux_kernel)]
pub(crate) const O_LARGEFILE: c_int = linux_raw_sys::general::O_LARGEFILE as _;

// Gated under `_LARGEFILE_SOURCE` but automatically set by the kernel.
// <https://github.com/illumos/illumos-gate/blob/fb2cb638e5604b214d8ea8d4f01ad2e77b437c17/usr/src/ucbhead/sys/fcntl.h#L64>
#[cfg(solarish)]
pub(crate) const O_LARGEFILE: c_int = 0x2000;

// On PowerPC, the regular `termios` has the `termios2` fields and there is no
// `termios2`, so we define aliases.
#[cfg(all(
    linux_kernel,
    feature = "termios",
    any(target_arch = "powerpc", target_arch = "powerpc64")
))]
pub(crate) use {
    termios as termios2, TCGETS as TCGETS2, TCSETS as TCSETS2, TCSETSF as TCSETSF2,
    TCSETSW as TCSETSW2,
};

// And PowerPC doesn't define `CIBAUD`, but it does define `IBSHIFT`, so we can
// compute `CIBAUD` ourselves.
#[cfg(all(
    linux_kernel,
    feature = "termios",
    any(target_arch = "powerpc", target_arch = "powerpc64")
))]
pub(crate) const CIBAUD: u32 = CBAUD << IBSHIFT;

// Automatically enable “large file” support (LFS) features.

#[cfg(target_os = "vxworks")]
pub(super) use _Vx_ticks64_t as _Vx_ticks_t;
#[cfg(linux_kernel)]
pub(super) use fallocate64 as fallocate;
#[cfg(not(any(target_arch = "aarch64", target_arch = "riscv64")))]
#[cfg(any(linux_like, target_os = "aix"))]
pub(super) use open64 as open;
#[cfg(any(
    linux_kernel,
    target_os = "aix",
    target_os = "hurd",
    target_os = "l4re"
))]
pub(super) use posix_fallocate64 as posix_fallocate;
#[cfg(any(all(linux_like, not(target_os = "android")), target_os = "aix"))]
pub(super) use {blkcnt64_t as blkcnt_t, rlim64_t as rlim_t};
// TODO: AIX has `stat64x`, `fstat64x`, `lstat64x`, and `stat64xat`; add them
// to the upstream libc crate and implement rustix's `statat` etc. with them.
#[cfg(target_os = "aix")]
pub(super) use {
    blksize64_t as blksize_t, fstat64 as fstat, fstatfs64 as fstatfs, fstatvfs64 as fstatvfs,
    ftruncate64 as ftruncate, getrlimit64 as getrlimit, ino_t, lseek64 as lseek, mmap,
    off64_t as off_t, openat, posix_fadvise64 as posix_fadvise, preadv, pwritev,
    rlimit64 as rlimit, setrlimit64 as setrlimit, stat64at as fstatat, statfs64 as statfs,
    statvfs64 as statvfs, RLIM_INFINITY,
};
#[cfg(any(linux_like, target_os = "hurd"))]
pub(super) use {
    fstat64 as fstat, fstatat64 as fstatat, fstatfs64 as fstatfs, fstatvfs64 as fstatvfs,
    ftruncate64 as ftruncate, getrlimit64 as getrlimit, ino64_t as ino_t, lseek64 as lseek,
    mmap64 as mmap, off64_t as off_t, openat64 as openat, posix_fadvise64 as posix_fadvise,
    rlimit64 as rlimit, setrlimit64 as setrlimit, statfs64 as statfs, statvfs64 as statvfs,
    RLIM64_INFINITY as RLIM_INFINITY,
};
#[cfg(apple)]
pub(super) use {
    host_info64_t as host_info_t, host_statistics64 as host_statistics,
    vm_statistics64_t as vm_statistics_t,
};
#[cfg(not(all(
    linux_kernel,
    any(
        target_pointer_width = "32",
        target_arch = "mips64",
        target_arch = "mips64r6"
    )
)))]
#[cfg(any(linux_like, target_os = "aix", target_os = "hurd"))]
pub(super) use {lstat64 as lstat, stat64 as stat};
#[cfg(any(
    linux_kernel,
    target_os = "aix",
    target_os = "hurd",
    target_os = "emscripten"
))]
pub(super) use {pread64 as pread, pwrite64 as pwrite};
#[cfg(any(target_os = "linux", target_os = "hurd", target_os = "emscripten"))]
pub(super) use {preadv64 as preadv, pwritev64 as pwritev};

#[cfg(all(target_os = "linux", any(target_env = "gnu", target_env = "uclibc")))]
pub(super) unsafe fn prlimit(
    pid: pid_t,
    resource: __rlimit_resource_t,
    new_limit: *const rlimit64,
    old_limit: *mut rlimit64,
) -> c_int {
    // `prlimit64` wasn't supported in glibc until 2.13.
    weak_or_syscall! {
        fn prlimit64(
            pid: pid_t,
            resource: __rlimit_resource_t,
            new_limit: *const rlimit64,
            old_limit: *mut rlimit64
        ) via SYS_prlimit64 -> c_int
    }

    prlimit64(pid, resource, new_limit, old_limit)
}

#[cfg(all(target_os = "linux", target_env = "musl"))]
pub(super) unsafe fn prlimit(
    pid: pid_t,
    resource: c_int,
    new_limit: *const rlimit64,
    old_limit: *mut rlimit64,
) -> c_int {
    weak_or_syscall! {
        fn prlimit64(
            pid: pid_t,
            resource: c_int,
            new_limit: *const rlimit64,
            old_limit: *mut rlimit64
        ) via SYS_prlimit64 -> c_int
    }

    prlimit64(pid, resource, new_limit, old_limit)
}

#[cfg(target_os = "android")]
pub(super) unsafe fn prlimit(
    pid: pid_t,
    resource: c_int,
    new_limit: *const rlimit64,
    old_limit: *mut rlimit64,
) -> c_int {
    weak_or_syscall! {
        fn prlimit64(
            pid: pid_t,
            resource: c_int,
            new_limit: *const rlimit64,
            old_limit: *mut rlimit64
        ) via SYS_prlimit64 -> c_int
    }

    prlimit64(pid, resource, new_limit, old_limit)
}

#[cfg(target_os = "android")]
mod readwrite_pv64 {
    use super::*;

    pub(in super::super) unsafe fn preadv64(
        fd: c_int,
        iov: *const iovec,
        iovcnt: c_int,
        offset: off64_t,
    ) -> ssize_t {
        // Older Android libc lacks `preadv64`, so use the `weak!` mechanism to
        // test for it, and call back to `syscall`. We don't use
        // `weak_or_syscall` here because we need to pass the 64-bit offset
        // specially.
        weak! {
            fn preadv64(c_int, *const iovec, c_int, off64_t) -> ssize_t
        }
        if let Some(fun) = preadv64.get() {
            fun(fd, iov, iovcnt, offset)
        } else {
            // Unlike the plain "p" functions, the "pv" functions pass their
            // offset in an endian-independent way, and always in two
            // registers.
            syscall! {
                fn preadv(
                    fd: c_int,
                    iov: *const iovec,
                    iovcnt: c_int,
                    offset_lo: usize,
                    offset_hi: usize
                ) via SYS_preadv -> ssize_t
            }
            preadv(fd, iov, iovcnt, offset as usize, (offset >> 32) as usize)
        }
    }
    pub(in super::super) unsafe fn pwritev64(
        fd: c_int,
        iov: *const iovec,
        iovcnt: c_int,
        offset: off64_t,
    ) -> ssize_t {
        // See the comments in `preadv64`.
        weak! {
            fn pwritev64(c_int, *const iovec, c_int, off64_t) -> ssize_t
        }
        if let Some(fun) = pwritev64.get() {
            fun(fd, iov, iovcnt, offset)
        } else {
            // Unlike the plain "p" functions, the "pv" functions pass their
            // offset in an endian-independent way, and always in two
            // registers.
            syscall! {
                fn pwritev(
                    fd: c_int,
                    iov: *const iovec,
                    iovcnt: c_int,
                    offset_lo: usize,
                    offset_hi: usize
                ) via SYS_pwritev -> ssize_t
            }
            pwritev(fd, iov, iovcnt, offset as usize, (offset >> 32) as usize)
        }
    }
}
#[cfg(target_os = "android")]
pub(super) use readwrite_pv64::{preadv64 as preadv, pwritev64 as pwritev};

// macOS added `preadv` and `pwritev` in version 11.0.
#[cfg(apple)]
mod readwrite_pv {
    use super::*;
    weakcall! {
        pub(in super::super) fn preadv(
            fd: c_int,
            iov: *const iovec,
            iovcnt: c_int,
            offset: off_t
        ) -> ssize_t
    }
    weakcall! {
        pub(in super::super) fn pwritev(
            fd: c_int,
            iov: *const iovec,
            iovcnt: c_int, offset: off_t
        ) -> ssize_t
    }
}
#[cfg(apple)]
pub(super) use readwrite_pv::{preadv, pwritev};

// glibc added `preadv64v2` and `pwritev64v2` in version 2.26.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod readwrite_pv64v2 {
    use super::*;

    pub(in super::super) unsafe fn preadv64v2(
        fd: c_int,
        iov: *const iovec,
        iovcnt: c_int,
        offset: off64_t,
        flags: c_int,
    ) -> ssize_t {
        // Older glibc lacks `preadv64v2`, so use the `weak!` mechanism to
        // test for it, and call back to `syscall`. We don't use
        // `weak_or_syscall` here because we need to pass the 64-bit offset
        // specially.
        weak! {
            fn preadv64v2(c_int, *const iovec, c_int, off64_t, c_int) -> ssize_t
        }
        if let Some(fun) = preadv64v2.get() {
            fun(fd, iov, iovcnt, offset, flags)
        } else {
            // Unlike the plain "p" functions, the "pv" functions pass their
            // offset in an endian-independent way, and always in two
            // registers.
            syscall! {
                fn preadv2(
                    fd: c_int,
                    iov: *const iovec,
                    iovcnt: c_int,
                    offset_lo: usize,
                    offset_hi: usize,
                    flags: c_int
                ) via SYS_preadv2 -> ssize_t
            }
            preadv2(
                fd,
                iov,
                iovcnt,
                offset as usize,
                (offset >> 32) as usize,
                flags,
            )
        }
    }
    pub(in super::super) unsafe fn pwritev64v2(
        fd: c_int,
        iov: *const iovec,
        iovcnt: c_int,
        offset: off64_t,
        flags: c_int,
    ) -> ssize_t {
        // See the comments in `preadv64v2`.
        weak! {
            fn pwritev64v2(c_int, *const iovec, c_int, off64_t, c_int) -> ssize_t
        }
        if let Some(fun) = pwritev64v2.get() {
            fun(fd, iov, iovcnt, offset, flags)
        } else {
            // Unlike the plain "p" functions, the "pv" functions pass their
            // offset in an endian-independent way, and always in two
            // registers.
            syscall! {
                fn pwritev2(
                    fd: c_int,
                    iov: *const iovec,
                    iovec: c_int,
                    offset_lo: usize,
                    offset_hi: usize,
                    flags: c_int
                ) via SYS_pwritev2 -> ssize_t
            }
            pwritev2(
                fd,
                iov,
                iovcnt,
                offset as usize,
                (offset >> 32) as usize,
                flags,
            )
        }
    }
}
#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub(super) use readwrite_pv64v2::{preadv64v2 as preadv2, pwritev64v2 as pwritev2};

// On non-glibc, assume we don't have `pwritev2`/`preadv2` in libc and use
// `c::syscall` instead.
#[cfg(any(
    target_os = "android",
    all(target_os = "linux", not(target_env = "gnu")),
))]
mod readwrite_pv64v2 {
    use super::*;

    pub(in super::super) unsafe fn preadv64v2(
        fd: c_int,
        iov: *const iovec,
        iovcnt: c_int,
        offset: off64_t,
        flags: c_int,
    ) -> ssize_t {
        // Unlike the plain "p" functions, the "pv" functions pass their offset
        // in an endian-independent way, and always in two registers.
        syscall! {
            fn preadv2(
                fd: c_int,
                iov: *const iovec,
                iovcnt: c_int,
                offset_lo: usize,
                offset_hi: usize,
                flags: c_int
            ) via SYS_preadv2 -> ssize_t
        }
        preadv2(
            fd,
            iov,
            iovcnt,
            offset as usize,
            (offset >> 32) as usize,
            flags,
        )
    }
    pub(in super::super) unsafe fn pwritev64v2(
        fd: c_int,
        iov: *const iovec,
        iovcnt: c_int,
        offset: off64_t,
        flags: c_int,
    ) -> ssize_t {
        // Unlike the plain "p" functions, the "pv" functions pass their offset
        // in an endian-independent way, and always in two registers.
        syscall! {
            fn pwritev2(
                fd: c_int,
                iov: *const iovec,
                iovcnt: c_int,
                offset_lo: usize,
                offset_hi: usize,
                flags: c_int
            ) via SYS_pwritev2 -> ssize_t
        }
        pwritev2(
            fd,
            iov,
            iovcnt,
            offset as usize,
            (offset >> 32) as usize,
            flags,
        )
    }
}
#[cfg(any(
    target_os = "android",
    all(target_os = "linux", not(target_env = "gnu")),
))]
pub(super) use readwrite_pv64v2::{preadv64v2 as preadv2, pwritev64v2 as pwritev2};

// Rust's libc crate lacks statx for Non-glibc targets.
#[cfg(feature = "fs")]
#[cfg(all(
    linux_like,
    not(any(target_os = "android", target_os = "emscripten", target_env = "gnu"))
))]
mod statx_flags {
    pub(crate) use linux_raw_sys::general::{
        STATX_ALL, STATX_ATIME, STATX_BASIC_STATS, STATX_BLOCKS, STATX_BTIME, STATX_CTIME,
        STATX_DIOALIGN, STATX_GID, STATX_INO, STATX_MNT_ID, STATX_MODE, STATX_MTIME, STATX_NLINK,
        STATX_SIZE, STATX_TYPE, STATX_UID,
    };

    pub(crate) use linux_raw_sys::general::{
        STATX_ATTR_APPEND, STATX_ATTR_AUTOMOUNT, STATX_ATTR_COMPRESSED, STATX_ATTR_DAX,
        STATX_ATTR_ENCRYPTED, STATX_ATTR_IMMUTABLE, STATX_ATTR_MOUNT_ROOT, STATX_ATTR_NODUMP,
        STATX_ATTR_VERITY,
    };
}
#[cfg(feature = "fs")]
#[cfg(all(
    linux_like,
    not(any(target_os = "android", target_os = "emscripten", target_env = "gnu"))
))]
pub(crate) use statx_flags::*;

#[cfg(feature = "fs")]
#[cfg(target_os = "android")]
pub(crate) use __fsid_t as fsid_t;

#[cfg(feature = "mm")]
#[cfg(target_os = "android")]
pub(crate) const MAP_DROPPABLE: c_int = bitcast!(linux_raw_sys::general::MAP_DROPPABLE);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(linux_kernel)]
    fn test_flags() {
        // libc may publicly define `O_LARGEFILE` to 0, but we want the real
        // non-zero value.
        assert_ne!(O_LARGEFILE, 0);
    }
}
