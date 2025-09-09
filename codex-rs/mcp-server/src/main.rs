use clap::Parser;
use codex_arg0::arg0_dispatch_or_else;
use codex_common::CliConfigOverrides;
use codex_mcp_server::run_main;

/// Codex MCP Server
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct McpCli {
    /// Enable compatibility mode for MCP clients that cannot handle async notifications
    #[arg(long)]
    compatibility_mode: bool,

    #[command(flatten)]
    config_overrides: CliConfigOverrides,
}

fn main() -> anyhow::Result<()> {
    arg0_dispatch_or_else(|codex_linux_sandbox_exe| async move {
        let cli = McpCli::parse();
        run_main(
            codex_linux_sandbox_exe,
            cli.config_overrides,
            cli.compatibility_mode,
        )
        .await?;
        Ok(())
    })
}
