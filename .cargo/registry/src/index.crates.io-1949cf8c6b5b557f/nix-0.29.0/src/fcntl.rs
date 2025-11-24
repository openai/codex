//! file control options
use crate::errno::Errno;
#[cfg(all(target_os = "freebsd", target_arch = "x86_64"))]
use core::slice;
use libc::{c_int, c_uint, size_t, ssize_t};
#[cfg(any(
    target_os = "netbsd",
    apple_targets,
    target_os = "dragonfly",
    all(target_os = "freebsd", target_arch = "x86_64"),
))]
use std::ffi::CStr;
use std::ffi::OsString;
#[cfg(not(any(target_os = "redox", target_os = "solaris")))]
use std::ops::{Deref, DerefMut};
#[cfg(not(target_os = "redox"))]
use std::os::raw;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::io::RawFd;
#[cfg(not(any(target_os = "redox", target_os = "solaris")))]
use std::os::unix::io::{AsRawFd, OwnedFd};
#[cfg(any(
    target_os = "netbsd",
    apple_targets,
    target_os = "dragonfly",
    all(target_os = "freebsd", target_arch = "x86_64"),
))]
use std::path::PathBuf;
#[cfg(any(linux_android, target_os = "freebsd"))]
use std::{os::unix::io::AsFd, ptr};

#[cfg(feature = "fs")]
use crate::{sys::stat::Mode, NixPath, Result};

#[cfg(any(
    linux_android,
    target_os = "emscripten",
    target_os = "fuchsia",
    target_os = "wasi",
    target_env = "uclibc",
    target_os = "freebsd"
))]
#[cfg(feature = "fs")]
pub use self::posix_fadvise::{posix_fadvise, PosixFadviseAdvice};

#[cfg(not(target_os = "redox"))]
#[cfg(any(feature = "fs", feature = "process", feature = "user"))]
libc_bitflags! {
    /// Flags that control how the various *at syscalls behave.
    #[cfg_attr(docsrs, doc(cfg(any(feature = "fs", feature = "process"))))]
    pub struct AtFlags: c_int {
        #[allow(missing_docs)]
        #[doc(hidden)]
        // Should not be used by the public API, but only internally.
        AT_REMOVEDIR;
        /// Used with [`linkat`](crate::unistd::linkat`) to create a link to a symbolic link's
        /// target, instead of to the symbolic link itself.
        AT_SYMLINK_FOLLOW;
        /// Used with functions like [`fstatat`](crate::sys::stat::fstatat`) to operate on a link
        /// itself, instead of the symbolic link's target.
        AT_SYMLINK_NOFOLLOW;
        /// Don't automount the terminal ("basename") component of pathname if it is a directory
        /// that is an automount point.
        #[cfg(linux_android)]
        AT_NO_AUTOMOUNT;
        /// If the provided path is an empty string, operate on the provided directory file
        /// descriptor instead.
        #[cfg(any(linux_android, target_os = "freebsd", target_os = "hurd"))]
        AT_EMPTY_PATH;
        /// Used with [`faccessat`](crate::unistd::faccessat), the checks for accessibility are
        /// performed using the effective user and group IDs instead of the real user and group ID
        #[cfg(not(target_os = "android"))]
        AT_EACCESS;
    }
}

#[cfg(any(
    feature = "fs",
    feature = "term",
    all(feature = "fanotify", target_os = "linux")
))]
libc_bitflags!(
    /// Configuration options for opened files.
    #[cfg_attr(docsrs, doc(cfg(any(feature = "fs", feature = "term", all(feature = "fanotify", target_os = "linux")))))]
    pub struct OFlag: c_int {
        /// Mask for the access mode of the file.
        O_ACCMODE;
        /// Use alternate I/O semantics.
        #[cfg(target_os = "netbsd")]
        O_ALT_IO;
        /// Open the file in append-only mode.
        O_APPEND;
        /// Generate a signal when input or output becomes possible.
        #[cfg(not(any(
            solarish,
            target_os = "aix",
            target_os = "haiku"
        )))]
        O_ASYNC;
        /// Closes the file descriptor once an `execve` call is made.
        ///
        /// Also sets the file offset to the beginning of the file.
        O_CLOEXEC;
        /// Create the file if it does not exist.
        O_CREAT;
        /// Try to minimize cache effects of the I/O for this file.
        #[cfg(any(
            freebsdlike,
            linux_android,
            solarish,
            target_os = "netbsd"
        ))]
        O_DIRECT;
        /// If the specified path isn't a directory, fail.
        O_DIRECTORY;
        /// Implicitly follow each `write()` with an `fdatasync()`.
        #[cfg(any(linux_android, apple_targets, target_os = "freebsd", netbsdlike))]
        O_DSYNC;
        /// Error out if a file was not created.
        O_EXCL;
        /// Open for execute only.
        #[cfg(target_os = "freebsd")]
        O_EXEC;
        /// Open with an exclusive file lock.
        #[cfg(any(bsd, target_os = "redox"))]
        O_EXLOCK;
        /// Same as `O_SYNC`.
        #[cfg(any(bsd,
                  all(target_os = "linux", not(target_env = "musl"), not(target_env = "ohos")),
                  target_os = "redox"))]
        O_FSYNC;
        /// Allow files whose sizes can't be represented in an `off_t` to be opened.
        #[cfg(linux_android)]
        O_LARGEFILE;
        /// Do not update the file last access time during `read(2)`s.
        #[cfg(linux_android)]
        O_NOATIME;
        /// Don't attach the device as the process' controlling terminal.
        #[cfg(not(target_os = "redox"))]
        O_NOCTTY;
        /// Same as `O_NONBLOCK`.
        #[cfg(not(any(target_os = "redox", target_os = "haiku")))]
        O_NDELAY;
        /// `open()` will fail if the given path is a symbolic link.
        O_NOFOLLOW;
        /// When possible, open the file in nonblocking mode.
        O_NONBLOCK;
        /// Don't deliver `SIGPIPE`.
        #[cfg(target_os = "netbsd")]
        O_NOSIGPIPE;
        /// Obtain a file descriptor for low-level access.
        ///
        /// The file itself is not opened and other file operations will fail.
        #[cfg(any(linux_android, target_os = "redox", target_os = "freebsd", target_os = "fuchsia"))]
        O_PATH;
        /// Only allow reading.
        ///
        /// This should not be combined with `O_WRONLY` or `O_RDWR`.
        O_RDONLY;
        /// Allow both reading and writing.
        ///
        /// This should not be combined with `O_WRONLY` or `O_RDONLY`.
        O_RDWR;
        /// Similar to `O_DSYNC` but applies to `read`s instead.
        #[cfg(any(target_os = "linux", netbsdlike))]
        O_RSYNC;
        /// Open directory for search only. Skip search permission checks on
        /// later `openat()` calls using the obtained file descriptor.
        #[cfg(any(
            apple_targets,
            solarish,
            target_os = "netbsd",
            target_os = "freebsd",
            target_os = "fuchsia",
            target_os = "emscripten",
            target_os = "aix",
            target_os = "wasi"
        ))]
        O_SEARCH;
        /// Open with a shared file lock.
        #[cfg(any(bsd, target_os = "redox"))]
        O_SHLOCK;
        /// Implicitly follow each `write()` with an `fsync()`.
        #[cfg(not(target_os = "redox"))]
        O_SYNC;
        /// Create an unnamed temporary file.
        #[cfg(linux_android)]
        O_TMPFILE;
        /// Truncate an existing regular file to 0 length if it allows writing.
        O_TRUNC;
        /// Restore default TTY attributes.
        #[cfg(target_os = "freebsd")]
        O_TTY_INIT;
        /// Only allow writing.
        ///
        /// This should not be combined with `O_RDONLY` or `O_RDWR`.
        O_WRONLY;
    }
);

