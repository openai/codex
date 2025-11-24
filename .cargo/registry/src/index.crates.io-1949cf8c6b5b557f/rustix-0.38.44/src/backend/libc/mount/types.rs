use crate::backend::c;
use bitflags::bitflags;

#[cfg(linux_kernel)]
bitflags! {
    /// `MS_*` constants for use with [`mount`].
    ///
    /// [`mount`]: crate::mount::mount
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct MountFlags: c::c_ulong {
        /// `MS_BIND`
        const BIND = c::MS_BIND;

        /// `MS_DIRSYNC`
        const DIRSYNC = c::MS_DIRSYNC;

        /// `MS_LAZYTIME`
        const LAZYTIME = c::MS_LAZYTIME;

        /// `MS_MANDLOCK`
        #[doc(alias = "MANDLOCK")]
        const PERMIT_MANDATORY_FILE_LOCKING = c::MS_MANDLOCK;

        /// `MS_NOATIME`
        const NOATIME = c::MS_NOATIME;

        /// `MS_NODEV`
        const NODEV = c::MS_NODEV;

        /// `MS_NODIRATIME`
        const NODIRATIME = c::MS_NODIRATIME;

        /// `MS_NOEXEC`
        const NOEXEC = c::MS_NOEXEC;

        /// `MS_NOSUID`
        const NOSUID = c::MS_NOSUID;

        /// `MS_RDONLY`
        const RDONLY = c::MS_RDONLY;

        /// `MS_REC`
        const REC = c::MS_REC;

        /// `MS_RELATIME`
        const RELATIME = c::MS_RELATIME;

        /// `MS_SILENT`
        const SILENT = c::MS_SILENT;

        /// `MS_STRICTATIME`
        const STRICTATIME = c::MS_STRICTATIME;

        /// `MS_SYNCHRONOUS`
        const SYNCHRONOUS = c::MS_SYNCHRONOUS;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[cfg(linux_kernel)]
bitflags! {
    /// `MNT_*` constants for use with [`unmount`].
    ///
    /// [`unmount`]: crate::mount::unmount
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct UnmountFlags: u32 {
        /// `MNT_FORCE`
        const FORCE = bitcast!(c::MNT_FORCE);
        /// `MNT_DETACH`
        const DETACH = bitcast!(c::MNT_DETACH);
        /// `MNT_EXPIRE`
        const EXPIRE = bitcast!(c::MNT_EXPIRE);
        /// `UMOUNT_NOFOLLOW`
        const NOFOLLOW = bitcast!(c::UMOUNT_NOFOLLOW);

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[cfg(feature = "mount")]
#[cfg(linux_kernel)]
bitflags! {
    /// `FSOPEN_*` constants for use with [`fsopen`].
    ///
    /// [`fsopen`]: crate::mount::fsopen
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct FsOpenFlags: c::c_uint {
        /// `FSOPEN_CLOEXEC`
        const FSOPEN_CLOEXEC = 0x0000_0001;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[cfg(feature = "mount")]
#[cfg(linux_kernel)]
bitflags! {
    /// `FSMOUNT_*` constants for use with [`fsmount`].
    ///
    /// [`fsmount`]: crate::mount::fsmount
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct FsMountFlags: c::c_uint {
        /// `FSMOUNT_CLOEXEC`
        const FSMOUNT_CLOEXEC = 0x0000_0001;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

/// `FSCONFIG_*` constants for use with the `fsconfig` syscall.
#[cfg(feature = "mount")]
#[cfg(linux_kernel)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
pub(crate) enum FsConfigCmd {
    /// `FSCONFIG_SET_FLAG`
    SetFlag = 0,

    /// `FSCONFIG_SET_STRING`
    SetString = 1,

    /// `FSCONFIG_SET_BINARY`
    SetBinary = 2,

    /// `FSCONFIG_SET_PATH`
    SetPath = 3,

    /// `FSCONFIG_SET_PATH_EMPTY`
    SetPathEmpty = 4,

    /// `FSCONFIG_SET_FD`
    SetFd = 5,

    /// `FSCONFIG_CMD_CREATE`
    Create = 6,

    /// `FSCONFIG_CMD_RECONFIGURE`
    Reconfigure = 7,
}

#[cfg(feature = "mount")]
#[cfg(linux_kernel)]
bitflags! {
    /// `MOUNT_ATTR_*` constants for use with [`fsmount`].
    ///
    /// [`fsmount`]: crate::mount::fsmount
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct MountAttrFlags: c::c_uint {
        /// `MOUNT_ATTR_RDONLY`
        const MOUNT_ATTR_RDONLY = 0x0000_0001;

        /// `MOUNT_ATTR_NOSUID`
        const MOUNT_ATTR_NOSUID = 0x0000_0002;

        /// `MOUNT_ATTR_NODEV`
        const MOUNT_ATTR_NODEV = 0x0000_0004;

        /// `MOUNT_ATTR_NOEXEC`
        const MOUNT_ATTR_NOEXEC = 0x0000_0008;

        /// `MOUNT_ATTR__ATIME`
        const MOUNT_ATTR__ATIME = 0x0000_0070;

        /// `MOUNT_ATTR_RELATIME`
        const MOUNT_ATTR_RELATIME = 0x0000_0000;

        /// `MOUNT_ATTR_NOATIME`
        const MOUNT_ATTR_NOATIME = 0x0000_0010;

        /// `MOUNT_ATTR_STRICTATIME`
        const MOUNT_ATTR_STRICTATIME = 0x0000_0020;

        /// `MOUNT_ATTR_NODIRATIME`
        const MOUNT_ATTR_NODIRATIME = 0x0000_0080;

        /// `MOUNT_ATTR_NOUSER`
        const MOUNT_ATTR_IDMAP = 0x0010_0000;

        /// `MOUNT_ATTR__ATIME_FLAGS`
        const MOUNT_ATTR_NOSYMFOLLOW = 0x0020_0000;

        /// `MOUNT_ATTR__ATIME_FLAGS`
        const MOUNT_ATTR_SIZE_VER0 = 32;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[cfg(feature = "mount")]
#[cfg(linux_kernel)]
bitflags! {
    /// `MOVE_MOUNT_*` constants for use with [`move_mount`].
    ///
    /// [`move_mount`]: crate::mount::move_mount
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct MoveMountFlags: c::c_uint {
        /// `MOVE_MOUNT_F_EMPTY_PATH`
        const MOVE_MOUNT_F_SYMLINKS = 0x0000_0001;

        /// `MOVE_MOUNT_F_AUTOMOUNTS`
        const MOVE_MOUNT_F_AUTOMOUNTS = 0x0000_0002;

        /// `MOVE_MOUNT_F_EMPTY_PATH`
        const MOVE_MOUNT_F_EMPTY_PATH = 0x0000_0004;

        /// `MOVE_MOUNT_T_SYMLINKS`
        const MOVE_MOUNT_T_SYMLINKS = 0x0000_0010;

        /// `MOVE_MOUNT_T_AUTOMOUNTS`
        const MOVE_MOUNT_T_AUTOMOUNTS = 0x0000_0020;

        /// `MOVE_MOUNT_T_EMPTY_PATH`
        const MOVE_MOUNT_T_EMPTY_PATH = 0x0000_0040;

        /// `MOVE_MOUNT__MASK`
        const MOVE_MOUNT_SET_GROUP = 0x0000_0100;

        /// `MOVE_MOUNT_BENEATH` (since Linux 6.5)
        const MOVE_MOUNT_BENEATH = 0x0000_0200;

        /// `MOVE_MOUNT__MASK`
        const MOVE_MOUNT__MASK = 0x0000_0377;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[cfg(feature = "mount")]
#[cfg(linux_kernel)]
bitflags! {
    /// `OPENTREE_*` constants for use with [`open_tree`].
    ///
    /// [`open_tree`]: crate::mount::open_tree
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct OpenTreeFlags: c::c_uint {
        /// `OPENTREE_CLONE`
        const OPEN_TREE_CLONE = 1;

        /// `OPENTREE_CLOEXEC`
        const OPEN_TREE_CLOEXEC = c::O_CLOEXEC as c::c_uint;

        /// `AT_EMPTY_PATH`
        const AT_EMPTY_PATH = c::AT_EMPTY_PATH as c::c_uint;

        /// `AT_NO_AUTOMOUNT`
        const AT_NO_AUTOMOUNT = c::AT_NO_AUTOMOUNT as c::c_uint;

        /// `AT_RECURSIVE`
        const AT_RECURSIVE = c::AT_RECURSIVE as c::c_uint;

        /// `AT_SYMLINK_NOFOLLOW`
        const AT_SYMLINK_NOFOLLOW = c::AT_SYMLINK_NOFOLLOW as c::c_uint;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[cfg(feature = "mount")]
#[cfg(linux_kernel)]
bitflags! {
    /// `FSPICK_*` constants for use with [`fspick`].
    ///
    /// [`fspick`]: crate::mount::fspick
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct FsPickFlags: c::c_uint {
        /// `FSPICK_CLOEXEC`
        const FSPICK_CLOEXEC = 0x0000_0001;

        /// `FSPICK_SYMLINK_NOFOLLOW`
        const FSPICK_SYMLINK_NOFOLLOW = 0x0000_0002;

        /// `FSPICK_NO_AUTOMOUNT`
        const FSPICK_NO_AUTOMOUNT = 0x0000_0004;

        /// `FSPICK_EMPTY_PATH`
        const FSPICK_EMPTY_PATH = 0x0000_0008;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[cfg(linux_kernel)]
bitflags! {
    /// `MS_*` constants for use with [`mount_change`].
    ///
    /// [`mount_change`]: crate::mount::mount_change
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct MountPropagationFlags: c::c_ulong {
        /// `MS_SILENT`
        const SILENT = c::MS_SILENT;
        /// `MS_SHARED`
        const SHARED = c::MS_SHARED;
        /// `MS_PRIVATE`
        const PRIVATE = c::MS_PRIVATE;
        /// `MS_SLAVE`
        const SLAVE = c::MS_SLAVE;
        /// `MS_UNBINDABLE`
        const UNBINDABLE = c::MS_UNBINDABLE;
        /// `MS_REC`
        const REC = c::MS_REC;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[cfg(linux_kernel)]
bitflags! {
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub(crate) struct InternalMountFlags: c::c_ulong {
        const REMOUNT = c::MS_REMOUNT;
        const MOVE = c::MS_MOVE;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[cfg(linux_kernel)]
pub(crate) struct MountFlagsArg(pub(crate) c::c_ulong);
