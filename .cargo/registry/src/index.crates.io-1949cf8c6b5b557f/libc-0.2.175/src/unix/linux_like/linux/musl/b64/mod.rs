use crate::prelude::*;

pub type regoff_t = c_long;

s! {
    // MIPS implementation is special, see the subfolder.
    #[cfg(not(target_arch = "mips64"))]
    pub struct stack_t {
        pub ss_sp: *mut c_void,
        pub ss_flags: c_int,
        pub ss_size: size_t,
    }

    pub struct pthread_attr_t {
        __size: [u64; 7],
    }

    pub struct sigset_t {
        __val: [c_ulong; 16],
    }

    // PowerPC implementation is special, see the subfolder.
    #[cfg(not(target_arch = "powerpc64"))]
    pub struct shmid_ds {
        pub shm_perm: crate::ipc_perm,
        pub shm_segsz: size_t,
        pub shm_atime: crate::time_t,
        pub shm_dtime: crate::time_t,
        pub shm_ctime: crate::time_t,
        pub shm_cpid: crate::pid_t,
        pub shm_lpid: crate::pid_t,
        pub shm_nattch: c_ulong,
        __pad1: c_ulong,
        __pad2: c_ulong,
    }

    pub struct msqid_ds {
        pub msg_perm: crate::ipc_perm,
        pub msg_stime: crate::time_t,
        pub msg_rtime: crate::time_t,
        pub msg_ctime: crate::time_t,
        pub __msg_cbytes: c_ulong,
        pub msg_qnum: crate::msgqnum_t,
        pub msg_qbytes: crate::msglen_t,
        pub msg_lspid: crate::pid_t,
        pub msg_lrpid: crate::pid_t,
        __pad1: c_ulong,
        __pad2: c_ulong,
    }

    pub struct msghdr {
        pub msg_name: *mut c_void,
        pub msg_namelen: crate::socklen_t,
        pub msg_iov: *mut crate::iovec,
        #[cfg(target_endian = "big")]
        __pad1: c_int,
        pub msg_iovlen: c_int,
        #[cfg(target_endian = "little")]
        __pad1: c_int,
        pub msg_control: *mut c_void,
        #[cfg(target_endian = "big")]
        __pad2: c_int,
        pub msg_controllen: crate::socklen_t,
        #[cfg(target_endian = "little")]
        __pad2: c_int,
        pub msg_flags: c_int,
    }

    pub struct cmsghdr {
        #[cfg(target_endian = "big")]
        pub __pad1: c_int,
        pub cmsg_len: crate::socklen_t,
        #[cfg(target_endian = "little")]
        pub __pad1: c_int,
        pub cmsg_level: c_int,
        pub cmsg_type: c_int,
    }

    pub struct sem_t {
        __val: [c_int; 8],
    }
}

pub const __SIZEOF_PTHREAD_RWLOCK_T: usize = 56;
pub const __SIZEOF_PTHREAD_MUTEX_T: usize = 40;
pub const __SIZEOF_PTHREAD_BARRIER_T: usize = 32;

cfg_if! {
    if #[cfg(target_arch = "aarch64")] {
        mod aarch64;
        pub use self::aarch64::*;
    } else if #[cfg(target_arch = "mips64")] {
        mod mips64;
        pub use self::mips64::*;
    } else if #[cfg(any(target_arch = "powerpc64"))] {
        mod powerpc64;
        pub use self::powerpc64::*;
    } else if #[cfg(any(target_arch = "s390x"))] {
        mod s390x;
        pub use self::s390x::*;
    } else if #[cfg(any(target_arch = "x86_64"))] {
        mod x86_64;
        pub use self::x86_64::*;
    } else if #[cfg(any(target_arch = "riscv64"))] {
        mod riscv64;
        pub use self::riscv64::*;
    } else if #[cfg(any(target_arch = "loongarch64"))] {
        mod loongarch64;
        pub use self::loongarch64::*;
    } else if #[cfg(any(target_arch = "wasm32"))] {
        mod wasm32;
        pub use self::wasm32::*;
    } else {
        // Unknown target_arch
    }
}
