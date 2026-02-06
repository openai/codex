use clap::Parser;
use std::ffi::CString;
use std::fs::File;
use std::io::Read;
use std::os::fd::FromRawFd;
use std::path::Path;
use std::path::PathBuf;

use crate::bwrap::BwrapOptions;
use crate::bwrap::create_bwrap_command_args;
use crate::landlock::apply_sandbox_policy_to_current_thread;
use crate::vendored_bwrap::exec_vendored_bwrap;
use crate::vendored_bwrap::run_vendored_bwrap_main;

#[derive(Debug, Parser)]
/// CLI surface for the Linux sandbox helper.
///
/// The type name remains `LandlockCommand` for compatibility with existing
/// wiring, but the filesystem sandbox now uses bubblewrap.
pub struct LandlockCommand {
    /// It is possible that the cwd used in the context of the sandbox policy
    /// is different from the cwd of the process to spawn.
    #[arg(long = "sandbox-policy-cwd")]
    pub sandbox_policy_cwd: PathBuf,

    #[arg(long = "sandbox-policy")]
    pub sandbox_policy: codex_core::protocol::SandboxPolicy,

    /// Opt-in: use the bubblewrap-based Linux sandbox pipeline.
    ///
    /// When not set, we fall back to the legacy Landlock + mount pipeline.
    #[arg(long = "use-bwrap-sandbox", hide = true, default_value_t = false)]
    pub use_bwrap_sandbox: bool,

    /// Internal: apply seccomp and `no_new_privs` in the already-sandboxed
    /// process, then exec the user command.
    ///
    /// This exists so we can run bubblewrap first (which may rely on setuid)
    /// and only tighten with seccomp after the filesystem view is established.
    #[arg(long = "apply-seccomp-then-exec", hide = true, default_value_t = false)]
    pub apply_seccomp_then_exec: bool,

    /// When set, skip mounting a fresh `/proc` even though PID isolation is
    /// still enabled. This is primarily intended for restrictive container
    /// environments that deny `--proc /proc`.
    #[arg(long = "no-proc", default_value_t = false)]
    pub no_proc: bool,

    /// Full command args to run under the Linux sandbox helper.
    #[arg(trailing_var_arg = true)]
    pub command: Vec<String>,
}

/// Entry point for the Linux sandbox helper.
///
/// The sequence is:
/// 1. When needed, wrap the command with bubblewrap to construct the
///    filesystem view.
/// 2. Apply in-process restrictions (no_new_privs + seccomp).
/// 3. `execvp` into the final command.
pub fn run_main() -> ! {
    let LandlockCommand {
        sandbox_policy_cwd,
        sandbox_policy,
        use_bwrap_sandbox,
        apply_seccomp_then_exec,
        no_proc,
        command,
    } = LandlockCommand::parse();

    if command.is_empty() {
        panic!("No command specified to execute.");
    }

    // Inner stage: apply seccomp/no_new_privs after bubblewrap has already
    // established the filesystem view.
    if apply_seccomp_then_exec {
        if let Err(e) =
            apply_sandbox_policy_to_current_thread(&sandbox_policy, &sandbox_policy_cwd, false)
        {
            panic!("error applying Linux sandbox restrictions: {e:?}");
        }
        exec_or_panic(command);
    }

    if sandbox_policy.has_full_disk_write_access() {
        if let Err(e) =
            apply_sandbox_policy_to_current_thread(&sandbox_policy, &sandbox_policy_cwd, false)
        {
            panic!("error applying Linux sandbox restrictions: {e:?}");
        }
        exec_or_panic(command);
    }

    if use_bwrap_sandbox {
        // Outer stage: bubblewrap first, then re-enter this binary in the
        // sandboxed environment to apply seccomp. This path never falls back
        // to legacy Landlock on failure.
        let inner = build_inner_seccomp_command(
            &sandbox_policy_cwd,
            &sandbox_policy,
            use_bwrap_sandbox,
            command,
        );
        run_bwrap_with_proc_fallback(&sandbox_policy_cwd, &sandbox_policy, inner, !no_proc);
    }

    // Legacy path: Landlock enforcement only, when bwrap sandboxing is not enabled.
    if let Err(e) =
        apply_sandbox_policy_to_current_thread(&sandbox_policy, &sandbox_policy_cwd, true)
    {
        panic!("error applying legacy Linux sandbox restrictions: {e:?}");
    }
    exec_or_panic(command);
}

