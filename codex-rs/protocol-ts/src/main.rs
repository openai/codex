use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(about = "Generate TypeScript bindings for the Codex protocol")]
struct Args {
    /// Output directory where .ts files will be written
    #[arg(short = 'o', long = "out", value_name = "DIR")]
    out_dir: PathBuf,

    /// Optional path to the Prettier executable to format generated files
    #[arg(short = 'p', long = "prettier", value_name = "PRETTIER_BIN")]
    prettier: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    codex_protocol_ts::generate_ts(&args.out_dir, args.prettier.as_deref())
}
