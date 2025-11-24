#[cfg(not(any(target_os = "redox", target_os = "haiku")))]
use std::fs;
use std::fs::File;
#[cfg(not(target_os = "redox"))]
use std::os::unix::fs::symlink;
#[cfg(not(any(target_os = "redox", target_os = "haiku")))]
use std::os::unix::fs::PermissionsExt;
#[cfg(not(target_os = "redox"))]
use std::path::Path;
#[cfg(not(any(target_os = "redox", target_os = "haiku")))]
use std::time::{Duration, UNIX_EPOCH};

use libc::mode_t;
#[cfg(not(any(target_os = "netbsd", target_os = "redox")))]
use libc::{S_IFLNK, S_IFMT};

#[cfg(not(target_os = "redox"))]
use nix::errno::Errno;
#[cfg(not(target_os = "redox"))]
use nix::fcntl;
#[cfg(any(
    target_os = "linux",
    apple_targets,
    target_os = "freebsd",
    target_os = "netbsd"
))]
use nix::sys::stat::lutimes;
#[cfg(not(any(target_os = "redox", target_os = "haiku")))]
use nix::sys::stat::utimensat;
#[cfg(not(target_os = "redox"))]
use nix::sys::stat::FchmodatFlags;
use nix::sys::stat::Mode;
#[cfg(not(any(target_os = "redox", target_os = "haiku")))]
use nix::sys::stat::UtimensatFlags;
#[cfg(not(target_os = "redox"))]
use nix::sys::stat::{self};
use nix::sys::stat::{fchmod, stat};
#[cfg(not(target_os = "redox"))]
use nix::sys::stat::{fchmodat, mkdirat};
#[cfg(not(any(target_os = "redox", target_os = "haiku")))]
use nix::sys::stat::{futimens, utimes};

#[cfg(not(any(target_os = "netbsd", target_os = "redox")))]
use nix::sys::stat::FileStat;

#[cfg(not(any(target_os = "redox", target_os = "haiku")))]
use nix::sys::time::{TimeSpec, TimeVal, TimeValLike};
#[cfg(not(target_os = "redox"))]
use nix::unistd::chdir;

#[cfg(not(any(target_os = "netbsd", target_os = "redox")))]
use nix::Result;

#[cfg(not(any(target_os = "netbsd", target_os = "redox")))]
fn assert_stat_results(stat_result: Result<FileStat>) {
    let stats = stat_result.expect("stat call failed");
    assert!(stats.st_dev > 0); // must be positive integer, exact number machine dependent
    assert!(stats.st_ino > 0); // inode is positive integer, exact number machine dependent
    assert!(stats.st_mode > 0); // must be positive integer
    assert_eq!(stats.st_nlink, 1); // there links created, must be 1
    assert_eq!(stats.st_size, 0); // size is 0 because we did not write anything to the file
    assert!(stats.st_blksize > 0); // must be positive integer, exact number machine dependent
    assert!(stats.st_blocks <= 16); // Up to 16 blocks can be allocated for a blank file
}

#[cfg(not(any(target_os = "netbsd", target_os = "redox")))]
// (Android's st_blocks is ulonglong which is always non-negative.)
#[cfg_attr(target_os = "android", allow(unused_comparisons))]
#[allow(clippy::absurd_extreme_comparisons)] // Not absurd on all OSes
fn assert_lstat_results(stat_result: Result<FileStat>) {
    let stats = stat_result.expect("stat call failed");
    assert!(stats.st_dev > 0); // must be positive integer, exact number machine dependent
    assert!(stats.st_ino > 0); // inode is positive integer, exact number machine dependent
    assert!(stats.st_mode > 0); // must be positive integer

    // st_mode is c_uint (u32 on Android) while S_IFMT is mode_t
    // (u16 on Android), and that will be a compile error.
    // On other platforms they are the same (either both are u16 or u32).
    assert_eq!(
        (stats.st_mode as usize) & (S_IFMT as usize),
        S_IFLNK as usize
    ); // should be a link
    assert_eq!(stats.st_nlink, 1); // there links created, must be 1
    assert!(stats.st_size > 0); // size is > 0 because it points to another file
    assert!(stats.st_blksize > 0); // must be positive integer, exact number machine dependent

    // st_blocks depends on whether the machine's file system uses fast
    // or slow symlinks, so just make sure it's not negative
    assert!(stats.st_blocks >= 0);
}