fn run_bwrap_with_proc_fallback(
    sandbox_policy_cwd: &Path,
    sandbox_policy: &codex_core::protocol::SandboxPolicy,
    inner: Vec<String>,
    mount_proc: bool,
) -> ! {
    let mut mount_proc = mount_proc;

    if mount_proc && !preflight_proc_mount_support(sandbox_policy_cwd, sandbox_policy) {
        eprintln!("codex-linux-sandbox: bwrap could not mount /proc; retrying with --no-proc");
        mount_proc = false;
    }

    let options = BwrapOptions { mount_proc };
    let argv = build_bwrap_argv(inner, sandbox_policy, sandbox_policy_cwd, options);
    exec_vendored_bwrap(argv);
}

fn build_bwrap_argv(
    inner: Vec<String>,
    sandbox_policy: &codex_core::protocol::SandboxPolicy,
    sandbox_policy_cwd: &Path,
    options: BwrapOptions,
) -> Vec<String> {
    let mut args = create_bwrap_command_args(inner, sandbox_policy, sandbox_policy_cwd, options)
        .unwrap_or_else(|err| panic!("error building bubblewrap command: {err:?}"));

    let command_separator_index = args
        .iter()
        .position(|arg| arg == "--")
        .unwrap_or_else(|| panic!("bubblewrap argv is missing command separator '--'"));
    args.splice(
        command_separator_index..command_separator_index,
        ["--argv0".to_string(), "codex-linux-sandbox".to_string()],
    );

    let mut argv = vec!["bwrap".to_string()];
    argv.extend(args);
    argv
}

fn preflight_proc_mount_support(
    sandbox_policy_cwd: &Path,
    sandbox_policy: &codex_core::protocol::SandboxPolicy,
) -> bool {
    let preflight_command = vec![resolve_true_command()];
    let preflight_argv = build_bwrap_argv(
        preflight_command,
        sandbox_policy,
        sandbox_policy_cwd,
        BwrapOptions { mount_proc: true },
    );
    let stderr = run_bwrap_in_child_capture_stderr(preflight_argv);
    !is_proc_mount_failure(stderr.as_str())
}

fn resolve_true_command() -> String {
    for candidate in ["/usr/bin/true", "/bin/true"] {
        if Path::new(candidate).exists() {
            return candidate.to_string();
        }
    }
    "true".to_string()
}

/// Run a short-lived bubblewrap preflight in a child process and capture stderr.
///
/// Strategy:
/// - This is used only by `preflight_proc_mount_support`, which runs `/bin/true`
///   under bubblewrap with `--proc /proc`.
/// - The goal is to detect environments where mounting `/proc` fails (for
///   example, restricted containers), so we can retry the real run with
///   `--no-proc`.
/// - We capture stderr from that preflight to match known mount-failure text.
///   We do not stream it because this is a one-shot probe with a trivial
///   command, and reads are bounded to a fixed max size.
fn run_bwrap_in_child_capture_stderr(argv: Vec<String>) -> String {
    const MAX_PREFLIGHT_STDERR_BYTES: u64 = 64 * 1024;

    let mut pipe_fds = [0; 2];
    let pipe_res = unsafe { libc::pipe2(pipe_fds.as_mut_ptr(), libc::O_CLOEXEC) };
    if pipe_res < 0 {
        let err = std::io::Error::last_os_error();
        panic!("failed to create stderr pipe for bubblewrap: {err}");
    }
    let read_fd = pipe_fds[0];
    let write_fd = pipe_fds[1];

    let pid = unsafe { libc::fork() };
    if pid < 0 {
        let err = std::io::Error::last_os_error();
        panic!("failed to fork for bubblewrap: {err}");
    }

    if pid == 0 {
        // Child: redirect stderr to the pipe, then run bubblewrap.
        unsafe {
            close_fd_or_panic(read_fd, "close read end in bubblewrap child");
            if libc::dup2(write_fd, libc::STDERR_FILENO) < 0 {
                let err = std::io::Error::last_os_error();
                panic!("failed to redirect stderr for bubblewrap: {err}");
            }
            close_fd_or_panic(write_fd, "close write end in bubblewrap child");
        }

        let exit_code = run_vendored_bwrap_main(&argv);
        std::process::exit(exit_code);
    }

    // Parent: close the write end and read stderr while the child runs.
    close_fd_or_panic(write_fd, "close write end in bubblewrap parent");

    // SAFETY: `read_fd` is a valid owned fd in the parent.
    let mut read_file = unsafe { File::from_raw_fd(read_fd) };
    let mut stderr_bytes = Vec::new();
    let mut limited_reader = (&mut read_file).take(MAX_PREFLIGHT_STDERR_BYTES);
    if let Err(err) = limited_reader.read_to_end(&mut stderr_bytes) {
        panic!("failed to read bubblewrap stderr: {err}");
    }

    let mut status: libc::c_int = 0;
    let wait_res = unsafe { libc::waitpid(pid, &mut status as *mut libc::c_int, 0) };
    if wait_res < 0 {
        let err = std::io::Error::last_os_error();
        panic!("waitpid failed for bubblewrap child: {err}");
    }

    String::from_utf8_lossy(&stderr_bytes).into_owned()
}

