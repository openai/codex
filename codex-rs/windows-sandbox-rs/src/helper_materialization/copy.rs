//! Copies legacy helpers using destination-inherited ACLs and the existing freshness check.

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CopyOutcome {
    Reused,
    ReCopied,
}

pub(super) fn copy_from_source_if_needed(source: &Path, destination: &Path) -> Result<CopyOutcome> {
    if destination_is_fresh(source, destination)? {
        return Ok(CopyOutcome::Reused);
    }

    let destination_dir = destination.parent().ok_or_else(|| {
        anyhow!(
            "helper destination has no parent: {}",
            destination.display()
        )
    })?;
    fs::create_dir_all(destination_dir).with_context(|| {
        format!(
            "create helper destination directory {}",
            destination_dir.display()
        )
    })?;

    let temp_path = NamedTempFile::new_in(destination_dir)
        .with_context(|| {
            format!(
                "create temporary helper file in {}",
                destination_dir.display()
            )
        })?
        .into_temp_path();
    let temp_path_buf = temp_path.to_path_buf();

    let mut source_file = fs::File::open(source)
        .with_context(|| format!("open helper source for read {}", source.display()))?;
    let mut temp_file = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&temp_path_buf)
        .with_context(|| format!("open temporary helper file {}", temp_path_buf.display()))?;

    // Write into a temp file created inside `.sandbox-bin` so the copied helper keeps the
    // destination directory's inherited ACLs instead of reusing the source file's descriptor.
    std::io::copy(&mut source_file, &mut temp_file).with_context(|| {
        format!(
            "copy helper from {} to {}",
            source.display(),
            temp_path_buf.display()
        )
    })?;
    temp_file
        .flush()
        .with_context(|| format!("flush temporary helper file {}", temp_path_buf.display()))?;
    drop(temp_file);

    if destination.exists() {
        fs::remove_file(destination).with_context(|| {
            format!("remove stale helper destination {}", destination.display())
        })?;
    }

    match fs::rename(&temp_path_buf, destination) {
        Ok(()) => Ok(CopyOutcome::ReCopied),
        Err(rename_err) => {
            if destination_is_fresh(source, destination)? {
                Ok(CopyOutcome::Reused)
            } else {
                Err(rename_err).with_context(|| {
                    format!(
                        "rename helper temp file {} to {}",
                        temp_path_buf.display(),
                        destination.display()
                    )
                })
            }
        }
    }
}

fn destination_is_fresh(source: &Path, destination: &Path) -> Result<bool> {
    let source_meta = fs::metadata(source)
        .with_context(|| format!("read helper source metadata {}", source.display()))?;
    let destination_meta = match fs::metadata(destination) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => {
            return Err(err).with_context(|| {
                format!("read helper destination metadata {}", destination.display())
            });
        }
    };

    if source_meta.len() != destination_meta.len() {
        return Ok(false);
    }

    let source_modified = source_meta
        .modified()
        .with_context(|| format!("read helper source mtime {}", source.display()))?;
    let destination_modified = destination_meta
        .modified()
        .with_context(|| format!("read helper destination mtime {}", destination.display()))?;

    Ok(destination_modified >= source_modified)
}

#[cfg(test)]
#[path = "copy_tests.rs"]
mod tests;
