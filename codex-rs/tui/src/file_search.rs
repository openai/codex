//! Helper that owns the debounce/cancellation logic for `@` file searches.
//!
//! `ChatComposer` publishes *every* change of the `@token` as
//! `AppEvent::StartFileSearch(query)`.
//! This struct receives those events and decides when to actually spawn the
//! expensive search (handled in the main `App` thread). It tries to ensure:
//!
//! - Even when the user types long text quickly, they will start seeing results
//!   after a short delay using an early version of what they typed.
//! - At most one search is in-flight at any time.
//!
//! It works as follows:
//!
//! 1. First query starts a debounce timer.
//! 2. While the timer is pending, the latest query from the user is stored.
//! 3. When the timer fires, it is cleared, and a search is done for the most
//!    recent query.
//! 4. If there is a in-flight search that is not a prefix of the latest thing
//!    the user typed, it is cancelled.

use codex_file_search as file_search;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;

#[allow(clippy::unwrap_used)]
const MAX_FILE_SEARCH_RESULTS: NonZeroUsize = NonZeroUsize::new(8).unwrap();

#[allow(clippy::unwrap_used)]
const NUM_FILE_SEARCH_THREADS: NonZeroUsize = NonZeroUsize::new(2).unwrap();

/// How long to wait after a keystroke before firing the first search when none
/// is currently running. Keeps early queries more meaningful.
const FILE_SEARCH_DEBOUNCE: Duration = Duration::from_millis(100);

const ACTIVE_SEARCH_COMPLETE_POLL_INTERVAL: Duration = Duration::from_millis(20);

/// State machine for file-search orchestration.
pub(crate) struct FileSearchManager {
    /// Unified state guarded by one mutex.
    state: Arc<Mutex<SearchState>>,

    search_dir: PathBuf,
    app_tx: AppEventSender,
}

struct SearchState {
    /// Latest query typed by user (updated every keystroke).
    latest_query: String,

    /// true if a search is currently scheduled.
    is_search_scheduled: bool,

    /// If there is an active search, this will be the query being searched.
    active_search: Option<ActiveSearch>,
}

struct ActiveSearch {
    query: String,
    cancellation_token: Arc<AtomicBool>,
}

impl FileSearchManager {
    pub fn new(search_dir: PathBuf, tx: AppEventSender) -> Self {
        Self {
            state: Arc::new(Mutex::new(SearchState {
                latest_query: String::new(),
                is_search_scheduled: false,
                active_search: None,
            })),
            search_dir,
            app_tx: tx,
        }
    }

    /// Call whenever the user edits the `@` token.
    pub fn on_user_query(&self, query: String) {
        {
            #[allow(clippy::unwrap_used)]
            let mut st = self.state.lock().unwrap();
            // If the query is empty, build quick suggestions immediately and return.
            // Do this BEFORE the unchanged short-circuit so the initial empty
            // query ("@") still yields results even though latest_query starts empty.
            if query.is_empty() {
                let search_dir = self.search_dir.clone();
                let tx = self.app_tx.clone();
                std::thread::spawn(move || {
                    let max_total = MAX_FILE_SEARCH_RESULTS.get();
                    let matches = collect_top_level_suggestions(&search_dir, max_total);
                    tx.send(AppEvent::FileSearchResult {
                        query: String::new(),
                        matches,
                    });
                });
                return;
            }

            if query == st.latest_query {
                // No change, nothing to do.
                return;
            }

            // Update latest query.
            st.latest_query.clear();
            st.latest_query.push_str(&query);

            // If there is an in-flight search that is definitely obsolete,
            // cancel it now.
            if let Some(active_search) = &st.active_search {
                if !query.starts_with(&active_search.query) {
                    active_search
                        .cancellation_token
                        .store(true, Ordering::Relaxed);
                    st.active_search = None;
                }
            }

            // Schedule a search to run after debounce.
            if !st.is_search_scheduled {
                st.is_search_scheduled = true;
            } else {
                return;
            }
        }

        // If we are here, we set `st.is_search_scheduled = true` before
        // dropping the lock. This means we are the only thread that can spawn a
        // debounce timer.
        let state = self.state.clone();
        let search_dir = self.search_dir.clone();
        let tx_clone = self.app_tx.clone();
        thread::spawn(move || {
            // Always do a minimum debounce, but then poll until the
            // `active_search` is cleared.
            thread::sleep(FILE_SEARCH_DEBOUNCE);
            loop {
                #[allow(clippy::unwrap_used)]
                if state.lock().unwrap().active_search.is_none() {
                    break;
                }
                thread::sleep(ACTIVE_SEARCH_COMPLETE_POLL_INTERVAL);
            }

            // The debounce timer has expired, so start a search using the
            // latest query.
            let cancellation_token = Arc::new(AtomicBool::new(false));
            let token = cancellation_token.clone();
            let query = {
                #[allow(clippy::unwrap_used)]
                let mut st = state.lock().unwrap();
                let query = st.latest_query.clone();
                st.is_search_scheduled = false;
                st.active_search = Some(ActiveSearch {
                    query: query.clone(),
                    cancellation_token: token,
                });
                query
            };

            FileSearchManager::spawn_file_search(
                query,
                search_dir,
                tx_clone,
                cancellation_token,
                state,
            );
        });
    }

