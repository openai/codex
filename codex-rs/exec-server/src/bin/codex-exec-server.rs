use clap::Parser;

use std::path::PathBuf;

#[derive(Debug, Parser)]
struct ExecServerArgs {
    /// Transport endpoint URL. Supported values: `ws://IP:PORT` (default).
    #[arg(
        long = "listen",
        value_name = "URL",
        default_value = codex_exec_server::DEFAULT_LISTEN_URL
    )]
    listen: String,
}

fn main() -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    codex_linux_sandbox::dispatch_if_requested();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(run_main(linux_sandbox_exe()))
}

async fn run_main(codex_linux_sandbox_exe: Option<PathBuf>) -> anyhow::Result<()> {
    let args = ExecServerArgs::parse();
    let runtime = codex_exec_server::ExecServerRuntimeConfig::new(codex_linux_sandbox_exe);
    codex_exec_server::run_main_with_runtime(&args.listen, runtime)
        .await
        .map_err(|err| anyhow::Error::msg(err.to_string()))?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn linux_sandbox_exe() -> Option<PathBuf> {
    std::env::current_exe().ok()
}

#[cfg(not(target_os = "linux"))]
fn linux_sandbox_exe() -> Option<PathBuf> {
    None
}
