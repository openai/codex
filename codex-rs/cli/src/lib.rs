pub mod agent_create_cmd;
pub mod ask_cmd;
pub mod blueprint_commands;
pub mod debug_sandbox;
pub mod delegate_cmd;
mod exit_status;
pub mod git_commands;
pub mod lock_cmd;
pub mod login;
pub mod mcp_cmd;
pub mod parallel_delegate_cmd;
pub mod pair_program_cmd;
pub mod research_cmd;
pub mod webhook_cmd;

use clap::Parser;
use codex_common::CliConfigOverrides;
use codex_core::config::Config;
use std::convert::TryFrom;

#[derive(Debug, Parser)]
pub struct SeatbeltCommand {
    /// Convenience alias for low-friction sandboxed automatic execution (network-disabled sandbox that can write to cwd and TMPDIR)
    #[arg(long = "full-auto", default_value_t = false)]
    pub full_auto: bool,

    #[clap(skip)]
    pub config_overrides: CliConfigOverrides,

    /// Full command args to run under seatbelt.
    #[arg(trailing_var_arg = true)]
    pub command: Vec<String>,
}

/// Resolve the runtime token budget for sub-agent execution.
///
/// The budget value in the config is stored as `Option<i64>` to mirror the
/// server-side representation. We clamp the value to a non-negative range and
/// downcast safely to `usize` so it can be consumed by the runtime.
pub fn resolve_runtime_budget(config: &Config, default_budget: i64) -> usize {
    let raw_budget = config.model_context_window.unwrap_or(default_budget).max(0);

    let as_u64 = u64::try_from(raw_budget).unwrap_or(u64::MAX);
    let capped = as_u64.min(usize::MAX as u64);

    usize::try_from(capped).unwrap_or(usize::MAX)
}

#[derive(Debug, Parser)]
pub struct LandlockCommand {
    /// Convenience alias for low-friction sandboxed automatic execution (network-disabled sandbox that can write to cwd and TMPDIR)
    #[arg(long = "full-auto", default_value_t = false)]
    pub full_auto: bool,

    #[clap(skip)]
    pub config_overrides: CliConfigOverrides,

    /// Full command args to run under landlock.
    #[arg(trailing_var_arg = true)]
    pub command: Vec<String>,
}

#[derive(Debug, Parser)]
pub struct WindowsCommand {
    /// Convenience alias for low-friction sandboxed automatic execution (network-disabled sandbox that can write to cwd and TMPDIR)
    #[arg(long = "full-auto", default_value_t = false)]
    pub full_auto: bool,

    #[clap(skip)]
    pub config_overrides: CliConfigOverrides,

    /// Full command args to run under Windows restricted token sandbox.
    #[arg(trailing_var_arg = true)]
    pub command: Vec<String>,
}
