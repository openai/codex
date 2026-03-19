use clap::Parser;
use codex_exec_server::DEFAULT_LISTEN_URL;

#[derive(Debug, Parser)]
struct ExecServerArgs {
    /// Transport endpoint URL. Supported values: `stdio://` (default),
    /// `ws://IP:PORT`.
    #[arg(
        long = "listen",
        value_name = "URL",
        default_value = DEFAULT_LISTEN_URL
    )]
    listen: String,
}

#[tokio::main]
async fn main() {
    let args = ExecServerArgs::parse();
    if let Err(err) = codex_exec_server::run_main_with_transport(&args.listen).await {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