/// Computes the raw fd consumed by a function of the form `*at`.
#[cfg(any(
    all(feature = "fs", not(target_os = "redox")),
    all(feature = "process", linux_android),
    all(feature = "fanotify", target_os = "linux")
))]
pub(crate) fn at_rawfd(fd: Option<RawFd>) -> raw::c_int {
    fd.unwrap_or(libc::AT_FDCWD)
}

feature! {
#![feature = "fs"]

/// open or create a file for reading, writing or executing
///
/// # See Also
/// [`open`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/open.html)
// The conversion is not identical on all operating systems.
#[allow(clippy::useless_conversion)]
pub fn open<P: ?Sized + NixPath>(
    path: &P,
    oflag: OFlag,
    mode: Mode,
) -> Result<RawFd> {
    let fd = path.with_nix_path(|cstr| unsafe {
        libc::open(cstr.as_ptr(), oflag.bits(), mode.bits() as c_uint)
    })?;

    Errno::result(fd)
}

/// open or create a file for reading, writing or executing
///
/// The `openat` function is equivalent to the [`open`] function except in the case where the path
/// specifies a relative path.  In that case, the file to be opened is determined relative to the
/// directory associated with the file descriptor `fd`.
///
/// # See Also
/// [`openat`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/openat.html)
// The conversion is not identical on all operating systems.
#[allow(clippy::useless_conversion)]
#[cfg(not(target_os = "redox"))]
pub fn openat<P: ?Sized + NixPath>(
    dirfd: Option<RawFd>,
    path: &P,
    oflag: OFlag,
    mode: Mode,
) -> Result<RawFd> {
    let fd = path.with_nix_path(|cstr| unsafe {
        libc::openat(at_rawfd(dirfd), cstr.as_ptr(), oflag.bits(), mode.bits() as c_uint)
    })?;
    Errno::result(fd)
}

cfg_if::cfg_if! {
    if #[cfg(target_os = "linux")] {
        libc_bitflags! {
            /// Path resolution flags.
            ///
            /// See [path resolution(7)](https://man7.org/linux/man-pages/man7/path_resolution.7.html)
            /// for details of the resolution process.
            pub struct ResolveFlag: libc::c_ulonglong {
                /// Do not permit the path resolution to succeed if any component of
                /// the resolution is not a descendant of the directory indicated by
                /// dirfd.  This causes absolute symbolic links (and absolute values of
                /// pathname) to be rejected.
                RESOLVE_BENEATH;

                /// Treat the directory referred to by dirfd as the root directory
                /// while resolving pathname.
                RESOLVE_IN_ROOT;

                /// Disallow all magic-link resolution during path resolution. Magic
                /// links are symbolic link-like objects that are most notably found
                /// in proc(5);  examples include `/proc/[pid]/exe` and `/proc/[pid]/fd/*`.
                ///
                /// See symlink(7) for more details.
                RESOLVE_NO_MAGICLINKS;

                /// Disallow resolution of symbolic links during path resolution. This
                /// option implies RESOLVE_NO_MAGICLINKS.
                RESOLVE_NO_SYMLINKS;

                /// Disallow traversal of mount points during path resolution (including
                /// all bind mounts).
                RESOLVE_NO_XDEV;
            }
        }

        /// Specifies how [openat2] should open a pathname.
        ///
        /// See <https://man7.org/linux/man-pages/man2/open_how.2type.html>
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug)]
        pub struct OpenHow(libc::open_how);

        impl OpenHow {
            /// Create a new zero-filled `open_how`.
            pub fn new() -> Self {
                // safety: according to the man page, open_how MUST be zero-initialized
                // on init so that unknown fields are also zeroed.
                Self(unsafe {
                    std::mem::MaybeUninit::zeroed().assume_init()
                })
            }

            /// Set the open flags used to open a file, completely overwriting any
            /// existing flags.
            pub fn flags(mut self, flags: OFlag) -> Self {
                let flags = flags.bits() as libc::c_ulonglong;
                self.0.flags = flags;
                self
            }

            /// Set the file mode new files will be created with, overwriting any
            /// existing flags.
            pub fn mode(mut self, mode: Mode) -> Self {
                let mode = mode.bits() as libc::c_ulonglong;
                self.0.mode = mode;
                self
            }

            /// Set resolve flags, completely overwriting any existing flags.
            ///
            /// See [ResolveFlag] for more detail.
            pub fn resolve(mut self, resolve: ResolveFlag) -> Self {
                let resolve = resolve.bits();
                self.0.resolve = resolve;
                self
            }
        }

        // safety: default isn't derivable because libc::open_how must be zeroed
        impl Default for OpenHow {
            fn default() -> Self {
                Self::new()
            }
        }

        /// Open or create a file for reading, writing or executing.
        ///
        /// `openat2` is an extension of the [`openat`] function that allows the caller
        /// to control how path resolution happens.
        ///
        /// # See also
        ///
        /// [openat2](https://man7.org/linux/man-pages/man2/openat2.2.html)
        pub fn openat2<P: ?Sized + NixPath>(
            dirfd: RawFd,
            path: &P,
            mut how: OpenHow,
        ) -> Result<RawFd> {
            let fd = path.with_nix_path(|cstr| unsafe {
                libc::syscall(
                    libc::SYS_openat2,
                    dirfd,
                    cstr.as_ptr(),
                    &mut how as *mut OpenHow,
                    std::mem::size_of::<libc::open_how>(),
                )
            })?;

            Errno::result(fd as RawFd)
        }
    }
}

