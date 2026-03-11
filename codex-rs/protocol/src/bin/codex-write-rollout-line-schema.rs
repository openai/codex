use std::error::Error;
use std::path::PathBuf;

use codex_protocol::protocol::RolloutLine;

fn main() -> Result<(), Box<dyn Error>> {
    let out_dir = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schema/rollout_line"));

    std::fs::create_dir_all(&out_dir)?;

    let schema_path = out_dir.join("RolloutLine.schema.json");
    let schema = schemars::schema_for!(RolloutLine);
    let schema_json = serde_json::to_vec_pretty(&schema)?;
    std::fs::write(&schema_path, schema_json)?;

    println!("wrote {}", schema_path.display());

    Ok(())
}
