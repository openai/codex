use clap::Parser;
use codex_arg0::Arg0DispatchPaths;
use codex_arg0::arg0_dispatch_or_else;

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
    arg0_dispatch_or_else(|arg0_paths: Arg0DispatchPaths| async move {
        let args = ExecServerArgs::parse();
        codex_exec_server::configure_arg0_paths(arg0_paths);
        codex_exec_server::run_main_with_listen_url(&args.listen).await?;
        Ok(())
    })
}
