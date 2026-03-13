use codex_protocol::protocol::RolloutLine;
use schemars::r#gen::SchemaSettings;
use serde_json::Map;
use serde_json::Value;
use std::io;
use std::path::PathBuf;
const JSON_SCHEMA_FILENAME: &str = "rollout-line.schema.json";

fn main() -> io::Result<()> {
    let out_dir = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let codex_rs_dir = manifest_dir.parent().unwrap_or(&manifest_dir);
            codex_rs_dir.join("out/rollout-line-schema")
        });

    std::fs::create_dir_all(&out_dir)?;
    std::fs::write(
        out_dir.join(JSON_SCHEMA_FILENAME),
        rollout_line_schema_json()?,
    )?;
    println!("Wrote {}", out_dir.join(JSON_SCHEMA_FILENAME).display());
    Ok(())
}

fn rollout_line_schema_json() -> io::Result<Vec<u8>> {
    let schema = SchemaSettings::draft07()
        .into_generator()
        .into_root_schema_for::<RolloutLine>();
    let value = serde_json::to_value(schema).map_err(io::Error::other)?;
    let value = canonicalize_json(&value);
    serde_json::to_vec_pretty(&value).map_err(io::Error::other)
}

fn canonicalize_json(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(canonicalize_json).collect()),
        Value::Object(map) => {
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            let mut sorted = Map::with_capacity(map.len());
            for (key, child) in entries {
                sorted.insert(key.clone(), canonicalize_json(child));
            }
            Value::Object(sorted)
        }
        _ => value.clone(),
    }
}
