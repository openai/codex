use crate::{Errno, NixPath, Result};
use libc::c_int;

libc_bitflags!(
    /// Used with [`mount()`] and [`unmount()`].
    pub struct MntFlags: c_int {
        /// Do not interpret special files on the filesystem.
        MNT_NODEV;
        /// Enable data protection on the filesystem if the filesystem is configured for it.
        MNT_CPROTECT;
        /// file system is quarantined
        MNT_QUARANTINE;
        /// filesystem is stored locally
        MNT_LOCAL;
        /// quotas are enabled on filesystem
        MNT_QUOTA;
        /// identifies the root filesystem
        MNT_ROOTFS;
        /// file system is not appropriate path to user data
        MNT_DONTBROWSE;
        /// VFS will ignore ownership information on filesystem objects
        MNT_IGNORE_OWNERSHIP;
        /// filesystem was mounted by automounter
        MNT_AUTOMOUNTED;
        /// filesystem is journaled
        MNT_JOURNALED;
        /// Don't allow user extended attributes
        MNT_NOUSERXATTR;
        /// filesystem should defer writes
        MNT_DEFWRITE;
        /// don't block unmount if not responding
        MNT_NOBLOCK;
        /// file system is exported
        MNT_EXPORTED;
        /// file system written asynchronously
        MNT_ASYNC;
        /// Force a read-write mount even if the file system appears to be
        /// unclean.
        MNT_FORCE;
        /// MAC support for objects.
        MNT_MULTILABEL;
        /// Do not update access times.
        MNT_NOATIME;
        /// Disallow program execution.
        MNT_NOEXEC;
        /// Do not honor setuid or setgid bits on files when executing them.
        MNT_NOSUID;
        /// Mount read-only.
        MNT_RDONLY;
        /// Causes the vfs subsystem to update its data structures pertaining to
        /// the specified already mounted file system.
        MNT_RELOAD;
        /// Create a snapshot of the file system.
        MNT_SNAPSHOT;
        /// All I/O to the file system should be done synchronously.
        MNT_SYNCHRONOUS;
        /// Union with underlying fs.
        MNT_UNION;
        /// Indicates that the mount command is being applied to an already
        /// mounted file system.
        MNT_UPDATE;
    }
);

/// Mount a file system.
///
/// # Arguments
/// - `source`  -   Specifies the file system.  e.g. `/dev/sd0`.
/// - `target` -    Specifies the destination.  e.g. `/mnt`.
/// - `flags` -     Optional flags controlling the mount.
/// - `data` -      Optional file system specific data.
///
/// # see also
/// [`mount`](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/mount.2.html)
pub fn mount<
    P1: ?Sized + NixPath,
    P2: ?Sized + NixPath,
    P3: ?Sized + NixPath,
>(
    source: &P1,
    target: &P2,
    flags: MntFlags,
    data: Option<&P3>,
) -> Result<()> {
    let res = source.with_nix_path(|s| {
        target.with_nix_path(|t| {
            crate::with_opt_nix_path(data, |d| unsafe {
                libc::mount(
                    s.as_ptr(),
                    t.as_ptr(),
                    flags.bits(),
                    d.cast_mut().cast(),
                )
            })
        })
    })???;

    Errno::result(res).map(drop)
}

/// Umount the file system mounted at `target`.
pub fn unmount<P>(target: &P, flags: MntFlags) -> Result<()>
where
    P: ?Sized + NixPath,
{
    let res = target.with_nix_path(|cstr| unsafe {
        libc::unmount(cstr.as_ptr(), flags.bits())
    })?;

    Errno::result(res).map(drop)
}
