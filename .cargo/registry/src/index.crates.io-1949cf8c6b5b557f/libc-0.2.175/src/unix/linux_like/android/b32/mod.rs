use crate::prelude::*;

// The following definitions are correct for arm and i686,
// but may be wrong for mips

pub type mode_t = u16;
pub type off64_t = c_longlong;
pub type sigset_t = c_ulong;
pub type socklen_t = i32;
pub type time64_t = i64;
pub type __u64 = c_ulonglong;
pub type __s64 = c_longlong;

s! {
    // FIXME(1.0): This should not implement `PartialEq`
    #[allow(unpredictable_function_pointer_comparisons)]
    pub struct sigaction {
        pub sa_sigaction: crate::sighandler_t,
        pub sa_mask: crate::sigset_t,
        pub sa_flags: c_int,
        pub sa_restorer: Option<extern "C" fn()>,
    }

    pub struct rlimit64 {
        pub rlim_cur: u64,
        pub rlim_max: u64,
    }

    pub struct stat {
        pub st_dev: c_ulonglong,
        __pad0: [c_uchar; 4],
        __st_ino: crate::ino_t,
        pub st_mode: c_uint,
        pub st_nlink: crate::nlink_t,
        pub st_uid: crate::uid_t,
        pub st_gid: crate::gid_t,
        pub st_rdev: c_ulonglong,
        __pad3: [c_uchar; 4],
        pub st_size: c_longlong,
        pub st_blksize: crate::blksize_t,
        pub st_blocks: c_ulonglong,
        pub st_atime: c_long,
        pub st_atime_nsec: c_long,
        pub st_mtime: c_long,
        pub st_mtime_nsec: c_long,
        pub st_ctime: c_long,
        pub st_ctime_nsec: c_long,
        pub st_ino: c_ulonglong,
    }

    pub struct stat64 {
        pub st_dev: c_ulonglong,
        __pad0: [c_uchar; 4],
        __st_ino: crate::ino_t,
        pub st_mode: c_uint,
        pub st_nlink: crate::nlink_t,
        pub st_uid: crate::uid_t,
        pub st_gid: crate::gid_t,
        pub st_rdev: c_ulonglong,
        __pad3: [c_uchar; 4],
        pub st_size: c_longlong,
        pub st_blksize: crate::blksize_t,
        pub st_blocks: c_ulonglong,
        pub st_atime: c_long,
        pub st_atime_nsec: c_long,
        pub st_mtime: c_long,
        pub st_mtime_nsec: c_long,
        pub st_ctime: c_long,
        pub st_ctime_nsec: c_long,
        pub st_ino: c_ulonglong,
    }

    pub struct statfs64 {
        pub f_type: u32,
        pub f_bsize: u32,
        pub f_blocks: u64,
        pub f_bfree: u64,
        pub f_bavail: u64,
        pub f_files: u64,
        pub f_ffree: u64,
        pub f_fsid: crate::__fsid_t,
        pub f_namelen: u32,
        pub f_frsize: u32,
        pub f_flags: u32,
        pub f_spare: [u32; 4],
    }

    pub struct statvfs64 {
        pub f_bsize: c_ulong,
        pub f_frsize: c_ulong,
        pub f_blocks: c_ulong,
        pub f_bfree: c_ulong,
        pub f_bavail: c_ulong,
        pub f_files: c_ulong,
        pub f_ffree: c_ulong,
        pub f_favail: c_ulong,
        pub f_fsid: c_ulong,
        pub f_flag: c_ulong,
        pub f_namemax: c_ulong,
    }

    pub struct pthread_attr_t {
        pub flags: u32,
        pub stack_base: *mut c_void,
        pub stack_size: size_t,
        pub guard_size: size_t,
        pub sched_policy: i32,
        pub sched_priority: i32,
    }

    pub struct pthread_mutex_t {
        value: c_int,
    }

    pub struct pthread_cond_t {
        value: c_int,
    }

    pub struct pthread_rwlock_t {
        lock: pthread_mutex_t,
        cond: pthread_cond_t,
        numLocks: c_int,
        writerThreadId: c_int,
        pendingReaders: c_int,
        pendingWriters: c_int,
        attr: i32,
        __reserved: [c_char; 12],
    }

    pub struct pthread_barrier_t {
        __private: [i32; 8],
    }

    pub struct pthread_spinlock_t {
        __private: [i32; 2],
    }

    pub struct passwd {
        pub pw_name: *mut c_char,
        pub pw_passwd: *mut c_char,
        pub pw_uid: crate::uid_t,
        pub pw_gid: crate::gid_t,
        pub pw_dir: *mut c_char,
        pub pw_shell: *mut c_char,
    }

    pub struct statfs {
        pub f_type: u32,
        pub f_bsize: u32,
        pub f_blocks: u64,
        pub f_bfree: u64,
        pub f_bavail: u64,
        pub f_files: u64,
        pub f_ffree: u64,
        pub f_fsid: crate::__fsid_t,
        pub f_namelen: u32,
        pub f_frsize: u32,
        pub f_flags: u32,
        pub f_spare: [u32; 4],
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
        pub pad: c_ushort,
        pub totalhigh: c_ulong,
        pub freehigh: c_ulong,
        pub mem_unit: c_uint,
        pub _f: [c_char; 8],
    }
}

