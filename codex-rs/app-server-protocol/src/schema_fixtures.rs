use anyhow::Context;
use anyhow::Result;
use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

pub fn read_schema_fixture_tree(schema_root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
    let typescript_root = schema_root.join("typescript");
    let json_root = schema_root.join("json");

    let mut all = BTreeMap::new();
    for (rel, bytes) in collect_files_recursive(&typescript_root)? {
        all.insert(PathBuf::from("typescript").join(rel), bytes);
    }
    for (rel, bytes) in collect_files_recursive(&json_root)? {
        all.insert(PathBuf::from("json").join(rel), bytes);
    }

    Ok(all)
}

/// Regenerates `schema/typescript/` and `schema/json/`.
///
/// This is intended to be used by tooling (e.g., `just write-app-server-schema`).
/// It deletes any previously generated files so stale artifacts are removed.
pub fn write_schema_fixtures(schema_root: &Path, prettier: Option<&Path>) -> Result<()> {
    let typescript_out_dir = schema_root.join("typescript");
    let json_out_dir = schema_root.join("json");

    ensure_empty_dir(&typescript_out_dir)?;
    ensure_empty_dir(&json_out_dir)?;

    crate::generate_ts(&typescript_out_dir, prettier)?;
    crate::generate_json(&json_out_dir)?;

    Ok(())
}

fn ensure_empty_dir(dir: &Path) -> Result<()> {
    if dir.exists() {
        std::fs::remove_dir_all(dir)
            .with_context(|| format!("failed to remove {}", dir.display()))?;
    }
    std::fs::create_dir_all(dir).with_context(|| format!("failed to create {}", dir.display()))?;
    Ok(())
}

fn read_file_bytes(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))
}

fn collect_files_recursive(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
    let mut files = BTreeMap::new();

    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)
            .with_context(|| format!("failed to read dir {}", dir.display()))?
        {
            let entry =
                entry.with_context(|| format!("failed to read dir entry in {}", dir.display()))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .with_context(|| format!("failed to stat {}", path.display()))?;
            if file_type.is_dir() {
                stack.push(path);
                continue;
            } else if !file_type.is_file() {
                continue;
            }

            let rel = path
                .strip_prefix(root)
                .with_context(|| {
                    format!(
                        "failed to strip prefix {} from {}",
                        root.display(),
                        path.display()
                    )
                })?
                .to_path_buf();

            files.insert(rel, read_file_bytes(&path)?);
        }
    }

    Ok(files)
}