#[test]
#[cfg(not(any(target_os = "netbsd", target_os = "redox")))]
fn test_stat_and_fstat() {
    use nix::sys::stat::fstat;

    let tempdir = tempfile::tempdir().unwrap();
    let filename = tempdir.path().join("foo.txt");
    let file = File::create(&filename).unwrap();

    let stat_result = stat(&filename);
    assert_stat_results(stat_result);

    let fstat_result = fstat(&file);
    assert_stat_results(fstat_result);
}

#[test]
#[cfg(not(any(target_os = "netbsd", target_os = "redox")))]
fn test_fstatat() {
    let tempdir = tempfile::tempdir().unwrap();
    let filename = tempdir.path().join("foo.txt");
    File::create(&filename).unwrap();
    let dirfd =
        fcntl::open(tempdir.path(), fcntl::OFlag::empty(), stat::Mode::empty())
            .unwrap();

    let result = stat::fstatat(&dirfd, &filename, fcntl::AtFlags::empty());
    assert_stat_results(result);
}

#[test]
#[cfg(not(any(target_os = "netbsd", target_os = "redox")))]
fn test_stat_fstat_lstat() {
    use nix::sys::stat::{fstat, lstat};

    let tempdir = tempfile::tempdir().unwrap();
    let filename = tempdir.path().join("bar.txt");
    let linkname = tempdir.path().join("barlink");

    File::create(&filename).unwrap();
    symlink("bar.txt", &linkname).unwrap();
    let link = File::open(&linkname).unwrap();

    // should be the same result as calling stat,
    // since it's a regular file
    let stat_result = stat(&filename);
    assert_stat_results(stat_result);

    let lstat_result = lstat(&linkname);
    assert_lstat_results(lstat_result);

    let fstat_result = fstat(&link);
    assert_stat_results(fstat_result);
}

#[test]
fn test_fchmod() {
    let tempdir = tempfile::tempdir().unwrap();
    let filename = tempdir.path().join("foo.txt");
    let file = File::create(&filename).unwrap();

    let mut mode1 = Mode::empty();
    mode1.insert(Mode::S_IRUSR);
    mode1.insert(Mode::S_IWUSR);
    fchmod(&file, mode1).unwrap();

    let file_stat1 = stat(&filename).unwrap();
    assert_eq!(file_stat1.st_mode as mode_t & 0o7777, mode1.bits());

    let mut mode2 = Mode::empty();
    mode2.insert(Mode::S_IROTH);
    fchmod(&file, mode2).unwrap();

    let file_stat2 = stat(&filename).unwrap();
    assert_eq!(file_stat2.st_mode as mode_t & 0o7777, mode2.bits());
}

#[test]
#[cfg(not(target_os = "redox"))]
fn test_fchmodat() {
    let _dr = crate::DirRestore::new();
    let tempdir = tempfile::tempdir().unwrap();
    let filename = "foo.txt";
    let fullpath = tempdir.path().join(filename);
    File::create(&fullpath).unwrap();

    let dirfd =
        fcntl::open(tempdir.path(), fcntl::OFlag::empty(), stat::Mode::empty())
            .unwrap();

    let mut mode1 = Mode::empty();
    mode1.insert(Mode::S_IRUSR);
    mode1.insert(Mode::S_IWUSR);
    fchmodat(&dirfd, filename, mode1, FchmodatFlags::FollowSymlink).unwrap();

    let file_stat1 = stat(&fullpath).unwrap();
    assert_eq!(file_stat1.st_mode as mode_t & 0o7777, mode1.bits());

    chdir(tempdir.path()).unwrap();

    let mut mode2 = Mode::empty();
    mode2.insert(Mode::S_IROTH);
    fchmodat(
        fcntl::AT_FDCWD,
        filename,
        mode2,
        FchmodatFlags::FollowSymlink,
    )
    .unwrap();

    let file_stat2 = stat(&fullpath).unwrap();
    assert_eq!(file_stat2.st_mode as mode_t & 0o7777, mode2.bits());
}

