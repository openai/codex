use codex_common::fuzzy_match::fuzzy_indices as common_fuzzy_indices;
use codex_common::fuzzy_match::fuzzy_match as common_fuzzy_match;
use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;
use serde::Serialize;
use std::cell::UnsafeCell;
use std::collections::BinaryHeap;
use std::num::NonZero;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tokio::process::Command;

mod cli;

pub use cli::Cli;

/// A single match result returned from the search.
///
/// * `score` – Relevance score from the fuzzy matcher (smaller is better).
/// * `path`  – Path to the matched file (relative to the search directory).
/// * `indices` – Optional list of character positions that matched the query.
///   These are unique and sorted so callers can use them directly for highlighting.
#[derive(Debug, Clone, Serialize)]
pub struct FileMatch {
    pub score: i32,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indices: Option<Vec<u32>>, // Sorted & deduplicated when present
}

pub struct FileSearchResults {
    pub matches: Vec<FileMatch>,
    pub total_match_count: usize,
}

pub trait Reporter {
    fn report_match(&self, file_match: &FileMatch);
    fn warn_matches_truncated(&self, total_match_count: usize, shown_match_count: usize);
    fn warn_no_search_pattern(&self, search_directory: &Path);
}

pub async fn run_main<T: Reporter>(
    Cli {
        pattern,
        limit,
        cwd,
        compute_indices,
        json: _,
        exclude,
        threads,
    }: Cli,
    reporter: T,
) -> anyhow::Result<()> {
    let search_directory = match cwd {
        Some(dir) => dir,
        None => std::env::current_dir()?,
    };
    let pattern_text = match pattern {
        Some(pattern) => pattern,
        None => {
            reporter.warn_no_search_pattern(&search_directory);
            #[cfg(unix)]
            Command::new("ls")
                .arg("-al")
                .current_dir(search_directory)
                .stdout(std::process::Stdio::inherit())
                .stderr(std::process::Stdio::inherit())
                .status()
                .await?;
            #[cfg(windows)]
            {
                Command::new("cmd")
                    .arg("/c")
                    .arg(search_directory)
                    .stdout(std::process::Stdio::inherit())
                    .stderr(std::process::Stdio::inherit())
                    .status()
                    .await?;
            }
            return Ok(());
        }
    };

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let FileSearchResults {
        total_match_count,
        matches,
    } = run(
        &pattern_text,
        limit,
        &search_directory,
        exclude,
        threads,
        cancel_flag,
        compute_indices,
    )?;
    let match_count = matches.len();
    let matches_truncated = total_match_count > match_count;

    for file_match in matches {
        reporter.report_match(&file_match);
    }
    if matches_truncated {
        reporter.warn_matches_truncated(total_match_count, match_count);
    }

    Ok(())
}

/// The worker threads will periodically check `cancel_flag` to see if they
/// should stop processing files.
pub fn run(
    pattern_text: &str,
    limit: NonZero<usize>,
    search_directory: &Path,
    exclude: Vec<String>,
    threads: NonZero<usize>,
    cancel_flag: Arc<AtomicBool>,
    compute_indices: bool,
) -> anyhow::Result<FileSearchResults> {
    // Create one BestMatchesList per worker thread so that each worker can
    // operate independently. The results across threads will be merged when
    // the traversal is complete.
    let WorkerCount {
        num_walk_builder_threads,
        num_best_matches_lists,
    } = create_worker_count(threads);
    let best_matchers_per_worker: Vec<UnsafeCell<BestMatchesList>> = (0..num_best_matches_lists)
        .map(|_| UnsafeCell::new(BestMatchesList::new(limit.get(), pattern_text.to_string())))
        .collect();

    // Use the same tree-walker library that ripgrep uses. We use it directly so
    // that we can leverage the parallelism it provides.
    let mut walk_builder = WalkBuilder::new(search_directory);
    walk_builder.threads(num_walk_builder_threads);
    if !exclude.is_empty() {
        let mut override_builder = OverrideBuilder::new(search_directory);
        for exclude in exclude {
            // The `!` prefix is used to indicate an exclude pattern.
            let exclude_pattern = format!("!{exclude}");
            override_builder.add(&exclude_pattern)?;
        }
        let override_matcher = override_builder.build()?;
        walk_builder.overrides(override_matcher);
    }
    let walker = walk_builder.build_parallel();

    // Each worker created by `WalkParallel::run()` will have its own
    // `BestMatchesList` to update.
    let index_counter = AtomicUsize::new(0);
    walker.run(|| {
        let index = index_counter.fetch_add(1, Ordering::Relaxed);
        let best_list_ptr = best_matchers_per_worker[index].get();
        let best_list = unsafe { &mut *best_list_ptr };

        // Each worker keeps a local counter so we only read the atomic flag
        // every N entries which is cheaper than checking on every file.
        const CHECK_INTERVAL: usize = 1024;
        let mut processed = 0;

        let cancel = cancel_flag.clone();

        Box::new(move |entry| {
            if let Some(path) = get_file_path(&entry, search_directory) {
                best_list.insert(path);
            }

            processed += 1;
            if processed % CHECK_INTERVAL == 0 && cancel.load(Ordering::Relaxed) {
                ignore::WalkState::Quit
            } else {
                ignore::WalkState::Continue
            }
        })
    });

    fn get_file_path<'a>(
        entry_result: &'a Result<ignore::DirEntry, ignore::Error>,
        search_directory: &std::path::Path,
    ) -> Option<&'a str> {
        let entry = match entry_result {
            Ok(e) => e,
            Err(_) => return None,
        };
        if entry.file_type().is_some_and(|ft| ft.is_dir()) {
            return None;
        }
        let path = entry.path();
        match path.strip_prefix(search_directory) {
            Ok(rel_path) => rel_path.to_str(),
            Err(_) => None,
        }
    }

    // If the cancel flag is set, we return early with an empty result.
    if cancel_flag.load(Ordering::Relaxed) {
        return Ok(FileSearchResults {
            matches: Vec::new(),
            total_match_count: 0,
        });
    }

    // Merge results across best_matchers_per_worker.
    let mut global_heap: BinaryHeap<(i32, String)> = BinaryHeap::new();
    let mut total_match_count = 0;
    for best_list_cell in best_matchers_per_worker.iter() {
        let best_list = unsafe { &*best_list_cell.get() };
        total_match_count += best_list.num_matches;
        for &(score, ref line) in best_list.binary_heap.iter() {
            if global_heap.len() < limit.get() {
                global_heap.push((score, line.clone()));
            } else if let Some(&(worst_score, _)) = global_heap.peek() {
                if score < worst_score {
                    global_heap.pop();
                    global_heap.push((score, line.clone()));
                }
            }
        }
    }

    let mut raw_matches: Vec<(i32, String)> = global_heap.into_iter().collect();
    sort_matches(&mut raw_matches);

    // Transform into `FileMatch`, optionally computing indices.
    let matches: Vec<FileMatch> = raw_matches
        .into_iter()
        .map(|(score, path)| {
            let indices = if compute_indices {
                common_fuzzy_indices(&path, pattern_text)
                    .map(|v| v.into_iter().map(|i| i as u32).collect())
            } else {
                None
            };

            FileMatch {
                score,
                path,
                indices,
            }
        })
        .collect();

    Ok(FileSearchResults {
        matches,
        total_match_count,
    })
}

