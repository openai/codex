use std::path::PathBuf;

use clap::Args;
use clap::Parser;

#[derive(Debug, Parser)]
#[command(bin_name = "codex privacy")]
pub(crate) struct PrivacyCommand {
    #[command(subcommand)]
    subcommand: PrivacySubcommand,
}

#[derive(Debug, clap::Subcommand)]
enum PrivacySubcommand {
    /// Export your local Codex data.
    Export(PrivacyExportCommand),
}

#[derive(Debug, Args)]
struct PrivacyExportCommand {
    /// Directory to copy the export into.
    #[arg(value_name = "PATH")]
    output: PathBuf,
}

#[cfg(test)]
#[path = "privacy_cmd_tests.rs"]
mod tests;