/// Asserts that the atime and mtime in a file's metadata match expected values.
///
/// The atime and mtime are expressed with a resolution of seconds because some file systems
/// (like macOS's HFS+) do not have higher granularity.
#[cfg(not(any(target_os = "redox", target_os = "haiku")))]
fn assert_times_eq(
    exp_atime_sec: u64,
    exp_mtime_sec: u64,
    attr: &fs::Metadata,
) {
    assert_eq!(
        Duration::new(exp_atime_sec, 0),
        attr.accessed().unwrap().duration_since(UNIX_EPOCH).unwrap()
    );
    assert_eq!(
        Duration::new(exp_mtime_sec, 0),
        attr.modified().unwrap().duration_since(UNIX_EPOCH).unwrap()
    );
}

#[test]
#[cfg(not(any(target_os = "redox", target_os = "haiku")))]
fn test_utimes() {
    let tempdir = tempfile::tempdir().unwrap();
    let fullpath = tempdir.path().join("file");
    drop(File::create(&fullpath).unwrap());

    utimes(&fullpath, &TimeVal::seconds(9990), &TimeVal::seconds(5550))
        .unwrap();
    assert_times_eq(9990, 5550, &fs::metadata(&fullpath).unwrap());
}

#[test]
#[cfg(any(
    target_os = "linux",
    apple_targets,
    target_os = "freebsd",
    target_os = "netbsd"
))]
fn test_lutimes() {
    let tempdir = tempfile::tempdir().unwrap();
    let target = tempdir.path().join("target");
    let fullpath = tempdir.path().join("symlink");
    drop(File::create(&target).unwrap());
    symlink(&target, &fullpath).unwrap();

    let exp_target_metadata = fs::symlink_metadata(&target).unwrap();
    lutimes(&fullpath, &TimeVal::seconds(4560), &TimeVal::seconds(1230))
        .unwrap();
    assert_times_eq(4560, 1230, &fs::symlink_metadata(&fullpath).unwrap());

    let target_metadata = fs::symlink_metadata(&target).unwrap();
    assert_eq!(
        exp_target_metadata.accessed().unwrap(),
        target_metadata.accessed().unwrap(),
        "atime of symlink target was unexpectedly modified"
    );
    assert_eq!(
        exp_target_metadata.modified().unwrap(),
        target_metadata.modified().unwrap(),
        "mtime of symlink target was unexpectedly modified"
    );
}

#[test]
#[cfg(not(any(target_os = "redox", target_os = "haiku")))]
fn test_futimens() {
    let tempdir = tempfile::tempdir().unwrap();
    let fullpath = tempdir.path().join("file");
    drop(File::create(&fullpath).unwrap());

    let fd = fcntl::open(&fullpath, fcntl::OFlag::empty(), stat::Mode::empty())
        .unwrap();

    futimens(&fd, &TimeSpec::seconds(10), &TimeSpec::seconds(20)).unwrap();
    assert_times_eq(10, 20, &fs::metadata(&fullpath).unwrap());
}

