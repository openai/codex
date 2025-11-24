//! 64-bit specific definitions for linux-like values

use crate::prelude::*;

pub type ino_t = u64;
pub type off_t = i64;
pub type blkcnt_t = i64;
pub type shmatt_t = u64;
pub type msgqnum_t = u64;
pub type msglen_t = u64;
pub type fsblkcnt_t = u64;
pub type fsfilcnt_t = u64;
pub type rlim_t = u64;
#[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
pub type __syscall_ulong_t = c_ulonglong;
#[cfg(not(all(target_arch = "x86_64", target_pointer_width = "32")))]
pub type __syscall_ulong_t = c_ulong;

cfg_if! {
    if #[cfg(all(target_arch = "aarch64", target_pointer_width = "32"))] {
        pub type clock_t = i32;
        pub type time_t = i32;
        pub type __fsword_t = i32;
    } else {
        pub type __fsword_t = i64;
        pub type clock_t = i64;
        pub type time_t = i64;
    }
}

s! {
    pub struct sigset_t {
        #[cfg(target_pointer_width = "32")]
        __val: [u32; 32],
        #[cfg(target_pointer_width = "64")]
        __val: [u64; 16],
    }

    pub struct sysinfo {
        pub uptime: i64,
        pub loads: [u64; 3],
        pub totalram: u64,
        pub freeram: u64,
        pub sharedram: u64,
        pub bufferram: u64,
        pub totalswap: u64,
        pub freeswap: u64,
        pub procs: c_ushort,
        pub pad: c_ushort,
        pub totalhigh: u64,
        pub freehigh: u64,
        pub mem_unit: c_uint,
        pub _f: [c_char; 0],
    }

    pub struct msqid_ds {
        pub msg_perm: crate::ipc_perm,
        pub msg_stime: crate::time_t,
        pub msg_rtime: crate::time_t,
        pub msg_ctime: crate::time_t,
        pub __msg_cbytes: u64,
        pub msg_qnum: crate::msgqnum_t,
        pub msg_qbytes: crate::msglen_t,
        pub msg_lspid: crate::pid_t,
        pub msg_lrpid: crate::pid_t,
        __glibc_reserved4: u64,
        __glibc_reserved5: u64,
    }

    pub struct semid_ds {
        pub sem_perm: ipc_perm,
        pub sem_otime: crate::time_t,
        #[cfg(not(any(
            target_arch = "aarch64",
            target_arch = "loongarch64",
            target_arch = "mips64",
            target_arch = "mips64r6",
            target_arch = "powerpc64",
            target_arch = "riscv64",
            target_arch = "sparc64",
            target_arch = "s390x",
        )))]
        __reserved: crate::__syscall_ulong_t,
        pub sem_ctime: crate::time_t,
        #[cfg(not(any(
            target_arch = "aarch64",
            target_arch = "loongarch64",
            target_arch = "mips64",
            target_arch = "mips64r6",
            target_arch = "powerpc64",
            target_arch = "riscv64",
            target_arch = "sparc64",
            target_arch = "s390x",
        )))]
        __reserved2: crate::__syscall_ulong_t,
        pub sem_nsems: crate::__syscall_ulong_t,
        __glibc_reserved3: crate::__syscall_ulong_t,
        __glibc_reserved4: crate::__syscall_ulong_t,
    }

    pub struct timex {
        pub modes: c_uint,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        pub offset: i64,
        #[cfg(not(all(target_arch = "x86_64", target_pointer_width = "32")))]
        pub offset: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        pub freq: i64,
        #[cfg(not(all(target_arch = "x86_64", target_pointer_width = "32")))]
        pub freq: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        pub maxerror: i64,
        #[cfg(not(all(target_arch = "x86_64", target_pointer_width = "32")))]
        pub maxerror: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        pub esterror: i64,
        #[cfg(not(all(target_arch = "x86_64", target_pointer_width = "32")))]
        pub esterror: c_long,
        pub status: c_int,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        pub constant: i64,
        #[cfg(not(all(target_arch = "x86_64", target_pointer_width = "32")))]
        pub constant: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        pub precision: i64,
        #[cfg(not(all(target_arch = "x86_64", target_pointer_width = "32")))]
        pub precision: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        pub tolerance: i64,
        #[cfg(not(all(target_arch = "x86_64", target_pointer_width = "32")))]
        pub tolerance: c_long,
        pub time: crate::timeval,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        pub tick: i64,
        #[cfg(not(all(target_arch = "x86_64", target_pointer_width = "32")))]
        pub tick: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        pub ppsfreq: i64,
        #[cfg(not(all(target_arch = "x86_64", target_pointer_width = "32")))]
        pub ppsfreq: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        pub jitter: i64,
        #[cfg(not(all(target_arch = "x86_64", target_pointer_width = "32")))]
        pub jitter: c_long,
        pub shift: c_int,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        pub stabil: i64,
        #[cfg(not(all(target_arch = "x86_64", target_pointer_width = "32")))]
        pub stabil: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        pub jitcnt: i64,
        #[cfg(not(all(target_arch = "x86_64", target_pointer_width = "32")))]
        pub jitcnt: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        pub calcnt: i64,
        #[cfg(not(all(target_arch = "x86_64", target_pointer_width = "32")))]
        pub calcnt: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        pub errcnt: i64,
        #[cfg(not(all(target_arch = "x86_64", target_pointer_width = "32")))]
        pub errcnt: c_long,
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
        pub stbcnt: i64,
        #[cfg(not(all(target_arch = "x86_64", target_pointer_width = "32")))]
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

pub const __SIZEOF_PTHREAD_RWLOCKATTR_T: usize = 8;

pub const O_LARGEFILE: c_int = 0;

cfg_if! {
    if #[cfg(target_arch = "aarch64")] {
        mod aarch64;
        pub use self::aarch64::*;
    } else if #[cfg(any(target_arch = "powerpc64"))] {
        mod powerpc64;
        pub use self::powerpc64::*;
    } else if #[cfg(any(target_arch = "sparc64"))] {
        mod sparc64;
        pub use self::sparc64::*;
    } else if #[cfg(any(target_arch = "mips64", target_arch = "mips64r6"))] {
        mod mips64;
        pub use self::mips64::*;
    } else if #[cfg(any(target_arch = "s390x"))] {
        mod s390x;
        pub use self::s390x::*;
    } else if #[cfg(target_arch = "x86_64")] {
        mod x86_64;
        pub use self::x86_64::*;
    } else if #[cfg(any(target_arch = "riscv64"))] {
        mod riscv64;
        pub use self::riscv64::*;
    } else if #[cfg(any(target_arch = "loongarch64"))] {
        mod loongarch64;
        pub use self::loongarch64::*;
    } else {
        // Unknown target_arch
    }
}