/// Change the name of a file.
///
/// The `renameat` function is equivalent to `rename` except in the case where either `old_path`
/// or `new_path` specifies a relative path.  In such cases, the file to be renamed (or the its new
/// name, respectively) is located relative to `old_dirfd` or `new_dirfd`, respectively
///
/// # See Also
/// [`renameat`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/rename.html)
#[cfg(not(target_os = "redox"))]
pub fn renameat<P1: ?Sized + NixPath, P2: ?Sized + NixPath>(
    old_dirfd: Option<RawFd>,
    old_path: &P1,
    new_dirfd: Option<RawFd>,
    new_path: &P2,
) -> Result<()> {
    let res = old_path.with_nix_path(|old_cstr| {
        new_path.with_nix_path(|new_cstr| unsafe {
            libc::renameat(
                at_rawfd(old_dirfd),
                old_cstr.as_ptr(),
                at_rawfd(new_dirfd),
                new_cstr.as_ptr(),
            )
        })
    })??;
    Errno::result(res).map(drop)
}
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
#[cfg(feature = "fs")]
libc_bitflags! {
    /// Flags for use with [`renameat2`].
    #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
    pub struct RenameFlags: u32 {
        /// Atomically exchange `old_path` and `new_path`.
        RENAME_EXCHANGE;
        /// Don't overwrite `new_path` of the rename.  Return an error if `new_path` already
        /// exists.
        RENAME_NOREPLACE;
        /// creates a "whiteout" object at the source of the rename at the same time as performing
        /// the rename.
        ///
        /// This operation makes sense only for overlay/union filesystem implementations.
        RENAME_WHITEOUT;
    }
}

feature! {
#![feature = "fs"]
/// Like [`renameat`], but with an additional `flags` argument.
///
/// A `renameat2` call with an empty flags argument is equivalent to `renameat`.
///
/// # See Also
/// * [`rename`](https://man7.org/linux/man-pages/man2/rename.2.html)
#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub fn renameat2<P1: ?Sized + NixPath, P2: ?Sized + NixPath>(
    old_dirfd: Option<RawFd>,
    old_path: &P1,
    new_dirfd: Option<RawFd>,
    new_path: &P2,
    flags: RenameFlags,
) -> Result<()> {
    let res = old_path.with_nix_path(|old_cstr| {
        new_path.with_nix_path(|new_cstr| unsafe {
            libc::renameat2(
                at_rawfd(old_dirfd),
                old_cstr.as_ptr(),
                at_rawfd(new_dirfd),
                new_cstr.as_ptr(),
                flags.bits(),
            )
        })
    })??;
    Errno::result(res).map(drop)
}

fn wrap_readlink_result(mut v: Vec<u8>, len: ssize_t) -> Result<OsString> {
    unsafe { v.set_len(len as usize) }
    v.shrink_to_fit();
    Ok(OsString::from_vec(v.to_vec()))
}

fn readlink_maybe_at<P: ?Sized + NixPath>(
    dirfd: Option<RawFd>,
    path: &P,
    v: &mut Vec<u8>,
) -> Result<libc::ssize_t> {
    path.with_nix_path(|cstr| unsafe {
        match dirfd {
            #[cfg(target_os = "redox")]
            Some(_) => unreachable!(),
            #[cfg(not(target_os = "redox"))]
            Some(dirfd) => libc::readlinkat(
                dirfd,
                cstr.as_ptr(),
                v.as_mut_ptr().cast(),
                v.capacity() as size_t,
            ),
            None => libc::readlink(
                cstr.as_ptr(),
                v.as_mut_ptr().cast(),
                v.capacity() as size_t,
            ),
        }
    })
}

fn inner_readlink<P: ?Sized + NixPath>(
    dirfd: Option<RawFd>,
    path: &P,
) -> Result<OsString> {
    #[cfg(not(target_os = "hurd"))]
    const PATH_MAX: usize = libc::PATH_MAX as usize;
    #[cfg(target_os = "hurd")]
    const PATH_MAX: usize = 1024; // Hurd does not define a hard limit, so try a guess first
    let mut v = Vec::with_capacity(PATH_MAX);

    {
        // simple case: result is strictly less than `PATH_MAX`
        let res = readlink_maybe_at(dirfd, path, &mut v)?;
        let len = Errno::result(res)?;
        debug_assert!(len >= 0);
        if (len as usize) < v.capacity() {
            return wrap_readlink_result(v, res);
        }
    }

    // Uh oh, the result is too long...
    // Let's try to ask lstat how many bytes to allocate.
    let mut try_size = {
        let reported_size = match dirfd {
            #[cfg(target_os = "redox")]
            Some(_) => unreachable!(),
            #[cfg(any(linux_android, target_os = "freebsd", target_os = "hurd"))]
            Some(dirfd) => {
                let flags = if path.is_empty() {
                    AtFlags::AT_EMPTY_PATH
                } else {
                    AtFlags::empty()
                };
                super::sys::stat::fstatat(
                    Some(dirfd),
                    path,
                    flags | AtFlags::AT_SYMLINK_NOFOLLOW,
                )
            }
            #[cfg(not(any(
                linux_android,
                target_os = "redox",
                target_os = "freebsd",
                target_os = "hurd"
            )))]
            Some(dirfd) => super::sys::stat::fstatat(
                Some(dirfd),
                path,
                AtFlags::AT_SYMLINK_NOFOLLOW,
            ),
            None => super::sys::stat::lstat(path),
        }
        .map(|x| x.st_size)
        .unwrap_or(0);

        if reported_size > 0 {
            // Note: even if `lstat`'s apparently valid answer turns out to be
            // wrong, we will still read the full symlink no matter what.
            reported_size as usize + 1
        } else {
            // If lstat doesn't cooperate, or reports an error, be a little less
            // precise.
            PATH_MAX.max(128) << 1
        }
    };

    loop {
        {
            v.reserve_exact(try_size);
            let res = readlink_maybe_at(dirfd, path, &mut v)?;
            let len = Errno::result(res)?;
            debug_assert!(len >= 0);
            if (len as usize) < v.capacity() {
                return wrap_readlink_result(v, res);
            }
        }

        // Ugh! Still not big enough!
        match try_size.checked_shl(1) {
            Some(next_size) => try_size = next_size,
            // It's absurd that this would happen, but handle it sanely
            // anyway.
            None => break Err(Errno::ENAMETOOLONG),
        }
    }
}