/// Sort matches in-place by ascending score, then ascending path.
fn sort_matches(matches: &mut [(i32, String)]) {
    matches.sort_by(|a, b| match a.0.cmp(&b.0) {
        std::cmp::Ordering::Equal => a.1.cmp(&b.1),
        other => other,
    });
}

/// Maintains the `max_count` best matches for a given pattern.
struct BestMatchesList {
    max_count: usize,
    num_matches: usize,
    pattern: String,
    binary_heap: BinaryHeap<(i32, String)>,
}

impl BestMatchesList {
    fn new(max_count: usize, pattern: String) -> Self {
        Self {
            max_count,
            num_matches: 0,
            pattern,
            binary_heap: BinaryHeap::new(),
        }
    }

    fn insert(&mut self, line: &str) {
        if let Some((_indices, score)) = common_fuzzy_match(line, &self.pattern) {
            // Count all matches; non-matches return None above.
            self.num_matches += 1;

            if self.binary_heap.len() < self.max_count {
                self.binary_heap.push((score, line.to_string()));
            } else if let Some(&(worst_score, _)) = self.binary_heap.peek() {
                if score < worst_score {
                    self.binary_heap.pop();
                    self.binary_heap.push((score, line.to_string()));
                }
            }
        }
    }
}

struct WorkerCount {
    num_walk_builder_threads: usize,
    num_best_matches_lists: usize,
}

fn create_worker_count(num_workers: NonZero<usize>) -> WorkerCount {
    // It appears that the number of times the function passed to
    // `WalkParallel::run()` is called is: the number of threads specified to
    // the builder PLUS ONE.
    //
    // In `WalkParallel::visit()`, the builder function gets called once here:
    // https://github.com/BurntSushi/ripgrep/blob/79cbe89deb1151e703f4d91b19af9cdcc128b765/crates/ignore/src/walk.rs#L1233
    //
    // And then once for every worker here:
    // https://github.com/BurntSushi/ripgrep/blob/79cbe89deb1151e703f4d91b19af9cdcc128b765/crates/ignore/src/walk.rs#L1288
    let num_walk_builder_threads = num_workers.get();
    let num_best_matches_lists = num_walk_builder_threads + 1;

    WorkerCount {
        num_walk_builder_threads,
        num_best_matches_lists,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_no_match_does_not_increment_or_push() {
        let mut list = BestMatchesList::new(5, "zzz".to_string());
        list.insert("hello");
        assert_eq!(list.num_matches, 0);
        assert_eq!(list.binary_heap.len(), 0);
    }

    #[test]
    fn tie_breakers_sort_by_path_when_scores_equal() {
        let mut matches = vec![
            (100, "b_path".to_string()),
            (100, "a_path".to_string()),
            (90, "zzz".to_string()),
        ];

        sort_matches(&mut matches);

        // Lowest score first; ties broken alphabetically.
        let expected = vec![
            (90, "zzz".to_string()),
            (100, "a_path".to_string()),
            (100, "b_path".to_string()),
        ];

        assert_eq!(matches, expected);
    }
}
