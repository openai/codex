use crate::backend::c;
use bitflags::bitflags;

#[cfg(not(any(target_os = "espidf", target_os = "vita")))]
bitflags! {
    /// `*_OK` constants for use with [`accessat`].
    ///
    /// [`accessat`]: fn.accessat.html
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct Access: c::c_int {
        /// `R_OK`
        const READ_OK = c::R_OK;

        /// `W_OK`
        const WRITE_OK = c::W_OK;

        /// `X_OK`
        const EXEC_OK = c::X_OK;

        /// `F_OK`
        const EXISTS = c::F_OK;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[cfg(not(any(target_os = "espidf", target_os = "redox")))]
bitflags! {
    /// `AT_*` constants for use with [`openat`], [`statat`], and other `*at`
    /// functions.
    ///
    /// [`openat`]: crate::fs::openat
    /// [`statat`]: crate::fs::statat
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct AtFlags: u32 {
        /// `AT_SYMLINK_NOFOLLOW`
        const SYMLINK_NOFOLLOW = bitcast!(c::AT_SYMLINK_NOFOLLOW);

        /// `AT_EACCESS`
        #[cfg(not(target_os = "android"))]
        const EACCESS = bitcast!(c::AT_EACCESS);

        /// `AT_REMOVEDIR`
        const REMOVEDIR = bitcast!(c::AT_REMOVEDIR);

        /// `AT_SYMLINK_FOLLOW`
        const SYMLINK_FOLLOW = bitcast!(c::AT_SYMLINK_FOLLOW);

        /// `AT_NO_AUTOMOUNT`
        #[cfg(any(linux_like, target_os = "fuchsia"))]
        const NO_AUTOMOUNT = bitcast!(c::AT_NO_AUTOMOUNT);

        /// `AT_EMPTY_PATH`
        #[cfg(any(
            linux_kernel,
            target_os = "freebsd",
            target_os = "fuchsia",
        ))]
        const EMPTY_PATH = bitcast!(c::AT_EMPTY_PATH);

        /// `AT_RESOLVE_BENEATH`
        #[cfg(target_os = "freebsd")]
        const RESOLVE_BENEATH = bitcast!(c::AT_RESOLVE_BENEATH);

        /// `AT_STATX_SYNC_AS_STAT`
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        const STATX_SYNC_AS_STAT = bitcast!(c::AT_STATX_SYNC_AS_STAT);

        /// `AT_STATX_FORCE_SYNC`
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        const STATX_FORCE_SYNC = bitcast!(c::AT_STATX_FORCE_SYNC);

        /// `AT_STATX_DONT_SYNC`
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        const STATX_DONT_SYNC = bitcast!(c::AT_STATX_DONT_SYNC);

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags! {
    /// `S_I*` constants for use with [`openat`], [`chmodat`], and [`fchmod`].
    ///
    /// [`openat`]: crate::fs::openat
    /// [`chmodat`]: crate::fs::chmodat
    /// [`fchmod`]: crate::fs::fchmod
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct Mode: RawMode {
        /// `S_IRWXU`
        #[cfg(not(any(target_os = "espidf", target_os = "vita")))]
        const RWXU = c::S_IRWXU as RawMode;

        /// `S_IRUSR`
        #[cfg(not(any(target_os = "espidf", target_os = "vita")))]
        const RUSR = c::S_IRUSR as RawMode;

        /// `S_IWUSR`
        #[cfg(not(any(target_os = "espidf", target_os = "vita")))]
        const WUSR = c::S_IWUSR as RawMode;

        /// `S_IXUSR`
        #[cfg(not(any(target_os = "espidf", target_os = "vita")))]
        const XUSR = c::S_IXUSR as RawMode;

        /// `S_IRWXG`
        #[cfg(not(any(target_os = "espidf", target_os = "vita")))]
        const RWXG = c::S_IRWXG as RawMode;

        /// `S_IRGRP`
        #[cfg(not(any(target_os = "espidf", target_os = "vita")))]
        const RGRP = c::S_IRGRP as RawMode;

        /// `S_IWGRP`
        #[cfg(not(any(target_os = "espidf", target_os = "vita")))]
        const WGRP = c::S_IWGRP as RawMode;

        /// `S_IXGRP`
        #[cfg(not(any(target_os = "espidf", target_os = "vita")))]
        const XGRP = c::S_IXGRP as RawMode;

        /// `S_IRWXO`
        #[cfg(not(any(target_os = "espidf", target_os = "vita")))]
        const RWXO = c::S_IRWXO as RawMode;

        /// `S_IROTH`
        #[cfg(not(any(target_os = "espidf", target_os = "vita")))]
        const ROTH = c::S_IROTH as RawMode;

        /// `S_IWOTH`
        #[cfg(not(any(target_os = "espidf", target_os = "vita")))]
        const WOTH = c::S_IWOTH as RawMode;

        /// `S_IXOTH`
        #[cfg(not(any(target_os = "espidf", target_os = "vita")))]
        const XOTH = c::S_IXOTH as RawMode;

        /// `S_ISUID`
        #[cfg(not(any(target_os = "espidf", target_os = "vita")))]
        const SUID = c::S_ISUID as RawMode;

        /// `S_ISGID`
        #[cfg(not(any(target_os = "espidf", target_os = "vita")))]
        const SGID = c::S_ISGID as RawMode;

        /// `S_ISVTX`
        #[cfg(not(any(target_os = "espidf", target_os = "vita")))]
        const SVTX = c::S_ISVTX as RawMode;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[cfg(not(target_os = "espidf"))]
impl Mode {
    /// Construct a `Mode` from the mode bits of the `st_mode` field of a
    /// `Mode`.
    #[inline]
    pub const fn from_raw_mode(st_mode: RawMode) -> Self {
        Self::from_bits_truncate(st_mode & !c::S_IFMT as RawMode)
    }

    /// Construct an `st_mode` value from a `Mode`.
    #[inline]
    pub const fn as_raw_mode(self) -> RawMode {
        self.bits()
    }
}

#[cfg(not(target_os = "espidf"))]
impl From<RawMode> for Mode {
    /// Support conversions from raw mode values to `Mode`.
    ///
    /// ```
    /// use rustix::fs::{Mode, RawMode};
    /// assert_eq!(Mode::from(0o700), Mode::RWXU);
    /// ```
    #[inline]
    fn from(st_mode: RawMode) -> Self {
        Self::from_raw_mode(st_mode)
    }
}

#[cfg(not(target_os = "espidf"))]
impl From<Mode> for RawMode {
    /// Support conversions from `Mode` to raw mode values.
    ///
    /// ```
    /// use rustix::fs::{Mode, RawMode};
    /// assert_eq!(RawMode::from(Mode::RWXU), 0o700);
    /// ```
    #[inline]
    fn from(mode: Mode) -> Self {
        mode.as_raw_mode()
    }
}

bitflags! {
    /// `O_*` constants for use with [`openat`].
    ///
    /// [`openat`]: crate::fs::openat
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct OFlags: u32 {
        /// `O_ACCMODE`
        const ACCMODE = bitcast!(c::O_ACCMODE);

        /// Similar to `ACCMODE`, but just includes the read/write flags, and
        /// no other flags.
        ///
        /// On some platforms, `PATH` may be included in `ACCMODE`, when
        /// sometimes we really just want the read/write bits. Caution is
        /// indicated, as the presence of `PATH` may mean that the read/write
        /// bits don't have their usual meaning.
        const RWMODE = bitcast!(c::O_RDONLY | c::O_WRONLY | c::O_RDWR);

        /// `O_APPEND`
        const APPEND = bitcast!(c::O_APPEND);

        /// `O_CREAT`
        #[doc(alias = "CREAT")]
        const CREATE = bitcast!(c::O_CREAT);

        /// `O_DIRECTORY`
        #[cfg(not(target_os = "espidf"))]
        const DIRECTORY = bitcast!(c::O_DIRECTORY);

        /// `O_DSYNC`
        #[cfg(not(any(target_os = "dragonfly", target_os = "espidf", target_os = "l4re", target_os = "redox", target_os = "vita")))]
        const DSYNC = bitcast!(c::O_DSYNC);

        /// `O_EXCL`
        const EXCL = bitcast!(c::O_EXCL);

        /// `O_FSYNC`
        #[cfg(any(
            bsd,
            all(target_os = "linux", not(target_env = "musl")),
        ))]
        const FSYNC = bitcast!(c::O_FSYNC);

        /// `O_NOFOLLOW`
        #[cfg(not(target_os = "espidf"))]
        const NOFOLLOW = bitcast!(c::O_NOFOLLOW);

        /// `O_NONBLOCK`
        const NONBLOCK = bitcast!(c::O_NONBLOCK);

        /// `O_RDONLY`
        const RDONLY = bitcast!(c::O_RDONLY);

        /// `O_WRONLY`
        const WRONLY = bitcast!(c::O_WRONLY);

        /// `O_RDWR`
        ///
        /// This is not equal to `RDONLY | WRONLY`. It's a distinct flag.
        const RDWR = bitcast!(c::O_RDWR);

        /// `O_NOCTTY`
        #[cfg(not(any(target_os = "espidf", target_os = "l4re", target_os = "redox", target_os = "vita")))]
        const NOCTTY = bitcast!(c::O_NOCTTY);

        /// `O_RSYNC`
        #[cfg(any(
            linux_kernel,
            netbsdlike,
            solarish,
            target_os = "emscripten",
            target_os = "wasi",
        ))]
        const RSYNC = bitcast!(c::O_RSYNC);

        /// `O_SYNC`
        #[cfg(not(any(target_os = "l4re", target_os = "redox")))]
        const SYNC = bitcast!(c::O_SYNC);

        /// `O_TRUNC`
        const TRUNC = bitcast!(c::O_TRUNC);

        /// `O_PATH`
        #[cfg(any(
            linux_kernel,
            target_os = "emscripten",
            target_os = "freebsd",
            target_os = "fuchsia",
            target_os = "redox",
        ))]
        const PATH = bitcast!(c::O_PATH);

        /// `O_CLOEXEC`
        const CLOEXEC = bitcast!(c::O_CLOEXEC);

        /// `O_TMPFILE`
        #[cfg(any(
            linux_kernel,
            target_os = "emscripten",
            target_os = "fuchsia",
        ))]
        const TMPFILE = bitcast!(c::O_TMPFILE);

        /// `O_NOATIME`
        #[cfg(any(
            linux_kernel,
            target_os = "fuchsia",
        ))]
        const NOATIME = bitcast!(c::O_NOATIME);

        /// `O_DIRECT`
        #[cfg(any(
            linux_kernel,
            target_os = "emscripten",
            target_os = "freebsd",
            target_os = "fuchsia",
            target_os = "netbsd",
        ))]
        const DIRECT = bitcast!(c::O_DIRECT);

        /// `O_RESOLVE_BENEATH`
        #[cfg(target_os = "freebsd")]
        const RESOLVE_BENEATH = bitcast!(c::O_RESOLVE_BENEATH);

        /// `O_EMPTY_PATH`
        #[cfg(target_os = "freebsd")]
        const EMPTY_PATH = bitcast!(c::O_EMPTY_PATH);

        /// `O_LARGEFILE`
        ///
        /// Note that rustix and/or libc will automatically set this flag when appropriate on
        /// `open(2)` and friends, thus typical users do not need to care about it.
        /// It will may be reported in return of `fcntl_getfl`, though.
        #[cfg(any(linux_kernel, solarish))]
        const LARGEFILE = bitcast!(c::O_LARGEFILE);

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[cfg(apple)]
bitflags! {
    /// `CLONE_*` constants for use with [`fclonefileat`].
    ///
    /// [`fclonefileat`]: crate::fs::fclonefileat
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct CloneFlags: u32 {
        /// `CLONE_NOFOLLOW`
        const NOFOLLOW = 1;

        /// `CLONE_NOOWNERCOPY`
        const NOOWNERCOPY = 2;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[cfg(apple)]
mod copyfile {
    pub(super) const ACL: u32 = 1 << 0;
    pub(super) const STAT: u32 = 1 << 1;
    pub(super) const XATTR: u32 = 1 << 2;
    pub(super) const DATA: u32 = 1 << 3;
    pub(super) const SECURITY: u32 = STAT | ACL;
    pub(super) const METADATA: u32 = SECURITY | XATTR;
    pub(super) const ALL: u32 = METADATA | DATA;
}

#[cfg(apple)]
bitflags! {
    /// `COPYFILE_*` constants for use with [`fcopyfile`].
    ///
    /// [`fcopyfile`]: crate::fs::fcopyfile
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct CopyfileFlags: c::c_uint {
        /// `COPYFILE_ACL`
        const ACL = copyfile::ACL;

        /// `COPYFILE_STAT`
        const STAT = copyfile::STAT;

        /// `COPYFILE_XATTR`
        const XATTR = copyfile::XATTR;

        /// `COPYFILE_DATA`
        const DATA = copyfile::DATA;

        /// `COPYFILE_SECURITY`
        const SECURITY = copyfile::SECURITY;

        /// `COPYFILE_METADATA`
        const METADATA = copyfile::METADATA;

        /// `COPYFILE_ALL`
        const ALL = copyfile::ALL;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[cfg(linux_kernel)]
bitflags! {
    /// `RESOLVE_*` constants for use with [`openat2`].
    ///
    /// [`openat2`]: crate::fs::openat2
    #[repr(transparent)]
    #[derive(Default, Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct ResolveFlags: u64 {
        /// `RESOLVE_NO_XDEV`
        const NO_XDEV = 0x01;

        /// `RESOLVE_NO_MAGICLINKS`
        const NO_MAGICLINKS = 0x02;

        /// `RESOLVE_NO_SYMLINKS`
        const NO_SYMLINKS = 0x04;

        /// `RESOLVE_BENEATH`
        const BENEATH = 0x08;

        /// `RESOLVE_IN_ROOT`
        const IN_ROOT = 0x10;

        /// `RESOLVE_CACHED` (since Linux 5.12)
        const CACHED = 0x20;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[cfg(linux_kernel)]
bitflags! {
    /// `RENAME_*` constants for use with [`renameat_with`].
    ///
    /// [`renameat_with`]: crate::fs::renameat_with
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct RenameFlags: c::c_uint {
        /// `RENAME_EXCHANGE`
        const EXCHANGE = bitcast!(c::RENAME_EXCHANGE);

        /// `RENAME_NOREPLACE`
        const NOREPLACE = bitcast!(c::RENAME_NOREPLACE);

        /// `RENAME_WHITEOUT`
        const WHITEOUT = bitcast!(c::RENAME_WHITEOUT);

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

/// `S_IF*` constants for use with [`mknodat`] and [`Stat`]'s `st_mode` field.
///
/// [`mknodat`]: crate::fs::mknodat
/// [`Stat`]: crate::fs::Stat
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileType {
    /// `S_IFREG`
    RegularFile = c::S_IFREG as isize,

    /// `S_IFDIR`
    Directory = c::S_IFDIR as isize,

    /// `S_IFLNK`
    Symlink = c::S_IFLNK as isize,

    /// `S_IFIFO`
    #[cfg(not(target_os = "wasi"))] // TODO: Use WASI's `S_IFIFO`.
    #[doc(alias = "IFO")]
    Fifo = c::S_IFIFO as isize,

    /// `S_IFSOCK`
    #[cfg(not(target_os = "wasi"))] // TODO: Use WASI's `S_IFSOCK`.
    Socket = c::S_IFSOCK as isize,

    /// `S_IFCHR`
    CharacterDevice = c::S_IFCHR as isize,

    /// `S_IFBLK`
    BlockDevice = c::S_IFBLK as isize,

    /// An unknown filesystem object.
    Unknown,
}

impl FileType {
    /// Construct a `FileType` from the `S_IFMT` bits of the `st_mode` field of
    /// a `Stat`.
    #[inline]
    pub const fn from_raw_mode(st_mode: RawMode) -> Self {
        match (st_mode as c::mode_t) & c::S_IFMT {
            c::S_IFREG => Self::RegularFile,
            c::S_IFDIR => Self::Directory,
            c::S_IFLNK => Self::Symlink,
            #[cfg(not(target_os = "wasi"))] // TODO: Use WASI's `S_IFIFO`.
            c::S_IFIFO => Self::Fifo,
            #[cfg(not(target_os = "wasi"))] // TODO: Use WASI's `S_IFSOCK`.
            c::S_IFSOCK => Self::Socket,
            c::S_IFCHR => Self::CharacterDevice,
            c::S_IFBLK => Self::BlockDevice,
            _ => Self::Unknown,
        }
    }

    /// Construct an `st_mode` value from a `FileType`.
    #[inline]
    pub const fn as_raw_mode(self) -> RawMode {
        match self {
            Self::RegularFile => c::S_IFREG as RawMode,
            Self::Directory => c::S_IFDIR as RawMode,
            Self::Symlink => c::S_IFLNK as RawMode,
            #[cfg(not(target_os = "wasi"))] // TODO: Use WASI's `S_IFIFO`.
            Self::Fifo => c::S_IFIFO as RawMode,
            #[cfg(not(target_os = "wasi"))] // TODO: Use WASI's `S_IFSOCK`.
            Self::Socket => c::S_IFSOCK as RawMode,
            Self::CharacterDevice => c::S_IFCHR as RawMode,
            Self::BlockDevice => c::S_IFBLK as RawMode,
            Self::Unknown => c::S_IFMT as RawMode,
        }
    }

    /// Construct a `FileType` from the `d_type` field of a `c::dirent`.
    #[cfg(not(any(
        solarish,
        target_os = "aix",
        target_os = "espidf",
        target_os = "haiku",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita"
    )))]
    #[inline]
    pub(crate) const fn from_dirent_d_type(d_type: u8) -> Self {
        match d_type {
            c::DT_REG => Self::RegularFile,
            c::DT_DIR => Self::Directory,
            c::DT_LNK => Self::Symlink,
            #[cfg(not(target_os = "wasi"))] // TODO: Use WASI's `DT_SOCK`.
            c::DT_SOCK => Self::Socket,
            #[cfg(not(target_os = "wasi"))] // TODO: Use WASI's `DT_FIFO`.
            c::DT_FIFO => Self::Fifo,
            c::DT_CHR => Self::CharacterDevice,
            c::DT_BLK => Self::BlockDevice,
            // c::DT_UNKNOWN |
            _ => Self::Unknown,
        }
    }
}

/// `POSIX_FADV_*` constants for use with [`fadvise`].
///
/// [`fadvise`]: crate::fs::fadvise
#[cfg(not(any(
    apple,
    netbsdlike,
    target_os = "solaris",
    target_os = "dragonfly",
    target_os = "espidf",
    target_os = "haiku",
    target_os = "redox",
    target_os = "vita",
)))]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
pub enum Advice {
    /// `POSIX_FADV_NORMAL`
    Normal = c::POSIX_FADV_NORMAL as c::c_uint,