/// Read value of a symbolic link
///
/// # See Also
/// * [`readlink`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/readlink.html)
pub fn readlink<P: ?Sized + NixPath>(path: &P) -> Result<OsString> {
    inner_readlink(None, path)
}

/// Read value of a symbolic link.
///
/// Equivalent to [`readlink` ] except where `path` specifies a relative path.  In that case,
/// interpret `path` relative to open file specified by `dirfd`.
///
/// # See Also
/// * [`readlink`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/readlink.html)
#[cfg(not(target_os = "redox"))]
pub fn readlinkat<P: ?Sized + NixPath>(
    dirfd: Option<RawFd>,
    path: &P,
) -> Result<OsString> {
    let dirfd = at_rawfd(dirfd);
    inner_readlink(Some(dirfd), path)
}
}

#[cfg(any(linux_android, target_os = "freebsd"))]
#[cfg(feature = "fs")]
libc_bitflags!(
    /// Additional flags for file sealing, which allows for limiting operations on a file.
    #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
    pub struct SealFlag: c_int {
        /// Prevents further calls to `fcntl()` with `F_ADD_SEALS`.
        F_SEAL_SEAL;
        /// The file cannot be reduced in size.
        F_SEAL_SHRINK;
        /// The size of the file cannot be increased.
        F_SEAL_GROW;
        /// The file contents cannot be modified.
        F_SEAL_WRITE;
        /// The file contents cannot be modified, except via shared writable mappings that were
        /// created prior to the seal being set. Since Linux 5.1.
        #[cfg(linux_android)]
        F_SEAL_FUTURE_WRITE;
    }
);

#[cfg(feature = "fs")]
libc_bitflags!(
    /// Additional configuration flags for `fcntl`'s `F_SETFD`.
    #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
    pub struct FdFlag: c_int {
        /// The file descriptor will automatically be closed during a successful `execve(2)`.
        FD_CLOEXEC;
    }
);