/// Close an owned file descriptor and panic with context on failure.
///
/// We use explicit close() checks here (instead of ignoring return codes)
/// because this code runs in low-level sandbox setup paths where fd leaks or
/// close errors can mask the root cause of later failures.
fn close_fd_or_panic(fd: libc::c_int, context: &str) {
    let close_res = unsafe { libc::close(fd) };
    if close_res < 0 {
        let err = std::io::Error::last_os_error();
        panic!("{context}: {err}");
    }
}

fn is_proc_mount_failure(stderr: &str) -> bool {
    stderr.contains("Can't mount proc")
        && stderr.contains("/newroot/proc")
        && stderr.contains("Invalid argument")
}

/// Build the inner command that applies seccomp after bubblewrap.
fn build_inner_seccomp_command(
    sandbox_policy_cwd: &Path,
    sandbox_policy: &codex_core::protocol::SandboxPolicy,
    use_bwrap_sandbox: bool,
    command: Vec<String>,
) -> Vec<String> {
    let current_exe = match std::env::current_exe() {
        Ok(path) => path,
        Err(err) => panic!("failed to resolve current executable path: {err}"),
    };
    let policy_json = match serde_json::to_string(sandbox_policy) {
        Ok(json) => json,
        Err(err) => panic!("failed to serialize sandbox policy: {err}"),
    };

    let mut inner = vec![
        current_exe.to_string_lossy().to_string(),
        "--sandbox-policy-cwd".to_string(),
        sandbox_policy_cwd.to_string_lossy().to_string(),
        "--sandbox-policy".to_string(),
        policy_json,
    ];
    if use_bwrap_sandbox {
        inner.push("--use-bwrap-sandbox".to_string());
        inner.push("--apply-seccomp-then-exec".to_string());
    }
    inner.push("--".to_string());
    inner.extend(command);
    inner
}

/// Exec the provided argv, panicking with context if it fails.
fn exec_or_panic(command: Vec<String>) -> ! {
    #[expect(clippy::expect_used)]
    let c_command =
        CString::new(command[0].as_str()).expect("Failed to convert command to CString");
    #[expect(clippy::expect_used)]
    let c_args: Vec<CString> = command
        .iter()
        .map(|arg| CString::new(arg.as_str()).expect("Failed to convert arg to CString"))
        .collect();

    let mut c_args_ptrs: Vec<*const libc::c_char> = c_args.iter().map(|arg| arg.as_ptr()).collect();
    c_args_ptrs.push(std::ptr::null());

    unsafe {
        libc::execvp(c_command.as_ptr(), c_args_ptrs.as_ptr());
    }

    // If execvp returns, there was an error.
    let err = std::io::Error::last_os_error();
    panic!("Failed to execvp {}: {err}", command[0].as_str());
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_core::protocol::SandboxPolicy;
    use pretty_assertions::assert_eq;

    #[test]
    fn detects_proc_mount_invalid_argument_failure() {
        let stderr = "bwrap: Can't mount proc on /newroot/proc: Invalid argument";
        assert_eq!(is_proc_mount_failure(stderr), true);
    }

    #[test]
    fn ignores_non_proc_mount_errors() {
        let stderr = "bwrap: Can't bind mount /dev/null: Operation not permitted";
        assert_eq!(is_proc_mount_failure(stderr), false);
    }

    #[test]
    fn inserts_bwrap_argv0_before_command_separator() {
        let argv = build_bwrap_argv(
            vec!["/bin/true".to_string()],
            &SandboxPolicy::ReadOnly,
            Path::new("/"),
            BwrapOptions { mount_proc: true },
        );
        assert_eq!(
            argv,
            vec![
                "bwrap".to_string(),
                "--new-session".to_string(),
                "--die-with-parent".to_string(),
                "--ro-bind".to_string(),
                "/".to_string(),
                "/".to_string(),
                "--dev-bind".to_string(),
                "/dev/null".to_string(),
                "/dev/null".to_string(),
                "--unshare-pid".to_string(),
                "--proc".to_string(),
                "/proc".to_string(),
                "--argv0".to_string(),
                "codex-linux-sandbox".to_string(),
                "--".to_string(),
                "/bin/true".to_string(),
            ]
        );
    }
}
