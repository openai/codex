use crate::prelude::*;

// The following definitions are correct for aarch64 and x86_64,
// but may be wrong for mips64

pub type mode_t = u32;
pub type off64_t = i64;
pub type socklen_t = u32;

s! {
    pub struct sigset_t {
        __val: [c_ulong; 1],
    }

    // FIXME(1.0): This should not implement `PartialEq`
    #[allow(unpredictable_function_pointer_comparisons)]
    pub struct sigaction {
        pub sa_flags: c_int,
        pub sa_sigaction: crate::sighandler_t,
        pub sa_mask: crate::sigset_t,
        pub sa_restorer: Option<extern "C" fn()>,
    }

    pub struct rlimit64 {
        pub rlim_cur: c_ulonglong,
        pub rlim_max: c_ulonglong,
    }

    pub struct pthread_attr_t {
        pub flags: u32,
        pub stack_base: *mut c_void,
        pub stack_size: size_t,
        pub guard_size: size_t,
        pub sched_policy: i32,
        pub sched_priority: i32,
        __reserved: [c_char; 16],
    }

    pub struct passwd {
        pub pw_name: *mut c_char,
        pub pw_passwd: *mut c_char,
        pub pw_uid: crate::uid_t,
        pub pw_gid: crate::gid_t,
        pub pw_gecos: *mut c_char,
        pub pw_dir: *mut c_char,
        pub pw_shell: *mut c_char,
    }

    pub struct statfs {
        pub f_type: u64,
        pub f_bsize: u64,
        pub f_blocks: u64,
        pub f_bfree: u64,
        pub f_bavail: u64,
        pub f_files: u64,
        pub f_ffree: u64,
        pub f_fsid: crate::__fsid_t,
        pub f_namelen: u64,
        pub f_frsize: u64,
        pub f_flags: u64,
        pub f_spare: [u64; 4],
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
        pub _f: [c_char; 0],
    }

    pub struct statfs64 {
        pub f_type: u64,
        pub f_bsize: u64,
        pub f_blocks: u64,
        pub f_bfree: u64,
        pub f_bavail: u64,
        pub f_files: u64,
        pub f_ffree: u64,
        pub f_fsid: crate::__fsid_t,
        pub f_namelen: u64,
        pub f_frsize: u64,
        pub f_flags: u64,
        pub f_spare: [u64; 4],
    }

    pub struct statvfs64 {
        pub f_bsize: c_ulong,
        pub f_frsize: c_ulong,
        pub f_blocks: u64,
        pub f_bfree: u64,
        pub f_bavail: u64,
        pub f_files: u64,
        pub f_ffree: u64,
        pub f_favail: u64,
        pub f_fsid: c_ulong,
        pub f_flag: c_ulong,
        pub f_namemax: c_ulong,
        __f_spare: [c_int; 6],
    }

    pub struct pthread_barrier_t {
        __private: [i64; 4],
    }

    pub struct pthread_spinlock_t {
        __private: i64,
    }
}

s_no_extra_traits! {
    pub struct pthread_mutex_t {
        value: c_int,
        __reserved: [c_char; 36],
    }

    pub struct pthread_cond_t {
        value: c_int,
        __reserved: [c_char; 44],
    }

    pub struct pthread_rwlock_t {
        numLocks: c_int,
        writerThreadId: c_int,
        pendingReaders: c_int,
        pendingWriters: c_int,
        attr: i32,
        __reserved: [c_char; 36],
    }

    pub struct sigset64_t {
        __bits: [c_ulong; 1],
    }
}

