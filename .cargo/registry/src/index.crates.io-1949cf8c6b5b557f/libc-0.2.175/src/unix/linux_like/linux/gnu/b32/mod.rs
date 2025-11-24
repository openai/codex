//! 32-bit specific definitions for linux-like values

use crate::prelude::*;
use crate::pthread_mutex_t;

pub type clock_t = i32;

pub type shmatt_t = c_ulong;
pub type msgqnum_t = c_ulong;
pub type msglen_t = c_ulong;
pub type nlink_t = u32;
pub type __u64 = c_ulonglong;
pub type __s64 = c_longlong;
pub type __fsword_t = i32;
pub type fsblkcnt64_t = u64;
pub type fsfilcnt64_t = u64;
pub type __syscall_ulong_t = c_ulong;
pub type __suseconds64_t = i64;

cfg_if! {
    if #[cfg(target_arch = "riscv32")] {
        pub type time_t = i64;
        pub type suseconds_t = i64;
        type __ino_t = c_ulong;
        type __ino64_t = u64;
        pub type ino_t = __ino64_t;
        pub type off_t = i64;
        pub type blkcnt_t = i64;
        pub type fsblkcnt_t = u64;
        pub type fsfilcnt_t = u64;
        pub type rlim_t = u64;
        pub type blksize_t = i64;
    } else if #[cfg(gnu_time_bits64)] {
        pub type time_t = i64;
        pub type suseconds_t = i32;
        type __ino_t = c_ulong;
        type __ino64_t = u64;
        pub type ino_t = __ino64_t;
        pub type off_t = i64;
        pub type blkcnt_t = i64;
        pub type fsblkcnt_t = u64;
        pub type fsfilcnt_t = u64;
        pub type rlim_t = u64;
        pub type blksize_t = i32;
    } else if #[cfg(gnu_file_offset_bits64)] {
        pub type time_t = i32;
        pub type suseconds_t = i32;
        type __ino_t = c_ulong;
        type __ino64_t = u64;
        pub type ino_t = __ino64_t;
        pub type off_t = i64;
        pub type blkcnt_t = i64;
        pub type fsblkcnt_t = u64;
        pub type fsfilcnt_t = u64;
        pub type rlim_t = u64;
        pub type blksize_t = i32;
    } else {
        pub type time_t = i32;
        pub type suseconds_t = i32;
        type __ino_t = c_ulong;
        type __ino64_t = u64;
        pub type ino_t = __ino_t;
        pub type off_t = i32;
        pub type blkcnt_t = i32;
        pub type fsblkcnt_t = c_ulong;
        pub type fsfilcnt_t = c_ulong;
        pub type rlim_t = c_ulong;
        pub type blksize_t = i32;
    }
}

cfg_if! {
    if #[cfg(not(any(
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "powerpc",
        target_arch = "sparc"
    )))] {
        s! {
            pub struct stat {
                pub st_dev: crate::dev_t,

                #[cfg(not(gnu_time_bits64))]
                __pad1: c_uint,

                #[cfg(any(gnu_time_bits64, not(gnu_file_offset_bits64)))]
                pub st_ino: crate::ino_t,
                #[cfg(all(not(gnu_time_bits64), gnu_file_offset_bits64))]
                __st_ino: __ino_t,

                pub st_mode: crate::mode_t,
                pub st_nlink: crate::nlink_t,
                pub st_uid: crate::uid_t,
                pub st_gid: crate::gid_t,

                pub st_rdev: crate::dev_t,

                #[cfg(not(gnu_time_bits64))]
                __pad2: c_uint,

                pub st_size: off_t,

                pub st_blksize: crate::blksize_t,
                pub st_blocks: crate::blkcnt_t,

                pub st_atime: crate::time_t,
                pub st_atime_nsec: c_long,
                #[cfg(gnu_time_bits64)]
                _atime_pad: c_int,
                pub st_mtime: crate::time_t,
                pub st_mtime_nsec: c_long,
                #[cfg(gnu_time_bits64)]
                _mtime_pad: c_int,
                pub st_ctime: crate::time_t,
                pub st_ctime_nsec: c_long,
                #[cfg(gnu_time_bits64)]
                _ctime_pad: c_int,

                #[cfg(not(gnu_file_offset_bits64))]
                __glibc_reserved4: c_long,
                #[cfg(not(gnu_file_offset_bits64))]
                __glibc_reserved5: c_long,
                #[cfg(all(not(gnu_time_bits64), gnu_file_offset_bits64))]
                pub st_ino: crate::ino_t,
            }
        }
    }
}