#[test]
#[cfg(not(any(target_os = "redox", target_os = "haiku")))]
fn test_utimensat() {
    let _dr = crate::DirRestore::new();
    let tempdir = tempfile::tempdir().unwrap();
    let filename = "foo.txt";
    let fullpath = tempdir.path().join(filename);
    drop(File::create(&fullpath).unwrap());

    let dirfd =
        fcntl::open(tempdir.path(), fcntl::OFlag::empty(), stat::Mode::empty())
            .unwrap();

    utimensat(
        &dirfd,
        filename,
        &TimeSpec::seconds(12345),
        &TimeSpec::seconds(678),
        UtimensatFlags::FollowSymlink,
    )
    .unwrap();
    assert_times_eq(12345, 678, &fs::metadata(&fullpath).unwrap());

    chdir(tempdir.path()).unwrap();

    utimensat(
        fcntl::AT_FDCWD,
        filename,
        &TimeSpec::seconds(500),
        &TimeSpec::seconds(800),
        UtimensatFlags::FollowSymlink,
    )
    .unwrap();
    assert_times_eq(500, 800, &fs::metadata(&fullpath).unwrap());
}

#[test]
#[cfg(not(target_os = "redox"))]
fn test_mkdirat_success_path() {
    let tempdir = tempfile::tempdir().unwrap();
    let filename = "example_subdir";
    let dirfd =
        fcntl::open(tempdir.path(), fcntl::OFlag::empty(), stat::Mode::empty())
            .unwrap();
    mkdirat(&dirfd, filename, Mode::S_IRWXU).expect("mkdirat failed");
    assert!(Path::exists(&tempdir.path().join(filename)));
}

#[test]
#[cfg(not(any(target_os = "redox", target_os = "haiku")))]
fn test_mkdirat_success_mode() {
    let expected_bits =
        stat::SFlag::S_IFDIR.bits() | stat::Mode::S_IRWXU.bits();
    let tempdir = tempfile::tempdir().unwrap();
    let filename = "example_subdir";
    let dirfd =
        fcntl::open(tempdir.path(), fcntl::OFlag::empty(), stat::Mode::empty())
            .unwrap();
    mkdirat(&dirfd, filename, Mode::S_IRWXU).expect("mkdirat failed");
    let permissions = fs::metadata(tempdir.path().join(filename))
        .unwrap()
        .permissions();
    let mode = permissions.mode();
    assert_eq!(mode as mode_t, expected_bits)
}

#[test]
#[cfg(not(target_os = "redox"))]
fn test_mkdirat_fail() {
    let tempdir = tempfile::tempdir().unwrap();
    let not_dir_filename = "example_not_dir";
    let filename = "example_subdir_dir";
    let dirfd = fcntl::open(
        &tempdir.path().join(not_dir_filename),
        fcntl::OFlag::O_CREAT,
        stat::Mode::empty(),
    )
    .unwrap();
    let result = mkdirat(dirfd, filename, Mode::S_IRWXU).unwrap_err();
    assert_eq!(result, Errno::ENOTDIR);
}

#[test]
#[cfg(not(any(
    freebsdlike,
    apple_targets,
    target_os = "haiku",
    target_os = "redox",
    target_os = "solaris"
)))]
fn test_mknod() {
    use stat::{lstat, mknod, SFlag};

    let file_name = "test_file";
    let tempdir = tempfile::tempdir().unwrap();
    let target = tempdir.path().join(file_name);
    mknod(&target, SFlag::S_IFREG, Mode::S_IRWXU, 0).unwrap();
    let mode = lstat(&target).unwrap().st_mode as mode_t;
    assert_eq!(mode & libc::S_IFREG, libc::S_IFREG);
    assert_eq!(mode & libc::S_IRWXU, libc::S_IRWXU);
}

