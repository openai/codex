use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;
use nucleo_matcher::Matcher;
use nucleo_matcher::Utf32Str;
use nucleo_matcher::pattern::AtomKind;
use nucleo_matcher::pattern::CaseMatching;
use nucleo_matcher::pattern::Normalization;
use nucleo_matcher::pattern::Pattern;
use std::cell::UnsafeCell;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::num::NonZero;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tokio::process::Command;

mod cli;

pub use cli::Cli;

pub struct FileSearchResults {
    pub matches: Vec<(u32, String)>,
    pub total_match_count: usize,
}

pub trait Reporter {
    fn report_match(&self, file: &str, score: u32);
    fn warn_matches_truncated(&self, total_match_count: usize, shown_match_count: usize);
    fn warn_no_search_pattern(&self, search_directory: &Path);
}

pub async fn run_main<T: Reporter>(
    Cli {
        pattern,
        limit,
        cwd,
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

    let FileSearchResults {
        total_match_count,
        matches,
    } = run(&pattern_text, limit, search_directory, exclude, threads).await?;
    let match_count = matches.len();
    let matches_truncated = total_match_count > match_count;

    for (score, file) in matches {
        reporter.report_match(&file, score);
    }
    if matches_truncated {
        reporter.warn_matches_truncated(total_match_count, match_count);
    }

    Ok(())
}

pub async fn run(
    pattern_text: &str,
    limit: NonZero<usize>,
    search_directory: PathBuf,
    exclude: Vec<String>,
    threads: NonZero<usize>,
) -> anyhow::Result<FileSearchResults> {
    let pattern = create_pattern(pattern_text);
    // Create one BestMatchesList per worker thread so that each worker can
    // operate independently. The results across threads will be merged when
    // the traversal is complete.
    let WorkerCount {
        num_walk_builder_threads,
        num_best_matches_lists,
    } = create_worker_count(threads);
    let best_matchers_per_worker: Vec<UnsafeCell<BestMatchesList>> = (0..num_best_matches_lists)
        .map(|_| {
            UnsafeCell::new(BestMatchesList::new(
                limit.get(),
                pattern.clone(),
                Matcher::new(nucleo_matcher::Config::DEFAULT),
            ))
        })
        .collect();

    // Use the same tree-walker library that ripgrep uses. We use it directly so
    // that we can leverage the parallelism it provides.
    let mut walk_builder = WalkBuilder::new(&search_directory);
    walk_builder.threads(num_walk_builder_threads);
    if !exclude.is_empty() {
        let mut override_builder = OverrideBuilder::new(&search_directory);
        for exclude in exclude {
            // The `!` prefix is used to indicate an exclude pattern.
            let exclude_pattern = format!("!{}", exclude);
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
        let search_directory = search_directory.clone();
        let index = index_counter.fetch_add(1, Ordering::Relaxed);
        let best_list_ptr = best_matchers_per_worker[index].get();
        let best_list = unsafe { &mut *best_list_ptr };
        Box::new(move |entry| {
            if let Some(path) = get_file_path(&entry, &search_directory) {
                best_list.insert(path);
            }
            ignore::WalkState::Continue
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

    // Merge results across best_matchers_per_worker.
    let mut global_heap: BinaryHeap<Reverse<(u32, String)>> = BinaryHeap::new();
    let mut total_match_count = 0;
    for best_list_cell in best_matchers_per_worker.iter() {
        let best_list = unsafe { &*best_list_cell.get() };
        total_match_count += best_list.num_matches;
        for &Reverse((score, ref line)) in best_list.binary_heap.iter() {
            if global_heap.len() < limit.get() {
                global_heap.push(Reverse((score, line.clone())));
            } else if let Some(min_element) = global_heap.peek() {
                if score > min_element.0.0 {
                    global_heap.pop();
                    global_heap.push(Reverse((score, line.clone())));
                }
            }
        }
    }

    let mut matches: Vec<(u32, String)> = global_heap.into_iter().map(|r| r.0).collect();
    matches.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    Ok(FileSearchResults {
        matches,
        total_match_count,
    })
}

/// Maintains the `max_count` best matches for a given pattern.
struct BestMatchesList {
    max_count: usize,
    num_matches: usize,
    pattern: Pattern,
    matcher: Matcher,
    binary_heap: BinaryHeap<Reverse<(u32, String)>>,

    /// Internal buffer for converting strings to UTF-32.
    utf32buf: Vec<char>,
}

impl BestMatchesList {
    fn new(max_count: usize, pattern: Pattern, matcher: Matcher) -> Self {
        Self {
            max_count,
            num_matches: 0,
            pattern,
            matcher,
            binary_heap: BinaryHeap::new(),
            utf32buf: Vec::<char>::new(),
        }
    }

    fn insert(&mut self, line: &str) {
        let haystack: Utf32Str<'_> = Utf32Str::new(line, &mut self.utf32buf);
        if let Some(score) = self.pattern.score(haystack, &mut self.matcher) {
            // In the tests below, we verify that score() returns None for a
            // non-match, so we can categorically increment the count here.
            self.num_matches += 1;

            if self.binary_heap.len() < self.max_count {
                self.binary_heap.push(Reverse((score, line.to_string())));
            } else if let Some(min_element) = self.binary_heap.peek() {
                if score > min_element.0.0 {
                    self.binary_heap.pop();
                    self.binary_heap.push(Reverse((score, line.to_string())));
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

fn create_pattern(pattern: &str) -> Pattern {
    Pattern::new(
        pattern,
        CaseMatching::Smart,
        Normalization::Smart,
        AtomKind::Fuzzy,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_score_is_none_for_non_match() {
        let mut utf32buf = Vec::<char>::new();
        let line = "hello";
        let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
        let haystack: Utf32Str<'_> = Utf32Str::new(line, &mut utf32buf);
        let pattern = create_pattern("zzz");
        let score = pattern.score(haystack, &mut matcher);
        assert_eq!(score, None);
    }
}
