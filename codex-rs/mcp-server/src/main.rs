use codex_mcp_server::run_main;

fn main() -> anyhow::Result<()> {
    codex_linux_sandbox::run_with_sandbox(|codex_linux_sandbox_exe| async move {
        run_main(codex_linux_sandbox_exe).await?;
        Ok(())
    })
}
