use libc::{_exit, mode_t, off_t};
use nix::errno::Errno;
#[cfg(not(any(target_os = "redox", target_os = "haiku")))]
use nix::fcntl::readlink;
use nix::fcntl::OFlag;
#[cfg(not(target_os = "redox"))]
use nix::fcntl::{self, open};
#[cfg(not(any(
    target_os = "redox",
    target_os = "fuchsia",
    target_os = "haiku"
)))]
use nix::pty::{grantpt, posix_openpt, ptsname, unlockpt};
#[cfg(not(target_os = "redox"))]
use nix::sys::signal::{
    sigaction, SaFlags, SigAction, SigHandler, SigSet, Signal,
};
use nix::sys::stat::{self, Mode, SFlag};
use nix::sys::wait::*;
use nix::unistd::ForkResult::*;
use nix::unistd::*;
use std::env;
#[cfg(not(any(target_os = "fuchsia", target_os = "redox")))]
use std::ffi::CString;
#[cfg(not(target_os = "redox"))]
use std::fs::DirBuilder;
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::prelude::*;
#[cfg(not(any(
    target_os = "fuchsia",
    target_os = "redox",
    target_os = "haiku"
)))]
use std::path::Path;
use tempfile::{tempdir, tempfile};

use crate::*;

#[test]
#[cfg(not(any(target_os = "netbsd")))]
fn test_fork_and_waitpid() {
    let _m = crate::FORK_MTX.lock();

    // Safe: Child only calls `_exit`, which is signal-safe
    match unsafe { fork() }.expect("Error: Fork Failed") {
        Child => unsafe { _exit(0) },
        Parent { child } => {
            // assert that child was created and pid > 0
            let child_raw: ::libc::pid_t = child.into();
            assert!(child_raw > 0);
            let wait_status = waitpid(child, None);
            match wait_status {
                // assert that waitpid returned correct status and the pid is the one of the child
                Ok(WaitStatus::Exited(pid_t, _)) => assert_eq!(pid_t, child),

                // panic, must never happen
                s @ Ok(_) => {
                    panic!("Child exited {s:?}, should never happen")
                }

                // panic, waitpid should never fail
                Err(s) => panic!("Error: waitpid returned Err({s:?}"),
            }
        }
    }
}

#[test]
#[cfg(target_os = "freebsd")]
fn test_rfork_and_waitpid() {
    let _m = crate::FORK_MTX.lock();

    // Safe: Child only calls `_exit`, which is signal-safe
    match unsafe { rfork(RforkFlags::RFPROC | RforkFlags::RFTHREAD) }
        .expect("Error: Rfork Failed")
    {
        Child => unsafe { _exit(0) },
        Parent { child } => {
            // assert that child was created and pid > 0
            let child_raw: ::libc::pid_t = child.into();
            assert!(child_raw > 0);
            let wait_status = waitpid(child, None);
            match wait_status {
                // assert that waitpid returned correct status and the pid is the one of the child
                Ok(WaitStatus::Exited(pid_t, _)) => assert_eq!(pid_t, child),

                // panic, must never happen
                s @ Ok(_) => {
                    panic!("Child exited {s:?}, should never happen")
                }

                // panic, waitpid should never fail
                Err(s) => panic!("Error: waitpid returned Err({s:?}"),
            }
        }
    }
}

#[test]
fn test_wait() {
    // Grab FORK_MTX so wait doesn't reap a different test's child process
    let _m = crate::FORK_MTX.lock();

    // Safe: Child only calls `_exit`, which is signal-safe
    match unsafe { fork() }.expect("Error: Fork Failed") {
        Child => unsafe { _exit(0) },
        Parent { child } => {
            let wait_status = wait();

            // just assert that (any) one child returns with WaitStatus::Exited
            assert_eq!(wait_status, Ok(WaitStatus::Exited(child, 0)));
        }
    }
}

#[test]
fn test_mkstemp() {
    let mut path = env::temp_dir();
    path.push("nix_tempfile.XXXXXX");

    let result = mkstemp(&path);
    match result {
        Ok((fd, path)) => {
            close(fd).unwrap();
            unlink(path.as_path()).unwrap();
        }
        Err(e) => panic!("mkstemp failed: {e}"),
    }
}

#[test]
fn test_mkstemp_directory() {
    // mkstemp should fail if a directory is given
    mkstemp(&env::temp_dir()).expect_err("assertion failed");
}

#[test]
#[cfg(not(target_os = "redox"))]
fn test_mkfifo() {
    let tempdir = tempdir().unwrap();
    let mkfifo_fifo = tempdir.path().join("mkfifo_fifo");

    mkfifo(&mkfifo_fifo, Mode::S_IRUSR).unwrap();

    let stats = stat::stat(&mkfifo_fifo).unwrap();
    let typ = stat::SFlag::from_bits_truncate(stats.st_mode as mode_t);
    assert_eq!(typ, SFlag::S_IFIFO);
}

#[test]
#[cfg(not(target_os = "redox"))]
fn test_mkfifo_directory() {
    // mkfifo should fail if a directory is given
    mkfifo(&env::temp_dir(), Mode::S_IRUSR).expect_err("assertion failed");
}

#[test]
#[cfg(not(any(
    apple_targets,
    target_os = "android",
    target_os = "redox",
    target_os = "haiku"
)))]
fn test_mkfifoat_none() {
    let _m = crate::CWD_LOCK.read();

    let tempdir = tempdir().unwrap();
    let mkfifoat_fifo = tempdir.path().join("mkfifoat_fifo");

    mkfifoat(None, &mkfifoat_fifo, Mode::S_IRUSR).unwrap();

    let stats = stat::stat(&mkfifoat_fifo).unwrap();
    let typ = stat::SFlag::from_bits_truncate(stats.st_mode);
    assert_eq!(typ, SFlag::S_IFIFO);
}

