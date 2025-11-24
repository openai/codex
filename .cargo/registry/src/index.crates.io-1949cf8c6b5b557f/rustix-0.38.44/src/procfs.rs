//! Utilities for working with `/proc`, where Linux's `procfs` is typically
//! mounted.
//!
//! `/proc` serves as an adjunct to Linux's main syscall surface area,
//! providing additional features with an awkward interface.
//!
//! This module does a considerable amount of work to determine whether `/proc`
//! is mounted, with actual `procfs`, and without any additional mount points
//! on top of the paths we open.
//!
//! Why all the effort to detect bind mount points? People are doing all kinds
//! of things with Linux containers these days, with many different privilege
//! schemes, and we want to avoid making any unnecessary assumptions. Rustix
//! and its users will sometimes use procfs *implicitly* (when Linux gives them
//! no better options), in ways that aren't obvious from their public APIs.
//! These filesystem accesses might not be visible to someone auditing the main
//! code of an application for places which may be influenced by the filesystem
//! namespace. So with the checking here, they may fail, but they won't be able
//! to succeed with bogus results.

use crate::fd::{AsFd, BorrowedFd, OwnedFd};
use crate::ffi::CStr;
use crate::fs::{
    fstat, fstatfs, major, openat, renameat, seek, FileType, FsWord, Mode, OFlags, RawDir,
    SeekFrom, Stat, CWD, PROC_SUPER_MAGIC,
};
use crate::io;
use crate::path::DecInt;
#[cfg(feature = "rustc-dep-of-std")]
use core::lazy::OnceCell;
use core::mem::MaybeUninit;
#[cfg(not(feature = "rustc-dep-of-std"))]
use once_cell::sync::OnceCell;

/// Linux's procfs always uses inode 1 for its root directory.
const PROC_ROOT_INO: u64 = 1;

// Identify an entry within "/proc", to determine which anomalies to check for.
#[derive(Copy, Clone, Debug)]
enum Kind {
    Proc,
    Pid,
    Fd,
    File,
    Symlink,
}

/// Check a subdirectory of "/proc" for anomalies.
fn check_proc_entry(
    kind: Kind,
    entry: BorrowedFd<'_>,
    proc_stat: Option<&Stat>,
) -> io::Result<Stat> {
    let entry_stat = fstat(entry)?;
    check_proc_entry_with_stat(kind, entry, entry_stat, proc_stat)
}

/// Check a subdirectory of "/proc" for anomalies, using the provided `Stat`.
fn check_proc_entry_with_stat(
    kind: Kind,
    entry: BorrowedFd<'_>,
    entry_stat: Stat,
    proc_stat: Option<&Stat>,
) -> io::Result<Stat> {
    // Check the filesystem magic.
    check_procfs(entry)?;

    match kind {
        Kind::Proc => check_proc_root(entry, &entry_stat)?,
        Kind::Pid | Kind::Fd => check_proc_subdir(entry, &entry_stat, proc_stat)?,
        Kind::File => check_proc_file(&entry_stat, proc_stat)?,
        Kind::Symlink => check_proc_symlink(&entry_stat, proc_stat)?,
    }

    // "/proc" directories are typically mounted r-xr-xr-x.
    // "/proc/self/fd" is r-x------. Allow them to have fewer permissions, but
    // not more.
    match kind {
        Kind::Symlink => {
            // On Linux, symlinks don't have their own permissions.
        }
        _ => {
            let expected_mode = if let Kind::Fd = kind { 0o500 } else { 0o555 };
            if entry_stat.st_mode & 0o777 & !expected_mode != 0 {
                return Err(io::Errno::NOTSUP);
            }
        }
    }

    match kind {
        Kind::Fd => {
            // Check that the "/proc/self/fd" directory doesn't have any
            // extraneous links into it (which might include unexpected
            // subdirectories).
            if entry_stat.st_nlink != 2 {
                return Err(io::Errno::NOTSUP);
            }
        }
        Kind::Pid | Kind::Proc => {
            // Check that the "/proc" and "/proc/self" directories aren't
            // empty.
            if entry_stat.st_nlink <= 2 {
                return Err(io::Errno::NOTSUP);
            }
        }
        Kind::File => {
            // Check that files in procfs don't have extraneous hard links to
            // them (which might indicate hard links to other things).
            if entry_stat.st_nlink != 1 {
                return Err(io::Errno::NOTSUP);
            }
        }
        Kind::Symlink => {
            // Check that symlinks in procfs don't have extraneous hard links
            // to them (which might indicate hard links to other things).
            if entry_stat.st_nlink != 1 {
                return Err(io::Errno::NOTSUP);
            }
        }
    }

    Ok(entry_stat)
}

