mod exit_status;
#[cfg(unix)]
pub mod landlock;
pub mod proto;
pub mod seatbelt;

use clap::Parser;
use codex_common::SandboxPermissionOption;
use codex_core::protocol::SandboxPolicy;

#[derive(Debug, Parser)]
pub struct SeatbeltCommand {
    /// Convenience alias for low-friction sandboxed automatic execution (network-disabled sandbox that can write to cwd and TMPDIR)
    #[arg(long = "full-auto", default_value_t = false)]
    pub full_auto: bool,

    #[clap(flatten)]
    pub sandbox: SandboxPermissionOption,

    /// Full command args to run under seatbelt.
    #[arg(trailing_var_arg = true)]
    pub command: Vec<String>,
}

#[derive(Debug, Parser)]
pub struct LandlockCommand {
    /// Convenience alias for low-friction sandboxed automatic execution (network-disabled sandbox that can write to cwd and TMPDIR)
    #[arg(long = "full-auto", default_value_t = false)]
    pub full_auto: bool,

    #[clap(flatten)]
    pub sandbox: SandboxPermissionOption,

    /// Full command args to run under landlock.
    #[arg(trailing_var_arg = true)]
    pub command: Vec<String>,
}

pub fn create_sandbox_policy(full_auto: bool, sandbox: SandboxPermissionOption) -> SandboxPolicy {
    if full_auto {
        SandboxPolicy::new_full_auto_policy()
    } else {
        match sandbox.permissions.map(Into::into) {
            Some(sandbox_policy) => sandbox_policy,
            None => SandboxPolicy::new_read_only_policy(),
        }
    }
}
