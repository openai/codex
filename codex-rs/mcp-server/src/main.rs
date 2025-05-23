use std::path::PathBuf;

use codex_mcp_server::run_main;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let codex_linux_sandbox_exe: Option<PathBuf> = if cfg!(target_os = "linux") {
        std::env::current_exe().ok()
    } else {
        None
    };

    run_main(codex_linux_sandbox_exe).await?;
    Ok(())
}
