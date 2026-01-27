use anyhow::Result;
use clap::Parser;
use codex_network_proxy::Args;
use codex_network_proxy::NetworkProxy;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    let _ = args;
    let proxy = NetworkProxy::builder().build().await?;
    proxy.run().await?.wait().await
}
