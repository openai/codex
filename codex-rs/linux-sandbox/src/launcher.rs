use std::ffi::CString;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::os::raw::c_char;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::path::PathBuf;

use crate::vendored_bwrap::exec_vendored_bwrap;

const SYSTEM_BWRAP_PATH: &str = "/usr/bin/bwrap";
const UBUNTU_APPARMOR_BWRAP_USERNS_RESTRICT_PROFILE: &str = "/etc/apparmor.d/bwrap-userns-restrict";

#[derive(Debug, Clone, PartialEq, Eq)]
enum BubblewrapLauncher {
    System(PathBuf),
    Vendored { warn_missing_system_bwrap: bool },
}

pub(crate) fn exec_bwrap(
    argv: Vec<String>,
    preserved_files: Vec<File>,
    emit_missing_system_bwrap_warning: bool,
) -> ! {
    match preferred_bwrap_launcher() {
        BubblewrapLauncher::System(program) => exec_system_bwrap(&program, argv, preserved_files),
        BubblewrapLauncher::Vendored {
            warn_missing_system_bwrap,
        } => {
            if warn_missing_system_bwrap && emit_missing_system_bwrap_warning {
                eprintln!(
                    "warning: Codex could not find system bubblewrap at {SYSTEM_BWRAP_PATH}. Please install bubblewrap with your package manager. Codex will use the vendored bubblewrap in the meantime."
                );
            }
            exec_vendored_bwrap(argv, preserved_files)
        }
    }
}

fn preferred_bwrap_launcher() -> BubblewrapLauncher {
    let apparmor_profile_exists = Path::new(UBUNTU_APPARMOR_BWRAP_USERNS_RESTRICT_PROFILE).exists();
    let system_bwrap_exists = Path::new(SYSTEM_BWRAP_PATH).exists();

    preferred_bwrap_launcher_with(apparmor_profile_exists, system_bwrap_exists)
}

fn preferred_bwrap_launcher_with(
    apparmor_profile_exists: bool,
    system_bwrap_exists: bool,
) -> BubblewrapLauncher {
    match (apparmor_profile_exists, system_bwrap_exists) {
        (true, true) => BubblewrapLauncher::System(PathBuf::from(SYSTEM_BWRAP_PATH)),
        (true, false) => BubblewrapLauncher::Vendored {
            warn_missing_system_bwrap: true,
        },
        (false, _) => BubblewrapLauncher::Vendored {
            warn_missing_system_bwrap: false,
        },
    }
}

fn exec_system_bwrap(program: &Path, argv: Vec<String>, preserved_files: Vec<File>) -> ! {
    make_files_inheritable(&preserved_files);

    let program_path = program.display().to_string();
    let program = CString::new(program.as_os_str().as_bytes())
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
    use tempfile::NamedTempFile;

    #[test]
    fn prefers_system_bwrap_for_ubuntu_apparmor_profile() {
        assert_eq!(
            preferred_bwrap_launcher_with(
                /*apparmor_profile_exists*/ true, /*system_bwrap_exists*/ true
            ),
            BubblewrapLauncher::System(PathBuf::from(SYSTEM_BWRAP_PATH))
        );
    }

    #[test]
    fn falls_back_to_vendored_without_warning_when_apparmor_profile_is_absent() {
        assert_eq!(
            preferred_bwrap_launcher_with(
                /*apparmor_profile_exists*/ false, /*system_bwrap_exists*/ true
            ),
            BubblewrapLauncher::Vendored {
                warn_missing_system_bwrap: false,
            }
        );
    }

    #[test]
    fn falls_back_to_vendored_with_warning_when_apparmor_profile_exists_but_system_bwrap_is_missing()
     {
        assert_eq!(
            preferred_bwrap_launcher_with(
                /*apparmor_profile_exists*/ true, /*system_bwrap_exists*/ false
            ),
            BubblewrapLauncher::Vendored {
                warn_missing_system_bwrap: true,
            }
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
}
