use anyhow::Result;
use clap::Parser;
use codex_core::network_proxy_loader;
use codex_network_proxy::Args;
use codex_network_proxy::NetworkProxy;
use codex_network_proxy::NetworkProxyState;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    let (state, reloader) = network_proxy_loader::build_network_proxy_state_and_reloader().await?;
    let state = Arc::new(NetworkProxyState::with_reloader(state, Arc::new(reloader)));
    let _ = args;
    let proxy = NetworkProxy::builder().state(state).build().await?;
    proxy.run().await?.wait().await
}
