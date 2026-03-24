use std::ffi::CString;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::os::raw::c_char;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

use crate::vendored_bwrap::exec_vendored_bwrap;
use codex_utils_absolute_path::AbsolutePathBuf;

const SYSTEM_BWRAP_PATH: &str = "/usr/bin/bwrap";

#[derive(Debug, Clone, PartialEq, Eq)]
enum BubblewrapLauncher {
    System(AbsolutePathBuf),
    Vendored,
}

pub(crate) fn exec_bwrap(argv: Vec<String>, preserved_files: Vec<File>) -> ! {
    match preferred_bwrap_launcher() {
        BubblewrapLauncher::System(program) => exec_system_bwrap(&program, argv, preserved_files),
        BubblewrapLauncher::Vendored => exec_vendored_bwrap(argv, preserved_files),
    }
}

fn preferred_bwrap_launcher() -> BubblewrapLauncher {
    static LAUNCHER: OnceLock<BubblewrapLauncher> = OnceLock::new();
    LAUNCHER
        .get_or_init(|| preferred_bwrap_launcher_for_path(Path::new(SYSTEM_BWRAP_PATH)))
        .clone()
}

fn preferred_bwrap_launcher_for_path(system_bwrap_path: &Path) -> BubblewrapLauncher {
    if !system_bwrap_supports_argv0(system_bwrap_path) {
        return BubblewrapLauncher::Vendored;
    }

    let system_bwrap_path = match AbsolutePathBuf::from_absolute_path(system_bwrap_path) {
        Ok(path) => path,
        Err(err) => panic!(
            "failed to normalize system bubblewrap path {}: {err}",
            system_bwrap_path.display()
        ),
    };
    BubblewrapLauncher::System(system_bwrap_path)
}