#[test]
#[cfg(not(any(
    apple_targets,
    target_os = "android",
    target_os = "redox",
    target_os = "haiku"
)))]
fn test_mkfifoat() {
    use nix::fcntl;

    let tempdir = tempdir().unwrap();
    let dirfd = open(tempdir.path(), OFlag::empty(), Mode::empty()).unwrap();
    let mkfifoat_name = "mkfifoat_name";

    mkfifoat(Some(dirfd), mkfifoat_name, Mode::S_IRUSR).unwrap();

    let stats =
        stat::fstatat(Some(dirfd), mkfifoat_name, fcntl::AtFlags::empty())
            .unwrap();
    let typ = stat::SFlag::from_bits_truncate(stats.st_mode);
    assert_eq!(typ, SFlag::S_IFIFO);
}

#[test]
#[cfg(not(any(
    apple_targets,
    target_os = "android",
    target_os = "redox",
    target_os = "haiku"
)))]
fn test_mkfifoat_directory_none() {
    let _m = crate::CWD_LOCK.read();

    // mkfifoat should fail if a directory is given
    mkfifoat(None, &env::temp_dir(), Mode::S_IRUSR)
        .expect_err("assertion failed");
}

#[test]
#[cfg(not(any(
    apple_targets,
    target_os = "android",
    target_os = "redox",
    target_os = "haiku"
)))]
fn test_mkfifoat_directory() {
    // mkfifoat should fail if a directory is given
    let tempdir = tempdir().unwrap();
    let dirfd = open(tempdir.path(), OFlag::empty(), Mode::empty()).unwrap();
    let mkfifoat_dir = "mkfifoat_dir";
    stat::mkdirat(Some(dirfd), mkfifoat_dir, Mode::S_IRUSR).unwrap();

    mkfifoat(Some(dirfd), mkfifoat_dir, Mode::S_IRUSR)
        .expect_err("assertion failed");
}

#[test]
fn test_getpid() {
    let pid: ::libc::pid_t = getpid().into();
    let ppid: ::libc::pid_t = getppid().into();
    assert!(pid > 0);
    assert!(ppid > 0);
}

#[test]
#[cfg(not(target_os = "redox"))]
fn test_getsid() {
    let none_sid: ::libc::pid_t = getsid(None).unwrap().into();
    let pid_sid: ::libc::pid_t = getsid(Some(getpid())).unwrap().into();
    assert!(none_sid > 0);
    assert_eq!(none_sid, pid_sid);
}

#[cfg(linux_android)]
mod linux_android {
    use nix::unistd::gettid;

    #[test]
    fn test_gettid() {
        let tid: ::libc::pid_t = gettid().into();
        assert!(tid > 0);
    }
}

#[test]
// `getgroups()` and `setgroups()` do not behave as expected on Apple platforms
#[cfg(not(any(
    apple_targets,
    target_os = "redox",
    target_os = "fuchsia",
    target_os = "haiku"
)))]
fn test_setgroups() {
    // Skip this test when not run as root as `setgroups()` requires root.
    skip_if_not_root!("test_setgroups");

    let _m = crate::GROUPS_MTX.lock();

    // Save the existing groups
    let old_groups = getgroups().unwrap();

    // Set some new made up groups
    let groups = [Gid::from_raw(123), Gid::from_raw(456)];
    setgroups(&groups).unwrap();

    let new_groups = getgroups().unwrap();
    assert_eq!(new_groups, groups);

    // Revert back to the old groups
    setgroups(&old_groups).unwrap();
}

#[test]
// `getgroups()` and `setgroups()` do not behave as expected on Apple platforms
#[cfg(not(any(
    apple_targets,
    target_os = "redox",
    target_os = "fuchsia",
    target_os = "haiku",
    solarish
)))]
fn test_initgroups() {
    // Skip this test when not run as root as `initgroups()` and `setgroups()`
    // require root.
    skip_if_not_root!("test_initgroups");

    let _m = crate::GROUPS_MTX.lock();

    // Save the existing groups
    let old_groups = getgroups().unwrap();

    // It doesn't matter if the root user is not called "root" or if a user
    // called "root" doesn't exist. We are just checking that the extra,
    // made-up group, `123`, is set.
    // FIXME: Test the other half of initgroups' functionality: whether the
    // groups that the user belongs to are also set.
    let user = CString::new("root").unwrap();
    let group = Gid::from_raw(123);
    let group_list = getgrouplist(&user, group).unwrap();
    assert!(group_list.contains(&group));

    initgroups(&user, group).unwrap();

    let new_groups = getgroups().unwrap();
    assert_eq!(new_groups, group_list);

    // Revert back to the old groups
    setgroups(&old_groups).unwrap();
}