s_no_extra_traits! {
    pub struct sigset64_t {
        __bits: [c_ulong; 2],
    }
}

// These constants must be of the same type of sigaction.sa_flags
pub const SA_NOCLDSTOP: c_int = 0x00000001;
pub const SA_NOCLDWAIT: c_int = 0x00000002;
pub const SA_NODEFER: c_int = 0x40000000;
pub const SA_ONSTACK: c_int = 0x08000000;
pub const SA_RESETHAND: c_int = 0x80000000;
pub const SA_RESTART: c_int = 0x10000000;
pub const SA_SIGINFO: c_int = 0x00000004;

pub const RTLD_GLOBAL: c_int = 2;
pub const RTLD_NOW: c_int = 0;
pub const RTLD_DEFAULT: *mut c_void = -1isize as *mut c_void;

pub const PTRACE_GETFPREGS: c_int = 14;
pub const PTRACE_SETFPREGS: c_int = 15;

pub const PTHREAD_MUTEX_INITIALIZER: pthread_mutex_t = pthread_mutex_t { value: 0 };
pub const PTHREAD_COND_INITIALIZER: pthread_cond_t = pthread_cond_t { value: 0 };
pub const PTHREAD_RWLOCK_INITIALIZER: pthread_rwlock_t = pthread_rwlock_t {
    lock: PTHREAD_MUTEX_INITIALIZER,
    cond: PTHREAD_COND_INITIALIZER,
    numLocks: 0,
    writerThreadId: 0,
    pendingReaders: 0,
    pendingWriters: 0,
    attr: 0,
    __reserved: [0; 12],
};
pub const PTHREAD_STACK_MIN: size_t = 4096 * 2;
pub const CPU_SETSIZE: size_t = 32;
pub const __CPU_BITS: size_t = 32;

pub const UT_LINESIZE: usize = 8;
pub const UT_NAMESIZE: usize = 8;
pub const UT_HOSTSIZE: usize = 16;

pub const SIGSTKSZ: size_t = 8192;
pub const MINSIGSTKSZ: size_t = 2048;

extern "C" {
    pub fn timegm64(tm: *const crate::tm) -> crate::time64_t;
}

cfg_if! {
    if #[cfg(target_arch = "x86")] {
        mod x86;
        pub use self::x86::*;
    } else if #[cfg(target_arch = "arm")] {
        mod arm;
        pub use self::arm::*;
    } else {
        // Unknown target_arch
    }
}