s! {
    pub struct statvfs {
        pub f_bsize: c_ulong,
        pub f_frsize: c_ulong,
        pub f_blocks: crate::fsblkcnt_t,
        pub f_bfree: crate::fsblkcnt_t,
        pub f_bavail: crate::fsblkcnt_t,
        pub f_files: crate::fsfilcnt_t,
        pub f_ffree: crate::fsfilcnt_t,
        pub f_favail: crate::fsfilcnt_t,
        pub f_fsid: c_ulong,
        __f_unused: c_int,
        pub f_flag: c_ulong,
        pub f_namemax: c_ulong,
        __f_spare: [c_int; 6],
    }

    pub struct pthread_attr_t {
        __size: [u32; 9],
    }

    pub struct sigset_t {
        __val: [c_ulong; 32],
    }

    pub struct sysinfo {
        pub uptime: c_long,
        pub loads: [c_ulong; 3],
        pub totalram: c_ulong,
        pub freeram: c_ulong,
        pub sharedram: c_ulong,
        pub bufferram: c_ulong,
        pub totalswap: c_ulong,
        pub freeswap: c_ulong,
        pub procs: c_ushort,
        #[deprecated(
            since = "0.2.58",
            note = "This padding field might become private in the future"
        )]
        pub pad: c_ushort,
        pub totalhigh: c_ulong,
        pub freehigh: c_ulong,
        pub mem_unit: c_uint,
        pub _f: [c_char; 8],
    }

    pub struct semid_ds {
        pub sem_perm: ipc_perm,
        #[cfg(all(not(gnu_time_bits64), target_arch = "powerpc"))]
        __reserved: crate::__syscall_ulong_t,
        pub sem_otime: crate::time_t,
        #[cfg(not(any(
            gnu_time_bits64,
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "powerpc"
        )))]
        __reserved: crate::__syscall_ulong_t,
        #[cfg(all(not(gnu_time_bits64), target_arch = "powerpc"))]
        __reserved2: crate::__syscall_ulong_t,
        pub sem_ctime: crate::time_t,
        #[cfg(not(any(
            gnu_time_bits64,
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "powerpc"
        )))]
        __reserved2: crate::__syscall_ulong_t,
        pub sem_nsems: crate::__syscall_ulong_t,
        #[cfg(all(
            gnu_time_bits64,
            not(any(
                target_arch = "mips",
                target_arch = "mips32r6",
                target_arch = "powerpc",
                target_arch = "arm",
                target_arch = "x86"
            ))
        ))]
        __reserved2: crate::__syscall_ulong_t,
        __glibc_reserved3: crate::__syscall_ulong_t,
        __glibc_reserved4: crate::__syscall_ulong_t,
    }

    #[cfg(gnu_time_bits64)]
    pub struct timex {
        pub modes: c_uint,
        _pad1: c_int,
        pub offset: c_longlong,
        pub freq: c_longlong,
        pub maxerror: c_longlong,
        pub esterror: c_longlong,
        pub status: c_int,
        _pad2: c_int,
        pub constant: c_longlong,
        pub precision: c_longlong,
        pub tolerance: c_longlong,
        pub time: crate::timeval,
        pub tick: c_longlong,
        pub ppsfreq: c_longlong,
        pub jitter: c_longlong,
        pub shift: c_int,
        _pad3: c_int,
        pub stabil: c_longlong,
        pub jitcnt: c_longlong,
        pub calcnt: c_longlong,
        pub errcnt: c_longlong,
        pub stbcnt: c_longlong,
        pub tai: c_int,
        pub __unused1: i32,
        pub __unused2: i32,
        pub __unused3: i32,
        pub __unused4: i32,
        pub __unused5: i32,
        pub __unused6: i32,
        pub __unused7: i32,
        pub __unused8: i32,
        pub __unused9: i32,
        pub __unused10: i32,
        pub __unused11: i32,
    }

    #[cfg(not(gnu_time_bits64))]
    pub struct timex {
        pub modes: c_uint,
        pub offset: c_long,
        pub freq: c_long,
        pub maxerror: c_long,
        pub esterror: c_long,
        pub status: c_int,
        pub constant: c_long,
        pub precision: c_long,
        pub tolerance: c_long,
        pub time: crate::timeval,
        pub tick: c_long,
        pub ppsfreq: c_long,
        pub jitter: c_long,
        pub shift: c_int,
        pub stabil: c_long,
        pub jitcnt: c_long,
        pub calcnt: c_long,
        pub errcnt: c_long,
        pub stbcnt: c_long,
        pub tai: c_int,
        pub __unused1: i32,
        pub __unused2: i32,
        pub __unused3: i32,
        pub __unused4: i32,
        pub __unused5: i32,
        pub __unused6: i32,
        pub __unused7: i32,
        pub __unused8: i32,
        pub __unused9: i32,
        pub __unused10: i32,
        pub __unused11: i32,
    }
}

