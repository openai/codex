use std::ffi::CStr;
use std::ffi::CString;
use std::fs::File;
use std::os::raw::c_char;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

use crate::bundled_bwrap;
use crate::bundled_bwrap::BundledBwrapLauncher;
use crate::exec_util::argv_to_cstrings;
use crate::exec_util::make_files_inheritable;
use codex_sandboxing::find_system_bwrap_in_path;
use codex_utils_absolute_path::AbsolutePathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
enum BubblewrapLauncher {
    System(SystemBwrapLauncher),
    Bundled(BundledBwrapLauncher),
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SystemBwrapLauncher {
    program: AbsolutePathBuf,
    supports_argv0: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SystemBwrapCapabilities {
    supports_argv0: bool,
    supports_perms: bool,
}

pub(crate) fn exec_bwrap(argv: Vec<String>, preserved_files: Vec<File>) -> ! {
    match preferred_bwrap_launcher() {
        BubblewrapLauncher::System(launcher) => {
            exec_system_bwrap(&launcher.program, argv, preserved_files)
        }
        BubblewrapLauncher::Bundled(launcher) => launcher.exec(argv, preserved_files),
        BubblewrapLauncher::Unavailable => {
            panic!(
                "bubblewrap is unavailable: no system bwrap was found on PATH and no bundled \
                 codex-resources/bwrap binary was found next to the Codex executable"
            )
        }
    }
}

fn preferred_bwrap_launcher() -> BubblewrapLauncher {
    static LAUNCHER: OnceLock<BubblewrapLauncher> = OnceLock::new();
    LAUNCHER
        .get_or_init(|| {
            if let Some(path) = find_system_bwrap_in_path()
                && let Some(launcher) = system_bwrap_launcher_for_path(&path)
            {
                return BubblewrapLauncher::System(launcher);
            }

            match bundled_bwrap::launcher() {
                Some(launcher) => BubblewrapLauncher::Bundled(launcher),
                None => BubblewrapLauncher::Unavailable,
            }
        })
        .clone()
}

fn system_bwrap_launcher_for_path(system_bwrap_path: &Path) -> Option<SystemBwrapLauncher> {
    system_bwrap_launcher_for_path_with_probe(system_bwrap_path, system_bwrap_capabilities)
}

fn system_bwrap_launcher_for_path_with_probe(
    system_bwrap_path: &Path,
    system_bwrap_capabilities: impl FnOnce(&Path) -> Option<SystemBwrapCapabilities>,
) -> Option<SystemBwrapLauncher> {
    if !system_bwrap_path.is_file() {
        return None;
    }

    let Some(SystemBwrapCapabilities {
        supports_argv0,
        supports_perms: true,
    }) = system_bwrap_capabilities(system_bwrap_path)
    else {
        return None;
    };
    let system_bwrap_path = match AbsolutePathBuf::from_absolute_path(system_bwrap_path) {
        Ok(path) => path,
        Err(err) => panic!(
            "failed to normalize system bubblewrap path {}: {err}",
            system_bwrap_path.display()
        ),
    };
    Some(SystemBwrapLauncher {
        program: system_bwrap_path,
        supports_argv0,
    })
}

pub(crate) fn preferred_bwrap_supports_argv0() -> bool {
    match preferred_bwrap_launcher() {
        BubblewrapLauncher::System(launcher) => launcher.supports_argv0,
        BubblewrapLauncher::Bundled(_) | BubblewrapLauncher::Unavailable => true,
    }
}

fn system_bwrap_capabilities(system_bwrap_path: &Path) -> Option<SystemBwrapCapabilities> {
    // bubblewrap added `--argv0` in v0.9.0:
    // https://github.com/containers/bubblewrap/releases/tag/v0.9.0
    // Older distro packages (for example Ubuntu 20.04/22.04) ship builds that
    // reject `--argv0`, so use the system binary's no-argv0 compatibility path
    // in that case.
    let output = match Command::new(system_bwrap_path).arg("--help").output() {
        Ok(output) => output,
        Err(_) => return None,
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Some(SystemBwrapCapabilities {
        supports_argv0: stdout.contains("--argv0") || stderr.contains("--argv0"),
        supports_perms: stdout.contains("--perms") || stderr.contains("--perms"),
    })
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
    let mut argv_ptrs: Vec<*const c_char> = cstrings
        .iter()
        .map(CString::as_c_str)
        .map(CStr::as_ptr)
        .collect();
    argv_ptrs.push(std::ptr::null());

    // SAFETY: `program` and every entry in `argv_ptrs` are valid C strings for
    // the duration of the call. On success `execv` does not return.
    unsafe {
        libc::execv(program.as_ptr(), argv_ptrs.as_ptr());
    }
    let err = std::io::Error::last_os_error();
    panic!("failed to exec system bubblewrap {program_path}: {err}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::NamedTempFile;

    #[test]
    fn prefers_system_bwrap_when_help_lists_argv0() {
        let fake_bwrap = NamedTempFile::new().expect("temp file");
        let fake_bwrap_path = fake_bwrap.path();
        let expected = AbsolutePathBuf::from_absolute_path(fake_bwrap_path).expect("absolute");

        assert_eq!(
            system_bwrap_launcher_for_path_with_probe(fake_bwrap_path, |_| {
                Some(SystemBwrapCapabilities {
                    supports_argv0: true,
                    supports_perms: true,
                })
            }),
            Some(SystemBwrapLauncher {
                program: expected,
                supports_argv0: true,
            })
        );
    }

    #[test]
    fn prefers_system_bwrap_when_system_bwrap_lacks_argv0() {
        let fake_bwrap = NamedTempFile::new().expect("temp file");
        let fake_bwrap_path = fake_bwrap.path();

        assert_eq!(
            system_bwrap_launcher_for_path_with_probe(fake_bwrap_path, |_| {
                Some(SystemBwrapCapabilities {
                    supports_argv0: false,
                    supports_perms: true,
                })
            }),
            Some(SystemBwrapLauncher {
                program: AbsolutePathBuf::from_absolute_path(fake_bwrap_path).expect("absolute"),
                supports_argv0: false,
            })
        );
    }

    #[test]
    fn ignores_system_bwrap_when_system_bwrap_lacks_perms() {
        let fake_bwrap = NamedTempFile::new().expect("temp file");

        assert_eq!(
            system_bwrap_launcher_for_path_with_probe(fake_bwrap.path(), |_| {
                Some(SystemBwrapCapabilities {
                    supports_argv0: false,
                    supports_perms: false,
                })
            }),
            None
        );
    }

    #[test]
    fn ignores_system_bwrap_when_system_bwrap_is_missing() {
        assert_eq!(
            system_bwrap_launcher_for_path(Path::new("/definitely/not/a/bwrap")),
            None
        );
    }
}
