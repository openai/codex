use clap::Parser;
use codex_cli::migrate::MigrateCli;

fn main() {
    if let Err(err) = MigrateCli::parse().run() {
        eprintln!("{err:?}");
        std::process::exit(1);
    }
}