pub const POSIX_FADV_DONTNEED: c_int = 4;
pub const POSIX_FADV_NOREUSE: c_int = 5;

pub const F_OFD_GETLK: c_int = 36;
pub const F_OFD_SETLK: c_int = 37;
pub const F_OFD_SETLKW: c_int = 38;

pub const __SIZEOF_PTHREAD_CONDATTR_T: usize = 4;
pub const __SIZEOF_PTHREAD_MUTEX_T: usize = 24;
pub const __SIZEOF_PTHREAD_RWLOCK_T: usize = 32;
pub const __SIZEOF_PTHREAD_BARRIER_T: usize = 20;
pub const __SIZEOF_PTHREAD_MUTEXATTR_T: usize = 4;
pub const __SIZEOF_PTHREAD_RWLOCKATTR_T: usize = 8;
pub const __SIZEOF_PTHREAD_BARRIERATTR_T: usize = 4;

cfg_if! {
    if #[cfg(target_arch = "sparc")] {
        pub const O_NOATIME: c_int = 0x200000;
        pub const O_PATH: c_int = 0x1000000;
        pub const O_TMPFILE: c_int = 0x2000000 | O_DIRECTORY;

        pub const SA_ONSTACK: c_int = 1;

        pub const PTRACE_DETACH: c_uint = 11;

        pub const F_RDLCK: c_int = 1;
        pub const F_WRLCK: c_int = 2;
        pub const F_UNLCK: c_int = 3;

        pub const SFD_CLOEXEC: c_int = 0x400000;

        pub const NCCS: usize = 17;

        pub const O_TRUNC: c_int = 0x400;
        pub const O_CLOEXEC: c_int = 0x400000;

        pub const EBFONT: c_int = 109;
        pub const ENOSTR: c_int = 72;
        pub const ENODATA: c_int = 111;
        pub const ETIME: c_int = 73;
        pub const ENOSR: c_int = 74;
        pub const ENONET: c_int = 80;
        pub const ENOPKG: c_int = 113;
        pub const EREMOTE: c_int = 71;
        pub const ENOLINK: c_int = 82;
        pub const EADV: c_int = 83;
        pub const ESRMNT: c_int = 84;
        pub const ECOMM: c_int = 85;
        pub const EPROTO: c_int = 86;
        pub const EDOTDOT: c_int = 88;

        pub const SA_NODEFER: c_int = 0x20;
        pub const SA_RESETHAND: c_int = 0x4;
        pub const SA_RESTART: c_int = 0x2;
        pub const SA_NOCLDSTOP: c_int = 0x00000008;

        pub const EPOLL_CLOEXEC: c_int = 0x400000;

        pub const EFD_CLOEXEC: c_int = 0x400000;
    } else {
        pub const O_NOATIME: c_int = 0o1000000;
        pub const O_PATH: c_int = 0o10000000;
        pub const O_TMPFILE: c_int = 0o20000000 | O_DIRECTORY;

        pub const SA_ONSTACK: c_int = 0x08000000;

        pub const PTRACE_DETACH: c_uint = 17;

        pub const F_RDLCK: c_int = 0;
        pub const F_WRLCK: c_int = 1;
        pub const F_UNLCK: c_int = 2;

        pub const SFD_CLOEXEC: c_int = 0x080000;

        pub const NCCS: usize = 32;

        pub const O_TRUNC: c_int = 512;
        pub const O_CLOEXEC: c_int = 0x80000;
        pub const EBFONT: c_int = 59;
        pub const ENOSTR: c_int = 60;
        pub const ENODATA: c_int = 61;
        pub const ETIME: c_int = 62;
        pub const ENOSR: c_int = 63;
        pub const ENONET: c_int = 64;
        pub const ENOPKG: c_int = 65;
        pub const EREMOTE: c_int = 66;
        pub const ENOLINK: c_int = 67;
        pub const EADV: c_int = 68;
        pub const ESRMNT: c_int = 69;
        pub const ECOMM: c_int = 70;
        pub const EPROTO: c_int = 71;
        pub const EDOTDOT: c_int = 73;

        pub const SA_NODEFER: c_int = 0x40000000;
        pub const SA_RESETHAND: c_int = 0x80000000;
        pub const SA_RESTART: c_int = 0x10000000;
        pub const SA_NOCLDSTOP: c_int = 0x00000001;

        pub const EPOLL_CLOEXEC: c_int = 0x80000;

        pub const EFD_CLOEXEC: c_int = 0x80000;
    }
}
cfg_if! {
    if #[cfg(target_arch = "sparc")] {
        pub const F_SETLK: c_int = 8;
        pub const F_SETLKW: c_int = 9;
    } else if #[cfg(all(
        gnu_file_offset_bits64,
        any(target_arch = "mips", target_arch = "mips32r6")
    ))] {
        pub const F_SETLK: c_int = 34;
        pub const F_SETLKW: c_int = 35;
    } else if #[cfg(gnu_file_offset_bits64)] {
        pub const F_SETLK: c_int = 13;
        pub const F_SETLKW: c_int = 14;
    } else {
        pub const F_SETLK: c_int = 6;
        pub const F_SETLKW: c_int = 7;
    }
}