#[test]
#[cfg(not(any(
    solarish,
    freebsdlike,
    apple_targets,
    target_os = "haiku",
    target_os = "redox"
)))]
fn test_mknodat() {
    use fcntl::{AtFlags, OFlag};
    use nix::dir::Dir;
    use stat::{fstatat, mknodat, SFlag};

    let file_name = "test_file";
    let tempdir = tempfile::tempdir().unwrap();
    let target_dir =
        Dir::open(tempdir.path(), OFlag::O_DIRECTORY, Mode::S_IRWXU).unwrap();
    mknodat(&target_dir, file_name, SFlag::S_IFREG, Mode::S_IRWXU, 0).unwrap();
    let mode = fstatat(&target_dir, file_name, AtFlags::AT_SYMLINK_NOFOLLOW)
        .unwrap()
        .st_mode as mode_t;
    assert_eq!(mode & libc::S_IFREG, libc::S_IFREG);
    assert_eq!(mode & libc::S_IRWXU, libc::S_IRWXU);
}

#[test]
#[cfg(not(any(target_os = "redox", target_os = "haiku")))]
fn test_futimens_unchanged() {
    let tempdir = tempfile::tempdir().unwrap();
    let fullpath = tempdir.path().join("file");
    drop(File::create(&fullpath).unwrap());
    let fd = fcntl::open(&fullpath, fcntl::OFlag::empty(), stat::Mode::empty())
        .unwrap();

    let old_atime = fs::metadata(fullpath.as_path())
        .unwrap()
        .accessed()
        .unwrap();
    let old_mtime = fs::metadata(fullpath.as_path())
        .unwrap()
        .modified()
        .unwrap();

    futimens(&fd, &TimeSpec::UTIME_OMIT, &TimeSpec::UTIME_OMIT).unwrap();

    let new_atime = fs::metadata(fullpath.as_path())
        .unwrap()
        .accessed()
        .unwrap();
    let new_mtime = fs::metadata(fullpath.as_path())
        .unwrap()
        .modified()
        .unwrap();
    assert_eq!(old_atime, new_atime);
    assert_eq!(old_mtime, new_mtime);
}

#[test]
#[cfg(not(any(target_os = "redox", target_os = "haiku")))]
fn test_utimensat_unchanged() {
    let _dr = crate::DirRestore::new();
    let tempdir = tempfile::tempdir().unwrap();
    let filename = "foo.txt";
    let fullpath = tempdir.path().join(filename);
    drop(File::create(&fullpath).unwrap());
    let dirfd =
        fcntl::open(tempdir.path(), fcntl::OFlag::empty(), stat::Mode::empty())
            .unwrap();

    let old_atime = fs::metadata(fullpath.as_path())
        .unwrap()
        .accessed()
        .unwrap();
    let old_mtime = fs::metadata(fullpath.as_path())
        .unwrap()
        .modified()
        .unwrap();
    utimensat(
        &dirfd,
        filename,
        &TimeSpec::UTIME_OMIT,
        &TimeSpec::UTIME_OMIT,
        UtimensatFlags::NoFollowSymlink,
    )
    .unwrap();
    let new_atime = fs::metadata(fullpath.as_path())
        .unwrap()
        .accessed()
        .unwrap();
    let new_mtime = fs::metadata(fullpath.as_path())
        .unwrap()
        .modified()
        .unwrap();
    assert_eq!(old_atime, new_atime);
    assert_eq!(old_mtime, new_mtime);
}

// The conversion is not useless on all platforms.
#[allow(clippy::useless_conversion)]
#[cfg(target_os = "freebsd")]
#[test]
fn test_chflags() {
    use nix::{
        sys::stat::{fstat, FileFlag},
        unistd::chflags,
    };
    use tempfile::NamedTempFile;

    let f = NamedTempFile::new().unwrap();

    let initial =
        FileFlag::from_bits_truncate(fstat(&f).unwrap().st_flags.into());
    // UF_OFFLINE is preserved by all FreeBSD file systems, but not interpreted
    // in any way, so it's handy for testing.
    let commanded = initial ^ FileFlag::UF_OFFLINE;

    chflags(f.path(), commanded).unwrap();

    let changed =
        FileFlag::from_bits_truncate(fstat(&f).unwrap().st_flags.into());

    assert_eq!(commanded, changed);
}
