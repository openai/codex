use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;
use nucleo::Config;
use nucleo::Nucleo;
use nucleo::Snapshot;
use nucleo::Status;
use nucleo_matcher::Matcher;
use nucleo_matcher::pattern::CaseMatching;
use nucleo_matcher::pattern::Normalization;
use std::num::NonZero;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread::JoinHandle;
use std::thread::{self};
use std::time::Duration;

use crate::FileMatch;
use crate::FileSearchResults;

#[derive(Debug, Clone)]
pub struct SearchItem {
    pub path: String,
}

impl SearchItem {
    fn new(path: String) -> Self {
        Self { path }
    }
}


pub struct SearchManager {
    nucleo: Nucleo<SearchItem>,
    cancel_flag: Arc<AtomicBool>,
    walk_handle: Option<JoinHandle<()>>,
    limit: NonZero<usize>,
    compute_indices: bool,
    matcher: Mutex<Matcher>,
    search_directory: PathBuf,
    case_matching: CaseMatching,
    normalization: Normalization,
    current_pattern: String,
}

impl SearchManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pattern: &str,
        limit: NonZero<usize>,
        search_directory: &Path,
        exclude: Vec<String>,
        threads: NonZero<usize>,
        compute_indices: bool,
        notify: Arc<dyn Fn() + Sync + Send>,
    ) -> anyhow::Result<Self> {
        let search_directory_buf = search_directory.to_path_buf();
        let override_matcher = build_override_matcher(search_directory, exclude)?;
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let mut nucleo = Nucleo::new(
            Config::DEFAULT,
            notify,
            Some(threads.get()),
            1, // Single column containing the relative file path.
        );
        nucleo
            .pattern
            .reparse(0, pattern, CaseMatching::Smart, Normalization::Smart, false);
        let injector = nucleo.injector();
        let walk_handle = Some(spawn_walker(
            search_directory_buf.clone(),
            threads.get(),
            override_matcher,
            cancel_flag.clone(),
            injector,
        )?);

        Ok(Self {
            nucleo,
            cancel_flag,
            walk_handle,
            limit,
            compute_indices,
            matcher: Mutex::new(Matcher::new(nucleo_matcher::Config::DEFAULT)),
            search_directory: search_directory_buf,
            case_matching: CaseMatching::Smart,
            normalization: Normalization::Smart,
            current_pattern: pattern.to_string(),
        })
    }

    pub fn update_pattern(&mut self, pattern: &str) {
        let append = pattern.starts_with(&self.current_pattern);
        self.nucleo
            .pattern
            .reparse(0, pattern, self.case_matching, self.normalization, append);
        self.current_pattern.clear();
        self.current_pattern.push_str(pattern);
    }

    pub fn tick(&mut self, timeout: Duration) -> Status {
        let millis = timeout.as_millis();
        let timeout_ms = millis.try_into().unwrap_or(u64::MAX);
        self.nucleo.tick(timeout_ms)
    }

    pub fn injector(&self) -> nucleo::Injector<SearchItem> {
        self.nucleo.injector()
    }

    pub fn snapshot(&self) -> &Snapshot<SearchItem> {
        self.nucleo.snapshot()
    }

    pub fn current_results(&self) -> FileSearchResults {
        let snapshot = self.nucleo.snapshot();
        let matched = snapshot.matched_item_count();
        let max_results = u32::try_from(self.limit.get()).unwrap_or(u32::MAX);
        let take = std::cmp::min(max_results, matched);
        let mut matcher = self.matcher.lock().expect("matcher mutex poisoned");
        let pattern = snapshot.pattern().column_pattern(0);
        let pattern_empty = pattern.atoms.is_empty();
        let compute_indices = self.compute_indices;

        let matches = snapshot
            .matched_items(0..take)
            .filter_map(|item| {
                let haystack = item.matcher_columns[0].slice(..);
                if pattern_empty {
                    Some(FileMatch {
                        score: 0,
                        path: item.data.path.clone(),
                        indices: None,
                    })
                } else if compute_indices {
                    let mut indices = Vec::new();
                    let score = pattern.indices(haystack, &mut matcher, &mut indices)?;
                    indices.sort_unstable();
                    indices.dedup();
                    Some(FileMatch {
                        score,
                        path: item.data.path.clone(),
                        indices: Some(indices),
                    })
                } else {
                    let score = pattern.score(haystack, &mut matcher)?;
                    Some(FileMatch {
                        score,
                        path: item.data.path.clone(),
                        indices: None,
                    })
                }
            })
            .collect();

        FileSearchResults {
            matches,
            total_match_count: matched as usize,
        }
    }

    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }

    pub fn search_directory(&self) -> &Path {
        &self.search_directory
    }
}

impl Drop for SearchManager {
    fn drop(&mut self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
        if let Some(handle) = self.walk_handle.take() {
            let _ = handle.join();
        }
    }
}

fn spawn_walker(
    search_directory: PathBuf,
    threads: usize,
    override_matcher: Option<ignore::overrides::Override>,
    cancel_flag: Arc<AtomicBool>,
    injector: nucleo::Injector<SearchItem>,
) -> anyhow::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("codex-file-search-walker".to_string())
        .spawn(move || {
            let search_directory = Arc::new(search_directory);
            let mut walk_builder = WalkBuilder::new(search_directory.as_path());
            walk_builder
                .threads(threads)
                .hidden(false)
                .require_git(false);

            if let Some(override_matcher) = override_matcher {
                walk_builder.overrides(override_matcher);
            }

            let walker = walk_builder.build_parallel();
            walker.run(|| {
                let injector = injector.clone();
                let cancel_flag = cancel_flag.clone();
                let search_directory = Arc::clone(&search_directory);
                Box::new(move |entry| {
                    if cancel_flag.load(Ordering::Relaxed) {
                        return ignore::WalkState::Quit;
                    }
                    let entry = match entry {
                        Ok(entry) => entry,
                        Err(_) => return ignore::WalkState::Continue,
                    };
                    if entry.file_type().is_some_and(|ft| ft.is_dir()) {
                        return ignore::WalkState::Continue;
                    }
                    let path = entry.path();
                    let rel_path = match path.strip_prefix(search_directory.as_path()) {
                        Ok(rel) => rel,
                        Err(_) => path,
                    };
                    let Some(path_str) = rel_path.to_str() else {
                        return ignore::WalkState::Continue;
                    };
                    injector.push(SearchItem::new(path_str.to_string()), |item, columns| {
                        columns[0] = item.path.as_str().into();
                    });
                    ignore::WalkState::Continue
                })
            });
        })
        .map_err(anyhow::Error::new)
}

fn build_override_matcher(
    search_directory: &Path,
    exclude: Vec<String>,
) -> anyhow::Result<Option<ignore::overrides::Override>> {
    if exclude.is_empty() {
        return Ok(None);
    }

    let mut builder = OverrideBuilder::new(search_directory);
    for pattern in exclude {
        let exclude_pattern = format!("!{pattern}");
        builder.add(&exclude_pattern)?;
    }
    Ok(Some(builder.build()?))
}