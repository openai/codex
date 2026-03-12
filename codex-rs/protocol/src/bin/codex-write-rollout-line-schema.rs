use std::io;
use std::path::PathBuf;

use codex_protocol::default_rollout_line_schema_dir;
use codex_protocol::write_rollout_line_schema_artifacts;

const JSON_SCHEMA_FILENAME: &str = "rollout-line.schema.json";
const TYPESCRIPT_FILENAME: &str = "rollout-line.schema.ts";

fn main() -> io::Result<()> {
    let out_dir = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(default_rollout_line_schema_dir);
    write_rollout_line_schema_artifacts(&out_dir)?;
    for filename in [JSON_SCHEMA_FILENAME, TYPESCRIPT_FILENAME] {
        println!("Wrote {}", out_dir.join(filename).display());
    }
    Ok(())
}
