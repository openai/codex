//! Generates the canonical config schema fixture for development and releases.

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

/// Generate the JSON Schema for `config.toml` and write it to `config.schema.json`.
#[derive(Parser)]
#[command(name = "codex-write-config-schema")]
struct Args {
    #[arg(short, long, value_name = "PATH")]
    out: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let out_path = match args.out {
        Some(path) => path,
        None => default_out_path()?,
    };
    codex_config::schema::write_config_schema(&out_path)?;
    Ok(())
}

fn default_out_path() -> Result<PathBuf> {
    // Cargo supplies this when running the tool, including from outside the checkout.
    if let Some(manifest_dir) = std::env::var_os("CARGO_MANIFEST_DIR") {
        return Ok(PathBuf::from(manifest_dir).join("../core/config.schema.json"));
    }

    let cwd = std::env::current_dir()?;
    for directory in cwd.ancestors() {
        for workspace in [directory.to_path_buf(), directory.join("codex-rs")] {
            if workspace.join("config-schema/Cargo.toml").is_file()
                && workspace.join("core/Cargo.toml").is_file()
            {
                return Ok(workspace.join("core/config.schema.json"));
            }
        }
    }

    anyhow::bail!("could not locate the Codex checkout; specify the schema output with --out")
}
