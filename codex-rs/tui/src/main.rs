use clap::Parser;
use codex_tui::Cli;
use codex_tui::run_main;
// entry point
#[tokio::main]
async fn main() -> std::io::Result<()> {
    let cli = Cli::parse();   /* parses command line argument*/
    run_main(cli)?;
    Ok(())
}
