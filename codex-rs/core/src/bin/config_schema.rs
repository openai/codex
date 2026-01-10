use anyhow::Context;
use anyhow::Result;
use std::env;
use std::path::PathBuf;

/// Generate the JSON Schema for `config.toml` and write it to `config.schema.json`.
fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let mut out_path = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" | "-o" => {
                let value = args.next().context("expected a path after --out/-o")?;
                out_path = Some(PathBuf::from(value));
            }
            _ => anyhow::bail!("unknown argument: {arg}"),
        }
    }

    let out_path = out_path
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config.schema.json"));
    codex_core::config::schema::write_config_schema(&out_path)?;
    Ok(())
}
