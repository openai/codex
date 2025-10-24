use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Child;
use tokio::process::Command;
use tracing::trace;

use crate::protocol::SandboxPolicy;

/// Experimental environment variable that will be set to some non-empty value
/// if both of the following are true:
///
/// 1. The process was spawned by Codex as part of a shell tool call.
/// 2. SandboxPolicy.has_full_network_access() was false for the tool call.
///
/// We may try to have just one environment variable for all sandboxing
/// attributes, so this may change in the future.
pub const CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR: &str = "CODEX_SANDBOX_NETWORK_DISABLED";

/// Should be set when the process is spawned under a sandbox. Currently, the
/// value is "seatbelt" for macOS, but it may change in the future to
/// accommodate sandboxing configuration and other sandboxing mechanisms.
pub const CODEX_SANDBOX_ENV_VAR: &str = "CODEX_SANDBOX";

#[derive(Debug, Clone, Copy)]
pub enum StdioPolicy {
    RedirectForShellTool,
    Inherit,
}

/// Spawns the appropriate child process for the ExecParams and SandboxPolicy,
/// ensuring the args and environment variables used to create the `Command`
/// (and `Child`) honor the configuration.
///
/// For now, we take `SandboxPolicy` as a parameter to spawn_child() because
/// we need to determine whether to set the
/// `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR` environment variable.
pub(crate) async fn spawn_child_async(
    program: PathBuf,
    args: Vec<String>,
    #[cfg_attr(not(unix), allow(unused_variables))] arg0: Option<&str>,
    cwd: PathBuf,
    sandbox_policy: &SandboxPolicy,
    stdio_policy: StdioPolicy,
    env: HashMap<String, String>,
) -> std::io::Result<Child> {
    trace!(
        "spawn_child_async: {program:?} {args:?} {arg0:?} {cwd:?} {sandbox_policy:?} {stdio_policy:?} {env:?}"
    );

    let mut cmd = Command::new(&program);
    #[cfg(unix)]
    cmd.arg0(arg0.map_or_else(|| program.to_string_lossy().to_string(), String::from));
    cmd.args(args);
    cmd.current_dir(cwd);
    cmd.env_clear();
    cmd.envs(env);

    if !sandbox_policy.has_full_network_access() {
        cmd.env(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR, "1");
    }

    // If this Codex process dies (including being killed via SIGKILL), we want
    // any child processes that were spawned as part of a `"shell"` tool call
    // to also be terminated.

    // This relies on prctl(2), so it only works on Linux.
    #[cfg(target_os = "linux")]
    unsafe {
        cmd.pre_exec(|| {
            // This prctl call effectively requests, "deliver SIGTERM when my
            // current parent dies."
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) == -1 {
                return Err(std::io::Error::last_os_error());
            }

            // Though if there was a race condition and this pre_exec() block is
            // run _after_ the parent (i.e., the Codex process) has already
            // exited, then the parent is the _init_ process (which will never
            // die), so we should just terminate the child process now.
            if libc::getppid() == 1 {
                libc::raise(libc::SIGTERM);
            }
            Ok(())
        });
    }

    match stdio_policy {
        StdioPolicy::RedirectForShellTool => {
            // Do not create a file descriptor for stdin because otherwise some
            // commands may hang forever waiting for input. For example, ripgrep has
            // a heuristic where it may try to read from stdin as explained here:
            // https://github.com/BurntSushi/ripgrep/blob/e2362d4d5185d02fa857bf381e7bd52e66fafc73/crates/core/flags/hiargs.rs#L1101-L1103
            cmd.stdin(Stdio::null());

            cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        }
        StdioPolicy::Inherit => {
            // Inherit stdin, stdout, and stderr from the parent process.
            cmd.stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());
        }
    }

    cmd.kill_on_drop(true).spawn()
}
