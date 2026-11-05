//! Ranks a stable matcher snapshot in slices so the worker can handle new queries.

use crate::FileMatch;
use crate::FileSearchSnapshot;
use crate::IndexedEntry;
use crate::MatchType;
use crate::SessionInner;
use nucleo::Match;
use nucleo::Matcher;
use nucleo::Nucleo;
use nucleo::Utf32Str;
use nucleo::Utf32String;
use nucleo::pattern::AtomKind;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

// A single Nucleo atom returns a u16. Leave room for its full score within each group.
const NAME_BONUS: u32 = u16::MAX as u32 + 1;

#[derive(Default)]
pub(super) struct Ranking {
    next: usize,
    best: BinaryHeap<(Reverse<u32>, usize)>,
    found_exact_folder: bool,
    rank_names: bool,
    walk_complete: bool,
}

impl Ranking {
    pub(super) fn start(
        nucleo: &Nucleo<IndexedEntry>,
        query: &str,
        walk_complete: bool,
    ) -> Option<Self> {
        let pattern = nucleo.snapshot().pattern().column_pattern(0);
        // A tick can collect the previous query while the new one is running.
        if pattern.atoms != nucleo.pattern.column_pattern(0).atoms {
            return None;
        }
        let rank_names = matches!(pattern.atoms.as_slice(), [atom] if atom.kind == AtomKind::Fuzzy && !atom.negative)
            && !query.contains(['/', '\\']);
        Some(Self {
            rank_names,
            walk_complete,
            ..Self::default()
        })
    }

    /// Yields to the worker between slices so a new query or cancellation can preempt this snapshot.
    pub(super) fn advance(
        &mut self,
        inner: &SessionInner,
        nucleo: &Nucleo<IndexedEntry>,
        query: &str,
        matcher: &mut Matcher,
        budget: Duration,
    ) -> Option<FileSearchSnapshot> {
        let snapshot = nucleo.snapshot();
        let ranked = snapshot.matches();
        if !self.rank_names {
            return Some(collect(
                inner,
                nucleo,
                query,
                matcher,
                self.walk_complete,
                ranked
                    .iter()
                    .copied()
                    .take(inner.limit)
                    .map(|matched| (matched, false)),
            ));
        }
        let name = &snapshot.pattern().column_pattern(0).atoms[0];
        let deadline = Instant::now() + budget;
        for (matched, item) in ranked[self.next..]
            .iter()
            .zip(snapshot.matched_items(self.next as u32..))
        {
            let kind = match item.data {
                IndexedEntry::Ascii { match_type, .. } => *match_type,
                IndexedEntry::Unicode(entry) => entry.match_type,
            };
            let basename = basename(item.matcher_columns[0].slice(..));
            let exact_length = basename.len() == name.needle_text().len();
            let score = match kind {
                MatchType::File => name.score(basename, matcher).map(|score| {
                    let bonus = if exact_length {
                        3 * NAME_BONUS
                    } else {
                        NAME_BONUS
                    };
                    bonus + u32::from(score)
                }),
                // Keep the strongest exact folder. More folders with the same name
                // compete in path order so they cannot crowd out matching files.
                MatchType::Directory
                    if !self.found_exact_folder
                        && exact_length
                        && name.score(basename, matcher).is_some() =>
                {
                    self.found_exact_folder = true;
                    Some(2 * NAME_BONUS + matched.score)
                }
                MatchType::Directory => None,
            }
            .unwrap_or(matched.score);
            // The heap head is the lowest score, then the latest path on a tie.
            let candidate = (Reverse(score), self.next);
            if self.best.len() < inner.limit {
                self.best.push(candidate);
            } else if let Some(mut worst) = self.best.peek_mut()
                && candidate < *worst
            {
                *worst = candidate;
            }
            self.next += 1;
            if self.next.is_multiple_of(64) && Instant::now() >= deadline {
                return None;
            }
        }
        let selected = std::mem::take(&mut self.best)
            .into_sorted_vec()
            .into_iter()
            .map(|(Reverse(score), index)| {
                (
                    Match {
                        score,
                        idx: ranked[index].idx,
                    },
                    score >= NAME_BONUS,
                )
            });
        Some(collect(
            inner,
            nucleo,
            query,
            matcher,
            self.walk_complete,
            selected,
        ))
    }
}

fn collect(
    inner: &SessionInner,
    nucleo: &Nucleo<IndexedEntry>,
    query: &str,
    matcher: &mut Matcher,
    walk_complete: bool,
    candidates: impl Iterator<Item = (Match, bool)>,
) -> FileSearchSnapshot {
    let snapshot = nucleo.snapshot();
    let pattern = snapshot.pattern().column_pattern(0);
    let matches = candidates
        .filter_map(|(match_, matched_name)| {
            let item = snapshot.get_item(match_.idx)?;
            let (root_idx, relative_path, match_type) = match item.data {
                IndexedEntry::Ascii {
                    root_idx,
                    match_type,
                } => {
                    let Utf32String::Ascii(path) = &item.matcher_columns[0] else {
                        return None;
                    };
                    (*root_idx, path.as_ref(), *match_type)
                }
                IndexedEntry::Unicode(entry) => (
                    entry.root_idx,
                    entry.relative_path.as_ref(),
                    entry.match_type,
                ),
            };
            let indices = if inner.compute_indices {
                let mut indices = Vec::<u32>::new();
                let path = item.matcher_columns[0].slice(..);
                let haystack = if matched_name { basename(path) } else { path };
                let _ = pattern.indices(haystack, matcher, &mut indices);
                for index in &mut indices {
                    *index += (path.len() - haystack.len()) as u32;
                }
                indices.sort_unstable();
                indices.dedup();
                Some(indices)
            } else {
                None
            };
            Some(FileMatch {
                score: match_.score,
                path: PathBuf::from(relative_path),
                match_type,
                root: inner.search_directories[root_idx].clone(),
                indices,
            })
        })
        .collect();
    FileSearchSnapshot {
        query: query.to_string(),
        matches,
        total_match_count: snapshot.matched_item_count() as usize,
        scanned_file_count: snapshot.item_count() as usize,
        walk_complete,
    }
}

fn basename(path: Utf32Str<'_>) -> Utf32Str<'_> {
    let len = path
        .chars()
        .rev()
        .take_while(|c| !std::path::is_separator(*c))
        .count();
    path.slice(path.len() - len..)
}
