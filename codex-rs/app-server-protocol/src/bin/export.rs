use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    about = "Generate TypeScript bindings and JSON Schemas for the Codex app-server protocol"
)]
struct Args {
    /// Output directory where generated files will be written
    #[arg(short = 'o', long = "out", value_name = "DIR")]
    out_dir: PathBuf,

    /// Optional Prettier executable path to format generated TypeScript files
    #[arg(short = 'p', long = "prettier", value_name = "PRETTIER_BIN")]
    prettier: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    codex_app_server_protocol::generate_types(&args.out_dir, args.prettier.as_deref())
}
