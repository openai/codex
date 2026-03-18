use clap::Parser;
use codex_exec_server::ExecServerTransport;

#[derive(Debug, Parser)]
struct ExecServerArgs {
    /// Transport endpoint URL. Supported values: `ws://IP:PORT` (default),
    /// `stdio://`.
    #[arg(
        long = "listen",
        value_name = "URL",
        default_value = ExecServerTransport::DEFAULT_LISTEN_URL
    )]
    listen: ExecServerTransport,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = ExecServerArgs::parse();
    codex_exec_server::run_main_with_transport(args.listen).await
}