    /// `POSIX_FADV_SEQUENTIAL`
    Sequential = c::POSIX_FADV_SEQUENTIAL as c::c_uint,

    /// `POSIX_FADV_RANDOM`
    Random = c::POSIX_FADV_RANDOM as c::c_uint,

    /// `POSIX_FADV_NOREUSE`
    NoReuse = c::POSIX_FADV_NOREUSE as c::c_uint,

    /// `POSIX_FADV_WILLNEED`
    WillNeed = c::POSIX_FADV_WILLNEED as c::c_uint,

    /// `POSIX_FADV_DONTNEED`
    DontNeed = c::POSIX_FADV_DONTNEED as c::c_uint,
}

#[cfg(any(linux_kernel, target_os = "freebsd"))]
bitflags! {
    /// `MFD_*` constants for use with [`memfd_create`].
    ///
    /// [`memfd_create`]: crate::fs::memfd_create
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct MemfdFlags: c::c_uint {
        /// `MFD_CLOEXEC`
        const CLOEXEC = c::MFD_CLOEXEC;

        /// `MFD_ALLOW_SEALING`
        const ALLOW_SEALING = c::MFD_ALLOW_SEALING;

        /// `MFD_HUGETLB` (since Linux 4.14)
        const HUGETLB = c::MFD_HUGETLB;

        /// `MFD_NOEXEC_SEAL` (since Linux 6.3)
        #[cfg(linux_kernel)]
        const NOEXEC_SEAL = c::MFD_NOEXEC_SEAL;
        /// `MFD_EXEC` (since Linux 6.3)
        #[cfg(linux_kernel)]
        const EXEC = c::MFD_EXEC;

        /// `MFD_HUGE_64KB`
        const HUGE_64KB = c::MFD_HUGE_64KB;
        /// `MFD_HUGE_512JB`
        const HUGE_512KB = c::MFD_HUGE_512KB;
        /// `MFD_HUGE_1MB`
        const HUGE_1MB = c::MFD_HUGE_1MB;
        /// `MFD_HUGE_2MB`
        const HUGE_2MB = c::MFD_HUGE_2MB;
        /// `MFD_HUGE_8MB`
        const HUGE_8MB = c::MFD_HUGE_8MB;
        /// `MFD_HUGE_16MB`
        const HUGE_16MB = c::MFD_HUGE_16MB;
        /// `MFD_HUGE_32MB`
        const HUGE_32MB = c::MFD_HUGE_32MB;
        /// `MFD_HUGE_256MB`
        const HUGE_256MB = c::MFD_HUGE_256MB;
        /// `MFD_HUGE_512MB`
        const HUGE_512MB = c::MFD_HUGE_512MB;
        /// `MFD_HUGE_1GB`
        const HUGE_1GB = c::MFD_HUGE_1GB;
        /// `MFD_HUGE_2GB`
        const HUGE_2GB = c::MFD_HUGE_2GB;
        /// `MFD_HUGE_16GB`
        const HUGE_16GB = c::MFD_HUGE_16GB;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[cfg(any(linux_kernel, target_os = "freebsd", target_os = "fuchsia"))]
bitflags! {
    /// `F_SEAL_*` constants for use with [`fcntl_add_seals`] and
    /// [`fcntl_get_seals`].
    ///
    /// [`fcntl_add_seals`]: crate::fs::fcntl_add_seals
    /// [`fcntl_get_seals`]: crate::fs::fcntl_get_seals
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct SealFlags: u32 {
        /// `F_SEAL_SEAL`
        const SEAL = bitcast!(c::F_SEAL_SEAL);
        /// `F_SEAL_SHRINK`
        const SHRINK = bitcast!(c::F_SEAL_SHRINK);
        /// `F_SEAL_GROW`
        const GROW = bitcast!(c::F_SEAL_GROW);
        /// `F_SEAL_WRITE`
        const WRITE = bitcast!(c::F_SEAL_WRITE);
        /// `F_SEAL_FUTURE_WRITE` (since Linux 5.1)
        #[cfg(linux_kernel)]
        const FUTURE_WRITE = bitcast!(c::F_SEAL_FUTURE_WRITE);

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
bitflags! {
    /// `STATX_*` constants for use with [`statx`].
    ///
    /// [`statx`]: crate::fs::statx
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct StatxFlags: u32 {
        /// `STATX_TYPE`
        const TYPE = c::STATX_TYPE;

        /// `STATX_MODE`
        const MODE = c::STATX_MODE;

        /// `STATX_NLINK`
        const NLINK = c::STATX_NLINK;

        /// `STATX_UID`
        const UID = c::STATX_UID;

        /// `STATX_GID`
        const GID = c::STATX_GID;

        /// `STATX_ATIME`
        const ATIME = c::STATX_ATIME;

        /// `STATX_MTIME`
        const MTIME = c::STATX_MTIME;

        /// `STATX_CTIME`
        const CTIME = c::STATX_CTIME;

        /// `STATX_INO`
        const INO = c::STATX_INO;

        /// `STATX_SIZE`
        const SIZE = c::STATX_SIZE;

        /// `STATX_BLOCKS`
        const BLOCKS = c::STATX_BLOCKS;

        /// `STATX_BASIC_STATS`
        const BASIC_STATS = c::STATX_BASIC_STATS;

        /// `STATX_BTIME`
        const BTIME = c::STATX_BTIME;

        /// `STATX_MNT_ID` (since Linux 5.8)
        const MNT_ID = c::STATX_MNT_ID;

        /// `STATX_DIOALIGN` (since Linux 6.1)
        const DIOALIGN = c::STATX_DIOALIGN;

        /// `STATX_ALL`
        const ALL = c::STATX_ALL;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[cfg(any(
    target_os = "android",
    all(target_os = "linux", not(target_env = "gnu")),
))]
bitflags! {
    /// `STATX_*` constants for use with [`statx`].
    ///
    /// [`statx`]: crate::fs::statx
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct StatxFlags: u32 {
        /// `STATX_TYPE`
        const TYPE = 0x0001;

        /// `STATX_MODE`
        const MODE = 0x0002;

        /// `STATX_NLINK`
        const NLINK = 0x0004;

        /// `STATX_UID`
        const UID = 0x0008;

        /// `STATX_GID`
        const GID = 0x0010;

        /// `STATX_ATIME`
        const ATIME = 0x0020;

        /// `STATX_MTIME`
        const MTIME = 0x0040;

        /// `STATX_CTIME`
        const CTIME = 0x0080;

        /// `STATX_INO`
        const INO = 0x0100;

        /// `STATX_SIZE`
        const SIZE = 0x0200;

        /// `STATX_BLOCKS`
        const BLOCKS = 0x0400;

        /// `STATX_BASIC_STATS`
        const BASIC_STATS = 0x07ff;

        /// `STATX_BTIME`
        const BTIME = 0x800;

        /// `STATX_MNT_ID` (since Linux 5.8)
        const MNT_ID = 0x1000;

        /// `STATX_ALL`
        const ALL = 0xfff;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[cfg(not(any(
    netbsdlike,
    target_os = "espidf",
    target_os = "nto",
    target_os = "redox",
    target_os = "vita"
)))]
bitflags! {
    /// `FALLOC_FL_*` constants for use with [`fallocate`].
    ///
    /// [`fallocate`]: crate::fs::fallocate
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct FallocateFlags: u32 {
        /// `FALLOC_FL_KEEP_SIZE`
        #[cfg(not(any(
            bsd,
            solarish,
            target_os = "aix",
            target_os = "haiku",
            target_os = "hurd",
            target_os = "wasi",
        )))]
        const KEEP_SIZE = bitcast!(c::FALLOC_FL_KEEP_SIZE);
        /// `FALLOC_FL_PUNCH_HOLE`
        #[cfg(not(any(
            bsd,
            solarish,
            target_os = "aix",
            target_os = "haiku",
            target_os = "hurd",
            target_os = "wasi",
        )))]
        const PUNCH_HOLE = bitcast!(c::FALLOC_FL_PUNCH_HOLE);
        /// `FALLOC_FL_NO_HIDE_STALE`
        #[cfg(not(any(
            bsd,
            solarish,
            target_os = "aix",
            target_os = "emscripten",
            target_os = "fuchsia",
            target_os = "haiku",
            target_os = "hurd",
            target_os = "l4re",
            target_os = "linux",
            target_os = "wasi",
        )))]
        const NO_HIDE_STALE = bitcast!(c::FALLOC_FL_NO_HIDE_STALE);
        /// `FALLOC_FL_COLLAPSE_RANGE`
        #[cfg(not(any(
            bsd,
            solarish,
            target_os = "aix",
            target_os = "haiku",
            target_os = "hurd",
            target_os = "emscripten",
            target_os = "wasi",
        )))]
        const COLLAPSE_RANGE = bitcast!(c::FALLOC_FL_COLLAPSE_RANGE);
        /// `FALLOC_FL_ZERO_RANGE`
        #[cfg(not(any(
            bsd,
            solarish,
            target_os = "aix",
            target_os = "haiku",
            target_os = "hurd",
            target_os = "emscripten",
            target_os = "wasi",
        )))]
        const ZERO_RANGE = bitcast!(c::FALLOC_FL_ZERO_RANGE);
        /// `FALLOC_FL_INSERT_RANGE`
        #[cfg(not(any(
            bsd,
            solarish,
            target_os = "aix",
            target_os = "haiku",
            target_os = "hurd",
            target_os = "emscripten",
            target_os = "wasi",
        )))]
        const INSERT_RANGE = bitcast!(c::FALLOC_FL_INSERT_RANGE);
        /// `FALLOC_FL_UNSHARE_RANGE`
        #[cfg(not(any(
            bsd,
            solarish,
            target_os = "aix",
            target_os = "haiku",
            target_os = "hurd",
            target_os = "emscripten",
            target_os = "wasi",
        )))]
        const UNSHARE_RANGE = bitcast!(c::FALLOC_FL_UNSHARE_RANGE);

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[cfg(not(any(target_os = "haiku", target_os = "redox", target_os = "wasi")))]
bitflags! {
    /// `ST_*` constants for use with [`StatVfs`].
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct StatVfsMountFlags: u64 {
        /// `ST_MANDLOCK`
        #[cfg(any(linux_kernel, target_os = "emscripten", target_os = "fuchsia"))]
        const MANDLOCK = c::ST_MANDLOCK as u64;

        /// `ST_NOATIME`
        #[cfg(any(linux_kernel, target_os = "emscripten", target_os = "fuchsia"))]
        const NOATIME = c::ST_NOATIME as u64;

        /// `ST_NODEV`
        #[cfg(any(
            linux_kernel,
            target_os = "aix",
            target_os = "emscripten",
            target_os = "fuchsia"
        ))]
        const NODEV = c::ST_NODEV as u64;

        /// `ST_NODIRATIME`
        #[cfg(any(linux_kernel, target_os = "emscripten", target_os = "fuchsia"))]
        const NODIRATIME = c::ST_NODIRATIME as u64;

        /// `ST_NOEXEC`
        #[cfg(any(linux_kernel, target_os = "emscripten", target_os = "fuchsia"))]
        const NOEXEC = c::ST_NOEXEC as u64;

        /// `ST_NOSUID`
        #[cfg(not(any(target_os = "espidf", target_os = "vita")))]
        const NOSUID = c::ST_NOSUID as u64;

        /// `ST_RDONLY`
        #[cfg(not(any(target_os = "espidf", target_os = "vita")))]
        const RDONLY = c::ST_RDONLY as u64;

        /// `ST_RELATIME`
        #[cfg(any(target_os = "android", all(target_os = "linux", target_env = "gnu")))]
        const RELATIME = c::ST_RELATIME as u64;

        /// `ST_SYNCHRONOUS`
        #[cfg(any(linux_kernel, target_os = "emscripten", target_os = "fuchsia"))]
        const SYNCHRONOUS = c::ST_SYNCHRONOUS as u64;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

