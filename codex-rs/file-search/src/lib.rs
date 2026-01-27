use crossbeam_channel::Receiver;
use crossbeam_channel::Sender;
use crossbeam_channel::unbounded;
use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;
use nucleo::Config;
use nucleo::Injector;
use nucleo::Matcher;
use nucleo::Nucleo;
use nucleo::Utf32String;
use nucleo::pattern::CaseMatching;
use nucleo::pattern::Normalization;
use serde::Serialize;
use std::num::NonZero;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::sync::RwLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use std::time::Instant;
use tokio::process::Command;

#[cfg(test)]
use nucleo::Utf32Str;
#[cfg(test)]
use nucleo::pattern::AtomKind;
#[cfg(test)]
use nucleo::pattern::Pattern;

mod cli;

pub use cli::Cli;

/// A single match result returned from the search.
///
/// * `score` – Relevance score returned by `nucleo`.
/// * `path`  – Path to the matched file (relative to the search directory).
/// * `indices` – Optional list of character indices that matched the query.
///   These are only filled when the caller of [`run`] sets
///   `compute_indices` to `true`.  The indices vector follows the
///   guidance from `nucleo::pattern::Pattern::indices`: they are
///   unique and sorted in ascending order so that callers can use
///   them directly for highlighting.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FileMatch {
    pub score: u32,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indices: Option<Vec<u32>>, // Sorted & deduplicated when present
}