fn system_bwrap_supports_argv0(system_bwrap_path: &Path) -> bool {
    // bubblewrap added `--argv0` in v0.9.0:
    // https://github.com/containers/bubblewrap/releases/tag/v0.9.0
    // Older distro packages (for example Ubuntu 20.04/22.04) ship builds that
    // reject `--argv0`, so prefer the vendored build in that case.
    let output = match Command::new(system_bwrap_path).arg("--help").output() {
        Ok(output) => output,
        Err(_) => return false,
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    stdout.contains("--argv0") || stderr.contains("--argv0")
}

fn exec_system_bwrap(
    program: &AbsolutePathBuf,
    argv: Vec<String>,
    preserved_files: Vec<File>,
) -> ! {
    // System bwrap runs across an exec boundary, so preserved fds must survive exec.
    make_files_inheritable(&preserved_files);

    let program_path = program.as_path().display().to_string();
    let program = CString::new(program.as_path().as_os_str().as_bytes())
        .unwrap_or_else(|err| panic!("invalid system bubblewrap path: {err}"));
    let cstrings = argv_to_cstrings(&argv);
    let mut argv_ptrs: Vec<*const c_char> = cstrings.iter().map(|arg| arg.as_ptr()).collect();
    argv_ptrs.push(std::ptr::null());

    // SAFETY: `program` and every entry in `argv_ptrs` are valid C strings for
    // the duration of the call. On success `execv` does not return.
    unsafe {
        libc::execv(program.as_ptr(), argv_ptrs.as_ptr());
    }
    let err = std::io::Error::last_os_error();
    panic!("failed to exec system bubblewrap {program_path}: {err}");
}

fn argv_to_cstrings(argv: &[String]) -> Vec<CString> {
    let mut cstrings: Vec<CString> = Vec::with_capacity(argv.len());
    for arg in argv {
        match CString::new(arg.as_str()) {
            Ok(value) => cstrings.push(value),
            Err(err) => panic!("failed to convert argv to CString: {err}"),
        }
    }
    cstrings
}

fn make_files_inheritable(files: &[File]) {
    for file in files {
        clear_cloexec(file.as_raw_fd());
    }
}

fn clear_cloexec(fd: libc::c_int) {
    // SAFETY: `fd` is an owned descriptor kept alive by `files`.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags < 0 {
        let err = std::io::Error::last_os_error();
        panic!("failed to read fd flags for preserved bubblewrap file descriptor {fd}: {err}");
    }
    let cleared_flags = flags & !libc::FD_CLOEXEC;
    if cleared_flags == flags {
        return;
    }

    // SAFETY: `fd` is valid and we are only clearing FD_CLOEXEC.
    let result = unsafe { libc::fcntl(fd, libc::F_SETFD, cleared_flags) };
    if result < 0 {
        let err = std::io::Error::last_os_error();
        panic!("failed to clear CLOEXEC for preserved bubblewrap file descriptor {fd}: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::NamedTempFile;
    use tempfile::TempPath;

    #[test]
    fn prefers_system_bwrap_when_help_lists_argv0() {
        let fake_bwrap = write_fake_bwrap(
            r#"#!/bin/sh
if [ "$1" = "--help" ]; then
  echo '  --argv0 PROGRAM'
  exit 0
fi
exit 1
"#,
        );
        let fake_bwrap_path: &Path = fake_bwrap.as_ref();
        let expected = AbsolutePathBuf::from_absolute_path(fake_bwrap_path).expect("absolute");

        assert_eq!(
            preferred_bwrap_launcher_for_path(fake_bwrap_path),
            BubblewrapLauncher::System(expected)
        );
    }

    #[test]
    fn falls_back_to_vendored_when_system_bwrap_lacks_argv0() {
        let fake_bwrap = write_fake_bwrap(
            r#"#!/bin/sh
if [ "$1" = "--help" ]; then
  echo 'usage: bwrap [OPTION...] COMMAND'
  exit 0
fi
exit 1
"#,
        );
        let fake_bwrap_path: &Path = fake_bwrap.as_ref();

        assert_eq!(
            preferred_bwrap_launcher_for_path(fake_bwrap_path),
            BubblewrapLauncher::Vendored
        );
    }

    #[test]
    fn falls_back_to_vendored_when_system_bwrap_is_missing() {
        assert_eq!(
            preferred_bwrap_launcher_for_path(Path::new("/definitely/not/a/bwrap")),
            BubblewrapLauncher::Vendored
        );
    }

    #[test]
    fn preserved_files_are_made_inheritable_for_system_exec() {
        let file = NamedTempFile::new().expect("temp file");
        set_cloexec(file.as_file().as_raw_fd());

        make_files_inheritable(std::slice::from_ref(file.as_file()));

        assert_eq!(fd_flags(file.as_file().as_raw_fd()) & libc::FD_CLOEXEC, 0);
    }

    fn set_cloexec(fd: libc::c_int) {
        let flags = fd_flags(fd);
        // SAFETY: `fd` is valid for the duration of the test.
        let result = unsafe { libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) };
        if result < 0 {
            let err = std::io::Error::last_os_error();
            panic!("failed to set CLOEXEC for test fd {fd}: {err}");
        }
    }

    fn fd_flags(fd: libc::c_int) -> libc::c_int {
        // SAFETY: `fd` is valid for the duration of the test.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
        if flags < 0 {
            let err = std::io::Error::last_os_error();
            panic!("failed to read fd flags for test fd {fd}: {err}");
        }
        flags
    }

    fn write_fake_bwrap(contents: &str) -> TempPath {
        // Bazel can mount the OS temp directory `noexec`, so prefer the current
        // working directory for fake executables and fall back to the default temp
        // dir outside that environment.
        let temp_file = std::env::current_dir()
            .ok()
            .and_then(|dir| NamedTempFile::new_in(dir).ok())
            .unwrap_or_else(|| NamedTempFile::new().expect("temp file"));
        // Linux rejects exec-ing a file that is still open for writing.
        let path = temp_file.into_temp_path();
        fs::write(&path, contents).expect("write fake bwrap");
        let permissions = fs::Permissions::from_mode(0o755);
        fs::set_permissions(&path, permissions).expect("chmod fake bwrap");
        path
    }
}