fn check_proc_root(entry: BorrowedFd<'_>, stat: &Stat) -> io::Result<()> {
    // We use `O_DIRECTORY` for proc directories, so open should fail if we
    // don't get a directory when we expect one.
    assert_eq!(FileType::from_raw_mode(stat.st_mode), FileType::Directory);

    // Check the root inode number.
    if stat.st_ino != PROC_ROOT_INO {
        return Err(io::Errno::NOTSUP);
    }

    // Proc is a non-device filesystem, so check for major number 0.
    // <https://www.kernel.org/doc/Documentation/admin-guide/devices.txt>
    if major(stat.st_dev) != 0 {
        return Err(io::Errno::NOTSUP);
    }

    // Check that "/proc" is a mountpoint.
    if !is_mountpoint(entry) {
        return Err(io::Errno::NOTSUP);
    }

    Ok(())
}

fn check_proc_subdir(
    entry: BorrowedFd<'_>,
    stat: &Stat,
    proc_stat: Option<&Stat>,
) -> io::Result<()> {
    // We use `O_DIRECTORY` for proc directories, so open should fail if we
    // don't get a directory when we expect one.
    assert_eq!(FileType::from_raw_mode(stat.st_mode), FileType::Directory);

    check_proc_nonroot(stat, proc_stat)?;

    // Check that subdirectories of "/proc" are not mount points.
    if is_mountpoint(entry) {
        return Err(io::Errno::NOTSUP);
    }

    Ok(())
}

fn check_proc_file(stat: &Stat, proc_stat: Option<&Stat>) -> io::Result<()> {
    // Check that we have a regular file.
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
        return Err(io::Errno::NOTSUP);
    }

    check_proc_nonroot(stat, proc_stat)?;

    Ok(())
}

fn check_proc_symlink(stat: &Stat, proc_stat: Option<&Stat>) -> io::Result<()> {
    // Check that we have a symbolic link.
    if FileType::from_raw_mode(stat.st_mode) != FileType::Symlink {
        return Err(io::Errno::NOTSUP);
    }

    check_proc_nonroot(stat, proc_stat)?;

    Ok(())
}

fn check_proc_nonroot(stat: &Stat, proc_stat: Option<&Stat>) -> io::Result<()> {
    // Check that we haven't been linked back to the root of "/proc".
    if stat.st_ino == PROC_ROOT_INO {
        return Err(io::Errno::NOTSUP);
    }

    // Check that we're still in procfs.
    if stat.st_dev != proc_stat.unwrap().st_dev {
        return Err(io::Errno::NOTSUP);
    }

    Ok(())
}

/// Check that `file` is opened on a `procfs` filesystem.
fn check_procfs(file: BorrowedFd<'_>) -> io::Result<()> {
    let statfs = fstatfs(file)?;
    let f_type = statfs.f_type;
    if f_type != FsWord::from(PROC_SUPER_MAGIC) {
        return Err(io::Errno::NOTSUP);
    }

    Ok(())
}

/// Check whether the given directory handle is a mount point.
fn is_mountpoint(file: BorrowedFd<'_>) -> bool {
    // We use a `renameat` call that would otherwise fail, but which fails with
    // `XDEV` first if it would cross a mount point.
    let err = renameat(file, cstr!("../."), file, cstr!(".")).unwrap_err();
    match err {
        io::Errno::XDEV => true,  // the rename failed due to crossing a mount point
        io::Errno::BUSY => false, // the rename failed normally
        _ => panic!("Unexpected error from `renameat`: {:?}", err),
    }
}