/// `LOCK_*` constants for use with [`flock`] and [`fcntl_lock`].
///
/// [`flock`]: crate::fs::flock
/// [`fcntl_lock`]: crate::fs::fcntl_lock
// Solaris doesn't support `flock` and doesn't define `LOCK_SH` etc., but we
// reuse this `FlockOperation` enum for `fcntl_lock`, so on Solaris we use
// our own made-up integer values.
#[cfg(not(any(target_os = "espidf", target_os = "vita", target_os = "wasi")))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum FlockOperation {
    /// `LOCK_SH`
    #[cfg(not(target_os = "solaris"))]
    LockShared = bitcast!(c::LOCK_SH),
    /// `LOCK_SH`
    #[cfg(target_os = "solaris")]
    LockShared = bitcast!(1),
    /// `LOCK_EX`
    #[cfg(not(target_os = "solaris"))]
    LockExclusive = bitcast!(c::LOCK_EX),
    /// `LOCK_EX`
    #[cfg(target_os = "solaris")]
    LockExclusive = bitcast!(2),
    /// `LOCK_UN`
    #[cfg(not(target_os = "solaris"))]
    Unlock = bitcast!(c::LOCK_UN),
    /// `LOCK_UN`
    #[cfg(target_os = "solaris")]
    Unlock = bitcast!(8),
    /// `LOCK_SH | LOCK_NB`
    #[cfg(not(target_os = "solaris"))]
    NonBlockingLockShared = bitcast!(c::LOCK_SH | c::LOCK_NB),
    /// `LOCK_SH | LOCK_NB`
    #[cfg(target_os = "solaris")]
    NonBlockingLockShared = bitcast!(1 | 4),
    /// `LOCK_EX | LOCK_NB`
    #[cfg(not(target_os = "solaris"))]
    NonBlockingLockExclusive = bitcast!(c::LOCK_EX | c::LOCK_NB),
    /// `LOCK_EX | LOCK_NB`
    #[cfg(target_os = "solaris")]
    NonBlockingLockExclusive = bitcast!(2 | 4),
    /// `LOCK_UN | LOCK_NB`
    #[cfg(not(target_os = "solaris"))]
    NonBlockingUnlock = bitcast!(c::LOCK_UN | c::LOCK_NB),
    /// `LOCK_UN | LOCK_NB`
    #[cfg(target_os = "solaris")]
    NonBlockingUnlock = bitcast!(8 | 4),
}

