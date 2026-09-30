//! Resolves rollout metadata with shared plain-before-compressed precedence.

use std::fs::Metadata;
use std::path::Path;
use std::path::PathBuf;

use super::path::compressed_rollout_path;
use super::path::plain_rollout_path;

/// Resolves a regular rollout file and its metadata without leaving the blocking worker.
pub(crate) fn existing_rollout_with_metadata_sync(path: &Path) -> Option<(PathBuf, Metadata)> {
    let plain_path = plain_rollout_path(path);
    let compressed_path = compressed_rollout_path(&plain_path);
    [plain_path, compressed_path].into_iter().find_map(|path| {
        let metadata = std::fs::metadata(&path).ok()?;
        metadata.is_file().then_some((path, metadata))
    })
}
