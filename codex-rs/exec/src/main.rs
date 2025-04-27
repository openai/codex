use clap::Parser;
use codex_exec::run_main;
use codex_exec::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    run_main(cli).await?;

    Ok(())
}
