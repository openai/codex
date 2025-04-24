use clap::Parser;
use codex_interactive::run_main;
use codex_interactive::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    run_main(cli).await?;

    Ok(())
}
