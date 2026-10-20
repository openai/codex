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
use codex_protocol::permissions::FileSystemSandboxPolicy;
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
    supports_ro_bind_fd: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SystemBwrapCapabilities {
    supports_argv0: bool,
    supports_perms: bool,
    supports_ro_bind_fd: bool,
}

static LAUNCHER: OnceLock<BubblewrapLauncher> = OnceLock::new();

pub(crate) fn exec_bwrap(mut argv: Vec<String>, preserved_files: Vec<File>) -> ! {
    if argv
        .iter()
        .take_while(|arg| arg.as_str() != "--")
        .any(|arg| arg == "--unshare-pid")
    {
        argv.insert(1, "--as-pid-1".to_string());
    }

    match preferred_bwrap_launcher() {
        BubblewrapLauncher::System(launcher) => {
            if !launcher.supports_ro_bind_fd {
                translate_legacy_bwrap_fd_mounts(&mut argv)
                    .unwrap_or_else(|error| panic!("invalid legacy bubblewrap fd mount: {error}"));
            }
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

fn translate_legacy_bwrap_fd_mounts(argv: &mut Vec<String>) -> Result<(), String> {
    let command_separator = argv
        .iter()
        .position(|argument| argument == "--")
        .ok_or_else(|| "bubblewrap argv is missing the command separator '--'".to_string())?;
    let mut verification_args = Vec::new();
    let mut verified_fds = Vec::new();
    let mut argument_index = 0;

    while argument_index < command_separator {
        if argv[argument_index] != "--ro-bind-fd" {
            argument_index += 1;
            continue;
        }

        let fd_argument = argv
            .get(argument_index + 1)
            .filter(|_| argument_index + 1 < command_separator)
            .ok_or_else(|| "--ro-bind-fd is missing its file descriptor".to_string())?;
        let fd = fd_argument
            .parse::<libc::c_int>()
            .map_err(|_| format!("invalid --ro-bind-fd file descriptor: {fd_argument}"))?;
        if fd <= libc::STDERR_FILENO {
            return Err(format!(
                "--ro-bind-fd file descriptor must not use standard descriptors: {fd}"
            ));
        }
        if verified_fds.contains(&fd) {
            return Err(format!("duplicate --ro-bind-fd file descriptor: {fd}"));
        }

        let destination = argv
            .get(argument_index + 2)
            .filter(|_| argument_index + 2 < command_separator)
            .ok_or_else(|| "--ro-bind-fd is missing its mount destination".to_string())?;
        if !Path::new(destination).is_absolute() {
            return Err(format!(
                "--ro-bind-fd mount destination must be absolute: {destination}"
            ));
        }

        verification_args.push("--verify-fd-mount".to_string());
        verification_args.push(format!("{fd}:{destination}"));
        verified_fds.push(fd);
        argv[argument_index] = "--ro-bind".to_string();
        argv[argument_index + 1] = format!("/proc/self/fd/{fd}");
        argument_index += 3;
    }

    if verification_args.is_empty() {
        return Ok(());
    }
    let inner_command = command_separator + 1;
    if argv.get(inner_command).is_none() {
        return Err("bubblewrap argv is missing the inner command after '--'".to_string());
    }
    if !argv[inner_command + 1..]
        .iter()
        .take_while(|argument| argument.as_str() != "--")
        .any(|argument| argument == "--apply-seccomp-then-exec")
    {
        return Err("descriptor-backed mounts require the trusted inner sandbox stage".to_string());
    }
    argv.splice(inner_command + 1..inner_command + 1, verification_args);
    Ok(())
}

pub(crate) fn initialize_bwrap_launcher(
    file_system_policy: &FileSystemSandboxPolicy,
    sandbox_policy_cwd: &Path,
) {
    LAUNCHER.get_or_init(|| {
        if let Some(path) = find_system_bwrap_in_path(file_system_policy, sandbox_policy_cwd)
            && let Some(launcher) = system_bwrap_launcher_for_path(&path)
        {
            return BubblewrapLauncher::System(launcher);
        }

        match bundled_bwrap::launcher() {
            Some(launcher) => BubblewrapLauncher::Bundled(launcher),
            None => BubblewrapLauncher::Unavailable,
        }
    });
}

fn preferred_bwrap_launcher() -> &'static BubblewrapLauncher {
    LAUNCHER.get().unwrap_or_else(|| {
        panic!("bubblewrap launcher must be initialized with the command's permissions")
    })
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
        supports_ro_bind_fd,
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
        supports_ro_bind_fd,
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
    if !stdout.contains("--as-pid-1") && !stderr.contains("--as-pid-1") {
        return None;
    }
    Some(SystemBwrapCapabilities {
        supports_argv0: stdout.contains("--argv0") || stderr.contains("--argv0"),
        supports_perms: stdout.contains("--perms") || stderr.contains("--perms"),
        supports_ro_bind_fd: stdout.contains("--ro-bind-fd") || stderr.contains("--ro-bind-fd"),
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
#[path = "launcher_tests.rs"]
mod tests;