#[cfg(not(any(target_os = "fuchsia", target_os = "redox")))]
macro_rules! execve_test_factory (
    ($test_name:ident, $syscall:ident, $exe: expr $(, $pathname:expr, $flags:expr)*) => (

    #[cfg(test)]
    mod $test_name {
    use std::ffi::CStr;
    use super::*;

    const EMPTY: &'static [u8] = b"\0";
    const DASH_C: &'static [u8] = b"-c\0";
    const BIGARG: &'static [u8] = b"echo nix!!! && echo foo=$foo && echo baz=$baz\0";
    const FOO: &'static [u8] = b"foo=bar\0";
    const BAZ: &'static [u8] = b"baz=quux\0";

    fn syscall_cstr_ref() -> Result<std::convert::Infallible, nix::Error> {
        $syscall(
            $exe,
            $(CString::new($pathname).unwrap().as_c_str(), )*
            &[CStr::from_bytes_with_nul(EMPTY).unwrap(),
              CStr::from_bytes_with_nul(DASH_C).unwrap(),
              CStr::from_bytes_with_nul(BIGARG).unwrap()],
            &[CStr::from_bytes_with_nul(FOO).unwrap(),
              CStr::from_bytes_with_nul(BAZ).unwrap()]
            $(, $flags)*)
    }

    fn syscall_cstring() -> Result<std::convert::Infallible, nix::Error> {
        $syscall(
            $exe,
            $(CString::new($pathname).unwrap().as_c_str(), )*
            &[CString::from(CStr::from_bytes_with_nul(EMPTY).unwrap()),
              CString::from(CStr::from_bytes_with_nul(DASH_C).unwrap()),
              CString::from(CStr::from_bytes_with_nul(BIGARG).unwrap())],
            &[CString::from(CStr::from_bytes_with_nul(FOO).unwrap()),
              CString::from(CStr::from_bytes_with_nul(BAZ).unwrap())]
            $(, $flags)*)
    }

    fn common_test(syscall: fn() -> Result<std::convert::Infallible, nix::Error>) {
        if "execveat" == stringify!($syscall) {
            // Though undocumented, Docker's default seccomp profile seems to
            // block this syscall.  https://github.com/nix-rust/nix/issues/1122
            skip_if_seccomp!($test_name);
        }

        let m = crate::FORK_MTX.lock();
        // The `exec`d process will write to `writer`, and we'll read that
        // data from `reader`.
        let (reader, writer) = pipe().unwrap();

        // Safe: Child calls `exit`, `dup`, `close` and the provided `exec*` family function.
        // NOTE: Technically, this makes the macro unsafe to use because you could pass anything.
        //       The tests make sure not to do that, though.
        match unsafe{fork()}.unwrap() {
            Child => {
                // Make `writer` be the stdout of the new process.
                dup2(writer.as_raw_fd(), 1).unwrap();
                let r = syscall();
                let _ = std::io::stderr()
                    .write_all(format!("{:?}", r).as_bytes());
                // Should only get here in event of error
                unsafe{ _exit(1) };
            },
            Parent { child } => {
                // Wait for the child to exit.
                let ws = waitpid(child, None);
                drop(m);
                assert_eq!(ws, Ok(WaitStatus::Exited(child, 0)));
                // Read 1024 bytes.
                let mut buf = [0u8; 1024];
                read(reader.as_raw_fd(), &mut buf).unwrap();
                // It should contain the things we printed using `/bin/sh`.
                let string = String::from_utf8_lossy(&buf);
                assert!(string.contains("nix!!!"));
                assert!(string.contains("foo=bar"));
                assert!(string.contains("baz=quux"));
            }
        }
    }

    // These tests frequently fail on musl, probably due to
        // https://github.com/nix-rust/nix/issues/555
    #[cfg_attr(target_env = "musl", ignore)]
    #[test]
    fn test_cstr_ref() {
        common_test(syscall_cstr_ref);
    }

    // These tests frequently fail on musl, probably due to
        // https://github.com/nix-rust/nix/issues/555
    #[cfg_attr(target_env = "musl", ignore)]
    #[test]
    fn test_cstring() {
        common_test(syscall_cstring);
    }
    }

    )
);

cfg_if! {
    if #[cfg(target_os = "android")] {
        execve_test_factory!(test_execve, execve, CString::new("/system/bin/sh").unwrap().as_c_str());
        execve_test_factory!(test_fexecve, fexecve, File::open("/system/bin/sh").unwrap().into_raw_fd());
    } else if #[cfg(any(freebsdlike, target_os = "linux", target_os = "hurd"))] {
        // These tests frequently fail on musl, probably due to
        // https://github.com/nix-rust/nix/issues/555
        execve_test_factory!(test_execve, execve, CString::new("/bin/sh").unwrap().as_c_str());
        execve_test_factory!(test_fexecve, fexecve, File::open("/bin/sh").unwrap().into_raw_fd());
    } else if #[cfg(any(solarish, apple_targets, netbsdlike))] {
        execve_test_factory!(test_execve, execve, CString::new("/bin/sh").unwrap().as_c_str());
        // No fexecve() on ios, macos, NetBSD, OpenBSD.
    }
}

#[cfg(any(
    target_os = "haiku",
    target_os = "hurd",
    target_os = "linux",
    target_os = "openbsd"
))]
execve_test_factory!(test_execvpe, execvpe, &CString::new("sh").unwrap());

cfg_if! {
    if #[cfg(target_os = "android")] {
        use nix::fcntl::AtFlags;
        execve_test_factory!(test_execveat_empty, execveat,
                             Some(File::open("/system/bin/sh").unwrap().into_raw_fd()),
                             "", AtFlags::AT_EMPTY_PATH);
        execve_test_factory!(test_execveat_relative, execveat,
                             Some(File::open("/system/bin/").unwrap().into_raw_fd()),
                             "./sh", AtFlags::empty());
        execve_test_factory!(test_execveat_absolute, execveat,
                             Some(File::open("/").unwrap().into_raw_fd()),
                             "/system/bin/sh", AtFlags::empty());
    } else if #[cfg(all(target_os = "linux", any(target_arch ="x86_64", target_arch ="x86")))] {
        use nix::fcntl::AtFlags;
        execve_test_factory!(test_execveat_empty, execveat, Some(File::open("/bin/sh").unwrap().into_raw_fd()),
                             "", AtFlags::AT_EMPTY_PATH);
        execve_test_factory!(test_execveat_relative, execveat, Some(File::open("/bin/").unwrap().into_raw_fd()),
                             "./sh", AtFlags::empty());
        execve_test_factory!(test_execveat_absolute, execveat, Some(File::open("/").unwrap().into_raw_fd()),
                             "/bin/sh", AtFlags::empty());
    }
}