cfg_if! {
    if #[cfg(feature = "extra_traits")] {
        impl PartialEq for pthread_mutex_t {
            fn eq(&self, other: &pthread_mutex_t) -> bool {
                self.value == other.value
                    && self
                        .__reserved
                        .iter()
                        .zip(other.__reserved.iter())
                        .all(|(a, b)| a == b)
            }
        }

        impl Eq for pthread_mutex_t {}

        impl hash::Hash for pthread_mutex_t {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.value.hash(state);
                self.__reserved.hash(state);
            }
        }

        impl PartialEq for pthread_cond_t {
            fn eq(&self, other: &pthread_cond_t) -> bool {
                self.value == other.value
                    && self
                        .__reserved
                        .iter()
                        .zip(other.__reserved.iter())
                        .all(|(a, b)| a == b)
            }
        }

        impl Eq for pthread_cond_t {}

        impl hash::Hash for pthread_cond_t {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.value.hash(state);
                self.__reserved.hash(state);
            }
        }

        impl PartialEq for pthread_rwlock_t {
            fn eq(&self, other: &pthread_rwlock_t) -> bool {
                self.numLocks == other.numLocks
                    && self.writerThreadId == other.writerThreadId
                    && self.pendingReaders == other.pendingReaders
                    && self.pendingWriters == other.pendingWriters
                    && self.attr == other.attr
                    && self
                        .__reserved
                        .iter()
                        .zip(other.__reserved.iter())
                        .all(|(a, b)| a == b)
            }
        }

        impl Eq for pthread_rwlock_t {}

        impl hash::Hash for pthread_rwlock_t {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.numLocks.hash(state);
                self.writerThreadId.hash(state);
                self.pendingReaders.hash(state);
                self.pendingWriters.hash(state);
                self.attr.hash(state);
                self.__reserved.hash(state);
            }
        }
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

pub const RTLD_GLOBAL: c_int = 0x00100;
pub const RTLD_NOW: c_int = 2;
pub const RTLD_DEFAULT: *mut c_void = 0i64 as *mut c_void;

pub const PTHREAD_MUTEX_INITIALIZER: pthread_mutex_t = pthread_mutex_t {
    value: 0,
    __reserved: [0; 36],
};
pub const PTHREAD_COND_INITIALIZER: pthread_cond_t = pthread_cond_t {
    value: 0,
    __reserved: [0; 44],
};
pub const PTHREAD_RWLOCK_INITIALIZER: pthread_rwlock_t = pthread_rwlock_t {
    numLocks: 0,
    writerThreadId: 0,
    pendingReaders: 0,
    pendingWriters: 0,
    attr: 0,
    __reserved: [0; 36],
};
pub const PTHREAD_STACK_MIN: size_t = 4096 * 4;
pub const CPU_SETSIZE: size_t = 1024;
pub const __CPU_BITS: size_t = 64;

pub const UT_LINESIZE: usize = 32;
pub const UT_NAMESIZE: usize = 32;
pub const UT_HOSTSIZE: usize = 256;

f! {
    // Sadly, Android before 5.0 (API level 21), the accept4 syscall is not
    // exposed by the libc. As work-around, we implement it through `syscall`
    // directly. This workaround can be removed if the minimum version of
    // Android is bumped. When the workaround is removed, `accept4` can be
    // moved back to `linux_like/mod.rs`
    pub fn accept4(
        fd: c_int,
        addr: *mut crate::sockaddr,
        len: *mut crate::socklen_t,
        flg: c_int,
    ) -> c_int {
        crate::syscall(SYS_accept4, fd, addr, len, flg) as c_int
    }
}

extern "C" {
    pub fn __system_property_wait(
        pi: *const crate::prop_info,
        __old_serial: u32,
        __new_serial_ptr: *mut u32,
        __relative_timeout: *const crate::timespec,
    ) -> bool;
}

cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        mod x86_64;
        pub use self::x86_64::*;
    } else if #[cfg(target_arch = "aarch64")] {
        mod aarch64;
        pub use self::aarch64::*;
    } else if #[cfg(target_arch = "riscv64")] {
        mod riscv64;
        pub use self::riscv64::*;
    } else {
        // Unknown target_arch
    }
}
