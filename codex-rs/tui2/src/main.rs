//! CLI entry point for the Codex TUI (v2) binary.
//!
//! This wrapper merges top-level config overrides with the TUI CLI arguments,
//! dispatches into the async runtime entry point, and prints token usage
//! summaries when available.

use clap::Parser;
use codex_arg0::arg0_dispatch_or_else;
use codex_common::CliConfigOverrides;
use codex_tui2::Cli;
use codex_tui2::run_main;

/// Top-level CLI arguments that wrap the TUI CLI with shared overrides.
#[derive(Parser, Debug)]
struct TopCli {
    /// Config override arguments collected at the top level.
    #[clap(flatten)]
    config_overrides: CliConfigOverrides,

    /// The nested TUI CLI arguments.
    #[clap(flatten)]
    inner: Cli,
}

/// Parse CLI args, run the async TUI entry point, and emit token usage.
fn main() -> anyhow::Result<()> {
    arg0_dispatch_or_else(|codex_linux_sandbox_exe| async move {
        let top_cli = TopCli::parse();
        let mut inner = top_cli.inner;
        inner
            .config_overrides
            .raw_overrides
            .splice(0..0, top_cli.config_overrides.raw_overrides);
        let exit_info = run_main(inner, codex_linux_sandbox_exe).await?;
        let token_usage = exit_info.token_usage;
        if !token_usage.is_zero() {
            println!("{}", codex_core::protocol::FinalOutput::from(token_usage),);
        }
        Ok(())
    })
}
