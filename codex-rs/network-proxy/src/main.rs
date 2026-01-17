use anyhow::Result;
use clap::Parser;
use codex_network_proxy::Args;
use codex_network_proxy::Command;
use codex_network_proxy::NetworkProxy;
use codex_network_proxy::run_init;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    if let Some(Command::Init) = args.command {
        run_init()?;
        return Ok(());
    }

    let proxy = NetworkProxy::from_cli_args(args).await?;
    proxy.run().await?.wait().await
}