#[test]
#[cfg(not(target_os = "fuchsia"))]
fn test_fchdir() {
    // fchdir changes the process's cwd
    let _dr = crate::DirRestore::new();

    let tmpdir = tempdir().unwrap();
    let tmpdir_path = tmpdir.path().canonicalize().unwrap();
    let tmpdir_fd = File::open(&tmpdir_path).unwrap().into_raw_fd();

    fchdir(tmpdir_fd).expect("assertion failed");
    assert_eq!(getcwd().unwrap(), tmpdir_path);

    close(tmpdir_fd).expect("assertion failed");
}

#[test]
fn test_getcwd() {
    // chdir changes the process's cwd
    let _dr = crate::DirRestore::new();

    let tmpdir = tempdir().unwrap();
    let tmpdir_path = tmpdir.path().canonicalize().unwrap();
    chdir(&tmpdir_path).expect("assertion failed");
    assert_eq!(getcwd().unwrap(), tmpdir_path);

    // make path 500 chars longer so that buffer doubling in getcwd
    // kicks in.  Note: One path cannot be longer than 255 bytes
    // (NAME_MAX) whole path cannot be longer than PATH_MAX (usually
    // 4096 on linux, 1024 on macos)
    let mut inner_tmp_dir = tmpdir_path;
    for _ in 0..5 {
        let newdir = "a".repeat(100);
        inner_tmp_dir.push(newdir);
        mkdir(inner_tmp_dir.as_path(), Mode::S_IRWXU)
            .expect("assertion failed");
    }
    chdir(inner_tmp_dir.as_path()).expect("assertion failed");
    assert_eq!(getcwd().unwrap(), inner_tmp_dir.as_path());
}

#[test]
fn test_chown() {
    // Testing for anything other than our own UID/GID is hard.
    let uid = Some(getuid());
    let gid = Some(getgid());

    let tempdir = tempdir().unwrap();
    let path = tempdir.path().join("file");
    {
        File::create(&path).unwrap();
    }

    chown(&path, uid, gid).unwrap();
    chown(&path, uid, None).unwrap();
    chown(&path, None, gid).unwrap();

    fs::remove_file(&path).unwrap();
    chown(&path, uid, gid).unwrap_err();
}

#[test]
fn test_fchown() {
    // Testing for anything other than our own UID/GID is hard.
    let uid = Some(getuid());
    let gid = Some(getgid());

    let path = tempfile().unwrap();
    let fd = path.as_raw_fd();

    fchown(fd, uid, gid).unwrap();
    fchown(fd, uid, None).unwrap();
    fchown(fd, None, gid).unwrap();
    fchown(999999999, uid, gid).unwrap_err();
}

#[test]
#[cfg(not(target_os = "redox"))]
fn test_fchownat() {
    use nix::fcntl::AtFlags;

    let _dr = crate::DirRestore::new();
    // Testing for anything other than our own UID/GID is hard.
    let uid = Some(getuid());
    let gid = Some(getgid());

    let tempdir = tempdir().unwrap();
    let path = tempdir.path().join("file");
    {
        File::create(&path).unwrap();
    }

    let dirfd = open(tempdir.path(), OFlag::empty(), Mode::empty()).unwrap();

    fchownat(Some(dirfd), "file", uid, gid, AtFlags::empty()).unwrap();

    chdir(tempdir.path()).unwrap();
    fchownat(None, "file", uid, gid, AtFlags::empty()).unwrap();

    fs::remove_file(&path).unwrap();
    fchownat(None, "file", uid, gid, AtFlags::empty()).unwrap_err();
}

#[test]
fn test_lseek() {
    const CONTENTS: &[u8] = b"abcdef123456";
    let mut tmp = tempfile().unwrap();
    tmp.write_all(CONTENTS).unwrap();

    let offset: off_t = 5;
    lseek(tmp.as_raw_fd(), offset, Whence::SeekSet).unwrap();

    let mut buf = [0u8; 7];
    crate::read_exact(&tmp, &mut buf);
    assert_eq!(b"f123456", &buf);
}

#[cfg(linux_android)]
#[test]
fn test_lseek64() {
    const CONTENTS: &[u8] = b"abcdef123456";
    let mut tmp = tempfile().unwrap();
    tmp.write_all(CONTENTS).unwrap();

    lseek64(tmp.as_raw_fd(), 5, Whence::SeekSet).unwrap();

    let mut buf = [0u8; 7];
    crate::read_exact(&tmp, &mut buf);
    assert_eq!(b"f123456", &buf);
}

cfg_if! {
    if #[cfg(linux_android)] {
        macro_rules! require_acct{
            () => {
                require_capability!("test_acct", CAP_SYS_PACCT);
            }
        }
    } else if #[cfg(target_os = "freebsd")] {
        macro_rules! require_acct{
            () => {
                skip_if_not_root!("test_acct");
                skip_if_jailed!("test_acct");
            }
        }
    } else if #[cfg(not(any(target_os = "redox", target_os = "fuchsia", target_os = "haiku")))] {
        macro_rules! require_acct{
            () => {
                skip_if_not_root!("test_acct");
            }
        }
    }
}

#[test]
#[cfg(not(any(
    target_os = "redox",
    target_os = "fuchsia",
    target_os = "haiku"
)))]
fn test_acct() {
    use std::process::Command;
    use std::{thread, time};
    use tempfile::NamedTempFile;

    let _m = crate::FORK_MTX.lock();
    require_acct!();

    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_str().unwrap();

    acct::enable(path).unwrap();

    loop {
        Command::new("echo").arg("Hello world").output().unwrap();
        let len = fs::metadata(path).unwrap().len();
        if len > 0 {
            break;
        }
        thread::sleep(time::Duration::from_millis(10));
    }
    acct::disable().unwrap();
}

#[cfg_attr(target_os = "hurd", ignore)]
#[test]
fn test_fpathconf_limited() {
    let f = tempfile().unwrap();
    // PATH_MAX is limited on most platforms, so it makes a good test
    let path_max = fpathconf(f, PathconfVar::PATH_MAX);
    assert!(
        path_max
            .expect("fpathconf failed")
            .expect("PATH_MAX is unlimited")
            > 0
    );
}

