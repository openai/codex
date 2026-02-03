use clap::Parser;
use std::ffi::CString;
use std::path::Path;
use std::path::PathBuf;

use crate::bwrap::BwrapOptions;
use crate::bwrap::create_bwrap_command_args;
use crate::bwrap::create_bwrap_command_args_vendored;
use crate::landlock::apply_sandbox_policy_to_current_thread;
use crate::vendored_bwrap::exec_vendored_bwrap;

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

    /// Optional explicit path to the `bwrap` binary to use.
    ///
    /// When provided, this implies bubblewrap opt-in and avoids PATH lookups.
    #[arg(long = "bwrap-path", hide = true)]
    pub bwrap_path: Option<PathBuf>,

    /// Experimental: call a build-time bubblewrap `main()` via FFI.
    ///
    /// This is opt-in and only works when the build script compiles bwrap.
    #[arg(long = "use-vendored-bwrap", hide = true, default_value_t = false)]
    pub use_vendored_bwrap: bool,

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
        bwrap_path,
        use_vendored_bwrap,
        apply_seccomp_then_exec,
        no_proc,
        command,
    } = LandlockCommand::parse();
    let use_bwrap_sandbox = use_bwrap_sandbox || bwrap_path.is_some() || use_vendored_bwrap;

    if command.is_empty() {
        panic!("No command specified to execute.");
    }

    // Inner stage: apply seccomp/no_new_privs after bubblewrap has already
    // established the filesystem view.
    if apply_seccomp_then_exec {
        if let Err(e) = apply_sandbox_policy_to_current_thread(&sandbox_policy, &sandbox_policy_cwd)
        {
            panic!("error applying Linux sandbox restrictions: {e:?}");
        }
        exec_or_panic(command);
    }

    let command = if sandbox_policy.has_full_disk_write_access() {
        if let Err(e) = apply_sandbox_policy_to_current_thread(&sandbox_policy, &sandbox_policy_cwd)
        {
            panic!("error applying Linux sandbox restrictions: {e:?}");
        }
        command
    } else if use_bwrap_sandbox {
        // Outer stage: bubblewrap first, then re-enter this binary in the
        // sandboxed environment to apply seccomp.
        let inner = build_inner_seccomp_command(
            &sandbox_policy_cwd,
            &sandbox_policy,
            use_bwrap_sandbox,
            bwrap_path.as_deref(),
            command,
        );
        let options = BwrapOptions {
            mount_proc: !no_proc,
        };
        if use_vendored_bwrap {
            let mut argv0 = bwrap_path
                .as_deref()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_else(|| "bwrap".to_string());
            if argv0.is_empty() {
                argv0 = "bwrap".to_string();
            }

            let mut argv = vec![argv0];
            argv.extend(
                create_bwrap_command_args_vendored(
                    inner,
                    &sandbox_policy,
                    &sandbox_policy_cwd,
                    options,
                )
                .unwrap_or_else(|err| {
                    panic!("error building build-time bubblewrap command: {err:?}")
                }),
            );
            exec_vendored_bwrap(argv);
        }
        ensure_bwrap_available(bwrap_path.as_deref());
        create_bwrap_command_args(
            inner,
            &sandbox_policy,
            &sandbox_policy_cwd,
            options,
            bwrap_path.as_deref(),
        )
        .unwrap_or_else(|err| panic!("error building bubblewrap command: {err:?}"))
    } else {
        // Legacy path: Landlock enforcement only.
        if let Err(e) = apply_sandbox_policy_to_current_thread(&sandbox_policy, &sandbox_policy_cwd)
        {
            panic!("error applying legacy Linux sandbox restrictions: {e:?}");
        }
        command
    };

    exec_or_panic(command);
}

/// Build the inner command that applies seccomp after bubblewrap.
fn build_inner_seccomp_command(
    sandbox_policy_cwd: &Path,
    sandbox_policy: &codex_core::protocol::SandboxPolicy,
    use_bwrap_sandbox: bool,
    bwrap_path: Option<&Path>,
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
    if let Some(bwrap_path) = bwrap_path {
        inner.push("--bwrap-path".to_string());
        inner.push(bwrap_path.to_string_lossy().to_string());
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

/// Ensure the `bwrap` binary is available when the sandbox needs it.
fn ensure_bwrap_available(bwrap_path: Option<&Path>) {
    if let Some(path) = bwrap_path {
        if path.exists() {
            return;
        }
        panic!(
            "bubblewrap (bwrap) is required for Linux filesystem sandboxing but was not found at the configured path: {}\n\
Install it and retry. Examples:\n\
- Debian/Ubuntu: apt-get install bubblewrap\n\
- Fedora/RHEL: dnf install bubblewrap\n\
- Arch: pacman -S bubblewrap\n\
If you are running the Codex Node package, ensure bwrap is installed on the host system.",
            path.display()
        );
    }
    if which::which("bwrap").is_ok() {
        return;
    }

    panic!(
        "bubblewrap (bwrap) is required for Linux filesystem sandboxing but was not found on PATH.\n\
Install it and retry. Examples:\n\
- Debian/Ubuntu: apt-get install bubblewrap\n\
- Fedora/RHEL: dnf install bubblewrap\n\
- Arch: pacman -S bubblewrap\n\
If you are running the Codex Node package, ensure bwrap is installed on the host system."
    );
}
