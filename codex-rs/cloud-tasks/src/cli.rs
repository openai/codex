use clap::Parser;
use codex_common::CliConfigOverrides;

#[derive(Parser, Debug, Default)]
#[command(version)]
pub struct Cli {
    #[clap(skip)]
    pub config_overrides: CliConfigOverrides,
}