#[cfg_attr(target_os = "hurd", ignore)]
#[test]
fn test_pathconf_limited() {
    // PATH_MAX is limited on most platforms, so it makes a good test
    let path_max = pathconf("/", PathconfVar::PATH_MAX);
    assert!(
        path_max
            .expect("pathconf failed")
            .expect("PATH_MAX is unlimited")
            > 0
    );
}

#[cfg_attr(target_os = "hurd", ignore)]
#[test]
fn test_sysconf_limited() {
    // OPEN_MAX is limited on most platforms, so it makes a good test
    let open_max = sysconf(SysconfVar::OPEN_MAX);
    assert!(
        open_max
            .expect("sysconf failed")
            .expect("OPEN_MAX is unlimited")
            > 0
    );
}

#[cfg(target_os = "freebsd")]
#[test]
fn test_sysconf_unsupported() {
    // I know of no sysconf variables that are unsupported everywhere, but
    // _XOPEN_CRYPT is unsupported on FreeBSD 11.0, which is one of the platforms
    // we test.
    let open_max = sysconf(SysconfVar::_XOPEN_CRYPT);
    assert!(open_max.expect("sysconf failed").is_none())
}

#[cfg(any(linux_android, freebsdlike, target_os = "openbsd"))]
#[test]
fn test_getresuid() {
    let resuids = getresuid().unwrap();
    assert_ne!(resuids.real.as_raw(), libc::uid_t::MAX);
    assert_ne!(resuids.effective.as_raw(), libc::uid_t::MAX);
    assert_ne!(resuids.saved.as_raw(), libc::uid_t::MAX);
}

#[cfg(any(linux_android, freebsdlike, target_os = "openbsd"))]
#[test]
fn test_getresgid() {
    let resgids = getresgid().unwrap();
    assert_ne!(resgids.real.as_raw(), libc::gid_t::MAX);
    assert_ne!(resgids.effective.as_raw(), libc::gid_t::MAX);
    assert_ne!(resgids.saved.as_raw(), libc::gid_t::MAX);
}

// Test that we can create a pair of pipes.  No need to verify that they pass
// data; that's the domain of the OS, not nix.
#[test]
fn test_pipe() {
    let (fd0, fd1) = pipe().unwrap();
    let m0 = stat::SFlag::from_bits_truncate(
        stat::fstat(fd0.as_raw_fd()).unwrap().st_mode as mode_t,
    );
    // S_IFIFO means it's a pipe
    assert_eq!(m0, SFlag::S_IFIFO);
    let m1 = stat::SFlag::from_bits_truncate(
        stat::fstat(fd1.as_raw_fd()).unwrap().st_mode as mode_t,
    );
    assert_eq!(m1, SFlag::S_IFIFO);
}

// pipe2(2) is the same as pipe(2), except it allows setting some flags.  Check
// that we can set a flag.
#[cfg(any(
    linux_android,
    freebsdlike,
    solarish,
    netbsdlike,
    target_os = "emscripten",
    target_os = "redox",
))]
#[test]
fn test_pipe2() {
    use nix::fcntl::{fcntl, FcntlArg, FdFlag};

    let (fd0, fd1) = pipe2(OFlag::O_CLOEXEC).unwrap();
    let f0 = FdFlag::from_bits_truncate(
        fcntl(fd0.as_raw_fd(), FcntlArg::F_GETFD).unwrap(),
    );
    assert!(f0.contains(FdFlag::FD_CLOEXEC));
    let f1 = FdFlag::from_bits_truncate(
        fcntl(fd1.as_raw_fd(), FcntlArg::F_GETFD).unwrap(),
    );
    assert!(f1.contains(FdFlag::FD_CLOEXEC));
}

#[test]
#[cfg(not(any(target_os = "redox", target_os = "fuchsia")))]
fn test_truncate() {
    let tempdir = tempdir().unwrap();
    let path = tempdir.path().join("file");

    {
        let mut tmp = File::create(&path).unwrap();
        const CONTENTS: &[u8] = b"12345678";
        tmp.write_all(CONTENTS).unwrap();
    }

    truncate(&path, 4).unwrap();

    let metadata = fs::metadata(&path).unwrap();
    assert_eq!(4, metadata.len());
}

#[test]
fn test_ftruncate() {
    let tempdir = tempdir().unwrap();
    let path = tempdir.path().join("file");

    let mut file = File::create(&path).unwrap();
    const CONTENTS: &[u8] = b"12345678";
    file.write_all(CONTENTS).unwrap();

    ftruncate(&file, 2).unwrap();
    drop(file);

    let metadata = fs::metadata(&path).unwrap();
    assert_eq!(2, metadata.len());
}

// Used in `test_alarm`.
#[cfg(not(target_os = "redox"))]
static mut ALARM_CALLED: bool = false;

// Used in `test_alarm`.
#[cfg(not(target_os = "redox"))]
pub extern "C" fn alarm_signal_handler(raw_signal: libc::c_int) {
    assert_eq!(raw_signal, libc::SIGALRM, "unexpected signal: {raw_signal}");
    unsafe { ALARM_CALLED = true };
}

#[test]
#[cfg(not(target_os = "redox"))]
fn test_alarm() {
    use std::{
        thread,
        time::{Duration, Instant},
    };

    // Maybe other tests that fork interfere with this one?
    let _m = crate::SIGNAL_MTX.lock();

    let handler = SigHandler::Handler(alarm_signal_handler);
    let signal_action =
        SigAction::new(handler, SaFlags::SA_RESTART, SigSet::empty());
    let old_handler = unsafe {
        sigaction(Signal::SIGALRM, &signal_action)
            .expect("unable to set signal handler for alarm")
    };

    // Set an alarm.
    assert_eq!(alarm::set(60), None);

    // Overwriting an alarm should return the old alarm.
    assert_eq!(alarm::set(1), Some(60));

    // We should be woken up after 1 second by the alarm, so we'll sleep for 3
    // seconds to be sure.
    let starttime = Instant::now();
    loop {
        thread::sleep(Duration::from_millis(100));
        if unsafe { ALARM_CALLED } {
            break;
        }
        if starttime.elapsed() > Duration::from_secs(3) {
            panic!("Timeout waiting for SIGALRM");
        }
    }

    // Reset the signal.
    unsafe {
        sigaction(Signal::SIGALRM, &old_handler)
            .expect("unable to set signal handler for alarm");
    }
}