#[cfg(target_endian = "little")]
pub const PTHREAD_RECURSIVE_MUTEX_INITIALIZER_NP: crate::pthread_mutex_t = pthread_mutex_t {
    size: [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ],
};
#[cfg(target_endian = "little")]
pub const PTHREAD_ERRORCHECK_MUTEX_INITIALIZER_NP: crate::pthread_mutex_t = pthread_mutex_t {
    size: [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ],
};
#[cfg(target_endian = "little")]
pub const PTHREAD_ADAPTIVE_MUTEX_INITIALIZER_NP: crate::pthread_mutex_t = pthread_mutex_t {
    size: [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ],
};
#[cfg(target_endian = "big")]
pub const PTHREAD_RECURSIVE_MUTEX_INITIALIZER_NP: crate::pthread_mutex_t = pthread_mutex_t {
    size: [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0,
    ],
};
#[cfg(target_endian = "big")]
pub const PTHREAD_ERRORCHECK_MUTEX_INITIALIZER_NP: crate::pthread_mutex_t = pthread_mutex_t {
    size: [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0,
    ],
};
#[cfg(target_endian = "big")]
pub const PTHREAD_ADAPTIVE_MUTEX_INITIALIZER_NP: crate::pthread_mutex_t = pthread_mutex_t {
    size: [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0,
    ],
};

pub const PTRACE_GETFPREGS: c_uint = 14;
pub const PTRACE_SETFPREGS: c_uint = 15;
pub const PTRACE_GETREGS: c_uint = 12;
pub const PTRACE_SETREGS: c_uint = 13;

extern "C" {
    pub fn sysctl(
        name: *mut c_int,
        namelen: c_int,
        oldp: *mut c_void,
        oldlenp: *mut size_t,
        newp: *mut c_void,
        newlen: size_t,
    ) -> c_int;
}

cfg_if! {
    if #[cfg(target_arch = "x86")] {
        mod x86;
        pub use self::x86::*;
    } else if #[cfg(target_arch = "arm")] {
        mod arm;
        pub use self::arm::*;
    } else if #[cfg(any(target_arch = "mips", target_arch = "mips32r6"))] {
        mod mips;
        pub use self::mips::*;
    } else if #[cfg(target_arch = "m68k")] {
        mod m68k;
        pub use self::m68k::*;
    } else if #[cfg(target_arch = "powerpc")] {
        mod powerpc;
        pub use self::powerpc::*;
    } else if #[cfg(target_arch = "sparc")] {
        mod sparc;
        pub use self::sparc::*;
    } else if #[cfg(target_arch = "riscv32")] {
        mod riscv32;
        pub use self::riscv32::*;
    } else if #[cfg(target_arch = "csky")] {
        mod csky;
        pub use self::csky::*;
    } else {
        // Unknown target_arch
    }
}
