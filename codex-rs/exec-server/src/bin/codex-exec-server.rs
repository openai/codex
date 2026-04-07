#[cfg(target_os = "linux")]
use std::path::Path;

use clap::Parser;
#[cfg(target_os = "linux")]
use codex_sandboxing::landlock::CODEX_LINUX_SANDBOX_ARG0;

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
    dispatch_arg0();

    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let args = ExecServerArgs::parse();
        codex_exec_server::run_main_with_listen_url(&args.listen)
            .await
            .map_err(|err| anyhow::Error::msg(err.to_string()))
    })
}

#[cfg(target_os = "linux")]
fn dispatch_arg0() {
    let argv0 = std::env::args_os().next().unwrap_or_default();
    let exe_name = Path::new(&argv0)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();

    if exe_name == CODEX_LINUX_SANDBOX_ARG0 {
        codex_linux_sandbox::run_main();
    }
}

#[cfg(not(target_os = "linux"))]
fn dispatch_arg0() {}
