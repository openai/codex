pub mod debug_sandbox;
mod exit_status;
pub mod login;

use clap::Parser;
use codex_common::CliConfigOverrides;

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
