use anyhow::Result;
use clap::Parser;
use codex_network_proxy::Args;

#[tokio::main]
async fn main() -> Result<()> {
    codex_network_proxy::run_main(Args::parse()).await
}
