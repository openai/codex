use codex_arg0::PreMainArgs;
use codex_arg0::arg0_dispatch_or_else;
use codex_common::CliConfigOverrides;
use codex_mcp_server::run_main;

fn main() -> anyhow::Result<()> {
    arg0_dispatch_or_else(false, |pre_main_args| async move {
        let PreMainArgs {
            codex_linux_sandbox_exe,
            openai_api_key: _,
        } = pre_main_args;
        run_main(codex_linux_sandbox_exe, CliConfigOverrides::default()).await?;
        Ok(())
    })
}