    fn spawn_file_search(
        query: String,
        search_dir: PathBuf,
        tx: AppEventSender,
        cancellation_token: Arc<AtomicBool>,
        search_state: Arc<Mutex<SearchState>>,
    ) {
        let compute_indices = true;
        std::thread::spawn(move || {
            let matches = file_search::run(
                &query,
                MAX_FILE_SEARCH_RESULTS,
                &search_dir,
                Vec::new(),
                NUM_FILE_SEARCH_THREADS,
                cancellation_token.clone(),
                compute_indices,
            )
            .map(|res| res.matches)
            .unwrap_or_default();

            let is_cancelled = cancellation_token.load(Ordering::Relaxed);
            if !is_cancelled {
                tx.send(AppEvent::FileSearchResult { query, matches });
            }

            // Reset the active search state. Do a pointer comparison to verify
            // that we are clearing the ActiveSearch that corresponds to the
            // cancellation token we were given.
            {
                #[allow(clippy::unwrap_used)]
                let mut st = search_state.lock().unwrap();
                if let Some(active_search) = &st.active_search {
                    if Arc::ptr_eq(&active_search.cancellation_token, &cancellation_token) {
                        st.active_search = None;
                    }
                }
            }
        });
    }
}

/// Build a small, fast set of suggestions for an empty `@` mention.
/// Strategy: list top-level non-hidden files first, then top-level directories
/// (with trailing '/'), capped by `max_total`.
fn collect_top_level_suggestions(
    cwd: &std::path::Path,
    max_total: usize,
) -> Vec<file_search::FileMatch> {
    use std::collections::HashSet;
    use std::fs;

    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<file_search::FileMatch> = Vec::new();
    let mut total_added: usize = 0;

    // 1) Top-level non-hidden files in cwd (files only).
    if let Ok(rd) = fs::read_dir(cwd) {
        for entry in rd.flatten() {
            if total_added >= max_total {
                break;
            }
            let path = entry.path();
            let file_name = match path.strip_prefix(cwd).ok().and_then(|p| p.to_str()) {
                Some(s) => s,
                None => continue,
            };
            if file_name.starts_with('.') {
                continue;
            }
            match entry.file_type() {
                Ok(ft) if ft.is_file() => {
                    push_mention_path(&mut out, &mut seen, &mut total_added, file_name.to_string());
                }
                _ => {}
            }
        }
    }

    // 2) If still under cap, add top-level non-hidden directories (with trailing '/').
    if total_added < max_total {
        if let Ok(rd) = fs::read_dir(cwd) {
            for entry in rd.flatten() {
                if total_added >= max_total {
                    break;
                }
                let path = entry.path();
                let file_name = match path.strip_prefix(cwd).ok().and_then(|p| p.to_str()) {
                    Some(s) => s,
                    None => continue,
                };
                if file_name.starts_with('.') {
                    continue;
                }
                if let Ok(ft) = entry.file_type() {
                    if ft.is_dir() {
                        push_mention_path(
                            &mut out,
                            &mut seen,
                            &mut total_added,
                            format!("{file_name}/"),
                        );
                    }
                }
            }
        }
    }

    if total_added > max_total {
        out.truncate(max_total);
    }
    out
}

/// Insert a suggestion if not seen yet; updates `total_added` and returns true when inserted.
fn push_mention_path(
    out: &mut Vec<file_search::FileMatch>,
    seen: &mut std::collections::HashSet<String>,
    total_added: &mut usize,
    rel: String,
) {
    if seen.insert(rel.clone()) {
        out.push(file_search::FileMatch {
            score: 0,
            path: rel,
            indices: None,
        });
        *total_added += 1;
    }
}
