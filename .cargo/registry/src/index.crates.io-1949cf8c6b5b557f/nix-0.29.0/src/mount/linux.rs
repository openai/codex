use crate::errno::Errno;
use crate::{NixPath, Result};
use libc::{self, c_int, c_ulong};

libc_bitflags!(
    /// Used with [`mount`].
    pub struct MsFlags: c_ulong {
        /// Mount read-only
        MS_RDONLY;
        /// Ignore suid and sgid bits
        MS_NOSUID;
        /// Disallow access to device special files
        MS_NODEV;
        /// Disallow program execution
        MS_NOEXEC;
        /// Writes are synced at once
        MS_SYNCHRONOUS;
        /// Alter flags of a mounted FS
        MS_REMOUNT;
        /// Allow mandatory locks on a FS
        MS_MANDLOCK;
        /// Directory modifications are synchronous
        MS_DIRSYNC;
        /// Do not update access times
        MS_NOATIME;
        /// Do not update directory access times
        MS_NODIRATIME;
        /// Linux 2.4.0 - Bind directory at different place
        MS_BIND;
        /// Move an existing mount to a new location
        MS_MOVE;
        /// Used to create a recursive bind mount.
        MS_REC;
        /// Suppress the display of certain kernel warning messages.
        MS_SILENT;
        /// VFS does not apply the umask
        MS_POSIXACL;
        /// The resulting mount cannot subsequently be bind mounted.
        MS_UNBINDABLE;
        /// Make this mount point private.
        MS_PRIVATE;
        /// If this is a shared mount point that is a member of a peer group
        /// that  contains  other  members, convert it to a slave mount.
        MS_SLAVE;
        /// Make  this mount point shared.
        MS_SHARED;
        /// When a file on this filesystem is accessed,  update  the  file's
        /// last  access  time (atime) only if the current value of atime is
        /// less than or equal to the file's last modification time  (mtime) or
        /// last  status change time (ctime).
        MS_RELATIME;
        /// Mount request came from within the kernel
        #[deprecated(since = "0.27.0", note = "Should only be used in-kernel")]
        MS_KERNMOUNT;
        /// Update inode I_version field
        MS_I_VERSION;
        /// Always  update  the  last access time (atime) when files on this
        /// filesystem are accessed.
        MS_STRICTATIME;
        /// Reduce on-disk updates of inode timestamps (atime, mtime, ctime) by
        /// maintaining these changes only in memory.
        MS_LAZYTIME;
        #[deprecated(since = "0.27.0", note = "Should only be used in-kernel")]
        #[allow(missing_docs)]  // Not documented in Linux
        MS_ACTIVE;
        #[deprecated(since = "0.27.0", note = "Should only be used in-kernel")]
        #[allow(missing_docs)]  // Not documented in Linux
        MS_NOUSER;
        #[allow(missing_docs)]  // Not documented in Linux; possibly kernel-only
        MS_RMT_MASK;
        #[allow(missing_docs)]  // Not documented in Linux; possibly kernel-only
        MS_MGC_VAL;
        #[allow(missing_docs)]  // Not documented in Linux; possibly kernel-only
        MS_MGC_MSK;
    }
);

libc_bitflags!(
    /// Used with [`umount2].
    pub struct MntFlags: c_int {
        /// Attempt to unmount even if still in use, aborting pending requests.
        MNT_FORCE;
        /// Lazy unmount.  Disconnect the file system immediately, but don't
        /// actually unmount it until it ceases to be busy.
        MNT_DETACH;
        /// Mark the mount point as expired.
        MNT_EXPIRE;
        /// Don't dereference `target` if it is a symlink.
        UMOUNT_NOFOLLOW;
    }
);

/// Mount a file system.
///
/// # Arguments
/// - `source`  -   Specifies the file system.  e.g. `/dev/sd0`.
/// - `target` -    Specifies the destination.  e.g. `/mnt`.
/// - `fstype` -    The file system type, e.g. `ext4`.
/// - `flags` -     Optional flags controlling the mount.
/// - `data` -      Optional file system specific data.
///
/// # See Also
/// [`mount`](https://man7.org/linux/man-pages/man2/mount.2.html)
pub fn mount<
    P1: ?Sized + NixPath,
    P2: ?Sized + NixPath,
    P3: ?Sized + NixPath,
    P4: ?Sized + NixPath,
>(
    source: Option<&P1>,
    target: &P2,
    fstype: Option<&P3>,
    flags: MsFlags,
    data: Option<&P4>,
) -> Result<()> {
    let res = crate::with_opt_nix_path(source, |s| {
        target.with_nix_path(|t| {
            crate::with_opt_nix_path(fstype, |ty| {
                crate::with_opt_nix_path(data, |d| unsafe {
                    libc::mount(
                        s,
                        t.as_ptr(),
                        ty,
                        flags.bits(),
                        d as *const libc::c_void,
                    )
                })
            })
        })
    })????;

    Errno::result(res).map(drop)
}

/// Unmount the file system mounted at `target`.
pub fn umount<P: ?Sized + NixPath>(target: &P) -> Result<()> {
    let res =
        target.with_nix_path(|cstr| unsafe { libc::umount(cstr.as_ptr()) })?;

    Errno::result(res).map(drop)
}

/// Unmount the file system mounted at `target`.
///
/// See also [`umount`](https://man7.org/linux/man-pages/man2/umount.2.html)
pub fn umount2<P: ?Sized + NixPath>(target: &P, flags: MntFlags) -> Result<()> {
    let res = target.with_nix_path(|cstr| unsafe {
        libc::umount2(cstr.as_ptr(), flags.bits())
    })?;

    Errno::result(res).map(drop)
}