feature! {
#![feature = "fs"]

/// Commands for use with [`fcntl`].
#[cfg(not(target_os = "redox"))]
#[derive(Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum FcntlArg<'a> {
    /// Duplicate the provided file descriptor
    F_DUPFD(RawFd),
    /// Duplicate the provided file descriptor and set the `FD_CLOEXEC` flag on it.
    F_DUPFD_CLOEXEC(RawFd),
    /// Get the close-on-exec flag associated with the file descriptor
    F_GETFD,
    /// Set the close-on-exec flag associated with the file descriptor
    F_SETFD(FdFlag), // FD_FLAGS
    /// Get descriptor status flags
    F_GETFL,
    /// Set descriptor status flags
    F_SETFL(OFlag), // O_NONBLOCK
    /// Set or clear a file segment lock
    F_SETLK(&'a libc::flock),
    /// Like [`F_SETLK`](FcntlArg::F_SETLK) except that if a shared or exclusive lock is blocked by
    /// other locks, the process waits until the request can be satisfied.
    F_SETLKW(&'a libc::flock),
    /// Get the first lock that blocks the lock description
    F_GETLK(&'a mut libc::flock),
    /// Acquire or release an open file description lock
    #[cfg(linux_android)]
    F_OFD_SETLK(&'a libc::flock),
    /// Like [`F_OFD_SETLK`](FcntlArg::F_OFD_SETLK) except that if a conflicting lock is held on
    /// the file, then wait for that lock to be released.
    #[cfg(linux_android)]
    F_OFD_SETLKW(&'a libc::flock),
    /// Determine whether it would be possible to create the given lock.  If not, return details
    /// about one existing lock that would prevent it.
    #[cfg(linux_android)]
    F_OFD_GETLK(&'a mut libc::flock),
    /// Add seals to the file
    #[cfg(any(
        linux_android,
        target_os = "freebsd"
    ))]
    F_ADD_SEALS(SealFlag),
    /// Get seals associated with the file
    #[cfg(any(
        linux_android,
        target_os = "freebsd"
    ))]
    F_GET_SEALS,
    /// Asks the drive to flush all buffered data to permanent storage.
    #[cfg(apple_targets)]
    F_FULLFSYNC,
    /// fsync + issue barrier to drive
    #[cfg(apple_targets)]
    F_BARRIERFSYNC,
    /// Return the capacity of a pipe
    #[cfg(linux_android)]
    F_GETPIPE_SZ,
    /// Change the capacity of a pipe
    #[cfg(linux_android)]
    F_SETPIPE_SZ(c_int),
    /// Look up the path of an open file descriptor, if possible.
    #[cfg(any(
        target_os = "netbsd",
        target_os = "dragonfly",
        apple_targets,
    ))]
    F_GETPATH(&'a mut PathBuf),
    /// Look up the path of an open file descriptor, if possible.
    #[cfg(all(target_os = "freebsd", target_arch = "x86_64"))]
    F_KINFO(&'a mut PathBuf),
    /// Return the full path without firmlinks of the fd.
    #[cfg(apple_targets)]
    F_GETPATH_NOFIRMLINK(&'a mut PathBuf),
    // TODO: Rest of flags
}

/// Commands for use with [`fcntl`].
#[cfg(target_os = "redox")]
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum FcntlArg {
    /// Duplicate the provided file descriptor
    F_DUPFD(RawFd),
    /// Duplicate the provided file descriptor and set the `FD_CLOEXEC` flag on it.
    F_DUPFD_CLOEXEC(RawFd),
    /// Get the close-on-exec flag associated with the file descriptor
    F_GETFD,
    /// Set the close-on-exec flag associated with the file descriptor
    F_SETFD(FdFlag), // FD_FLAGS
    /// Get descriptor status flags
    F_GETFL,
    /// Set descriptor status flags
    F_SETFL(OFlag), // O_NONBLOCK
}
pub use self::FcntlArg::*;

/// Perform various operations on open file descriptors.
///
/// # See Also
/// * [`fcntl`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/fcntl.html)
// TODO: Figure out how to handle value fcntl returns
pub fn fcntl(fd: RawFd, arg: FcntlArg) -> Result<c_int> {
    let res = unsafe {
        match arg {
            F_DUPFD(rawfd) => libc::fcntl(fd, libc::F_DUPFD, rawfd),
            F_DUPFD_CLOEXEC(rawfd) => {
                libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, rawfd)
            }
            F_GETFD => libc::fcntl(fd, libc::F_GETFD),
            F_SETFD(flag) => libc::fcntl(fd, libc::F_SETFD, flag.bits()),
            F_GETFL => libc::fcntl(fd, libc::F_GETFL),
            F_SETFL(flag) => libc::fcntl(fd, libc::F_SETFL, flag.bits()),
            #[cfg(not(target_os = "redox"))]
            F_SETLK(flock) => libc::fcntl(fd, libc::F_SETLK, flock),
            #[cfg(not(target_os = "redox"))]
            F_SETLKW(flock) => libc::fcntl(fd, libc::F_SETLKW, flock),
            #[cfg(not(target_os = "redox"))]
            F_GETLK(flock) => libc::fcntl(fd, libc::F_GETLK, flock),
            #[cfg(linux_android)]
            F_OFD_SETLK(flock) => libc::fcntl(fd, libc::F_OFD_SETLK, flock),
            #[cfg(linux_android)]
            F_OFD_SETLKW(flock) => libc::fcntl(fd, libc::F_OFD_SETLKW, flock),
            #[cfg(linux_android)]
            F_OFD_GETLK(flock) => libc::fcntl(fd, libc::F_OFD_GETLK, flock),
            #[cfg(any(
                linux_android,
                target_os = "freebsd"
            ))]
            F_ADD_SEALS(flag) => {
                libc::fcntl(fd, libc::F_ADD_SEALS, flag.bits())
            }
            #[cfg(any(
                linux_android,
                target_os = "freebsd"
            ))]
            F_GET_SEALS => libc::fcntl(fd, libc::F_GET_SEALS),
            #[cfg(apple_targets)]
            F_FULLFSYNC => libc::fcntl(fd, libc::F_FULLFSYNC),
            #[cfg(apple_targets)]
            F_BARRIERFSYNC => libc::fcntl(fd, libc::F_BARRIERFSYNC),
            #[cfg(linux_android)]
            F_GETPIPE_SZ => libc::fcntl(fd, libc::F_GETPIPE_SZ),
            #[cfg(linux_android)]
            F_SETPIPE_SZ(size) => libc::fcntl(fd, libc::F_SETPIPE_SZ, size),
            #[cfg(any(
                target_os = "dragonfly",
                target_os = "netbsd",
                apple_targets,
            ))]
            F_GETPATH(path) => {
                let mut buffer = vec![0; libc::PATH_MAX as usize];
                let res = libc::fcntl(fd, libc::F_GETPATH, buffer.as_mut_ptr());
                let ok_res = Errno::result(res)?;
                let optr = CStr::from_bytes_until_nul(&buffer).unwrap();
                *path = PathBuf::from(OsString::from(optr.to_str().unwrap()));
                return Ok(ok_res)
            },
            #[cfg(all(target_os = "freebsd", target_arch = "x86_64"))]
            F_KINFO(path) => {
                let mut info: libc::kinfo_file = std::mem::zeroed();
                info.kf_structsize = std::mem::size_of::<libc::kinfo_file>() as i32;
                let res = libc::fcntl(fd, libc::F_KINFO, &mut info);
                let ok_res = Errno::result(res)?;
                let p = info.kf_path;
                let u8_slice = slice::from_raw_parts(p.as_ptr().cast(), p.len());
                let optr = CStr::from_bytes_until_nul(u8_slice).unwrap();
                *path = PathBuf::from(OsString::from(optr.to_str().unwrap()));
                return Ok(ok_res)
            },
            #[cfg(apple_targets)]
            F_GETPATH_NOFIRMLINK(path) => {
                let mut buffer = vec![0; libc::PATH_MAX as usize];
                let res = libc::fcntl(fd, libc::F_GETPATH_NOFIRMLINK, buffer.as_mut_ptr());
                let ok_res = Errno::result(res)?;
                let optr = CStr::from_bytes_until_nul(&buffer).unwrap();
                *path = PathBuf::from(OsString::from(optr.to_str().unwrap()));
                return Ok(ok_res)
            },
        }
    };

    Errno::result(res)
}

/// Operations for use with [`Flock::lock`].
#[cfg(not(any(target_os = "redox", target_os = "solaris")))]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum FlockArg {
    /// shared file lock
    LockShared,
    /// exclusive file lock
    LockExclusive,
    /// Unlock file
    Unlock,
    /// Shared lock.  Do not block when locking.
    LockSharedNonblock,
    /// Exclusive lock.  Do not block when locking.
    LockExclusiveNonblock,
    #[allow(missing_docs)]
    #[deprecated(since = "0.28.0", note = "Use FlockArg::Unlock instead")]
    UnlockNonblock,
}

#[allow(missing_docs)]
#[cfg(not(any(target_os = "redox", target_os = "solaris")))]
#[deprecated(since = "0.28.0", note = "`fcntl::Flock` should be used instead.")]
pub fn flock(fd: RawFd, arg: FlockArg) -> Result<()> {
    use self::FlockArg::*;

    let res = unsafe {
        match arg {
            LockShared => libc::flock(fd, libc::LOCK_SH),
            LockExclusive => libc::flock(fd, libc::LOCK_EX),
            Unlock => libc::flock(fd, libc::LOCK_UN),
            LockSharedNonblock => {
                libc::flock(fd, libc::LOCK_SH | libc::LOCK_NB)
            }
            LockExclusiveNonblock => {
                libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB)
            }
            #[allow(deprecated)]
            UnlockNonblock => libc::flock(fd, libc::LOCK_UN | libc::LOCK_NB),
        }
    };

    Errno::result(res).map(drop)
}

/// Represents valid types for flock.
///
/// # Safety
/// Types implementing this must not be `Clone`.
#[cfg(not(any(target_os = "redox", target_os = "solaris")))]
pub unsafe trait Flockable: AsRawFd {}

/// Represents an owned flock, which unlocks on drop.
///
/// See [flock(2)](https://linux.die.net/man/2/flock) for details on locking semantics.
#[cfg(not(any(target_os = "redox", target_os = "solaris")))]
#[derive(Debug)]
pub struct Flock<T: Flockable>(T);

