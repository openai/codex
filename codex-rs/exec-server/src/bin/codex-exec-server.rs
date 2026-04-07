use clap::Parser;

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = ExecServerArgs::parse();
    codex_exec_server::run_main_with_listen_url(&args.listen)
        .await
        .map_err(|err| anyhow::Error::msg(err.to_string()))?;
    Ok(())
}