/// `struct stat` for use with [`statat`] and [`fstat`].
///
/// [`statat`]: crate::fs::statat
/// [`fstat`]: crate::fs::fstat
#[cfg(not(any(linux_like, target_os = "hurd")))]
pub type Stat = c::stat;

/// `struct stat` for use with [`statat`] and [`fstat`].
///
/// [`statat`]: crate::fs::statat
/// [`fstat`]: crate::fs::fstat
#[cfg(any(
    all(linux_kernel, target_pointer_width = "64"),
    target_os = "hurd",
    target_os = "emscripten",
    target_os = "l4re",
))]
pub type Stat = c::stat64;

/// `struct stat` for use with [`statat`] and [`fstat`].
///
/// [`statat`]: crate::fs::statat
/// [`fstat`]: crate::fs::fstat
// On 32-bit, Linux's `struct stat64` has a 32-bit `st_mtime` and friends, so
// we use our own struct, populated from `statx` where possible, to avoid the
// y2038 bug.
#[cfg(all(linux_kernel, target_pointer_width = "32"))]
#[repr(C)]
#[derive(Debug, Copy, Clone)]
#[allow(missing_docs)]
pub struct Stat {
    pub st_dev: u64,
    pub st_mode: u32,
    pub st_nlink: u32,
    pub st_uid: u32,
    pub st_gid: u32,
    pub st_rdev: u64,
    pub st_size: i64,
    pub st_blksize: u32,
    pub st_blocks: u64,
    #[deprecated(note = "Use `rustix::fs::StatExt::atime` instead.")]
    pub st_atime: u64,
    pub st_atime_nsec: u32,
    #[deprecated(note = "Use `rustix::fs::StatExt::mtime` instead.")]
    pub st_mtime: u64,
    pub st_mtime_nsec: u32,
    #[deprecated(note = "Use `rustix::fs::StatExt::ctime` instead.")]
    pub st_ctime: u64,
    pub st_ctime_nsec: u32,
    pub st_ino: u64,
}

