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
use std::time::Instant;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;

const MAX_FILE_SEARCH_RESULTS: NonZeroUsize = NonZeroUsize::new(8).unwrap();
const NUM_FILE_SEARCH_THREADS: NonZeroUsize = NonZeroUsize::new(2).unwrap();

/// How long to wait after a keystroke before firing the first search when none
/// is currently running. Keeps early queries more meaningful.
const FILE_SEARCH_DEBOUNCE: Duration = Duration::from_millis(100);

const ACTIVE_SEARCH_COMPLETE_POLL_INTERVAL: Duration = Duration::from_millis(20);
const SEARCH_MANAGER_TICK_TIMEOUT: Duration = Duration::from_millis(16);
const SEARCH_MANAGER_FIRST_RESULT_TIMEOUT: Duration = Duration::from_millis(200);

/// State machine for file-search orchestration.
pub(crate) struct FileSearchManager {
    /// Unified state guarded by one mutex.
    state: Arc<Mutex<SearchState>>,

    search_dir: PathBuf,
    app_tx: AppEventSender,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use std::time::Instant;
    use tokio::sync::mpsc::unbounded_channel;

    #[test]
    fn file_search_manager_emits_results() {
        let temp_dir = tempfile::tempdir().unwrap();
        let nested = temp_dir.path().join("src");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("gamma.rs"), "fn main() {}").unwrap();

        let (tx, mut rx) = unbounded_channel();
        let manager =
            FileSearchManager::new(temp_dir.path().to_path_buf(), AppEventSender::new(tx));
        manager.on_user_query("gam".to_string());

        let start = Instant::now();
        let mut saw_match = false;

        let mut captured: Vec<String> = Vec::new();
        while start.elapsed() < Duration::from_secs(2) {
            while let Ok(event) = rx.try_recv() {
                if let AppEvent::FileSearchResult { matches, .. } = &event {
                    if matches.iter().any(|m| m.path.ends_with("gamma.rs")) {
                        saw_match = true;
                        captured.push(format!("{event:?}"));
                        break;
                    }
                }
                captured.push(format!("{event:?}"));
            }
            if saw_match {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        if !saw_match {
            while let Ok(event) = rx.try_recv() {
                captured.push(format!("{event:?}"));
            }
        }

        assert!(
            saw_match,
            "file search did not emit expected result; captured events: {captured:?}"
        );
    }
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

struct ActiveSearchGuard {
    state: Arc<Mutex<SearchState>>,
    token: Arc<AtomicBool>,
}

impl ActiveSearchGuard {
    fn new(state: Arc<Mutex<SearchState>>, token: Arc<AtomicBool>) -> Self {
        Self { state, token }
    }
}

impl Drop for ActiveSearchGuard {
    fn drop(&mut self) {
        #[expect(clippy::unwrap_used)]
        let mut st = self.state.lock().unwrap();
        if let Some(active_search) = &st.active_search
            && Arc::ptr_eq(&active_search.cancellation_token, &self.token)
        {
            st.active_search = None;
        }
    }
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
            #[expect(clippy::unwrap_used)]
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
            if let Some(active_search) = &st.active_search
                && !query.starts_with(&active_search.query)
            {
                active_search
                    .cancellation_token
                    .store(true, Ordering::Relaxed);
                st.active_search = None;
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
                #[expect(clippy::unwrap_used)]
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
                #[expect(clippy::unwrap_used)]
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
            let _guard = ActiveSearchGuard::new(search_state.clone(), cancellation_token.clone());
            let notify_flag = Arc::new(AtomicBool::new(false));
            let notify = {
                let flag = notify_flag.clone();
                Arc::new(move || {
                    flag.store(true, Ordering::Release);
                })
            };

            let mut manager = match file_search::SearchManager::new(
                &query,
                MAX_FILE_SEARCH_RESULTS,
                &search_dir,
                Vec::new(),
                NUM_FILE_SEARCH_THREADS,
                compute_indices,
                notify,
            ) {
                Ok(manager) => manager,
                Err(err) => {
                    tracing::error!("file search initialization failed: {err:?}");
                    tx.send(AppEvent::FileSearchResult {
                        query: query.clone(),
                        matches: Vec::new(),
                    });
                    return;
                }
            };

            let mut last_sent_paths: Vec<String> = Vec::new();
            let mut sent_once = false;
            let start = Instant::now();
            let mut last_progress = start;

            loop {
                if cancellation_token.load(Ordering::Relaxed) {
                    manager.cancel();
                }

                let status = manager.tick(SEARCH_MANAGER_TICK_TIMEOUT);
                let flag_was_set = notify_flag.swap(false, Ordering::AcqRel);
                let results = manager.current_results();
                let matches = results.matches;
                let paths: Vec<String> = matches.iter().map(|m| m.path.clone()).collect();

                let paths_changed = paths != last_sent_paths;
                let timeout_elapsed = start.elapsed() >= SEARCH_MANAGER_FIRST_RESULT_TIMEOUT;

                let should_emit = !cancellation_token.load(Ordering::Relaxed)
                    && (paths_changed
                        || (!sent_once
                            && (flag_was_set
                                || status.changed
                                || !status.running
                                || timeout_elapsed)));

                if should_emit {
                    tx.send(AppEvent::FileSearchResult {
                        query: query.clone(),
                        matches: matches.clone(),
                    });
                    sent_once = true;
                    last_sent_paths = paths;
                    last_progress = Instant::now();
                }

                if cancellation_token.load(Ordering::Relaxed) && sent_once {
                    break;
                }

                if !status.running && !flag_was_set {
                    if sent_once {
                        if last_progress.elapsed() >= SEARCH_MANAGER_FIRST_RESULT_TIMEOUT {
                            break;
                        }
                    } else if timeout_elapsed {
                        tx.send(AppEvent::FileSearchResult {
                            query: query.clone(),
                            matches,
                        });
                        break;
                    }
                }
            }
        });
    }
}
