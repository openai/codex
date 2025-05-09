use clap::Parser;
use codex_tui::Cli;
use codex_tui::run_main;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let cli = Cli::parse();
    run_main(cli)?;
    Ok(())
}