#[cfg(not(any(target_os = "redox", target_os = "solaris")))]
impl<T: Flockable> Drop for Flock<T> {
    fn drop(&mut self) {
        let res = Errno::result(unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) });
        if res.is_err() && !std::thread::panicking() {
            panic!("Failed to remove flock: {}", res.unwrap_err());
        }
    }
}

#[cfg(not(any(target_os = "redox", target_os = "solaris")))]
impl<T: Flockable> Deref for Flock<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
#[cfg(not(any(target_os = "redox", target_os = "solaris")))]
impl<T: Flockable> DerefMut for Flock<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(not(any(target_os = "redox", target_os = "solaris")))]
impl<T: Flockable> Flock<T> {
    /// Obtain a/an flock.
    ///
    /// # Example
    /// ```
    /// # use std::io::Write;
    /// # use std::fs::File;
    /// # use nix::fcntl::{Flock, FlockArg};
    /// # fn do_stuff(file: File) {
    ///   let mut file = match Flock::lock(file, FlockArg::LockExclusive) {
    ///       Ok(l) => l,
    ///       Err(_) => return,
    ///   };
    ///
    ///   // Do stuff
    ///   let data = "Foo bar";
    ///   _ = file.write(data.as_bytes());
    ///   _ = file.sync_data();
    /// # }
    pub fn lock(t: T, args: FlockArg) -> std::result::Result<Self, (T, Errno)> {
        let flags = match args {
            FlockArg::LockShared => libc::LOCK_SH,
            FlockArg::LockExclusive => libc::LOCK_EX,
            FlockArg::LockSharedNonblock => libc::LOCK_SH | libc::LOCK_NB,
            FlockArg::LockExclusiveNonblock => libc::LOCK_EX | libc::LOCK_NB,
            #[allow(deprecated)]
            FlockArg::Unlock | FlockArg::UnlockNonblock => return Err((t, Errno::EINVAL)),
        };
        match Errno::result(unsafe { libc::flock(t.as_raw_fd(), flags) }) {
            Ok(_) => Ok(Self(t)),
            Err(errno) => Err((t, errno)),
        }
    }

    /// Remove the lock and return the object wrapped within.
    ///
    /// # Example
    /// ```
    /// # use std::fs::File;
    /// # use nix::fcntl::{Flock, FlockArg};
    /// fn do_stuff(file: File) -> nix::Result<()> {
    ///     let mut lock = match Flock::lock(file, FlockArg::LockExclusive) {
    ///         Ok(l) => l,
    ///         Err((_,e)) => return Err(e),
    ///     };
    ///
    ///     // Do critical section
    ///
    ///     // Unlock
    ///     let file = match lock.unlock() {
    ///         Ok(f) => f,
    ///         Err((_, e)) => return Err(e),
    ///     };
    ///
    ///     // Do anything else
    ///
    ///     Ok(())
    /// }
    pub fn unlock(self) -> std::result::Result<T, (Self, Errno)> {
        let inner = unsafe { match Errno::result(libc::flock(self.0.as_raw_fd(), libc::LOCK_UN)) {
            Ok(_) => std::ptr::read(&self.0),
            Err(errno) => return Err((self, errno)),
        }};

        std::mem::forget(self);
        Ok(inner)
    }

    /// Relock the file.  This can upgrade or downgrade the lock type.
    ///
    /// # Example
    /// ```
    /// # use std::fs::File;
    /// # use nix::fcntl::{Flock, FlockArg};
    /// # use tempfile::tempfile;
    /// let f: std::fs::File = tempfile().unwrap();
    /// let locked_file = Flock::lock(f, FlockArg::LockExclusive).unwrap();
    /// // Do stuff, then downgrade the lock
    /// locked_file.relock(FlockArg::LockShared).unwrap();
    /// ```
    pub fn relock(&self, arg: FlockArg) -> Result<()> {
         let flags = match arg {
            FlockArg::LockShared => libc::LOCK_SH,
            FlockArg::LockExclusive => libc::LOCK_EX,
            FlockArg::LockSharedNonblock => libc::LOCK_SH | libc::LOCK_NB,
            FlockArg::LockExclusiveNonblock => libc::LOCK_EX | libc::LOCK_NB,
            #[allow(deprecated)]
            FlockArg::Unlock | FlockArg::UnlockNonblock => return Err(Errno::EINVAL),
        };
        Errno::result(unsafe { libc::flock(self.as_raw_fd(), flags) }).map(drop)
    }
}

// Safety: `File` is not [std::clone::Clone].
#[cfg(not(any(target_os = "redox", target_os = "solaris")))]
unsafe impl Flockable for std::fs::File {}

// Safety: `OwnedFd` is not [std::clone::Clone].
#[cfg(not(any(target_os = "redox", target_os = "solaris")))]
unsafe impl Flockable for OwnedFd {}
}

#[cfg(linux_android)]
#[cfg(feature = "zerocopy")]
libc_bitflags! {
    /// Additional flags to `splice` and friends.
    #[cfg_attr(docsrs, doc(cfg(feature = "zerocopy")))]
    pub struct SpliceFFlags: c_uint {
        /// Request that pages be moved instead of copied.
        ///
        /// Not applicable to `vmsplice`.
        SPLICE_F_MOVE;
        /// Do not block on I/O.
        SPLICE_F_NONBLOCK;
        /// Hint that more data will be coming in a subsequent splice.
        ///
        /// Not applicable to `vmsplice`.
        SPLICE_F_MORE;
        /// Gift the user pages to the kernel.
        ///
        /// Not applicable to `splice`.
        SPLICE_F_GIFT;
    }
}

