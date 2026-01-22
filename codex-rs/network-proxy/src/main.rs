use anyhow::Result;
use clap::Parser;
use codex_network_proxy::Args;
use codex_network_proxy::NetworkProxy;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    let proxy = NetworkProxy::from_cli_args(args).await?;
    proxy.run().await?.wait().await
}
