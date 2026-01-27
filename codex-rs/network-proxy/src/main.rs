use anyhow::Result;
use clap::Parser;
use codex_network_proxy::Args;
use codex_network_proxy::run_main;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    run_main(args).await
}