#[test]
#[cfg(not(target_os = "redox"))]
fn test_canceling_alarm() {
    let _m = crate::SIGNAL_MTX.lock();

    assert_eq!(alarm::cancel(), None);

    assert_eq!(alarm::set(60), None);
    assert_eq!(alarm::cancel(), Some(60));
}

#[test]
#[cfg(not(any(target_os = "redox", target_os = "haiku")))]
fn test_symlinkat() {
    let _m = crate::CWD_LOCK.read();

    let tempdir = tempdir().unwrap();

    let target = tempdir.path().join("a");
    let linkpath = tempdir.path().join("b");
    symlinkat(&target, None, &linkpath).unwrap();
    assert_eq!(
        readlink(&linkpath).unwrap().to_str().unwrap(),
        target.to_str().unwrap()
    );

    let dirfd = open(tempdir.path(), OFlag::empty(), Mode::empty()).unwrap();
    let target = "c";
    let linkpath = "d";
    symlinkat(target, Some(dirfd), linkpath).unwrap();
    assert_eq!(
        readlink(&tempdir.path().join(linkpath))
            .unwrap()
            .to_str()
            .unwrap(),
        target
    );
}

#[test]
#[cfg(not(any(target_os = "redox", target_os = "haiku")))]
fn test_linkat_file() {
    use nix::fcntl::AtFlags;

    let tempdir = tempdir().unwrap();
    let oldfilename = "foo.txt";
    let oldfilepath = tempdir.path().join(oldfilename);

    let newfilename = "bar.txt";
    let newfilepath = tempdir.path().join(newfilename);

    // Create file
    File::create(oldfilepath).unwrap();

    // Get file descriptor for base directory
    let dirfd =
        fcntl::open(tempdir.path(), fcntl::OFlag::empty(), stat::Mode::empty())
            .unwrap();

    // Attempt hard link file at relative path
    linkat(
        Some(dirfd),
        oldfilename,
        Some(dirfd),
        newfilename,
        AtFlags::AT_SYMLINK_FOLLOW,
    )
    .unwrap();
    assert!(newfilepath.exists());
}

#[test]
#[cfg(not(any(target_os = "redox", target_os = "haiku")))]
fn test_linkat_olddirfd_none() {
    use nix::fcntl::AtFlags;

    let _dr = crate::DirRestore::new();

    let tempdir_oldfile = tempdir().unwrap();
    let oldfilename = "foo.txt";
    let oldfilepath = tempdir_oldfile.path().join(oldfilename);

    let tempdir_newfile = tempdir().unwrap();
    let newfilename = "bar.txt";
    let newfilepath = tempdir_newfile.path().join(newfilename);

    // Create file
    File::create(oldfilepath).unwrap();

    // Get file descriptor for base directory of new file
    let dirfd = fcntl::open(
        tempdir_newfile.path(),
        fcntl::OFlag::empty(),
        stat::Mode::empty(),
    )
    .unwrap();

    // Attempt hard link file using curent working directory as relative path for old file path
    chdir(tempdir_oldfile.path()).unwrap();
    linkat(
        None,
        oldfilename,
        Some(dirfd),
        newfilename,
        AtFlags::AT_SYMLINK_FOLLOW,
    )
    .unwrap();
    assert!(newfilepath.exists());
}

#[test]
#[cfg(not(any(target_os = "redox", target_os = "haiku")))]
fn test_linkat_newdirfd_none() {
    use nix::fcntl::AtFlags;

    let _dr = crate::DirRestore::new();

    let tempdir_oldfile = tempdir().unwrap();
    let oldfilename = "foo.txt";
    let oldfilepath = tempdir_oldfile.path().join(oldfilename);

    let tempdir_newfile = tempdir().unwrap();
    let newfilename = "bar.txt";
    let newfilepath = tempdir_newfile.path().join(newfilename);

    // Create file
    File::create(oldfilepath).unwrap();

    // Get file descriptor for base directory of old file
    let dirfd = fcntl::open(
        tempdir_oldfile.path(),
        fcntl::OFlag::empty(),
        stat::Mode::empty(),
    )
    .unwrap();

    // Attempt hard link file using current working directory as relative path for new file path
    chdir(tempdir_newfile.path()).unwrap();
    linkat(
        Some(dirfd),
        oldfilename,
        None,
        newfilename,
        AtFlags::AT_SYMLINK_FOLLOW,
    )
    .unwrap();
    assert!(newfilepath.exists());
}

#[test]
#[cfg(not(any(apple_targets, target_os = "redox", target_os = "haiku")))]
fn test_linkat_no_follow_symlink() {
    use nix::fcntl::AtFlags;

    let _m = crate::CWD_LOCK.read();

    let tempdir = tempdir().unwrap();
    let oldfilename = "foo.txt";
    let oldfilepath = tempdir.path().join(oldfilename);

    let symoldfilename = "symfoo.txt";
    let symoldfilepath = tempdir.path().join(symoldfilename);

    let newfilename = "nofollowsymbar.txt";
    let newfilepath = tempdir.path().join(newfilename);

    // Create file
    File::create(&oldfilepath).unwrap();

    // Create symlink to file
    symlinkat(&oldfilepath, None, &symoldfilepath).unwrap();

    // Get file descriptor for base directory
    let dirfd =
        fcntl::open(tempdir.path(), fcntl::OFlag::empty(), stat::Mode::empty())
            .unwrap();

    // Attempt link symlink of file at relative path
    linkat(
        Some(dirfd),
        symoldfilename,
        Some(dirfd),
        newfilename,
        AtFlags::empty(),
    )
    .unwrap();

    // Assert newfile is actually a symlink to oldfile.
    assert_eq!(
        readlink(&newfilepath).unwrap().to_str().unwrap(),
        oldfilepath.to_str().unwrap()
    );
}