/// Open a directory in `/proc`, mapping all errors to `io::Errno::NOTSUP`.
fn proc_opendirat<P: crate::path::Arg, Fd: AsFd>(dirfd: Fd, path: P) -> io::Result<OwnedFd> {
    // We don't add `PATH` here because that disables `DIRECTORY`. And we don't
    // add `NOATIME` for the same reason as the comment in `open_and_check_file`.
    let oflags =
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOCTTY;
    openat(dirfd, path, oflags, Mode::empty()).map_err(|_err| io::Errno::NOTSUP)
}

/// Returns a handle to Linux's `/proc` directory.
///
/// This ensures that `/proc` is procfs, that nothing is mounted on top of it,
/// and that it looks normal. It also returns the `Stat` of `/proc`.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man5/proc.5.html
fn proc() -> io::Result<(BorrowedFd<'static>, &'static Stat)> {
    static PROC: StaticFd = StaticFd::new();

    // `OnceBox` is “racy” in that the initialization function may run
    // multiple times. We're ok with that, since the initialization function
    // has no side effects.
    PROC.get_or_try_init(|| {
        // Open "/proc".
        let proc = proc_opendirat(CWD, cstr!("/proc"))?;
        let proc_stat =
            check_proc_entry(Kind::Proc, proc.as_fd(), None).map_err(|_err| io::Errno::NOTSUP)?;

        Ok(new_static_fd(proc, proc_stat))
    })
    .map(|(fd, stat)| (fd.as_fd(), stat))
}

/// Returns a handle to Linux's `/proc/self` directory.
///
/// This ensures that `/proc/self` is procfs, that nothing is mounted on top of
/// it, and that it looks normal. It also returns the `Stat` of `/proc/self`.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man5/proc.5.html
#[allow(unsafe_code)]
fn proc_self() -> io::Result<(BorrowedFd<'static>, &'static Stat)> {
    static PROC_SELF: StaticFd = StaticFd::new();

    // The init function here may run multiple times; see above.
    PROC_SELF
        .get_or_try_init(|| {
            let (proc, proc_stat) = proc()?;

            // `getpid` would return our pid in our own pid namespace, so
            // instead use `readlink` on the `self` symlink to learn our pid in
            // the procfs namespace.
            let self_symlink = open_and_check_file(proc, proc_stat, cstr!("self"), Kind::Symlink)?;
            let mut buf = [MaybeUninit::<u8>::uninit(); 20];
            let len = crate::backend::fs::syscalls::readlinkat(
                self_symlink.as_fd(),
                cstr!(""),
                &mut buf,
            )?;
            let pid: &[u8] = unsafe { core::mem::transmute(&buf[..len]) };

            // Open "/proc/self". Use our pid to compute the name rather than
            // literally using "self", as "self" is a symlink.
            let proc_self = proc_opendirat(proc, pid)?;
            let proc_self_stat = check_proc_entry(Kind::Pid, proc_self.as_fd(), Some(proc_stat))
                .map_err(|_err| io::Errno::NOTSUP)?;

            Ok(new_static_fd(proc_self, proc_self_stat))
        })
        .map(|(owned, stat)| (owned.as_fd(), stat))
}

/// Returns a handle to Linux's `/proc/self/fd` directory.
///
/// This ensures that `/proc/self/fd` is `procfs`, that nothing is mounted on
/// top of it, and that it looks normal.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man5/proc.5.html
#[cfg_attr(docsrs, doc(cfg(feature = "procfs")))]
pub fn proc_self_fd() -> io::Result<BorrowedFd<'static>> {
    static PROC_SELF_FD: StaticFd = StaticFd::new();

    // The init function here may run multiple times; see above.
    PROC_SELF_FD
        .get_or_try_init(|| {
            let (_, proc_stat) = proc()?;

            let (proc_self, _proc_self_stat) = proc_self()?;

            // Open "/proc/self/fd".
            let proc_self_fd = proc_opendirat(proc_self, cstr!("fd"))?;
            let proc_self_fd_stat =
                check_proc_entry(Kind::Fd, proc_self_fd.as_fd(), Some(proc_stat))
                    .map_err(|_err| io::Errno::NOTSUP)?;

            Ok(new_static_fd(proc_self_fd, proc_self_fd_stat))
        })
        .map(|(owned, _stat)| owned.as_fd())
}

