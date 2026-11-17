//! Compressed rollout discovery and best-effort candidate delivery.

use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use futures::future::join_all;
use tokio::sync::mpsc;

use super::RolloutSearchCandidate;
use super::RolloutSearchMatches;
use super::RolloutSnippetMatcher;
use super::compression;
use super::report_candidate;

pub(super) async fn scan_compressed_rollout_matches(
    root: &Path,
    search_term: &str,
    progress: Option<mpsc::Sender<RolloutSearchCandidate>>,
) -> io::Result<RolloutSearchMatches> {
    let mut matches = HashSet::new();
    let mut paths = Vec::new();
    let mut dirs = vec![root.to_path_buf()];

    while let Some(dir) = dirs.pop() {
        let mut entries = match tokio::fs::read_dir(dir).await {
            Ok(entries) => entries,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        };
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let file_type = entry.file_type().await?;
            if file_type.is_dir() {
                dirs.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let Some(rollout_file) = compression::RolloutFile::from_path(path) else {
                continue;
            };
            if !rollout_file.is_compressed() {
                continue;
            }
            paths.push(rollout_file.into_path());
        }
    }

    if paths.is_empty() {
        return Ok(matches);
    }
    const COMPRESSED_SEARCH_WORKERS: usize = 4;
    let scan = async |paths: &[PathBuf]| {
        let mut matches = HashSet::new();
        let mut progress = progress.clone();
        let mut matcher = RolloutSnippetMatcher::new(search_term);
        for path in paths {
            match matcher.first_match(path).await {
                Ok(Some(_)) => {
                    let logical_path = compression::plain_rollout_path(path);
                    report_candidate(
                        &mut progress,
                        RolloutSearchCandidate {
                            path: logical_path.clone(),
                        },
                    );
                    matches.insert(logical_path);
                }
                Ok(None) => {}
                Err(err) => {
                    tracing::warn!(path = %path.display(), %err, "Failed to search compressed rollout")
                }
            }
        }
        matches
    };
    let chunk_size = paths.len().div_ceil(COMPRESSED_SEARCH_WORKERS).max(1);
    for batch in join_all(paths.chunks(chunk_size).map(scan)).await {
        matches.extend(batch);
    }
    Ok(matches)
}