/// `struct statfs` for use with [`statfs`] and [`fstatfs`].
///
/// [`statfs`]: crate::fs::statfs
/// [`fstatfs`]: crate::fs::fstatfs
#[cfg(not(any(
    linux_like,
    solarish,
    target_os = "espidf",
    target_os = "haiku",
    target_os = "netbsd",
    target_os = "nto",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi",
)))]
#[allow(clippy::module_name_repetitions)]
pub type StatFs = c::statfs;

/// `struct statfs` for use with [`statfs`] and [`fstatfs`].
///
/// [`statfs`]: crate::fs::statfs
/// [`fstatfs`]: crate::fs::fstatfs
#[cfg(linux_like)]
pub type StatFs = c::statfs64;

/// `struct statvfs` for use with [`statvfs`] and [`fstatvfs`].
///
/// [`statvfs`]: crate::fs::statvfs
/// [`fstatvfs`]: crate::fs::fstatvfs
#[cfg(not(any(target_os = "haiku", target_os = "redox", target_os = "wasi")))]
#[allow(missing_docs)]
pub struct StatVfs {
    pub f_bsize: u64,
    pub f_frsize: u64,
    pub f_blocks: u64,
    pub f_bfree: u64,
    pub f_bavail: u64,
    pub f_files: u64,
    pub f_ffree: u64,
    pub f_favail: u64,
    pub f_fsid: u64,
    pub f_flag: StatVfsMountFlags,
    pub f_namemax: u64,
}