type StaticFd = OnceCell<(OwnedFd, Stat)>;

#[inline]
fn new_static_fd(fd: OwnedFd, stat: Stat) -> (OwnedFd, Stat) {
    (fd, stat)
}

/// Returns a handle to Linux's `/proc/self/fdinfo` directory.
///
/// This ensures that `/proc/self/fdinfo` is `procfs`, that nothing is mounted
/// on top of it, and that it looks normal. It also returns the `Stat` of
/// `/proc/self/fd`.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man5/proc.5.html
fn proc_self_fdinfo() -> io::Result<(BorrowedFd<'static>, &'static Stat)> {
    static PROC_SELF_FDINFO: StaticFd = StaticFd::new();

    PROC_SELF_FDINFO
        .get_or_try_init(|| {
            let (_, proc_stat) = proc()?;

            let (proc_self, _proc_self_stat) = proc_self()?;

            // Open "/proc/self/fdinfo".
            let proc_self_fdinfo = proc_opendirat(proc_self, cstr!("fdinfo"))?;
            let proc_self_fdinfo_stat =
                check_proc_entry(Kind::Fd, proc_self_fdinfo.as_fd(), Some(proc_stat))
                    .map_err(|_err| io::Errno::NOTSUP)?;

            Ok((proc_self_fdinfo, proc_self_fdinfo_stat))
        })
        .map(|(owned, stat)| (owned.as_fd(), stat))
}

/// Returns a handle to a Linux `/proc/self/fdinfo/<fd>` file.
///
/// This ensures that `/proc/self/fdinfo/<fd>` is `procfs`, that nothing is
/// mounted on top of it, and that it looks normal.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man5/proc.5.html
#[inline]
#[cfg_attr(docsrs, doc(cfg(feature = "procfs")))]
pub fn proc_self_fdinfo_fd<Fd: AsFd>(fd: Fd) -> io::Result<OwnedFd> {
    _proc_self_fdinfo(fd.as_fd())
}

fn _proc_self_fdinfo(fd: BorrowedFd<'_>) -> io::Result<OwnedFd> {
    let (proc_self_fdinfo, proc_self_fdinfo_stat) = proc_self_fdinfo()?;
    let fd_str = DecInt::from_fd(fd);
    open_and_check_file(
        proc_self_fdinfo,
        proc_self_fdinfo_stat,
        fd_str.as_c_str(),
        Kind::File,
    )
}

/// Returns a handle to a Linux `/proc/self/pagemap` file.
///
/// This ensures that `/proc/self/pagemap` is `procfs`, that nothing is
/// mounted on top of it, and that it looks normal.
///
/// # References
///  - [Linux]
///  - [Linux pagemap]
///
/// [Linux]: https://man7.org/linux/man-pages/man5/proc.5.html
/// [Linux pagemap]: https://www.kernel.org/doc/Documentation/vm/pagemap.txt
#[inline]
#[cfg_attr(docsrs, doc(cfg(feature = "procfs")))]
pub fn proc_self_pagemap() -> io::Result<OwnedFd> {
    proc_self_file(cstr!("pagemap"))
}

/// Returns a handle to a Linux `/proc/self/maps` file.
///
/// This ensures that `/proc/self/maps` is `procfs`, that nothing is
/// mounted on top of it, and that it looks normal.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man5/proc.5.html
#[inline]
#[cfg_attr(docsrs, doc(cfg(feature = "procfs")))]
pub fn proc_self_maps() -> io::Result<OwnedFd> {
    proc_self_file(cstr!("maps"))
}

