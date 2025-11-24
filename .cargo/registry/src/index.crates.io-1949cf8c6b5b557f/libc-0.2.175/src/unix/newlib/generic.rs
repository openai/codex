//! Common types used by most newlib platforms

use crate::off_t;
use crate::prelude::*;

s! {
    pub struct sigset_t {
        #[cfg(target_os = "horizon")]
        __val: [c_ulong; 16],
        #[cfg(not(target_os = "horizon"))]
        __val: u32,
    }

    pub struct stat {
        pub st_dev: crate::dev_t,
        pub st_ino: crate::ino_t,
        pub st_mode: crate::mode_t,
        pub st_nlink: crate::nlink_t,
        pub st_uid: crate::uid_t,
        pub st_gid: crate::gid_t,
        pub st_rdev: crate::dev_t,
        pub st_size: off_t,
        pub st_atime: crate::time_t,
        pub st_spare1: c_long,
        pub st_mtime: crate::time_t,
        pub st_spare2: c_long,
        pub st_ctime: crate::time_t,
        pub st_spare3: c_long,
        pub st_blksize: crate::blksize_t,
        pub st_blocks: crate::blkcnt_t,
        pub st_spare4: [c_long; 2usize],
    }

    pub struct dirent {
        pub d_ino: crate::ino_t,
        pub d_type: c_uchar,
        pub d_name: [c_char; 256usize],
    }
}
