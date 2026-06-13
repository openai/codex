use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::protocol::RolloutItem;
use codex_rollout::ARCHIVED_SESSIONS_SUBDIR;
use codex_rollout::ROTATED_ROLLOUT_SEGMENTS_SUBDIR;
use codex_rollout::RolloutRecorder;
use codex_rollout::SESSIONS_SUBDIR;
use tokio::fs;
use tracing::warn;

/// Remove immutable segments that are unreachable from every canonical active or archived
/// rollout. Forks may reference another thread's segment, so segment lifetime is determined by
/// reachability rather than by the thread id encoded in the storage path.
pub(super) async fn collect_unreferenced_segments(codex_home: &Path) -> io::Result<()> {
    let segment_root = codex_home.join(ROTATED_ROLLOUT_SEGMENTS_SUBDIR);
    if !fs::try_exists(segment_root.as_path()).await? {
        return Ok(());
    }
    let canonical_segment_root = fs::canonicalize(segment_root.as_path()).await?;
    let mut pending = Vec::new();
    for root in [SESSIONS_SUBDIR, ARCHIVED_SESSIONS_SUBDIR] {
        pending.extend(rollout_files_under(codex_home.join(root).as_path())?);
    }

    let mut visited = HashSet::new();
    let mut reachable_segments = HashSet::new();
    while let Some(path) = pending.pop() {
        let canonical_path = fs::canonicalize(path.as_path()).await?;
        if !visited.insert(canonical_path) {
            continue;
        }
        let (items, _, _) = RolloutRecorder::load_rollout_items(path.as_path()).await?;
        for reference in items.iter().filter_map(|item| match item {
            RolloutItem::RolloutReference(reference) => Some(reference),
            RolloutItem::Compacted(_)
            | RolloutItem::EventMsg(_)
            | RolloutItem::InterAgentCommunication(_)
            | RolloutItem::ResponseItem(_)
            | RolloutItem::SessionMeta(_)
            | RolloutItem::TurnContext(_) => None,
        }) {
            let referenced_path =
                codex_rollout::resolve_rollout_reference_rollout_path(codex_home, reference)
                    .await?;
            let canonical_referenced_path = fs::canonicalize(referenced_path.as_path()).await?;
            if canonical_referenced_path.starts_with(canonical_segment_root.as_path())
                && reachable_segments.insert(canonical_referenced_path)
            {
                pending.push(referenced_path);
            }
        }
    }

    let segment_files = rollout_files_under(segment_root.as_path())?;
    for segment_file in segment_files {
        let canonical_path = fs::canonicalize(segment_file.as_path()).await?;
        if !reachable_segments.contains(&canonical_path) {
            fs::remove_file(segment_file.as_path()).await?;
        }
    }
    remove_empty_directories(segment_root.as_path());
    Ok(())
}

fn rollout_files_under(root: &Path) -> io::Result<Vec<PathBuf>> {
    if !root.try_exists()? {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            if file_type.is_dir() {
                pending.push(path);
            } else if file_type.is_file() && is_rollout_file(path.as_path()) {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn is_rollout_file(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    file_name.starts_with("rollout-")
        && (file_name.ends_with(".jsonl") || file_name.ends_with(".jsonl.zst"))
}

fn remove_empty_directories(root: &Path) {
    let mut directories = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let entries = match std::fs::read_dir(directory.as_path()) {
            Ok(entries) => entries,
            Err(err) => {
                warn!(
                    "failed to inspect rollout segment directory {} during cleanup: {err}",
                    directory.display()
                );
                continue;
            }
        };
        directories.push(directory);
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|file_type| file_type.is_dir()) {
                pending.push(entry.path());
            }
        }
    }
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for directory in directories {
        if let Err(err) = std::fs::remove_dir(directory.as_path())
            && !matches!(
                err.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::DirectoryNotEmpty
            )
        {
            warn!(
                "failed to remove empty rollout segment directory {}: {err}",
                directory.display()
            );
        }
    }
}