/// Returns the final path component for a matched path, falling back to the full path.
pub fn file_name_from_path(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

#[derive(Debug)]
pub struct FileSearchResults {
    pub matches: Vec<FileMatch>,
    pub total_match_count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FileSearchSnapshot {
    pub query: String,
    pub matches: Vec<FileMatch>,
    pub total_match_count: usize,
    pub scanned_file_count: usize,
    pub walk_complete: bool,
}

#[derive(Debug, Clone)]
pub struct SessionOptions {
    pub limit: NonZero<usize>,
    pub exclude: Vec<String>,
    pub threads: NonZero<usize>,
    pub compute_indices: bool,
    pub respect_gitignore: bool,
    /// Minimum interval between streamed updates.
    pub update_interval: Duration,
}

impl Default for SessionOptions {
    fn default() -> Self {
        Self {
            #[expect(clippy::unwrap_used)]
            limit: NonZero::new(20).unwrap(),
            exclude: Vec::new(),
            #[expect(clippy::unwrap_used)]
            threads: NonZero::new(2).unwrap(),
            compute_indices: false,
            respect_gitignore: true,
            update_interval: Duration::from_millis(100),
        }
    }
}

pub trait SessionReporter: Send + Sync + 'static {
    /// Called when the debounced top-N changes.
    fn on_update(&self, snapshot: &FileSearchSnapshot);

    /// Called once when the walk completes (after a final update if needed).
    fn on_complete(&self, snapshot: &FileSearchSnapshot);

    /// Optional hook for non-fatal errors.
    fn on_error(&self, _error: &anyhow::Error) {}
}

pub struct FileSearchSession {
    inner: Arc<SessionInner>,
    walker_handle: Mutex<Option<JoinHandle<()>>>,
    matcher_handle: Mutex<Option<JoinHandle<anyhow::Result<()>>>>,
}

impl FileSearchSession {
    /// Update the query. This should be cheap relative to re-walking.
    pub fn update_query(&self, pattern_text: &str) {
        if self.inner.cancelled.load(Ordering::Relaxed) {
            return;
        }
        {
            #[expect(clippy::unwrap_used)]
            let mut query = self.inner.query.write().unwrap();
            query.clear();
            query.push_str(pattern_text);
        }
        self.inner.query_generation.fetch_add(1, Ordering::Relaxed);
        let _ = self.inner.work_tx.send(WorkSignal::QueryUpdated);
    }

    /// Cancel the session. After cancellation, no further updates are delivered.
    pub fn cancel(&self) {
        let was_cancelled = self.inner.cancelled.swap(true, Ordering::Relaxed);
        if was_cancelled {
            return;
        }
        let _ = self.inner.work_tx.send(WorkSignal::Cancelled);
        self.inner.completion_cv.notify_all();
    }

    /// Return the latest snapshot without forcing an update.
    pub fn snapshot(&self) -> FileSearchSnapshot {
        #[expect(clippy::unwrap_used)]
        self.inner.latest_snapshot.read().unwrap().clone()
    }

    /// Blocks until the walker thread finishes (best-effort for callers that want it).
    pub fn join(&self) -> anyhow::Result<FileSearchSnapshot> {
        let shutdown_requested = || self.inner.shutdown.load(Ordering::Relaxed);
        if let Some(handle) = {
            #[expect(clippy::unwrap_used)]
            self.walker_handle.lock().unwrap().take()
        } {
            let _ = handle.join();
        }
        #[expect(clippy::unwrap_used)]
        let mut guard = self.inner.completion_mutex.lock().unwrap();
        while !self.inner.complete_emitted.load(Ordering::Relaxed)
            && !self.inner.cancelled.load(Ordering::Relaxed)
            && !shutdown_requested()
        {
            #[expect(clippy::unwrap_used)]
            {
                guard = self.inner.completion_cv.wait(guard).unwrap();
            }
        }
        drop(guard);
        self.inner.shutdown.store(true, Ordering::Relaxed);
        let _ = self.inner.work_tx.send(WorkSignal::Shutdown);
        if let Some(handle) = {
            #[expect(clippy::unwrap_used)]
            self.matcher_handle.lock().unwrap().take()
        } {
            match handle.join() {
                Ok(result) => result?,
                Err(_) => anyhow::bail!("matcher thread panicked"),
            }
        }
        Ok(self.snapshot())
    }
}

impl Drop for FileSearchSession {
    fn drop(&mut self) {
        self.inner.shutdown.store(true, Ordering::Relaxed);
        let _ = self.inner.work_tx.send(WorkSignal::Shutdown);
        self.inner.completion_cv.notify_all();
    }
}

pub fn create_session(
    search_directory: &Path,
    options: SessionOptions,
    reporter: Arc<dyn SessionReporter>,
) -> anyhow::Result<FileSearchSession> {
    create_session_inner(search_directory, options, reporter, None)
}

fn create_session_inner(
    search_directory: &Path,
    options: SessionOptions,
    reporter: Arc<dyn SessionReporter>,
    cancel_flag: Option<Arc<AtomicBool>>,
) -> anyhow::Result<FileSearchSession> {
    let SessionOptions {
        limit,
        exclude,
        threads,
        compute_indices,
        respect_gitignore,
        update_interval,
    } = options;

    let override_matcher = build_override_matcher(search_directory, &exclude)?;
    let (work_tx, work_rx) = unbounded();

    let notify_tx = work_tx.clone();
    let notify = Arc::new(move || {
        let _ = notify_tx.send(WorkSignal::NucleoNotify);
    });
    let nucleo = Nucleo::new(
        Config::DEFAULT.match_paths(),
        notify,
        Some(threads.get()),
        1,
    );
    let injector = nucleo.injector();

    let latest_snapshot = FileSearchSnapshot {
        query: String::new(),
        matches: Vec::new(),
        total_match_count: 0,
        scanned_file_count: 0,
        walk_complete: false,
    };

    let cancelled = cancel_flag.unwrap_or_else(|| Arc::new(AtomicBool::new(false)));

    let inner = Arc::new(SessionInner {
        search_directory: search_directory.to_path_buf(),
        limit: limit.get(),
        threads: threads.get(),
        compute_indices,
        respect_gitignore,
        update_interval,
        cancelled: cancelled.clone(),
        shutdown: AtomicBool::new(false),
        walk_complete: AtomicBool::new(false),
        complete_emitted: AtomicBool::new(false),
        completion_mutex: Mutex::new(()),
        completion_cv: Condvar::new(),
        scanned_file_count: AtomicUsize::new(0),
        query_generation: AtomicU64::new(0),
        query: RwLock::new(String::new()),
        latest_snapshot: RwLock::new(latest_snapshot),
        reporter,
        work_tx: work_tx.clone(),
    });

    let matcher_inner = inner.clone();
    let matcher_handle = thread::spawn(move || matcher_worker(matcher_inner, work_rx, nucleo));

    let walker_inner = inner.clone();
    let walker_handle =
        thread::spawn(move || walker_worker(walker_inner, override_matcher, injector));

    Ok(FileSearchSession {
        inner,
        walker_handle: Mutex::new(Some(walker_handle)),
        matcher_handle: Mutex::new(Some(matcher_handle)),
    })
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
        true,
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
#[allow(clippy::too_many_arguments)]
pub fn run(
    pattern_text: &str,
    limit: NonZero<usize>,
    search_directory: &Path,
    exclude: Vec<String>,
    threads: NonZero<usize>,
    cancel_flag: Arc<AtomicBool>,
    compute_indices: bool,
    respect_gitignore: bool,
) -> anyhow::Result<FileSearchResults> {
    let reporter = Arc::new(RunReporter::default());
    let session = create_session_inner(
        search_directory,
        SessionOptions {
            limit,
            exclude,
            threads,
            compute_indices,
            respect_gitignore,
            update_interval: Duration::from_millis(100),
        },
        reporter.clone(),
        Some(cancel_flag.clone()),
    )?;

    session.update_query(pattern_text);
    let snapshot = session.join()?;
    Ok(FileSearchResults {
        matches: snapshot.matches,
        total_match_count: snapshot.total_match_count,
    })
}

/// Sort matches in-place by descending score, then ascending path.
#[cfg(test)]
fn sort_matches(matches: &mut [(u32, String)]) {
    matches.sort_by(cmp_by_score_desc_then_path_asc::<(u32, String), _, _>(
        |t| t.0,
        |t| t.1.as_str(),
    ));
}

/// Returns a comparator closure suitable for `slice.sort_by(...)` that orders
/// items by descending score and then ascending path using the provided accessors.
pub fn cmp_by_score_desc_then_path_asc<T, FScore, FPath>(
    score_of: FScore,
    path_of: FPath,
) -> impl FnMut(&T, &T) -> std::cmp::Ordering
where
    FScore: Fn(&T) -> u32,
    FPath: Fn(&T) -> &str,
{
    use std::cmp::Ordering;
    move |a, b| match score_of(b).cmp(&score_of(a)) {
        Ordering::Equal => path_of(a).cmp(path_of(b)),
        other => other,
    }
}

#[cfg(test)]
fn create_pattern(pattern: &str) -> Pattern {
    Pattern::new(
        pattern,
        CaseMatching::Smart,
        Normalization::Smart,
        AtomKind::Fuzzy,
    )
}

struct SessionInner {
    search_directory: PathBuf,
    limit: usize,
    threads: usize,
    compute_indices: bool,
    respect_gitignore: bool,
    update_interval: Duration,
    cancelled: Arc<AtomicBool>,
    shutdown: AtomicBool,
    walk_complete: AtomicBool,
    complete_emitted: AtomicBool,
    completion_mutex: Mutex<()>,
    completion_cv: Condvar,
    scanned_file_count: AtomicUsize,
    query_generation: AtomicU64,
    query: RwLock<String>,
    latest_snapshot: RwLock<FileSearchSnapshot>,
    reporter: Arc<dyn SessionReporter>,
    work_tx: Sender<WorkSignal>,
}

enum WorkSignal {
    CorpusAppended,
    QueryUpdated,
    NucleoNotify,
    WalkComplete,
    Cancelled,
    Shutdown,
}

fn build_override_matcher(
    search_directory: &Path,
    exclude: &[String],
) -> anyhow::Result<Option<ignore::overrides::Override>> {
    if exclude.is_empty() {
        return Ok(None);
    }
    let mut override_builder = OverrideBuilder::new(search_directory);
    for exclude in exclude {
        let exclude_pattern = format!("!{exclude}");
        override_builder.add(&exclude_pattern)?;
    }
    let matcher = override_builder.build()?;
    Ok(Some(matcher))
}

fn walker_worker(
    inner: Arc<SessionInner>,
    override_matcher: Option<ignore::overrides::Override>,
    injector: Injector<Arc<str>>,
) {
    let cancel_requested = || inner.cancelled.load(Ordering::Relaxed);
    let shutdown_requested = || inner.shutdown.load(Ordering::Relaxed);
    if cancel_requested() {
        inner.cancelled.store(true, Ordering::Relaxed);
        inner.completion_cv.notify_all();
        return;
    }
    if shutdown_requested() {
        inner.completion_cv.notify_all();
        return;
    }

    let mut walk_builder = WalkBuilder::new(&inner.search_directory);
    walk_builder
        .threads(inner.threads)
        // Allow hidden entries.
        .hidden(false)
        // Follow symlinks to search their contents.
        .follow_links(true)
        // Don't require git to be present to apply to apply git-related ignore rules.
        .require_git(false);
    if !inner.respect_gitignore {
        walk_builder
            .git_ignore(false)
            .git_global(false)
            .git_exclude(false)
            .ignore(false)
            .parents(false);
    }
    if let Some(override_matcher) = override_matcher {
        walk_builder.overrides(override_matcher);
    }

    let walker = walk_builder.build_parallel();

    struct WorkerBuffer {
        buffer: Vec<Arc<str>>,
        inner: Arc<SessionInner>,
        injector: Injector<Arc<str>>,
    }

    impl WorkerBuffer {
        const BATCH_SIZE: usize = 256;

        fn new(inner: Arc<SessionInner>, injector: Injector<Arc<str>>) -> Self {
            Self {
                buffer: Vec::with_capacity(Self::BATCH_SIZE),
                inner,
                injector,
            }
        }

        fn cancel_requested(&self) -> bool {
            self.inner.cancelled.load(Ordering::Relaxed)
        }

        fn shutdown_requested(&self) -> bool {
            self.inner.shutdown.load(Ordering::Relaxed)
        }

        fn signal_cancel(&self) {
            self.inner.cancelled.store(true, Ordering::Relaxed);
            let _ = self.inner.work_tx.send(WorkSignal::Cancelled);
            self.inner.completion_cv.notify_all();
        }

        fn push(&mut self, path: &str) {
            if self.cancel_requested() {
                self.signal_cancel();
                return;
            }
            if self.shutdown_requested() {
                self.buffer.clear();
                return;
            }
            self.buffer.push(Arc::<str>::from(path));
            if self.buffer.len() >= Self::BATCH_SIZE {
                self.flush();
            }
        }

        fn flush(&mut self) {
            if self.buffer.is_empty() {
                return;
            }
            if self.cancel_requested() {
                self.buffer.clear();
                self.signal_cancel();
                return;
            }
            if self.shutdown_requested() {
                self.buffer.clear();
                return;
            }
            let appended = self.buffer.len();
            self.injector.extend(self.buffer.drain(..), |path, cols| {
                cols[0] = Utf32String::from(path.as_ref());
            });
            self.inner
                .scanned_file_count
                .fetch_add(appended, Ordering::Relaxed);
            let _ = self.inner.work_tx.send(WorkSignal::CorpusAppended);
        }
    }

    impl Drop for WorkerBuffer {
        fn drop(&mut self) {
            self.flush();
        }
    }

    fn get_file_path<'a>(
        entry_result: &'a Result<ignore::DirEntry, ignore::Error>,
        search_directory: &Path,
    ) -> Option<&'a str> {
        let entry = match entry_result {
            Ok(entry) => entry,
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

    walker.run(|| {
        let mut worker_buffer = WorkerBuffer::new(inner.clone(), injector.clone());
        // Each worker keeps a local counter so we only read the atomic flag
        // every N entries which is cheaper than checking on every file.
        const CHECK_INTERVAL: usize = 1024;
        let mut processed = 0;

        Box::new(move |entry| {
            if let Some(path) = get_file_path(&entry, &worker_buffer.inner.search_directory) {
                worker_buffer.push(path);
            }
            processed += 1;
            if processed % CHECK_INTERVAL == 0 {
                if worker_buffer.cancel_requested() {
                    worker_buffer.signal_cancel();
                    return ignore::WalkState::Quit;
                }
                if worker_buffer.shutdown_requested() {
                    worker_buffer.inner.completion_cv.notify_all();
                    return ignore::WalkState::Quit;
                }
            }
            ignore::WalkState::Continue
        })
    });

    if cancel_requested() {
        inner.cancelled.store(true, Ordering::Relaxed);
        inner.completion_cv.notify_all();
        return;
    }
    if shutdown_requested() {
        inner.completion_cv.notify_all();
        return;
    }

    inner.walk_complete.store(true, Ordering::Relaxed);
    let _ = inner.work_tx.send(WorkSignal::WalkComplete);
}

fn matcher_worker(
    inner: Arc<SessionInner>,
    work_rx: Receiver<WorkSignal>,
    mut nucleo: Nucleo<Arc<str>>,
) -> anyhow::Result<()> {
    const TICK_TIMEOUT_MS: u64 = 10;

    let config = Config::DEFAULT.match_paths();
    let mut indices_matcher = inner.compute_indices.then(|| Matcher::new(config.clone()));
    let cancel_requested = || inner.cancelled.load(Ordering::Relaxed);
    let shutdown_requested = || inner.shutdown.load(Ordering::Relaxed);
    let idle_timeout = Duration::from_millis(50);

    let mut last_query_generation = inner.query_generation.load(Ordering::Relaxed);
    let mut last_query = String::new();
    let mut last_matches: Vec<FileMatch> = Vec::new();

    let mut emit_pending = false;
    let mut last_emit_at = Instant::now() - inner.update_interval;

    loop {
        if cancel_requested() {
            inner.cancelled.store(true, Ordering::Relaxed);
            let _ = inner.work_tx.send(WorkSignal::Cancelled);
            inner.completion_cv.notify_all();
            break;
        }
        if shutdown_requested() {
            break;
        }
        let timeout = if emit_pending {
            let elapsed = last_emit_at.elapsed();
            if elapsed >= inner.update_interval {
                Duration::ZERO
            } else {
                inner.update_interval - elapsed
            }
        } else {
            idle_timeout
        };

        let mut saw_signal = false;
        let mut saw_walk_complete_signal = false;
        let mut saw_cancel_signal = false;
        let mut saw_nucleo_notify = false;
        let mut saw_shutdown_signal = false;

        match work_rx.recv_timeout(timeout) {
            Ok(signal) => {
                saw_signal = true;
                match signal {
                    WorkSignal::WalkComplete => saw_walk_complete_signal = true,
                    WorkSignal::Cancelled => saw_cancel_signal = true,
                    WorkSignal::NucleoNotify => saw_nucleo_notify = true,
                    WorkSignal::Shutdown => saw_shutdown_signal = true,
                    WorkSignal::CorpusAppended | WorkSignal::QueryUpdated => {}
                }
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
        }

        for signal in work_rx.try_iter() {
            saw_signal = true;
            match signal {
                WorkSignal::WalkComplete => saw_walk_complete_signal = true,
                WorkSignal::Cancelled => saw_cancel_signal = true,
                WorkSignal::NucleoNotify => saw_nucleo_notify = true,
                WorkSignal::Shutdown => saw_shutdown_signal = true,
                WorkSignal::CorpusAppended | WorkSignal::QueryUpdated => {}
            }
        }

        if saw_cancel_signal || inner.cancelled.load(Ordering::Relaxed) {
            break;
        }
        if saw_shutdown_signal || shutdown_requested() {
            break;
        }

        let walk_complete = inner.walk_complete.load(Ordering::Relaxed) || saw_walk_complete_signal;
        if !saw_signal && !emit_pending {
            continue;
        }

        let current_query = {
            #[expect(clippy::unwrap_used)]
            inner.query.read().unwrap().clone()
        };
        let current_generation = inner.query_generation.load(Ordering::Relaxed);
        let query_changed =
            current_generation != last_query_generation || current_query != last_query;
        if query_changed {
            let append = current_query.starts_with(&last_query);
            nucleo.pattern.reparse(
                0,
                &current_query,
                CaseMatching::Smart,
                Normalization::Smart,
                append,
            );
            last_query_generation = current_generation;
            last_query = current_query.clone();
        }

        let query_is_empty = current_query.is_empty();
        let mut nucleo_running = false;
        if !query_is_empty && (query_changed || saw_signal || saw_nucleo_notify || emit_pending) {
            let status = nucleo.tick(TICK_TIMEOUT_MS);
            nucleo_running = status.running;
        }

        let matches = if query_is_empty {
            Vec::new()
        } else {
            let snapshot = nucleo.snapshot();
            let limit = inner.limit.min(snapshot.matched_item_count() as usize);
            let pattern = snapshot.pattern().column_pattern(0);
            snapshot
                .matches()
                .iter()
                .take(limit)
                .filter_map(|match_| {
                    let item = snapshot.get_item(match_.idx)?;
                    let indices = if let Some(indices_matcher) = indices_matcher.as_mut() {
                        let mut idx_vec = Vec::<u32>::new();
                        let haystack = item.matcher_columns[0].slice(..);
                        let _ = pattern.indices(haystack, indices_matcher, &mut idx_vec);
                        idx_vec.sort_unstable();
                        idx_vec.dedup();
                        Some(idx_vec)
                    } else {
                        None
                    };
                    Some(FileMatch {
                        score: match_.score,
                        path: item.data.as_ref().to_string(),
                        indices,
                    })
                })
                .collect()
        };

        let total_match_count = if query_is_empty {
            0
        } else {
            nucleo.snapshot().matched_item_count() as usize
        };

        let snapshot = FileSearchSnapshot {
            query: current_query.clone(),
            matches: matches.clone(),
            total_match_count,
            scanned_file_count: inner.scanned_file_count.load(Ordering::Relaxed),
            walk_complete,
        };
        {
            #[expect(clippy::unwrap_used)]
            let mut latest_snapshot = inner.latest_snapshot.write().unwrap();
            *latest_snapshot = snapshot.clone();
        }

        let top_changed = matches != last_matches;
        if top_changed {
            last_matches = matches;
        }

        let now = Instant::now();
        let throttle_ready = now.duration_since(last_emit_at) >= inner.update_interval;
        if top_changed && (throttle_ready || walk_complete) {
            if inner.cancelled.load(Ordering::Relaxed) {
                break;
            }
            inner.reporter.on_update(&snapshot);
            last_emit_at = now;
            emit_pending = false;
        } else if top_changed {
            emit_pending = true;
        } else if emit_pending && throttle_ready {
            if inner.cancelled.load(Ordering::Relaxed) {
                break;
            }
            inner.reporter.on_update(&snapshot);
            last_emit_at = now;
            emit_pending = false;
        }

        if walk_complete && nucleo_running {
            // The walk may complete before the matcher has processed all injected items.
            // Keep ticking until nucleo reports that it is no longer running to avoid
            // emitting an incomplete "complete" snapshot.
            emit_pending = true;
            continue;
        }

        if walk_complete && !inner.complete_emitted.load(Ordering::Relaxed) {
            if emit_pending {
                if inner.cancelled.load(Ordering::Relaxed) {
                    break;
                }
                inner.reporter.on_update(&snapshot);
                emit_pending = false;
            }
            if inner.cancelled.load(Ordering::Relaxed) {
                break;
            }
            inner.reporter.on_complete(&snapshot);
            inner.complete_emitted.store(true, Ordering::Relaxed);
            inner.completion_cv.notify_all();
        }
    }

    Ok(())
}

struct RunReporter {
    snapshot: RwLock<FileSearchSnapshot>,
}

impl Default for RunReporter {
    fn default() -> Self {
        Self {
            snapshot: RwLock::new(FileSearchSnapshot {
                query: String::new(),
                matches: Vec::new(),
                total_match_count: 0,
                scanned_file_count: 0,
                walk_complete: false,
            }),
        }
    }
}

impl SessionReporter for RunReporter {
    fn on_update(&self, snapshot: &FileSearchSnapshot) {
        #[expect(clippy::unwrap_used)]
        let mut guard = self.snapshot.write().unwrap();
        *guard = snapshot.clone();
    }

    fn on_complete(&self, snapshot: &FileSearchSnapshot) {
        #[expect(clippy::unwrap_used)]
        let mut guard = self.snapshot.write().unwrap();
        *guard = snapshot.clone();
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::sync::Arc;
    use std::sync::Condvar;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicBool;
    use std::thread;
    use std::time::Duration;
    use std::time::Instant;
    use tempfile::TempDir;

    #[test]
    fn verify_score_is_none_for_non_match() {
        let mut utf32buf = Vec::<char>::new();
        let line = "hello";
        let mut matcher = Matcher::new(Config::DEFAULT);
        let haystack: Utf32Str<'_> = Utf32Str::new(line, &mut utf32buf);
        let pattern = create_pattern("zzz");
        let score = pattern.score(haystack, &mut matcher);
        assert_eq!(score, None);
    }

    #[test]
    fn tie_breakers_sort_by_path_when_scores_equal() {
        let mut matches = vec![
            (100, "b_path".to_string()),
            (100, "a_path".to_string()),
            (90, "zzz".to_string()),
        ];

        sort_matches(&mut matches);

        // Highest score first; ties broken alphabetically.
        let expected = vec![
            (100, "a_path".to_string()),
            (100, "b_path".to_string()),
            (90, "zzz".to_string()),
        ];

        assert_eq!(matches, expected);
    }

    #[test]
    fn file_name_from_path_uses_basename() {
        assert_eq!(file_name_from_path("foo/bar.txt"), "bar.txt");
    }

    #[test]
    fn file_name_from_path_falls_back_to_full_path() {
        assert_eq!(file_name_from_path(""), "");
    }

    #[derive(Default)]
    struct RecordingReporter {
        updates: Mutex<Vec<FileSearchSnapshot>>,
        completes: Mutex<Vec<FileSearchSnapshot>>,
        complete_times: Mutex<Vec<Instant>>,
        complete_cv: Condvar,
        update_cv: Condvar,
    }

    impl RecordingReporter {
        fn wait_for_complete(&self, timeout: Duration) -> bool {
            let completes = self.completes.lock().unwrap();
            if !completes.is_empty() {
                return true;
            }
            let (completes, _) = self.complete_cv.wait_timeout(completes, timeout).unwrap();
            !completes.is_empty()
        }

        fn updates(&self) -> Vec<FileSearchSnapshot> {
            self.updates.lock().unwrap().clone()
        }

        fn wait_for_updates_at_least(&self, min_len: usize, timeout: Duration) -> bool {
            let updates = self.updates.lock().unwrap();
            if updates.len() >= min_len {
                return true;
            }
            let (updates, _) = self.update_cv.wait_timeout(updates, timeout).unwrap();
            updates.len() >= min_len
        }

        fn complete_times(&self) -> Vec<Instant> {
            self.complete_times.lock().unwrap().clone()
        }
    }

    impl SessionReporter for RecordingReporter {
        fn on_update(&self, snapshot: &FileSearchSnapshot) {
            let mut updates = self.updates.lock().unwrap();
            updates.push(snapshot.clone());
            self.update_cv.notify_all();
        }

        fn on_complete(&self, snapshot: &FileSearchSnapshot) {
            {
                let mut completes = self.completes.lock().unwrap();
                completes.push(snapshot.clone());
            }
            {
                let mut complete_times = self.complete_times.lock().unwrap();
                complete_times.push(Instant::now());
            }
            self.complete_cv.notify_all();
        }
    }

    fn create_temp_tree(file_count: usize) -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..file_count {
            let path = dir.path().join(format!("file-{i:04}.txt"));
            fs::write(path, format!("contents {i}")).unwrap();
        }
        dir
    }

    #[test]
    fn session_scanned_file_count_is_monotonic_across_queries() {
        let dir = create_temp_tree(200);
        let reporter = Arc::new(RecordingReporter::default());
        let session = create_session(
            dir.path(),
            SessionOptions {
                update_interval: Duration::from_millis(5),
                ..SessionOptions::default()
            },
            reporter.clone(),
        )
        .expect("session");

        session.update_query("file-00");
        thread::sleep(Duration::from_millis(20));
        let first_snapshot = session.snapshot();
        session.update_query("file-01");
        thread::sleep(Duration::from_millis(20));
        let second_snapshot = session.snapshot();
        let _ = session.join();

        assert!(second_snapshot.scanned_file_count >= first_snapshot.scanned_file_count);
        assert!(session.snapshot().scanned_file_count >= second_snapshot.scanned_file_count);
    }

    #[test]
    fn session_streams_updates_before_walk_complete() {
        let dir = create_temp_tree(600);
        let reporter = Arc::new(RecordingReporter::default());
        let session = create_session(
            dir.path(),
            SessionOptions {
                update_interval: Duration::from_millis(1),
                ..SessionOptions::default()
            },
            reporter.clone(),
        )
        .expect("session");

        session.update_query("file-0");
        let completed = reporter.wait_for_complete(Duration::from_secs(5));
        let _ = session.join();

        assert!(completed);
        let updates = reporter.updates();
        assert!(updates.iter().any(|snapshot| !snapshot.walk_complete));
    }

    #[test]
    fn session_accepts_query_updates_after_walk_complete() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("alpha.txt"), "alpha").unwrap();
        fs::write(dir.path().join("beta.txt"), "beta").unwrap();
        let reporter = Arc::new(RecordingReporter::default());
        let session = create_session(
            dir.path(),
            SessionOptions {
                update_interval: Duration::from_millis(1),
                ..SessionOptions::default()
            },
            reporter.clone(),
        )
        .expect("session");

        session.update_query("alpha");
        assert!(reporter.wait_for_complete(Duration::from_secs(5)));
        let updates_before = reporter.updates().len();

        session.update_query("beta");
        assert!(reporter.wait_for_updates_at_least(updates_before + 1, Duration::from_secs(5),));

        let updates = reporter.updates();
        let last_update = updates.last().cloned().expect("update");
        assert!(
            last_update
                .matches
                .iter()
                .any(|file_match| file_match.path.contains("beta.txt"))
        );

        session.cancel();
        let _ = session.join();
    }

    #[test]
    fn session_cancellation_skips_completion_callback() {
        let dir = create_temp_tree(800);
        let reporter = Arc::new(RecordingReporter::default());
        let session = create_session(
            dir.path(),
            SessionOptions {
                update_interval: Duration::from_millis(5),
                ..SessionOptions::default()
            },
            reporter.clone(),
        )
        .expect("session");

        session.update_query("file-");
        thread::sleep(Duration::from_millis(10));
        let cancel_time = Instant::now();
        session.cancel();
        let _ = session.join();

        let complete_times = reporter.complete_times();
        assert!(complete_times.iter().all(|time| *time <= cancel_time));
    }

    #[test]
    fn dropping_session_does_not_cancel_siblings_with_shared_cancel_flag() {
        let root_a = create_temp_tree(200);
        let root_b = create_temp_tree(4_000);
        let cancel_flag = Arc::new(AtomicBool::new(false));

        let reporter_a = Arc::new(RecordingReporter::default());
        let session_a = create_session_inner(
            root_a.path(),
            SessionOptions {
                update_interval: Duration::from_millis(1),
                ..SessionOptions::default()
            },
            reporter_a,
            Some(cancel_flag.clone()),
        )
        .expect("session_a");

        let reporter_b = Arc::new(RecordingReporter::default());
        let session_b = create_session_inner(
            root_b.path(),
            SessionOptions {
                update_interval: Duration::from_millis(1),
                ..SessionOptions::default()
            },
            reporter_b.clone(),
            Some(cancel_flag),
        )
        .expect("session_b");

        session_a.update_query("file-0");
        session_b.update_query("file-1");

        thread::sleep(Duration::from_millis(5));
        drop(session_a);

        let completed = reporter_b.wait_for_complete(Duration::from_secs(5));
        let _ = session_b.join();

        assert_eq!(completed, true);
    }

    #[test]
    fn session_does_not_emit_updates_when_top_n_is_unchanged() {
        let dir = create_temp_tree(200);
        let reporter = Arc::new(RecordingReporter::default());
        let session = create_session(
            dir.path(),
            SessionOptions {
                update_interval: Duration::from_millis(1),
                ..SessionOptions::default()
            },
            reporter.clone(),
        )
        .expect("session");

        session.update_query("zzzzzzzz");
        let _ = reporter.wait_for_complete(Duration::from_secs(5));
        let _ = session.join();

        let updates = reporter.updates();
        assert!(updates.is_empty());
    }
}