#[test]
#[cfg(not(any(target_os = "redox", target_os = "haiku")))]
fn test_linkat_follow_symlink() {
    use nix::fcntl::AtFlags;

    let _m = crate::CWD_LOCK.read();

    let tempdir = tempdir().unwrap();
    let oldfilename = "foo.txt";
    let oldfilepath = tempdir.path().join(oldfilename);

    let symoldfilename = "symfoo.txt";
    let symoldfilepath = tempdir.path().join(symoldfilename);

    let newfilename = "nofollowsymbar.txt";
    let newfilepath = tempdir.path().join(newfilename);

    // Create file
    File::create(&oldfilepath).unwrap();

    // Create symlink to file
    symlinkat(&oldfilepath, None, &symoldfilepath).unwrap();

    // Get file descriptor for base directory
    let dirfd =
        fcntl::open(tempdir.path(), fcntl::OFlag::empty(), stat::Mode::empty())
            .unwrap();

    // Attempt link target of symlink of file at relative path
    linkat(
        Some(dirfd),
        symoldfilename,
        Some(dirfd),
        newfilename,
        AtFlags::AT_SYMLINK_FOLLOW,
    )
    .unwrap();

    let newfilestat = stat::stat(&newfilepath).unwrap();

    // Check the file type of the new link
    assert_eq!(
        stat::SFlag::from_bits_truncate(newfilestat.st_mode as mode_t)
            & SFlag::S_IFMT,
        SFlag::S_IFREG
    );

    // Check the number of hard links to the original file
    assert_eq!(newfilestat.st_nlink, 2);
}

#[test]
#[cfg(not(target_os = "redox"))]
fn test_unlinkat_dir_noremovedir() {
    let tempdir = tempdir().unwrap();
    let dirname = "foo_dir";
    let dirpath = tempdir.path().join(dirname);

    // Create dir
    DirBuilder::new().recursive(true).create(dirpath).unwrap();

    // Get file descriptor for base directory
    let dirfd =
        fcntl::open(tempdir.path(), fcntl::OFlag::empty(), stat::Mode::empty())
            .unwrap();

    // Attempt unlink dir at relative path without proper flag
    let err_result =
        unlinkat(Some(dirfd), dirname, UnlinkatFlags::NoRemoveDir).unwrap_err();
    assert!(err_result == Errno::EISDIR || err_result == Errno::EPERM);
}

#[test]
#[cfg(not(target_os = "redox"))]
fn test_unlinkat_dir_removedir() {
    let tempdir = tempdir().unwrap();
    let dirname = "foo_dir";
    let dirpath = tempdir.path().join(dirname);

    // Create dir
    DirBuilder::new().recursive(true).create(&dirpath).unwrap();

    // Get file descriptor for base directory
    let dirfd =
        fcntl::open(tempdir.path(), fcntl::OFlag::empty(), stat::Mode::empty())
            .unwrap();

    // Attempt unlink dir at relative path with proper flag
    unlinkat(Some(dirfd), dirname, UnlinkatFlags::RemoveDir).unwrap();
    assert!(!dirpath.exists());
}

#[test]
#[cfg(not(target_os = "redox"))]
fn test_unlinkat_file() {
    let tempdir = tempdir().unwrap();
    let filename = "foo.txt";
    let filepath = tempdir.path().join(filename);

    // Create file
    File::create(&filepath).unwrap();

    // Get file descriptor for base directory
    let dirfd =
        fcntl::open(tempdir.path(), fcntl::OFlag::empty(), stat::Mode::empty())
            .unwrap();

    // Attempt unlink file at relative path
    unlinkat(Some(dirfd), filename, UnlinkatFlags::NoRemoveDir).unwrap();
    assert!(!filepath.exists());
}

#[test]
fn test_access_not_existing() {
    let tempdir = tempdir().unwrap();
    let dir = tempdir.path().join("does_not_exist.txt");
    assert_eq!(
        access(&dir, AccessFlags::F_OK).err().unwrap(),
        Errno::ENOENT
    );
}

#[test]
fn test_access_file_exists() {
    let tempdir = tempdir().unwrap();
    let path = tempdir.path().join("does_exist.txt");
    let _file = File::create(path.clone()).unwrap();
    access(&path, AccessFlags::R_OK | AccessFlags::W_OK)
        .expect("assertion failed");
}

#[cfg(not(target_os = "redox"))]
#[test]
fn test_user_into_passwd() {
    // get the UID of the "nobody" user
    #[cfg(not(target_os = "haiku"))]
    let test_username = "nobody";
    // "nobody" unavailable on haiku
    #[cfg(target_os = "haiku")]
    let test_username = "user";

    let nobody = User::from_name(test_username).unwrap().unwrap();
    let pwd: libc::passwd = nobody.into();
    let _: User = (&pwd).into();
}