feature! {
#![feature = "zerocopy"]

/// Copy a range of data from one file to another
///
/// The `copy_file_range` system call performs an in-kernel copy between
/// file descriptors `fd_in` and `fd_out` without the additional cost of
/// transferring data from the kernel to user space and back again. There may be
/// additional optimizations for specific file systems.  It copies up to `len`
/// bytes of data from file descriptor `fd_in` to file descriptor `fd_out`,
/// overwriting any data that exists within the requested range of the target
/// file.
///
/// If the `off_in` and/or `off_out` arguments are used, the values
/// will be mutated to reflect the new position within the file after
/// copying. If they are not used, the relevant file descriptors will be seeked
/// to the new position.
///
/// On successful completion the number of bytes actually copied will be
/// returned.
// Note: FreeBSD defines the offset argument as "off_t".  Linux and Android
// define it as "loff_t".  But on both OSes, on all supported platforms, those
// are 64 bits.  So Nix uses i64 to make the docs simple and consistent.
#[cfg(any(linux_android, target_os = "freebsd"))]
pub fn copy_file_range<Fd1: AsFd, Fd2: AsFd>(
    fd_in: Fd1,
    off_in: Option<&mut i64>,
    fd_out: Fd2,
    off_out: Option<&mut i64>,
    len: usize,
) -> Result<usize> {
    let off_in = off_in
        .map(|offset| offset as *mut i64)
        .unwrap_or(ptr::null_mut());
    let off_out = off_out
        .map(|offset| offset as *mut i64)
        .unwrap_or(ptr::null_mut());

    cfg_if::cfg_if! {
        if #[cfg(target_os = "freebsd")] {
            let ret = unsafe {
                libc::copy_file_range(
                    fd_in.as_fd().as_raw_fd(),
                    off_in,
                    fd_out.as_fd().as_raw_fd(),
                    off_out,
                    len,
                    0,
                )
            };
        } else {
            // May Linux distros still don't include copy_file_range in their
            // libc implementations, so we need to make a direct syscall.
            let ret = unsafe {
                libc::syscall(
                    libc::SYS_copy_file_range,
                    fd_in.as_fd().as_raw_fd(),
                    off_in,
                    fd_out.as_fd().as_raw_fd(),
                    off_out,
                    len,
                    0,
                )
            };
        }
    }
    Errno::result(ret).map(|r| r as usize)
}

/// Splice data to/from a pipe
///
/// # See Also
/// *[`splice`](https://man7.org/linux/man-pages/man2/splice.2.html)
#[cfg(linux_android)]
pub fn splice<Fd1: AsFd, Fd2: AsFd>(
    fd_in: Fd1,
    off_in: Option<&mut libc::loff_t>,
    fd_out: Fd2,
    off_out: Option<&mut libc::loff_t>,
    len: usize,
    flags: SpliceFFlags,
) -> Result<usize> {
    let off_in = off_in
        .map(|offset| offset as *mut libc::loff_t)
        .unwrap_or(ptr::null_mut());
    let off_out = off_out
        .map(|offset| offset as *mut libc::loff_t)
        .unwrap_or(ptr::null_mut());

    let ret = unsafe {
        libc::splice(fd_in.as_fd().as_raw_fd(), off_in, fd_out.as_fd().as_raw_fd(), off_out, len, flags.bits())
    };
    Errno::result(ret).map(|r| r as usize)
}

/// Duplicate pipe content
///
/// # See Also
/// *[`tee`](https://man7.org/linux/man-pages/man2/tee.2.html)
#[cfg(linux_android)]
pub fn tee<Fd1: AsFd, Fd2: AsFd>(
    fd_in: Fd1,
    fd_out: Fd2,
    len: usize,
    flags: SpliceFFlags,
) -> Result<usize> {
    let ret = unsafe { libc::tee(fd_in.as_fd().as_raw_fd(), fd_out.as_fd().as_raw_fd(), len, flags.bits()) };
    Errno::result(ret).map(|r| r as usize)
}

/// Splice user pages to/from a pipe
///
/// # See Also
/// *[`vmsplice`](https://man7.org/linux/man-pages/man2/vmsplice.2.html)
#[cfg(linux_android)]
pub fn vmsplice<F: AsFd>(
    fd: F,
    iov: &[std::io::IoSlice<'_>],
    flags: SpliceFFlags,
) -> Result<usize> {
    let ret = unsafe {
        libc::vmsplice(
            fd.as_fd().as_raw_fd(),
            iov.as_ptr().cast(),
            iov.len(),
            flags.bits(),
        )
    };
    Errno::result(ret).map(|r| r as usize)
}
}

#[cfg(target_os = "linux")]
#[cfg(feature = "fs")]
libc_bitflags!(
    /// Mode argument flags for fallocate determining operation performed on a given range.
    #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
    pub struct FallocateFlags: c_int {
        /// File size is not changed.
        ///
        /// offset + len can be greater than file size.
        FALLOC_FL_KEEP_SIZE;
        /// Deallocates space by creating a hole.
        ///
        /// Must be ORed with FALLOC_FL_KEEP_SIZE. Byte range starts at offset and continues for len bytes.
        FALLOC_FL_PUNCH_HOLE;
        /// Removes byte range from a file without leaving a hole.
        ///
        /// Byte range to collapse starts at offset and continues for len bytes.
        FALLOC_FL_COLLAPSE_RANGE;
        /// Zeroes space in specified byte range.
        ///
        /// Byte range starts at offset and continues for len bytes.
        FALLOC_FL_ZERO_RANGE;
        /// Increases file space by inserting a hole within the file size.
        ///
        /// Does not overwrite existing data. Hole starts at offset and continues for len bytes.
        FALLOC_FL_INSERT_RANGE;
        /// Shared file data extants are made private to the file.
        ///
        /// Gaurantees that a subsequent write will not fail due to lack of space.
        FALLOC_FL_UNSHARE_RANGE;
    }
);

feature! {
#![feature = "fs"]

/// Manipulates file space.
///
/// Allows the caller to directly manipulate the allocated disk space for the
/// file referred to by fd.
#[cfg(target_os = "linux")]
#[cfg(feature = "fs")]
pub fn fallocate(
    fd: RawFd,
    mode: FallocateFlags,
    offset: libc::off_t,
    len: libc::off_t,
) -> Result<()> {
    let res = unsafe { libc::fallocate(fd, mode.bits(), offset, len) };
    Errno::result(res).map(drop)
}

/// Argument to [`fspacectl`] describing the range to zero.  The first member is
/// the file offset, and the second is the length of the region.
#[cfg(any(target_os = "freebsd"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpacectlRange(pub libc::off_t, pub libc::off_t);

#[cfg(any(target_os = "freebsd"))]
impl SpacectlRange {
    /// Is the range empty?
    ///
    /// After a successful call to [`fspacectl`], A value of `true` for `SpacectlRange::is_empty`
    /// indicates that the operation is complete.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.1 == 0
    }

    /// Remaining length of the range
    #[inline]
    pub fn len(&self) -> libc::off_t {
        self.1
    }

    /// Next file offset to operate on
    #[inline]
    pub fn offset(&self) -> libc::off_t {
        self.0
    }
}

