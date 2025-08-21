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
use codex_file_search::FileMatch;
use std::num::NonZeroUsize;
use std::path::Component;
use std::path::Path;
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
            // Split the query into an optional directory scope and the fuzzy leaf.
            let (effective_dir, scope_prefix, leaf, prefix_len) = resolve_scope(
                &search_dir,
                &query,
            )
            .unwrap_or((search_dir.clone(), None, query.clone(), 0));

            let matches_raw = file_search::run(
                &leaf,
                MAX_FILE_SEARCH_RESULTS,
                &effective_dir,
                Vec::new(),
                NUM_FILE_SEARCH_THREADS,
                cancellation_token.clone(),
                compute_indices,
            )
            .map(|res| res.matches)
            .unwrap_or_default();

            // Prefix returned paths with the scope (if any) and shift indices accordingly.
            let matches: Vec<FileMatch> = if let Some(prefix) = scope_prefix {
                matches_raw
                    .into_iter()
                    .map(|mut m| {
                        let new_path = if m.path.is_empty() {
                            prefix.clone()
                        } else {
                            format!("{}/{}", prefix, m.path)
                        };
                        if let Some(ref mut idx) = m.indices {
                            for i in idx.iter_mut() {
                                *i = (*i as usize + prefix_len) as u32;
                            }
                        }
                        FileMatch {
                            path: new_path,
                            ..m
                        }
                    })
                    .collect()
            } else {
                matches_raw
            };

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

/// Parses a user query into a safe directory scope (relative to `root`) and a
/// fuzzy pattern leaf. Returns (effective_dir, scope_prefix_string, leaf, prefix_len_chars).
fn resolve_scope(root: &PathBuf, query: &str) -> Option<(PathBuf, Option<String>, String, usize)> {
    let q = query.trim_start_matches('/');
    let Some(pos) = q.rfind('/') else {
        return Some((root.clone(), None, q.to_string(), 0));
    };
    let (scope, leaf) = q.split_at(pos);
    // `split_at` keeps the slash at start of leaf; trim it.
    let leaf = leaf.trim_start_matches('/').to_string();
    if scope.is_empty() {
        return Some((root.clone(), None, leaf, 0));
    }
    // Reject unsafe components and absolute paths.
    if !is_safe_relative(scope) {
        return Some((root.clone(), None, q.to_string(), 0));
    }
    let candidate = root.join(scope);
    if candidate.is_dir() {
        let prefix = scope.replace("\\", "/");
        let prefix_len = prefix.chars().count() + 1; // account for the '/'
        Some((candidate, Some(prefix), leaf, prefix_len))
    } else {
        Some((root.clone(), None, q.to_string(), 0))
    }
}

fn is_safe_relative(p: &str) -> bool {
    let path = Path::new(p);
    if path.is_absolute() {
        return false;
    }
    for c in path.components() {
        match c {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return false,
            Component::CurDir | Component::Normal(_) => {}
        }
    }
    true
}
