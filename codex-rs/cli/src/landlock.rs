//! `debug landlock` implementation for the Codex CLI.
//!
//! On Linux the command is executed inside a Landlock + seccomp sandbox by
//! calling the low-level `exec_linux` helper from `codex_core::linux`.

use codex_core::protocol::SandboxPolicy;
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process;
use std::process::Command;
use std::process::ExitStatus;

/// Execute `command` in a Linux sandbox (Landlock + seccomp) the way Codex
/// would.
pub(crate) fn run_landlock(
    command: Vec<String>,
    sandbox_policy: SandboxPolicy,
    writable_roots: Vec<PathBuf>,
) -> anyhow::Result<()> {
    if command.is_empty() {
        anyhow::bail!("command args are empty");
    }

    // Spawn a new thread and apply the sandbox policies there.
    let handle = std::thread::spawn(move || -> anyhow::Result<ExitStatus> {
        // Apply sandbox policies inside this thread so only the child inherits
        // them, not the entire CLI process.
        if sandbox_policy.is_network_restricted() {
            codex_core::linux::install_network_seccomp_filter_on_current_thread()?;
        }

        if sandbox_policy.is_file_write_restricted() {
            codex_core::linux::install_filesystem_landlock_rules_on_current_thread(writable_roots)?;
        }

        let status = Command::new(&command[0]).args(&command[1..]).status()?;
        Ok(status)
    });
    let status = handle
        .join()
        .map_err(|e| anyhow::anyhow!("Failed to join thread: {e:?}"))??;

    // Use ExitStatus to derive the exit code.
    if let Some(code) = status.code() {
        process::exit(code);
    } else if let Some(signal) = status.signal() {
        process::exit(128 + signal);
    } else {
        process::exit(1);
    }
}
