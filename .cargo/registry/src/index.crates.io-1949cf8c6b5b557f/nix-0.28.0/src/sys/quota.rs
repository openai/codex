//! Set and configure disk quotas for users, groups, or projects.
//!
//! # Examples
//!
//! Enabling and setting a quota:
//!
//! ```rust,no_run
//! # use nix::sys::quota::{Dqblk, quotactl_on, quotactl_set, QuotaFmt, QuotaType, QuotaValidFlags};
//! quotactl_on(QuotaType::USRQUOTA, "/dev/sda1", QuotaFmt::QFMT_VFS_V1, "aquota.user").unwrap();
//! let mut dqblk: Dqblk = Default::default();
//! dqblk.set_blocks_hard_limit(10000);
//! dqblk.set_blocks_soft_limit(8000);
//! quotactl_set(QuotaType::USRQUOTA, "/dev/sda1", 50, &dqblk, QuotaValidFlags::QIF_BLIMITS).unwrap();
//! ```
use crate::errno::Errno;
use crate::{NixPath, Result};
use libc::{self, c_char, c_int};
use std::default::Default;
use std::{mem, ptr};

struct QuotaCmd(QuotaSubCmd, QuotaType);

impl QuotaCmd {
    fn as_int(&self) -> c_int {
        libc::QCMD(self.0 as i32, self.1 as i32)
    }
}

// linux quota version >= 2
libc_enum! {
    #[repr(i32)]
    enum QuotaSubCmd {
        Q_SYNC,
        Q_QUOTAON,
        Q_QUOTAOFF,
        Q_GETQUOTA,
        Q_SETQUOTA,
    }
}

libc_enum! {
    /// The scope of the quota.
    #[repr(i32)]
    #[non_exhaustive]
    pub enum QuotaType {
        /// Specify a user quota
        USRQUOTA,
        /// Specify a group quota
        GRPQUOTA,
    }
}

libc_enum! {
    /// The type of quota format to use.
    #[repr(i32)]
    #[non_exhaustive]
    pub enum QuotaFmt {
        /// Use the original quota format.
        QFMT_VFS_OLD,
        /// Use the standard VFS v0 quota format.
        ///
        /// Handles 32-bit UIDs/GIDs and quota limits up to 2<sup>32</sup> bytes/2<sup>32</sup> inodes.
        QFMT_VFS_V0,
        /// Use the VFS v1 quota format.
        ///
        /// Handles 32-bit UIDs/GIDs and quota limits of 2<sup>64</sup> bytes/2<sup>64</sup> inodes.
        QFMT_VFS_V1,
    }
}

libc_bitflags!(
    /// Indicates the quota fields that are valid to read from.
    #[derive(Default)]
    pub struct QuotaValidFlags: u32 {
        /// The block hard & soft limit fields.
        QIF_BLIMITS;
        /// The current space field.
        QIF_SPACE;
        /// The inode hard & soft limit fields.
        QIF_ILIMITS;
        /// The current inodes field.
        QIF_INODES;
        /// The disk use time limit field.
        QIF_BTIME;
        /// The file quote time limit field.
        QIF_ITIME;
        /// All block & inode limits.
        QIF_LIMITS;
        /// The space & inodes usage fields.
        QIF_USAGE;
        /// The time limit fields.
        QIF_TIMES;
        /// All fields.
        QIF_ALL;
    }
);

/// Wrapper type for `if_dqblk`
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Dqblk(libc::dqblk);

impl Default for Dqblk {
    fn default() -> Dqblk {
        Dqblk(libc::dqblk {
            dqb_bhardlimit: 0,
            dqb_bsoftlimit: 0,
            dqb_curspace: 0,
            dqb_ihardlimit: 0,
            dqb_isoftlimit: 0,
            dqb_curinodes: 0,
            dqb_btime: 0,
            dqb_itime: 0,
            dqb_valid: 0,
        })
    }
}

impl Dqblk {
    /// The absolute limit on disk quota blocks allocated.
    pub fn blocks_hard_limit(&self) -> Option<u64> {
        let valid_fields =
            QuotaValidFlags::from_bits_truncate(self.0.dqb_valid);
        if valid_fields.contains(QuotaValidFlags::QIF_BLIMITS) {
            Some(self.0.dqb_bhardlimit)
        } else {
            None
        }
    }

    /// Set the absolute limit on disk quota blocks allocated.
    pub fn set_blocks_hard_limit(&mut self, limit: u64) {
        self.0.dqb_bhardlimit = limit;
    }

    /// Preferred limit on disk quota blocks
    pub fn blocks_soft_limit(&self) -> Option<u64> {
        let valid_fields =
            QuotaValidFlags::from_bits_truncate(self.0.dqb_valid);
        if valid_fields.contains(QuotaValidFlags::QIF_BLIMITS) {
            Some(self.0.dqb_bsoftlimit)
        } else {
            None
        }
    }

    /// Set the preferred limit on disk quota blocks allocated.
    pub fn set_blocks_soft_limit(&mut self, limit: u64) {
        self.0.dqb_bsoftlimit = limit;
    }

    /// Current occupied space (bytes).
    pub fn occupied_space(&self) -> Option<u64> {
        let valid_fields =
            QuotaValidFlags::from_bits_truncate(self.0.dqb_valid);
        if valid_fields.contains(QuotaValidFlags::QIF_SPACE) {
            Some(self.0.dqb_curspace)
        } else {
            None
        }
    }

