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
    walker_running: Arc<AtomicBool>,
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
        let walker_running = Arc::new(AtomicBool::new(true));
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
            walker_running.clone(),
            injector,
        )?);

        Ok(Self {
            nucleo,
            cancel_flag,
            walker_running,
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

    pub fn walker_running(&self) -> bool {
        self.walker_running.load(Ordering::Relaxed)
    }

    pub fn search_directory(&self) -> &Path {
        &self.search_directory
    }
}

struct WalkerRunningGuard {
    flag: Arc<AtomicBool>,
}

impl WalkerRunningGuard {
    fn new(flag: Arc<AtomicBool>) -> Self {
        flag.store(true, Ordering::Relaxed);
        Self { flag }
    }
}

impl Drop for WalkerRunningGuard {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::Relaxed);
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

#[derive(Clone, Copy, Debug)]
pub struct DebounceConfig {
    /// How long to wait after a pattern change before starting a new search.
    pub debounce_delay: Duration,
    /// How often to poll for the previous search to finish before starting a new one.
    pub active_search_complete_poll_interval: Duration,
    /// How long each `tick` call should wait for new results.
    pub tick_timeout: Duration,
    /// Maximum time to wait for the first result before emitting a fallback update.
    pub first_result_timeout: Duration,
}

impl Default for DebounceConfig {
    fn default() -> Self {
        Self {
            debounce_delay: Duration::from_millis(100),
            active_search_complete_poll_interval: Duration::from_millis(20),
            tick_timeout: Duration::from_millis(16),
            first_result_timeout: Duration::from_millis(200),
        }
    }
}

struct DebouncedSearchState {
    latest_query: String,
    is_search_scheduled: bool,
    active_search: Option<ActiveDebouncedSearch>,
}

struct ActiveDebouncedSearch {
    query: String,
    cancellation_token: Arc<AtomicBool>,
}

struct ActiveDebouncedSearchGuard {
    state: Arc<Mutex<DebouncedSearchState>>,
    token: Arc<AtomicBool>,
}

impl ActiveDebouncedSearchGuard {
    fn new(state: Arc<Mutex<DebouncedSearchState>>, token: Arc<AtomicBool>) -> Self {
        Self { state, token }
    }
}

impl Drop for ActiveDebouncedSearchGuard {
    fn drop(&mut self) {
        #[expect(clippy::unwrap_used)]
        let mut state = self.state.lock().unwrap();
        if let Some(active_search) = &state.active_search
            && Arc::ptr_eq(&active_search.cancellation_token, &self.token)
        {
            state.active_search = None;
        }
    }
}

/// Debounced wrapper over [`SearchManager`] suitable for UI-style incremental search.
///
/// This helper owns the debounce/cancellation logic for a stream of pattern
/// updates. Call [`DebouncedSearchManager::on_query`] for each new pattern; it
/// will start searches after a debounce delay and emit updates via the
/// provided callback whenever results change or progress is made.
pub struct DebouncedSearchManager<C>
where
    C: Fn(String, FileSearchResults, bool) + Send + Sync + 'static,
{
    state: Arc<Mutex<DebouncedSearchState>>,
    search_dir: PathBuf,
    limit: NonZero<usize>,
    threads: NonZero<usize>,
    compute_indices: bool,
    exclude: Vec<String>,
    callback: Arc<C>,
    config: DebounceConfig,
}

impl<C> DebouncedSearchManager<C>
where
    C: Fn(String, FileSearchResults, bool) + Send + Sync + 'static,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        search_dir: PathBuf,
        limit: NonZero<usize>,
        threads: NonZero<usize>,
        compute_indices: bool,
        exclude: Vec<String>,
        callback: Arc<C>,
        config: DebounceConfig,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(DebouncedSearchState {
                latest_query: String::new(),
                is_search_scheduled: false,
                active_search: None,
            })),
            search_dir,
            limit,
            threads,
            compute_indices,
            exclude,
            callback,
            config,
        }
    }

    /// Call whenever the search pattern is updated.
    ///
    /// This method is cheap to call for each keystroke; a background worker
    /// will apply debouncing and cancellation.
    pub fn on_query(&self, query: String) {
        {
            #[expect(clippy::unwrap_used)]
            let mut state = self.state.lock().unwrap();
            if query == state.latest_query {
                return;
            }

            state.latest_query.clear();
            state.latest_query.push_str(&query);

            if let Some(active_search) = &state.active_search
                && !query.starts_with(&active_search.query)
            {
                active_search
                    .cancellation_token
                    .store(true, Ordering::Relaxed);
            }

            if state.is_search_scheduled {
                return;
            }

            state.is_search_scheduled = true;
        }

        let state = Arc::clone(&self.state);
        let search_dir = self.search_dir.clone();
        let limit = self.limit;
        let threads = self.threads;
        let compute_indices = self.compute_indices;
        let exclude = self.exclude.clone();
        let callback = Arc::clone(&self.callback);
        let config = self.config;

        thread::spawn(move || {
            // Always do a minimum debounce, but then poll until the active
            // search is cleared.
            thread::sleep(config.debounce_delay);
            loop {
                #[expect(clippy::unwrap_used)]
                if state.lock().unwrap().active_search.is_none() {
                    break;
                }
                thread::sleep(config.active_search_complete_poll_interval);
            }

            let cancellation_token = Arc::new(AtomicBool::new(false));
            let token = Arc::clone(&cancellation_token);
            let query = {
                #[expect(clippy::unwrap_used)]
                let mut locked_state = state.lock().unwrap();
                let query = locked_state.latest_query.clone();
                locked_state.is_search_scheduled = false;
                locked_state.active_search = Some(ActiveDebouncedSearch {
                    query: query.clone(),
                    cancellation_token: token,
                });
                query
            };

            DebouncedSearchManager::spawn_search(
                query,
                search_dir,
                limit,
                threads,
                compute_indices,
                exclude,
                callback,
                cancellation_token,
                state,
                config,
            );
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_search(
        query: String,
        search_dir: PathBuf,
        limit: NonZero<usize>,
        threads: NonZero<usize>,
        compute_indices: bool,
        exclude: Vec<String>,
        callback: Arc<C>,
        cancellation_token: Arc<AtomicBool>,
        search_state: Arc<Mutex<DebouncedSearchState>>,
        config: DebounceConfig,
    ) {
        thread::spawn(move || {
            let _guard = ActiveDebouncedSearchGuard::new(
                Arc::clone(&search_state),
                Arc::clone(&cancellation_token),
            );
            let notify_flag = Arc::new(AtomicBool::new(false));
            let notify = {
                let flag = Arc::clone(&notify_flag);
                Arc::new(move || {
                    flag.store(true, Ordering::Release);
                })
            };

            let mut manager = match SearchManager::new(
                &query,
                limit,
                &search_dir,
                exclude,
                threads,
                compute_indices,
                notify,
            ) {
                Ok(manager) => manager,
                Err(err) => {
                    // Emit an empty result set so the caller can clear any
                    // stale results and surface the failure if desired.
                    let empty_results = FileSearchResults {
                        matches: Vec::new(),
                        total_match_count: 0,
                    };
                    callback(query.clone(), empty_results, false);
                    eprintln!("debounced file search initialization failed: {err:?}");
                    return;
                }
            };

            let mut last_sent_paths: Vec<String> = Vec::new();
            let mut last_sent_query: String = String::new();
            let mut current_query = query.clone();
            let mut sent_once = false;
            let mut last_sent_running = false;
            let start = std::time::Instant::now();
            let mut last_progress = start;

            loop {
                if cancellation_token.load(Ordering::Relaxed) {
                    manager.cancel();
                }

                let latest_query = {
                    #[expect(clippy::unwrap_used)]
                    let state = search_state.lock().unwrap();
                    state.latest_query.clone()
                };
                if latest_query != current_query {
                    manager.update_pattern(&latest_query);
                    current_query = latest_query;

                    #[expect(clippy::unwrap_used)]
                    if let Some(active_search) = &mut search_state.lock().unwrap().active_search {
                        active_search.query.clear();
                        active_search.query.push_str(&current_query);
                    }
                }

                let status = manager.tick(config.tick_timeout);
                let flag_was_set = notify_flag.swap(false, Ordering::AcqRel);
                let results = manager.current_results();
                let paths: Vec<String> = results.matches.iter().map(|m| m.path.clone()).collect();

                let paths_changed = paths != last_sent_paths;
                let timeout_elapsed = start.elapsed() >= config.first_result_timeout;
                let walker_running = manager.walker_running();
                let ui_running = walker_running || status.running || flag_was_set || status.changed;
                let running_changed = sent_once && last_sent_running && !ui_running;

                let should_emit = !cancellation_token.load(Ordering::Relaxed)
                    && (paths_changed
                        || current_query != last_sent_query
                        || running_changed
                        || (!sent_once && (flag_was_set || status.changed || timeout_elapsed)));

                if should_emit {
                    callback(current_query.clone(), results.clone(), ui_running);
                    sent_once = true;
                    last_sent_paths = paths;
                    last_sent_query.clear();
                    last_sent_query.push_str(&current_query);
                    last_sent_running = ui_running;
                    last_progress = std::time::Instant::now();
                }

                if cancellation_token.load(Ordering::Relaxed) && sent_once {
                    break;
                }

                if !status.running && !flag_was_set && !walker_running {
                    if sent_once {
                        if last_progress.elapsed() >= config.first_result_timeout {
                            break;
                        }
                    } else if timeout_elapsed && !cancellation_token.load(Ordering::Relaxed) {
                        let ui_running =
                            walker_running || status.running || flag_was_set || status.changed;
                        callback(current_query.clone(), results, ui_running);
                        if !walker_running {
                            break;
                        }
                    }
                }
            }
        });
    }
}

fn spawn_walker(
    search_directory: PathBuf,
    threads: usize,
    override_matcher: Option<ignore::overrides::Override>,
    cancel_flag: Arc<AtomicBool>,
    walker_running: Arc<AtomicBool>,
    injector: nucleo::Injector<SearchItem>,
) -> anyhow::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("codex-file-search-walker".to_string())
        .spawn(move || {
            let _walker_running_guard = WalkerRunningGuard::new(walker_running);
            let search_directory = Arc::new(search_directory);
            let mut walk_builder = WalkBuilder::new(search_directory.as_path());
            walk_builder
                .threads(threads)
                .hidden(false)
                .follow_links(true)
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
                    let path = entry.path();
                    let rel_path = match path.strip_prefix(search_directory.as_path()) {
                        Ok(rel) => rel,
                        Err(_) => path,
                    };
                    if rel_path.as_os_str().is_empty() {
                        return ignore::WalkState::Continue;
                    }
                    let Some(mut path_string) = rel_path.to_str().map(|s| s.to_string()) else {
                        return ignore::WalkState::Continue;
                    };
                    if entry.file_type().is_some_and(|ft| ft.is_dir())
                        && !path_string.ends_with('/')
                    {
                        path_string.push('/');
                    }
                    injector.push(SearchItem::new(path_string), |item, columns| {
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