/// `struct statx` for use with [`statx`].
///
/// [`statx`]: crate::fs::statx
#[cfg(all(target_os = "linux", target_env = "gnu"))]
// Use the glibc `struct statx`.
pub type Statx = c::statx;

/// `struct statx_timestamp` for use with [`Statx`].
#[cfg(all(target_os = "linux", target_env = "gnu"))]
// Use the glibc `struct statx_timestamp`.
pub type StatxTimestamp = c::statx;

/// `struct statx` for use with [`statx`].
///
/// [`statx`]: crate::fs::statx
// Non-glibc ABIs don't currently declare a `struct statx`, so we declare it
// ourselves.
#[cfg(any(
    target_os = "android",
    all(target_os = "linux", not(target_env = "gnu")),
))]
#[repr(C)]
#[allow(missing_docs)]
pub struct Statx {
    pub stx_mask: u32,
    pub stx_blksize: u32,
    pub stx_attributes: u64,
    pub stx_nlink: u32,
    pub stx_uid: u32,
    pub stx_gid: u32,
    pub stx_mode: u16,
    __statx_pad1: [u16; 1],
    pub stx_ino: u64,
    pub stx_size: u64,
    pub stx_blocks: u64,
    pub stx_attributes_mask: u64,
    pub stx_atime: StatxTimestamp,
    pub stx_btime: StatxTimestamp,
    pub stx_ctime: StatxTimestamp,
    pub stx_mtime: StatxTimestamp,
    pub stx_rdev_major: u32,
    pub stx_rdev_minor: u32,
    pub stx_dev_major: u32,
    pub stx_dev_minor: u32,
    pub stx_mnt_id: u64,
    __statx_pad2: u64,
    __statx_pad3: [u64; 12],
}