/// Punch holes in a file.
///
/// `fspacectl` instructs the file system to deallocate a portion of a file.
/// After a successful operation, this region of the file will return all zeroes
/// if read.  If the file system supports deallocation, then it may free the
/// underlying storage, too.
///
/// # Arguments
///
/// - `fd`      -   File to operate on
/// - `range.0` -   File offset at which to begin deallocation
/// - `range.1` -   Length of the region to deallocate
///
/// # Returns
///
/// The operation may deallocate less than the entire requested region.  On
/// success, it returns the region that still remains to be deallocated.  The
/// caller should loop until the returned region is empty.
///
/// # Example
///
#[cfg_attr(fbsd14, doc = " ```")]
#[cfg_attr(not(fbsd14), doc = " ```no_run")]
/// # use std::io::Write;
/// # use std::os::unix::fs::FileExt;
/// # use std::os::unix::io::AsRawFd;
/// # use nix::fcntl::*;
/// # use tempfile::tempfile;
/// const INITIAL: &[u8] = b"0123456789abcdef";
/// let mut f = tempfile().unwrap();
/// f.write_all(INITIAL).unwrap();
/// let mut range = SpacectlRange(3, 6);
/// while (!range.is_empty()) {
///     range = fspacectl(f.as_raw_fd(), range).unwrap();
/// }
/// let mut buf = vec![0; INITIAL.len()];
/// f.read_exact_at(&mut buf, 0).unwrap();
/// assert_eq!(buf, b"012\0\0\0\0\0\09abcdef");
/// ```
#[cfg(target_os = "freebsd")]
#[inline] // Delays codegen, preventing linker errors with dylibs and --no-allow-shlib-undefined
pub fn fspacectl(fd: RawFd, range: SpacectlRange) -> Result<SpacectlRange> {
    let mut rqsr = libc::spacectl_range {
        r_offset: range.0,
        r_len: range.1,
    };
    let res = unsafe {
        libc::fspacectl(
            fd,
            libc::SPACECTL_DEALLOC, // Only one command is supported ATM
            &rqsr,
            0, // No flags are currently supported
            &mut rqsr,
        )
    };
    Errno::result(res).map(|_| SpacectlRange(rqsr.r_offset, rqsr.r_len))
}

/// Like [`fspacectl`], but will never return incomplete.
///
/// # Arguments
///
/// - `fd`      -   File to operate on
/// - `offset`  -   File offset at which to begin deallocation
/// - `len`     -   Length of the region to deallocate
///
/// # Returns
///
/// Returns `()` on success.  On failure, the region may or may not be partially
/// deallocated.
///
/// # Example
///
#[cfg_attr(fbsd14, doc = " ```")]
#[cfg_attr(not(fbsd14), doc = " ```no_run")]
/// # use std::io::Write;
/// # use std::os::unix::fs::FileExt;
/// # use std::os::unix::io::AsRawFd;
/// # use nix::fcntl::*;
/// # use tempfile::tempfile;
/// const INITIAL: &[u8] = b"0123456789abcdef";
/// let mut f = tempfile().unwrap();
/// f.write_all(INITIAL).unwrap();
/// fspacectl_all(f.as_raw_fd(), 3, 6).unwrap();
/// let mut buf = vec![0; INITIAL.len()];
/// f.read_exact_at(&mut buf, 0).unwrap();
/// assert_eq!(buf, b"012\0\0\0\0\0\09abcdef");
/// ```
#[cfg(target_os = "freebsd")]
#[inline] // Delays codegen, preventing linker errors with dylibs and --no-allow-shlib-undefined
pub fn fspacectl_all(
    fd: RawFd,
    offset: libc::off_t,
    len: libc::off_t,
) -> Result<()> {
    let mut rqsr = libc::spacectl_range {
        r_offset: offset,
        r_len: len,
    };
    while rqsr.r_len > 0 {
        let res = unsafe {
            libc::fspacectl(
                fd,
                libc::SPACECTL_DEALLOC, // Only one command is supported ATM
                &rqsr,
                0, // No flags are currently supported
                &mut rqsr,
            )
        };
        Errno::result(res)?;
    }
    Ok(())
}

#[cfg(any(
    linux_android,
    target_os = "emscripten",
    target_os = "fuchsia",
    target_os = "wasi",
    target_env = "uclibc",
    target_os = "freebsd"
))]
mod posix_fadvise {
    use crate::errno::Errno;
    use crate::Result;
    use std::os::unix::io::RawFd;

    #[cfg(feature = "fs")]
    libc_enum! {
        /// The specific advice provided to [`posix_fadvise`].
        #[repr(i32)]
        #[non_exhaustive]
        #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
        pub enum PosixFadviseAdvice {
            /// Revert to the default data access behavior.
            POSIX_FADV_NORMAL,
            /// The file data will be accessed sequentially.
            POSIX_FADV_SEQUENTIAL,
            /// A hint that file data will be accessed randomly, and prefetching is likely not
            /// advantageous.
            POSIX_FADV_RANDOM,
            /// The specified data will only be accessed once and then not reused.
            POSIX_FADV_NOREUSE,
            /// The specified data will be accessed in the near future.
            POSIX_FADV_WILLNEED,
            /// The specified data will not be accessed in the near future.
            POSIX_FADV_DONTNEED,
        }
    }

    feature! {
    #![feature = "fs"]
    /// Allows a process to describe to the system its data access behavior for an open file
    /// descriptor.
    ///
    /// # See Also
    /// * [`posix_fadvise`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_fadvise.html)
    pub fn posix_fadvise(
        fd: RawFd,
        offset: libc::off_t,
        len: libc::off_t,
        advice: PosixFadviseAdvice,
    ) -> Result<()> {
        let res = unsafe { libc::posix_fadvise(fd, offset, len, advice as libc::c_int) };

        if res == 0 {
            Ok(())
        } else {
            Err(Errno::from_raw(res))
        }
    }
    }
}

/// Pre-allocate storage for a range in a file
///
/// # See Also
/// * [`posix_fallocate`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_fallocate.html)
#[cfg(any(
    linux_android,
    freebsdlike,
    target_os = "emscripten",
    target_os = "fuchsia",
    target_os = "wasi",
))]
pub fn posix_fallocate(
    fd: RawFd,
    offset: libc::off_t,
    len: libc::off_t,
) -> Result<()> {
    let res = unsafe { libc::posix_fallocate(fd, offset, len) };
    match Errno::result(res) {
        Err(err) => Err(err),
        Ok(0) => Ok(()),
        Ok(errno) => Err(Errno::from_raw(errno)),
    }
}
}
