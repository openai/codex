use crate::prelude::*;

pub type nlink_t = u32;
pub type blksize_t = c_long;
pub type __u64 = c_ulonglong;
pub type __s64 = c_longlong;
pub type regoff_t = c_int;

s! {
    pub struct pthread_attr_t {
        __size: [u32; 9],
    }

    pub struct sigset_t {
        __val: [c_ulong; 32],
    }

    pub struct msghdr {
        pub msg_name: *mut c_void,
        pub msg_namelen: crate::socklen_t,
        pub msg_iov: *mut crate::iovec,
        pub msg_iovlen: c_int,
        pub msg_control: *mut c_void,
        pub msg_controllen: crate::socklen_t,
        pub msg_flags: c_int,
    }

    pub struct cmsghdr {
        pub cmsg_len: crate::socklen_t,
        pub cmsg_level: c_int,
        pub cmsg_type: c_int,
    }

    pub struct sem_t {
        __val: [c_int; 4],
    }
}

pub const __SIZEOF_PTHREAD_RWLOCK_T: usize = 32;
pub const __SIZEOF_PTHREAD_MUTEX_T: usize = 24;
pub const __SIZEOF_PTHREAD_BARRIER_T: usize = 20;

cfg_if! {
    if #[cfg(any(target_arch = "x86"))] {
        mod x86;
        pub use self::x86::*;
    } else if #[cfg(any(target_arch = "mips"))] {
        mod mips;
        pub use self::mips::*;
    } else if #[cfg(any(target_arch = "arm"))] {
        mod arm;
        pub use self::arm::*;
    } else if #[cfg(any(target_arch = "powerpc"))] {
        mod powerpc;
        pub use self::powerpc::*;
    } else if #[cfg(any(target_arch = "hexagon"))] {
        mod hexagon;
        pub use self::hexagon::*;
    } else if #[cfg(any(target_arch = "riscv32"))] {
        mod riscv32;
        pub use self::riscv32::*;
    } else {
        // Unknown target_arch
    }
}