/// `struct statx_timestamp` for use with [`Statx`].
// Non-glibc ABIs don't currently declare a `struct statx_timestamp`, so we
// declare it ourselves.
#[cfg(any(
    target_os = "android",
    all(target_os = "linux", not(target_env = "gnu")),
))]
#[repr(C)]
#[allow(missing_docs)]
pub struct StatxTimestamp {
    pub tv_sec: i64,
    pub tv_nsec: u32,
    pub __statx_timestamp_pad1: [i32; 1],
}

/// `mode_t`
#[cfg(not(all(target_os = "android", target_pointer_width = "32")))]
pub type RawMode = c::mode_t;

/// `mode_t`
#[cfg(all(target_os = "android", target_pointer_width = "32"))]
pub type RawMode = c::c_uint;

/// `dev_t`
#[cfg(not(all(target_os = "android", target_pointer_width = "32")))]
pub type Dev = c::dev_t;

/// `dev_t`
#[cfg(all(target_os = "android", target_pointer_width = "32"))]
pub type Dev = c::c_ulonglong;

/// `__fsword_t`
#[cfg(all(
    target_os = "linux",
    not(target_env = "musl"),
    not(target_arch = "s390x"),
))]
pub type FsWord = c::__fsword_t;

/// `__fsword_t`
#[cfg(all(
    any(target_os = "android", all(target_os = "linux", target_env = "musl")),
    target_pointer_width = "32",
))]
pub type FsWord = u32;

/// `__fsword_t`
#[cfg(all(
    any(target_os = "android", all(target_os = "linux", target_env = "musl")),
    not(target_arch = "s390x"),
    target_pointer_width = "64",
))]
pub type FsWord = u64;

/// `__fsword_t`
// s390x uses `u32` for `statfs` entries on glibc, even though `__fsword_t` is
// `u64`.
#[cfg(all(target_os = "linux", target_arch = "s390x", target_env = "gnu"))]
pub type FsWord = u32;

/// `__fsword_t`
// s390x uses `u64` for `statfs` entries on musl.
#[cfg(all(target_os = "linux", target_arch = "s390x", target_env = "musl"))]
pub type FsWord = u64;

/// `copyfile_state_t`—State for use with [`fcopyfile`].
///
/// [`fcopyfile`]: crate::fs::fcopyfile
#[cfg(apple)]
#[allow(non_camel_case_types)]
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct copyfile_state_t(pub(crate) *mut c::c_void);
