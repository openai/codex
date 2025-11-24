use crate::ffi;
use bitflags::bitflags;

bitflags! {
    /// `MS_*` constants for use with [`mount`].
    ///
    /// [`mount`]: crate::mount::mount
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct MountFlags: ffi::c_uint {
        /// `MS_BIND`
        const BIND = linux_raw_sys::general::MS_BIND;

        /// `MS_DIRSYNC`
        const DIRSYNC = linux_raw_sys::general::MS_DIRSYNC;

        /// `MS_LAZYTIME`
        const LAZYTIME = linux_raw_sys::general::MS_LAZYTIME;

        /// `MS_MANDLOCK`
        #[doc(alias = "MANDLOCK")]
        const PERMIT_MANDATORY_FILE_LOCKING = linux_raw_sys::general::MS_MANDLOCK;

        /// `MS_NOATIME`
        const NOATIME = linux_raw_sys::general::MS_NOATIME;

        /// `MS_NODEV`
        const NODEV = linux_raw_sys::general::MS_NODEV;

        /// `MS_NODIRATIME`
        const NODIRATIME = linux_raw_sys::general::MS_NODIRATIME;

        /// `MS_NOEXEC`
        const NOEXEC = linux_raw_sys::general::MS_NOEXEC;

        /// `MS_NOSUID`
        const NOSUID = linux_raw_sys::general::MS_NOSUID;

        /// `MS_RDONLY`
        const RDONLY = linux_raw_sys::general::MS_RDONLY;

        /// `MS_REC`
        const REC = linux_raw_sys::general::MS_REC;

        /// `MS_RELATIME`
        const RELATIME = linux_raw_sys::general::MS_RELATIME;

        /// `MS_SILENT`
        const SILENT = linux_raw_sys::general::MS_SILENT;

        /// `MS_STRICTATIME`
        const STRICTATIME = linux_raw_sys::general::MS_STRICTATIME;

        /// `MS_SYNCHRONOUS`
        const SYNCHRONOUS = linux_raw_sys::general::MS_SYNCHRONOUS;

        /// `MS_NOSYMFOLLOW`
        const NOSYMFOLLOW = linux_raw_sys::general::MS_NOSYMFOLLOW;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags! {
    /// `MNT_*` constants for use with [`unmount`].
    ///
    /// [`unmount`]: crate::mount::unmount
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct UnmountFlags: ffi::c_uint {
        /// `MNT_FORCE`
        const FORCE = linux_raw_sys::general::MNT_FORCE;
        /// `MNT_DETACH`
        const DETACH = linux_raw_sys::general::MNT_DETACH;
        /// `MNT_EXPIRE`
        const EXPIRE = linux_raw_sys::general::MNT_EXPIRE;
        /// `UMOUNT_NOFOLLOW`
        const NOFOLLOW = linux_raw_sys::general::UMOUNT_NOFOLLOW;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags! {
    /// `FSOPEN_*` constants for use with [`fsopen`].
    ///
    /// [`fsopen`]: crate::mount::fsopen
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct FsOpenFlags: ffi::c_uint {
        /// `FSOPEN_CLOEXEC`
        const FSOPEN_CLOEXEC = linux_raw_sys::general::FSOPEN_CLOEXEC;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags! {
    /// `FSMOUNT_*` constants for use with [`fsmount`].
    ///
    /// [`fsmount`]: crate::mount::fsmount
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct FsMountFlags: ffi::c_uint {
        /// `FSMOUNT_CLOEXEC`
        const FSMOUNT_CLOEXEC = linux_raw_sys::general::FSMOUNT_CLOEXEC;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

/// `FSCONFIG_*` constants for use with the `fsconfig` syscall.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
pub(crate) enum FsConfigCmd {
    /// `FSCONFIG_SET_FLAG`
    SetFlag = linux_raw_sys::general::fsconfig_command::FSCONFIG_SET_FLAG as u32,

    /// `FSCONFIG_SET_STRING`
    SetString = linux_raw_sys::general::fsconfig_command::FSCONFIG_SET_STRING as u32,

    /// `FSCONFIG_SET_BINARY`
    SetBinary = linux_raw_sys::general::fsconfig_command::FSCONFIG_SET_BINARY as u32,

    /// `FSCONFIG_SET_PATH`
    SetPath = linux_raw_sys::general::fsconfig_command::FSCONFIG_SET_PATH as u32,

    /// `FSCONFIG_SET_PATH_EMPTY`
    SetPathEmpty = linux_raw_sys::general::fsconfig_command::FSCONFIG_SET_PATH_EMPTY as u32,

    /// `FSCONFIG_SET_FD`
    SetFd = linux_raw_sys::general::fsconfig_command::FSCONFIG_SET_FD as u32,

    /// `FSCONFIG_CMD_CREATE`
    Create = linux_raw_sys::general::fsconfig_command::FSCONFIG_CMD_CREATE as u32,

    /// `FSCONFIG_CMD_RECONFIGURE`
    Reconfigure = linux_raw_sys::general::fsconfig_command::FSCONFIG_CMD_RECONFIGURE as u32,

    /// `FSCONFIG_CMD_CREATE_EXCL` (since Linux 6.6)
    CreateExclusive = linux_raw_sys::general::fsconfig_command::FSCONFIG_CMD_CREATE_EXCL as u32,
}

bitflags! {
    /// `MOUNT_ATTR_*` constants for use with [`fsmount`].
    ///
    /// [`fsmount`]: crate::mount::fsmount
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct MountAttrFlags: ffi::c_uint {
        /// `MOUNT_ATTR_RDONLY`
        const MOUNT_ATTR_RDONLY = linux_raw_sys::general::MOUNT_ATTR_RDONLY;

        /// `MOUNT_ATTR_NOSUID`
        const MOUNT_ATTR_NOSUID = linux_raw_sys::general::MOUNT_ATTR_NOSUID;

        /// `MOUNT_ATTR_NODEV`
        const MOUNT_ATTR_NODEV = linux_raw_sys::general::MOUNT_ATTR_NODEV;

        /// `MOUNT_ATTR_NOEXEC`
        const MOUNT_ATTR_NOEXEC = linux_raw_sys::general::MOUNT_ATTR_NOEXEC;

        /// `MOUNT_ATTR__ATIME`
        const MOUNT_ATTR__ATIME = linux_raw_sys::general::MOUNT_ATTR__ATIME;

        /// `MOUNT_ATTR_RELATIME`
        const MOUNT_ATTR_RELATIME = linux_raw_sys::general::MOUNT_ATTR_RELATIME;

        /// `MOUNT_ATTR_NOATIME`
        const MOUNT_ATTR_NOATIME = linux_raw_sys::general::MOUNT_ATTR_NOATIME;

        /// `MOUNT_ATTR_STRICTATIME`
        const MOUNT_ATTR_STRICTATIME = linux_raw_sys::general::MOUNT_ATTR_STRICTATIME;

        /// `MOUNT_ATTR_NODIRATIME`
        const MOUNT_ATTR_NODIRATIME = linux_raw_sys::general::MOUNT_ATTR_NODIRATIME;

        /// `MOUNT_ATTR_NOUSER`
        const MOUNT_ATTR_IDMAP = linux_raw_sys::general::MOUNT_ATTR_IDMAP;

        /// `MOUNT_ATTR__ATIME_FLAGS`
        const MOUNT_ATTR_NOSYMFOLLOW = linux_raw_sys::general::MOUNT_ATTR_NOSYMFOLLOW;

        /// `MOUNT_ATTR__ATIME_FLAGS`
        const MOUNT_ATTR_SIZE_VER0 = linux_raw_sys::general::MOUNT_ATTR_SIZE_VER0;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags! {
    /// `MOVE_MOUNT_*` constants for use with [`move_mount`].
    ///
    /// [`move_mount`]: crate::mount::move_mount
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct MoveMountFlags: ffi::c_uint {
        /// `MOVE_MOUNT_F_EMPTY_PATH`
        const MOVE_MOUNT_F_SYMLINKS = linux_raw_sys::general::MOVE_MOUNT_F_SYMLINKS;

        /// `MOVE_MOUNT_F_AUTOMOUNTS`
        const MOVE_MOUNT_F_AUTOMOUNTS = linux_raw_sys::general::MOVE_MOUNT_F_AUTOMOUNTS;

        /// `MOVE_MOUNT_F_EMPTY_PATH`
        const MOVE_MOUNT_F_EMPTY_PATH = linux_raw_sys::general::MOVE_MOUNT_F_EMPTY_PATH;

        /// `MOVE_MOUNT_T_SYMLINKS`
        const MOVE_MOUNT_T_SYMLINKS = linux_raw_sys::general::MOVE_MOUNT_T_SYMLINKS;

        /// `MOVE_MOUNT_T_AUTOMOUNTS`
        const MOVE_MOUNT_T_AUTOMOUNTS = linux_raw_sys::general::MOVE_MOUNT_T_AUTOMOUNTS;

        /// `MOVE_MOUNT_T_EMPTY_PATH`
        const MOVE_MOUNT_T_EMPTY_PATH = linux_raw_sys::general::MOVE_MOUNT_T_EMPTY_PATH;

        /// `MOVE_MOUNT__MASK`
        const MOVE_MOUNT_SET_GROUP = linux_raw_sys::general::MOVE_MOUNT_SET_GROUP;

        /// `MOVE_MOUNT_BENEATH` (since Linux 6.5)
        const MOVE_MOUNT_BENEATH = linux_raw_sys::general::MOVE_MOUNT_BENEATH;

        /// `MOVE_MOUNT__MASK`
        const MOVE_MOUNT__MASK = linux_raw_sys::general::MOVE_MOUNT__MASK;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags! {
    /// `OPENTREE_*` constants for use with [`open_tree`].
    ///
    /// [`open_tree`]: crate::mount::open_tree
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct OpenTreeFlags: ffi::c_uint {
        /// `OPENTREE_CLONE`
        const OPEN_TREE_CLONE = linux_raw_sys::general::OPEN_TREE_CLONE;

        /// `OPENTREE_CLOEXEC`
        const OPEN_TREE_CLOEXEC = linux_raw_sys::general::OPEN_TREE_CLOEXEC;

        /// `AT_EMPTY_PATH`
        const AT_EMPTY_PATH = linux_raw_sys::general::AT_EMPTY_PATH;

        /// `AT_NO_AUTOMOUNT`
        const AT_NO_AUTOMOUNT = linux_raw_sys::general::AT_NO_AUTOMOUNT;

        /// `AT_RECURSIVE`
        const AT_RECURSIVE = linux_raw_sys::general::AT_RECURSIVE;

        /// `AT_SYMLINK_NOFOLLOW`
        const AT_SYMLINK_NOFOLLOW = linux_raw_sys::general::AT_SYMLINK_NOFOLLOW;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags! {
    /// `FSPICK_*` constants for use with [`fspick`].
    ///
    /// [`fspick`]: crate::mount::fspick
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct FsPickFlags: ffi::c_uint {
        /// `FSPICK_CLOEXEC`
        const FSPICK_CLOEXEC = linux_raw_sys::general::FSPICK_CLOEXEC;

        /// `FSPICK_SYMLINK_NOFOLLOW`
        const FSPICK_SYMLINK_NOFOLLOW = linux_raw_sys::general::FSPICK_SYMLINK_NOFOLLOW;

        /// `FSPICK_NO_AUTOMOUNT`
        const FSPICK_NO_AUTOMOUNT = linux_raw_sys::general::FSPICK_NO_AUTOMOUNT;

        /// `FSPICK_EMPTY_PATH`
        const FSPICK_EMPTY_PATH = linux_raw_sys::general::FSPICK_EMPTY_PATH;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags! {
    /// `MS_*` constants for use with [`mount_change`].
    ///
    /// [`mount_change`]: crate::mount::mount_change
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct MountPropagationFlags: ffi::c_uint {
        /// `MS_SILENT`
        const SILENT = linux_raw_sys::general::MS_SILENT;
        /// `MS_SHARED`
        const SHARED = linux_raw_sys::general::MS_SHARED;
        /// `MS_PRIVATE`
        const PRIVATE = linux_raw_sys::general::MS_PRIVATE;
        /// Mark a mount as a downstream of its current peer group.
        ///
        /// Mount and unmount events propagate from the upstream peer group
        /// into the downstream.
        ///
        /// In Linux documentation, this flag is named `MS_SLAVE`, and the
        /// concepts of “upstream” and “downstream” are called
        /// “master” and “slave”.
        #[doc(alias = "SLAVE")]
        const DOWNSTREAM = linux_raw_sys::general::MS_SLAVE;
        /// `MS_UNBINDABLE`
        const UNBINDABLE = linux_raw_sys::general::MS_UNBINDABLE;
        /// `MS_REC`
        const REC = linux_raw_sys::general::MS_REC;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags! {
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub(crate) struct InternalMountFlags: ffi::c_uint {
        const REMOUNT = linux_raw_sys::general::MS_REMOUNT;
        const MOVE = linux_raw_sys::general::MS_MOVE;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[repr(transparent)]
pub(crate) struct MountFlagsArg(pub(crate) ffi::c_uint);