/// Returns a handle to a Linux `/proc/self/status` file.
///
/// This ensures that `/proc/self/status` is `procfs`, that nothing is
/// mounted on top of it, and that it looks normal.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man5/proc.5.html
#[inline]
#[cfg_attr(docsrs, doc(cfg(feature = "procfs")))]
pub fn proc_self_status() -> io::Result<OwnedFd> {
    proc_self_file(cstr!("status"))
}

/// Open a file under `/proc/self`.
fn proc_self_file(name: &CStr) -> io::Result<OwnedFd> {
    let (proc_self, proc_self_stat) = proc_self()?;
    open_and_check_file(proc_self, proc_self_stat, name, Kind::File)
}

/// Open a procfs file within in `dir` and check it for bind mounts.
fn open_and_check_file(
    dir: BorrowedFd<'_>,
    dir_stat: &Stat,
    name: &CStr,
    kind: Kind,
) -> io::Result<OwnedFd> {
    let (_, proc_stat) = proc()?;

    // Don't use `NOATIME`, because it [requires us to own the file], and when
    // a process sets itself non-dumpable Linux changes the user:group of its
    // `/proc/<pid>` files [to root:root].
    //
    // [requires us to own the file]: https://man7.org/linux/man-pages/man2/openat.2.html
    // [to root:root]: https://man7.org/linux/man-pages/man5/proc.5.html
    let mut oflags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NOCTTY;
    if let Kind::Symlink = kind {
        // Open symlinks with `O_PATH`.
        oflags |= OFlags::PATH;
    }
    let file = openat(dir, name, oflags, Mode::empty()).map_err(|_err| io::Errno::NOTSUP)?;
    let file_stat = fstat(&file)?;

    // `is_mountpoint` only works on directory mount points, not file mount
    // points. To detect file mount points, scan the parent directory to see if
    // we can find a regular file with an inode and name that matches the file
    // we just opened. If we can't find it, there could be a file bind mount on
    // top of the file we want.
    //
    // TODO: With Linux 5.8 we might be able to use `statx` and
    // `STATX_ATTR_MOUNT_ROOT` to detect mountpoints directly instead of doing
    // this scanning.

    let expected_type = match kind {
        Kind::File => FileType::RegularFile,
        Kind::Symlink => FileType::Symlink,
        _ => unreachable!(),
    };

    let mut found_file = false;
    let mut found_dot = false;

    // Open a new fd, so that if we're called on multiple threads, they don't
    // share a seek position.
    let oflags =
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NOCTTY | OFlags::DIRECTORY;
    let dir = openat(dir, cstr!("."), oflags, Mode::empty()).map_err(|_err| io::Errno::NOTSUP)?;
    let check_dir_stat = fstat(&dir)?;
    if check_dir_stat.st_dev != dir_stat.st_dev || check_dir_stat.st_ino != dir_stat.st_ino {
        return Err(io::Errno::NOTSUP);
    }

    // Position the directory iteration at the start.
    seek(&dir, SeekFrom::Start(0))?;

    let mut buf = [MaybeUninit::uninit(); 2048];
    let mut iter = RawDir::new(dir, &mut buf);
    while let Some(entry) = iter.next() {
        let entry = entry.map_err(|_err| io::Errno::NOTSUP)?;
        if entry.ino() == file_stat.st_ino
            && entry.file_type() == expected_type
            && entry.file_name() == name
        {
            // We found the file. Proceed to check the file handle.
            let _ = check_proc_entry_with_stat(kind, file.as_fd(), file_stat, Some(proc_stat))?;

            found_file = true;
        } else if entry.ino() == dir_stat.st_ino
            && entry.file_type() == FileType::Directory
            && entry.file_name() == cstr!(".")
        {
            // We found ".", and it's the right ".".
            found_dot = true;
        }
    }

    if found_file && found_dot {
        Ok(file)
    } else {
        Err(io::Errno::NOTSUP)
    }
}
