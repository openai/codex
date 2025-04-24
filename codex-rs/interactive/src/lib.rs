mod cli;
pub use cli::Cli;

pub async fn run_main(_cli: Cli) -> anyhow::Result<()> {
    eprintln!("Interactive mode is not implemented yet.");
    std::process::exit(1);
}
