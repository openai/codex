use crate::MAX_LIST_RESULTS;
use crate::backend::ListMemoriesRequest;
use crate::backend::ListMemoriesResponse;
use crate::backend::MemoriesBackendError;
use crate::backend::MemoryEntry;
use crate::backend::MemoryEntryType;

use super::LocalMemoriesBackend;
use super::path::display_relative_path;
use super::path::is_hidden_path;
use super::path::read_sorted_dir_paths;
use super::path::reject_symlink;

pub(super) async fn list(
    backend: &LocalMemoriesBackend,
    request: ListMemoriesRequest,
) -> Result<ListMemoriesResponse, MemoriesBackendError> {
    let max_results = request.max_results.min(MAX_LIST_RESULTS);
    let start = backend.resolve_scoped_path(request.path.as_deref()).await?;
    let start_index = match request.cursor.as_deref() {
        Some(cursor) => cursor.parse::<usize>().map_err(|_| {
            MemoriesBackendError::invalid_cursor(cursor, "must be a non-negative integer")
        })?,
        None => 0,
    };
    let Some(metadata) = LocalMemoriesBackend::metadata_or_none(&start).await? else {
        return Err(MemoriesBackendError::NotFound {
            path: request.path.unwrap_or_default(),
        });
    };
    reject_symlink(&display_relative_path(&backend.root, &start), &metadata)?;

    let mut entries = if metadata.is_file() {
        vec![MemoryEntry {
            path: display_relative_path(&backend.root, &start),
            entry_type: MemoryEntryType::File,
        }]
    } else if metadata.is_dir() {
        let mut entries = Vec::new();
        for path in read_sorted_dir_paths(&start).await? {
            if is_hidden_path(&path) {
                continue;
            }
            let Some(metadata) = LocalMemoriesBackend::metadata_or_none(&path).await? else {
                continue;
            };
            if metadata.file_type().is_symlink() {
                continue;
            }

            let entry_type = if metadata.is_dir() {
                MemoryEntryType::Directory
            } else if metadata.is_file() {
                MemoryEntryType::File
            } else {
                continue;
            };
            entries.push(MemoryEntry {
                path: display_relative_path(&backend.root, &path),
                entry_type,
            });
        }
        entries
    } else {
        Vec::new()
    };
    if start_index > entries.len() {
        return Err(MemoriesBackendError::invalid_cursor(
            start_index.to_string(),
            "exceeds result count",
        ));
    }

    let end_index = start_index.saturating_add(max_results).min(entries.len());
    let next_cursor = (end_index < entries.len()).then(|| end_index.to_string());
    let truncated = next_cursor.is_some();
    Ok(ListMemoriesResponse {
        path: request.path,
        entries: entries.drain(start_index..end_index).collect(),
        next_cursor,
        truncated,
    })
}
