use clap::Parser;
use codex_repl::run_main;
use codex_repl::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    run_main(cli).await?;

    Ok(())
}
