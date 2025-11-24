use crate::off_t;
use crate::prelude::*;

pub type __u64 = c_ulonglong;
pub type wchar_t = u32;
pub type nlink_t = c_ulong;
pub type blksize_t = c_long;

s! {
    pub struct stat {
        pub st_dev: crate::dev_t,
        pub st_ino: crate::ino_t,
        pub st_mode: crate::mode_t,
        pub st_nlink: crate::nlink_t,
        pub st_uid: crate::uid_t,
        pub st_gid: crate::gid_t,
        pub st_rdev: crate::dev_t,
        __pad0: c_ulong,
        pub st_size: off_t,
        pub st_blksize: crate::blksize_t,
        __pad1: c_int,
        pub st_blocks: crate::blkcnt_t,
        pub st_atime: crate::time_t,
        pub st_atime_nsec: c_long,
        pub st_mtime: crate::time_t,
        pub st_mtime_nsec: c_long,
        pub st_ctime: crate::time_t,
        pub st_ctime_nsec: c_long,
        __unused: [c_uint; 2],
    }

    pub struct stat64 {
        pub st_dev: crate::dev_t,
        pub st_ino: crate::ino_t,
        pub st_mode: crate::mode_t,
        pub st_nlink: crate::nlink_t,
        pub st_uid: crate::uid_t,
        pub st_gid: crate::gid_t,
        pub st_rdev: crate::dev_t,
        __pad0: c_ulong,
        pub st_size: off_t,
        pub st_blksize: crate::blksize_t,
        __pad1: c_int,
        pub st_blocks: crate::blkcnt_t,
        pub st_atime: crate::time_t,
        pub st_atime_nsec: c_long,
        pub st_mtime: crate::time_t,
        pub st_mtime_nsec: c_long,
        pub st_ctime: crate::time_t,
        pub st_ctime_nsec: c_long,
        __unused: [c_uint; 2],
    }

    pub struct ipc_perm {
        pub __ipc_perm_key: crate::key_t,
        pub uid: crate::uid_t,
        pub gid: crate::gid_t,
        pub cuid: crate::uid_t,
        pub cgid: crate::gid_t,
        pub mode: crate::mode_t,
        pub __seq: c_ushort,
        __unused1: c_ulong,
        __unused2: c_ulong,
    }
}

// From https://cs.opensource.google/fuchsia/fuchsia/+/main:zircon/third_party/ulib/musl/include/bits/signal.h;l=20-21;drc=0827b18ab9540c46f8037f407d17ea15a79e9ba7
pub const MINSIGSTKSZ: size_t = 6144;
pub const SIGSTKSZ: size_t = 12288;
