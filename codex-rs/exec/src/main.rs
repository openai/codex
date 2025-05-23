use std::path::PathBuf;

use clap::Parser;
use codex_exec::Cli;
use codex_exec::run_main;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let codex_linux_sandbox_exe: Option<PathBuf> = if cfg!(target_os = "linux") {
        std::env::current_exe().ok()
    } else {
        None
    };

    let cli = Cli::parse();
    run_main(cli, codex_linux_sandbox_exe).await?;

    Ok(())
}
