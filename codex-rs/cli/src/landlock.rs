//! `debug landlock` implementation for the Codex CLI.
//!
//! On Linux the command is executed inside a Landlock + seccomp sandbox by
//! calling the low-level `exec_linux` helper from `codex_core::linux`.

use codex_core::exec::StdioPolicy;
use codex_core::exec::spawn_child_sync;
use codex_core::exec_linux::apply_sandbox_policy_to_current_thread;
use codex_core::protocol::SandboxPolicy;
use std::process::ExitStatus;

use crate::exit_status::handle_exit_status;

/// Execute `command` in a Linux sandbox (Landlock + seccomp) the way Codex
/// would.
pub fn run_landlock(command: Vec<String>, sandbox_policy: SandboxPolicy) -> anyhow::Result<()> {
    if command.is_empty() {
        anyhow::bail!("command args are empty");
    }

    // Spawn a new thread and apply the sandbox policies there.
    let handle = std::thread::spawn(move || -> anyhow::Result<ExitStatus> {
        let cwd = std::env::current_dir()?;

        apply_sandbox_policy_to_current_thread(&sandbox_policy, &cwd)?;
        let mut child = spawn_child_sync(command, cwd, &sandbox_policy, StdioPolicy::Inherit)?;
        let status = child.wait()?;
        Ok(status)
    });
    let status = handle
        .join()
        .map_err(|e| anyhow::anyhow!("Failed to join thread: {e:?}"))??;

    handle_exit_status(status);
}
