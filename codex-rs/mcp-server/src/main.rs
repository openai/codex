use codex_arg0::arg0_dispatch_or_else;
use codex_common::CliConfigOverrides;
use codex_mcp_server::run_main;

fn main() -> anyhow::Result<()> {
    arg0_dispatch_or_else(|codex_linux_sandbox_exe| async move {
        // Launch the MCP server.
        let res = run_main(codex_linux_sandbox_exe, CliConfigOverrides::default()).await;
        // Opportunistically start scheduler when feature is enabled; it is no-op otherwise.
        #[cfg(feature = "scheduler")]
        {
            // Note: this call does not change behavior unless [scheduler] is enabled in ~/.codex/config.toml
            // and Arango config is present.
            super::scheduler_bootstrap::start_if_enabled();
        }
        res?;
        Ok(())
    })
}
