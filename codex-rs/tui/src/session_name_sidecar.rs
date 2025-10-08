use std::fs::OpenOptions;
use std::io::Read;
use std::io::Write;
use std::io::{self};
use std::path::Path;
use std::path::PathBuf;

/// Compute the sidecar path for a rollout file. For standard rollouts with a
/// `.jsonl` extension, this produces `*.jsonl.name` next to the rollout.
/// For other extensions, appends `.name` to the existing extension.
pub(crate) fn sidecar_path_for(rollout: &Path) -> PathBuf {
    match rollout.extension().and_then(|e| e.to_str()) {
        Some("jsonl") => rollout.with_extension("jsonl.name"),
        Some(ext) => rollout.with_extension(format!("{ext}.name")),
        None => rollout.with_extension("name"),
    }
}

/// Read the display name sidecar, if present. Returns Ok(None) when missing.
pub(crate) fn read(rollout: &Path) -> io::Result<Option<String>> {
    let path = sidecar_path_for(rollout);
    let mut f = match std::fs::File::open(&path) {
        Ok(f) => f,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    let mut buf = String::new();
    f.read_to_string(&mut buf)?;
    let trimmed = buf.trim().to_string();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed))
    }
}

/// Write or remove the sidecar file. When `name` is None or empty, removes the
/// sidecar (best-effort). When set, truncates/creates the file and writes the
/// trimmed name.
pub(crate) fn write(rollout: &Path, name: Option<&str>) -> io::Result<()> {
    let path = sidecar_path_for(rollout);
    let trimmed = name.unwrap_or("").trim();
    if trimmed.is_empty() {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&path)?;
    f.write_all(trimmed.as_bytes())?;
    Ok(())
}
