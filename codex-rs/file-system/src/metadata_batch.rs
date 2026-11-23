//! Bounded batches of independent metadata requests.
use crate::ExecutorFileSystem;
use crate::FileMetadata;
use crate::FileSystemSandboxContext;
use crate::GetMetadataOptions;
use codex_utils_path_uri::PathUri;
use futures::StreamExt;
use std::io;

const MAX_CONCURRENT_PROBES: usize = 256;
pub const MAX_METADATA_BATCH_PROBES: usize = 1024;
pub const MAX_METADATA_BATCH_BYTES: usize = 1024 * 1024;

/// Bound both the number of paths and encoded request bytes.
pub fn metadata_batch_fits(paths: &[PathUri]) -> bool {
    paths.len() <= MAX_METADATA_BATCH_PROBES
        && paths
            .iter()
            .try_fold(0usize, |bytes, path| {
                bytes
                    .checked_add(path.to_string().len())
                    .filter(|total| *total <= MAX_METADATA_BATCH_BYTES)
            })
            .is_some()
}

/// Fall back to individual calls while preserving each result and request order.
pub async fn resolve_metadata_batch(
    fs: &(impl ExecutorFileSystem + ?Sized),
    paths: &[PathUri],
    options: GetMetadataOptions,
    sandbox: Option<&FileSystemSandboxContext>,
) -> io::Result<Vec<io::Result<FileMetadata>>> {
    Ok(futures::stream::iter(0..paths.len())
        .map(|index: usize| async move { fs.get_metadata(&paths[index], options, sandbox).await })
        .buffered(MAX_CONCURRENT_PROBES)
        .collect()
        .await)
}
