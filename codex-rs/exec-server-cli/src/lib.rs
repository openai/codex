//! Startup shared by the full Codex CLI and the standalone executor.
//! Internal sandbox and filesystem re-exec modes always dispatch before argument parsing.

use clap::CommandFactory;
use clap::Parser;
use codex_utils_cli::CliConfigOverrides;
use codex_utils_cli::collect_config_overrides;

mod command;
mod config;
mod exec_server_auth;
mod exec_server_credential;
mod exec_server_telemetry;

pub use command::ExecServerCommand;

#[derive(Debug, Parser)]
#[command(name = "codex-exec-server", bin_name = "codex-exec-server", version)]
struct ExecServerCli {
    #[command(flatten)]
    config_overrides: CliConfigOverrides,
    #[command(flatten)]
    command: ExecServerCommand,
}

/// Starts the executor after dispatching internal helper modes and preparing runtime paths.
pub fn run_standalone() -> anyhow::Result<()> {
    codex_arg0::arg0_dispatch_or_else(|paths| async move {
        let args: Vec<_> = std::env::args_os().collect();
        let mut cli = ExecServerCli::parse_from(&args);
        cli.config_overrides = collect_config_overrides(ExecServerCli::command(), args)
            .unwrap_or_else(|error| error.exit());
        cli.command.run(&paths, &cli.config_overrides).await
    })
}