/// Tests setting the filesystem UID with `setfsuid`.
#[cfg(linux_android)]
#[test]
fn test_setfsuid() {
    use std::os::unix::fs::PermissionsExt;
    use std::{fs, io, thread};
    require_capability!("test_setfsuid", CAP_SETUID);

    // get the UID of the "nobody" user
    let nobody = User::from_name("nobody").unwrap().unwrap();

    // create a temporary file with permissions '-rw-r-----'
    let file = tempfile::NamedTempFile::new_in("/var/tmp").unwrap();
    let temp_path = file.into_temp_path();
    let temp_path_2 = temp_path.to_path_buf();
    let mut permissions = fs::metadata(&temp_path).unwrap().permissions();
    permissions.set_mode(0o640);

    // spawn a new thread where to test setfsuid
    thread::spawn(move || {
        // set filesystem UID
        let fuid = setfsuid(nobody.uid);
        // trying to open the temporary file should fail with EACCES
        let res = fs::File::open(&temp_path);
        let err = res.expect_err("assertion failed");
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);

        // assert fuid actually changes
        let prev_fuid = setfsuid(Uid::from_raw(-1i32 as u32));
        assert_ne!(prev_fuid, fuid);
    })
    .join()
    .unwrap();

    // open the temporary file with the current thread filesystem UID
    fs::File::open(temp_path_2).unwrap();
}

#[test]
#[cfg(not(any(
    target_os = "redox",
    target_os = "fuchsia",
    target_os = "haiku"
)))]
fn test_ttyname() {
    let fd = posix_openpt(OFlag::O_RDWR).expect("posix_openpt failed");
    assert!(fd.as_raw_fd() > 0);

    // on linux, we can just call ttyname on the pty master directly, but
    // apparently osx requires that ttyname is called on a slave pty (can't
    // find this documented anywhere, but it seems to empirically be the case)
    grantpt(&fd).expect("grantpt failed");
    unlockpt(&fd).expect("unlockpt failed");
    let sname = unsafe { ptsname(&fd) }.expect("ptsname failed");
    let fds = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(Path::new(&sname))
        .expect("open failed");

    let name = ttyname(fds).expect("ttyname failed");
    assert!(name.starts_with("/dev"));
}

#[test]
#[cfg(not(any(target_os = "redox", target_os = "fuchsia")))]
fn test_ttyname_not_pty() {
    let fd = File::open("/dev/zero").unwrap();
    assert_eq!(ttyname(fd), Err(Errno::ENOTTY));
}

#[test]
#[cfg(bsd)]
fn test_getpeereid() {
    use std::os::unix::net::UnixStream;
    let (sock_a, sock_b) = UnixStream::pair().unwrap();

    let (uid_a, gid_a) = getpeereid(sock_a).unwrap();
    let (uid_b, gid_b) = getpeereid(sock_b).unwrap();

    let uid = geteuid();
    let gid = getegid();

    assert_eq!(uid, uid_a);
    assert_eq!(gid, gid_a);
    assert_eq!(uid_a, uid_b);
    assert_eq!(gid_a, gid_b);
}

#[test]
#[cfg(not(target_os = "redox"))]
fn test_faccessat_none_not_existing() {
    use nix::fcntl::AtFlags;
    let tempdir = tempfile::tempdir().unwrap();
    let dir = tempdir.path().join("does_not_exist.txt");
    assert_eq!(
        faccessat(None, &dir, AccessFlags::F_OK, AtFlags::empty())
            .err()
            .unwrap(),
        Errno::ENOENT
    );
}

#[test]
#[cfg(not(target_os = "redox"))]
fn test_faccessat_not_existing() {
    use nix::fcntl::AtFlags;
    let tempdir = tempfile::tempdir().unwrap();
    let dirfd = open(tempdir.path(), OFlag::empty(), Mode::empty()).unwrap();
    let not_exist_file = "does_not_exist.txt";
    assert_eq!(
        faccessat(
            Some(dirfd),
            not_exist_file,
            AccessFlags::F_OK,
            AtFlags::empty(),
        )
        .err()
        .unwrap(),
        Errno::ENOENT
    );
}

#[test]
#[cfg(not(target_os = "redox"))]
fn test_faccessat_none_file_exists() {
    use nix::fcntl::AtFlags;
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("does_exist.txt");
    let _file = File::create(path.clone()).unwrap();
    assert!(faccessat(
        None,
        &path,
        AccessFlags::R_OK | AccessFlags::W_OK,
        AtFlags::empty(),
    )
    .is_ok());
}

#[test]
#[cfg(not(target_os = "redox"))]
fn test_faccessat_file_exists() {
    use nix::fcntl::AtFlags;
    let tempdir = tempfile::tempdir().unwrap();
    let dirfd = open(tempdir.path(), OFlag::empty(), Mode::empty()).unwrap();
    let exist_file = "does_exist.txt";
    let path = tempdir.path().join(exist_file);
    let _file = File::create(path.clone()).unwrap();
    assert!(faccessat(
        Some(dirfd),
        &path,
        AccessFlags::R_OK | AccessFlags::W_OK,
        AtFlags::empty(),
    )
    .is_ok());
}

#[test]
#[cfg(any(all(target_os = "linux", not(target_env = "uclibc")), freebsdlike))]
fn test_eaccess_not_existing() {
    let tempdir = tempdir().unwrap();
    let dir = tempdir.path().join("does_not_exist.txt");
    assert_eq!(
        eaccess(&dir, AccessFlags::F_OK).err().unwrap(),
        Errno::ENOENT
    );
}

#[test]
#[cfg(any(all(target_os = "linux", not(target_env = "uclibc")), freebsdlike))]
fn test_eaccess_file_exists() {
    let tempdir = tempdir().unwrap();
    let path = tempdir.path().join("does_exist.txt");
    let _file = File::create(path.clone()).unwrap();
    eaccess(&path, AccessFlags::R_OK | AccessFlags::W_OK)
        .expect("assertion failed");
}

#[test]
#[cfg(bsd)]
fn test_group_from() {
    let group = Group::from_name("wheel").unwrap().unwrap();
    assert!(group.name == "wheel");
    let group_id = group.gid;
    let group = Group::from_gid(group_id).unwrap().unwrap();
    assert_eq!(group.gid, group_id);
    assert_eq!(group.name, "wheel");
}
