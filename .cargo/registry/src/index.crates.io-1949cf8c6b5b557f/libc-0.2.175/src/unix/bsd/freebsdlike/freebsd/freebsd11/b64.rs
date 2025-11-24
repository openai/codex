use crate::off_t;
use crate::prelude::*;

#[repr(C)]
#[cfg_attr(feature = "extra_traits", derive(Debug, Eq, Hash, PartialEq))]
pub struct stat {
    pub st_dev: crate::dev_t,
    pub st_ino: crate::ino_t,
    pub st_mode: crate::mode_t,
    pub st_nlink: crate::nlink_t,
    pub st_uid: crate::uid_t,
    pub st_gid: crate::gid_t,
    pub st_rdev: crate::dev_t,
    pub st_atime: crate::time_t,
    pub st_atime_nsec: c_long,
    pub st_mtime: crate::time_t,
    pub st_mtime_nsec: c_long,
    pub st_ctime: crate::time_t,
    pub st_ctime_nsec: c_long,
    pub st_size: off_t,
    pub st_blocks: crate::blkcnt_t,
    pub st_blksize: crate::blksize_t,
    pub st_flags: crate::fflags_t,
    pub st_gen: u32,
    pub st_lspare: i32,
    pub st_birthtime: crate::time_t,
    pub st_birthtime_nsec: c_long,
}

impl Copy for crate::stat {}
impl Clone for crate::stat {
    fn clone(&self) -> crate::stat {
        *self
    }
}
