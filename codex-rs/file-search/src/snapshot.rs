//! Converts matcher snapshots without associating old results with a new query.

use crate::FileMatch;
use crate::FileSearchSnapshot;
use crate::IndexedEntry;
use crate::SessionInner;
use crate::get_file_path;
use nucleo::Matcher;
use nucleo::Nucleo;
use std::path::Path;
use std::path::PathBuf;

pub(super) fn for_query(
    inner: &SessionInner,
    nucleo: &Nucleo<IndexedEntry>,
    query: &str,
    indices_matcher: &mut Option<Matcher>,
    walk_complete: bool,
) -> Option<FileSearchSnapshot> {
    let snapshot = nucleo.snapshot();
    let pattern = snapshot.pattern().column_pattern(0);
    // A tick can collect the previous query's results while the new query is
    // still running. Wait for a matching snapshot before using the new label.
    if pattern.atoms != nucleo.pattern.column_pattern(0).atoms {
        return None;
    }
    let limit = inner.limit.min(snapshot.matched_item_count() as usize);
    let matches = snapshot
        .matches()
        .iter()
        .take(limit)
        .filter_map(|match_| {
            let item = snapshot.get_item(match_.idx)?;
            let full_path = item.data.full_path.as_ref();
            let (root_idx, relative_path) =
                get_file_path(Path::new(full_path), &inner.search_directories)?;
            let indices = if let Some(indices_matcher) = indices_matcher.as_mut() {
                let mut indices = Vec::<u32>::new();
                let haystack = item.matcher_columns[0].slice(..);
                let _ = pattern.indices(haystack, indices_matcher, &mut indices);
                indices.sort_unstable();
                indices.dedup();
                Some(indices)
            } else {
                None
            };
            Some(FileMatch {
                score: match_.score,
                path: PathBuf::from(relative_path),
                match_type: item.data.match_type,
                root: inner.search_directories[root_idx].clone(),
                indices,
            })
        })
        .collect();
    Some(FileSearchSnapshot {
        query: query.to_string(),
        matches,
        total_match_count: snapshot.matched_item_count() as usize,
        scanned_file_count: snapshot.item_count() as usize,
        walk_complete,
    })
}