    /// Maximum number of allocated inodes.
    pub fn inodes_hard_limit(&self) -> Option<u64> {
        let valid_fields =
            QuotaValidFlags::from_bits_truncate(self.0.dqb_valid);
        if valid_fields.contains(QuotaValidFlags::QIF_ILIMITS) {
            Some(self.0.dqb_ihardlimit)
        } else {
            None
        }
    }

    /// Set the maximum number of allocated inodes.
    pub fn set_inodes_hard_limit(&mut self, limit: u64) {
        self.0.dqb_ihardlimit = limit;
    }

    /// Preferred inode limit
    pub fn inodes_soft_limit(&self) -> Option<u64> {
        let valid_fields =
            QuotaValidFlags::from_bits_truncate(self.0.dqb_valid);
        if valid_fields.contains(QuotaValidFlags::QIF_ILIMITS) {
            Some(self.0.dqb_isoftlimit)
        } else {
            None
        }
    }

    /// Set the preferred limit of allocated inodes.
    pub fn set_inodes_soft_limit(&mut self, limit: u64) {
        self.0.dqb_isoftlimit = limit;
    }

    /// Current number of allocated inodes.
    pub fn allocated_inodes(&self) -> Option<u64> {
        let valid_fields =
            QuotaValidFlags::from_bits_truncate(self.0.dqb_valid);
        if valid_fields.contains(QuotaValidFlags::QIF_INODES) {
            Some(self.0.dqb_curinodes)
        } else {
            None
        }
    }

    /// Time limit for excessive disk use.
    pub fn block_time_limit(&self) -> Option<u64> {
        let valid_fields =
            QuotaValidFlags::from_bits_truncate(self.0.dqb_valid);
        if valid_fields.contains(QuotaValidFlags::QIF_BTIME) {
            Some(self.0.dqb_btime)
        } else {
            None
        }
    }

    /// Set the time limit for excessive disk use.
    pub fn set_block_time_limit(&mut self, limit: u64) {
        self.0.dqb_btime = limit;
    }

    /// Time limit for excessive files.
    pub fn inode_time_limit(&self) -> Option<u64> {
        let valid_fields =
            QuotaValidFlags::from_bits_truncate(self.0.dqb_valid);
        if valid_fields.contains(QuotaValidFlags::QIF_ITIME) {
            Some(self.0.dqb_itime)
        } else {
            None
        }
    }

    /// Set the time limit for excessive files.
    pub fn set_inode_time_limit(&mut self, limit: u64) {
        self.0.dqb_itime = limit;
    }
}

fn quotactl<P: ?Sized + NixPath>(
    cmd: QuotaCmd,
    special: Option<&P>,
    id: c_int,
    addr: *mut c_char,
) -> Result<()> {
    unsafe {
        Errno::clear();
        let res = match special {
            Some(dev) => dev.with_nix_path(|path| {
                libc::quotactl(cmd.as_int(), path.as_ptr(), id, addr)
            }),
            None => Ok(libc::quotactl(cmd.as_int(), ptr::null(), id, addr)),
        }?;

        Errno::result(res).map(drop)
    }
}

/// Turn on disk quotas for a block device.
pub fn quotactl_on<P: ?Sized + NixPath>(
    which: QuotaType,
    special: &P,
    format: QuotaFmt,
    quota_file: &P,
) -> Result<()> {
    quota_file.with_nix_path(|path| {
        let mut path_copy = path.to_bytes_with_nul().to_owned();
        let p: *mut c_char = path_copy.as_mut_ptr().cast();
        quotactl(
            QuotaCmd(QuotaSubCmd::Q_QUOTAON, which),
            Some(special),
            format as c_int,
            p,
        )
    })?
}

/// Disable disk quotas for a block device.
pub fn quotactl_off<P: ?Sized + NixPath>(
    which: QuotaType,
    special: &P,
) -> Result<()> {
    quotactl(
        QuotaCmd(QuotaSubCmd::Q_QUOTAOFF, which),
        Some(special),
        0,
        ptr::null_mut(),
    )
}

/// Update the on-disk copy of quota usages for a filesystem.
///
/// If `special` is `None`, then all file systems with active quotas are sync'd.
pub fn quotactl_sync<P: ?Sized + NixPath>(
    which: QuotaType,
    special: Option<&P>,
) -> Result<()> {
    quotactl(
        QuotaCmd(QuotaSubCmd::Q_SYNC, which),
        special,
        0,
        ptr::null_mut(),
    )
}

/// Get disk quota limits and current usage for the given user/group id.
pub fn quotactl_get<P: ?Sized + NixPath>(
    which: QuotaType,
    special: &P,
    id: c_int,
) -> Result<Dqblk> {
    let mut dqblk = mem::MaybeUninit::<libc::dqblk>::uninit();
    quotactl(
        QuotaCmd(QuotaSubCmd::Q_GETQUOTA, which),
        Some(special),
        id,
        dqblk.as_mut_ptr().cast(),
    )?;
    Ok(unsafe { Dqblk(dqblk.assume_init()) })
}

/// Configure quota values for the specified fields for a given user/group id.
pub fn quotactl_set<P: ?Sized + NixPath>(
    which: QuotaType,
    special: &P,
    id: c_int,
    dqblk: &Dqblk,
    fields: QuotaValidFlags,
) -> Result<()> {
    let mut dqblk_copy = *dqblk;
    dqblk_copy.0.dqb_valid = fields.bits();
    quotactl(
        QuotaCmd(QuotaSubCmd::Q_SETQUOTA, which),
        Some(special),
        id,
        &mut dqblk_copy as *mut _ as *mut c_char,
    )
}
