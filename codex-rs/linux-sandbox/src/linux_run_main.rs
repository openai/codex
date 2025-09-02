use clap::Parser;
use std::ffi::CString;
use std::path::PathBuf;

use crate::landlock::apply_sandbox_policy_to_current_thread;

#[derive(Debug, Parser)]
pub struct LandlockCommand {
    /// It is possible that the cwd used in the context of the sandbox policy
    /// is different from the cwd of the process to spawn.
    pub sandbox_policy_cwd: PathBuf,

    pub sandbox_policy: codex_core::protocol::SandboxPolicy,

    /// Full command args to run under landlock.
    #[arg(trailing_var_arg = true)]
    pub command: Vec<String>,
}

pub fn run_main() -> ! {
    let LandlockCommand {
        sandbox_policy_cwd,
        sandbox_policy,
        command,
    } = LandlockCommand::parse();

    if let Err(e) = apply_sandbox_policy_to_current_thread(&sandbox_policy, &sandbox_policy_cwd) {
        eprintln!(
            "error: failed to enable Linux sandbox (Landlock): {}\n\nThis usually means your kernel/container doesn't support Landlock or it is disabled (e.g., older kernel, WSL2, or a restricted container).",
            e
        );
        std::process::exit(1);
    }

    if command.is_empty() {
        eprintln!("error: no command specified to execute");
        std::process::exit(2);
    }

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
    eprintln!(
        "error: failed to execute {}: {}\n\nHint: ensure the command exists and is on PATH.",
        command[0].as_str(),
        err
    );
    std::process::exit(127);
}
