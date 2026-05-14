//! Session-based orchestration for `@` file searches.
//!
//! `ChatComposer` publishes every change of the `@token` as
//! `AppEvent::StartFileSearch(query)`. This manager owns a single
//! `codex-file-search` session for the current search root, updates the query
//! on every keystroke, and drops the session when the query becomes empty.

use codex_file_search as file_search;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;

pub(crate) struct FileSearchManager {
    state: Arc<Mutex<SearchState>>,
    search_dir: PathBuf,
    app_tx: AppEventSender,
}

struct SearchState {
    latest_query: String,
    latest_query_started_at: Option<Instant>,
    session: Option<file_search::FileSearchSession>,
    session_token: usize,
}

impl FileSearchManager {
    pub fn new(search_dir: PathBuf, tx: AppEventSender) -> Self {
        Self {
            state: Arc::new(Mutex::new(SearchState {
                latest_query: String::new(),
                latest_query_started_at: None,
                session: None,
                session_token: 0,
            })),
            search_dir,
            app_tx: tx,
        }
    }

    /// Updates the directory used for file searches.
    /// This should be called when the session's CWD changes on resume.
    /// Drops the current session so it will be recreated with the new directory on next query.
    pub fn update_search_dir(&mut self, new_dir: PathBuf) {
        self.search_dir = new_dir;
        #[expect(clippy::unwrap_used)]
        let mut st = self.state.lock().unwrap();
        st.session.take();
        st.latest_query.clear();
        st.latest_query_started_at = None;
    }

    /// Call whenever the user edits the `@` token.
    pub fn on_user_query(&self, query: String) {
        #[expect(clippy::unwrap_used)]
        let mut st = self.state.lock().unwrap();
        if query == st.latest_query {
            return;
        }
        st.latest_query.clear();
        st.latest_query.push_str(&query);
        st.latest_query_started_at = Some(Instant::now());

        if query.is_empty() {
            tracing::info!("TUI @ file search query cleared");
            st.latest_query_started_at = None;
            st.session.take();
            return;
        }

        let session_was_missing = st.session.is_none();
        if session_was_missing {
            self.start_session_locked(&mut st);
        }
        if let Some(session) = st.session.as_ref() {
            let update_started_at = Instant::now();
            session.update_query(&query);
            tracing::info!(
                elapsed_us = update_started_at.elapsed().as_micros(),
                query_len = query.len(),
                session_was_missing,
                "TUI @ file search query dispatched"
            );
        }
    }

    fn start_session_locked(&self, st: &mut SearchState) {
        let started_at = Instant::now();
        st.session_token = st.session_token.wrapping_add(1);
        let session_token = st.session_token;
        let reporter = Arc::new(TuiSessionReporter {
            state: self.state.clone(),
            app_tx: self.app_tx.clone(),
            session_token,
        });
        let session = file_search::create_session(
            vec![self.search_dir.clone()],
            file_search::FileSearchOptions {
                compute_indices: true,
                ..Default::default()
            },
            reporter,
            /*cancel_flag*/ None,
        );
        match session {
            Ok(session) => {
                tracing::info!(
                    elapsed_ms = started_at.elapsed().as_millis(),
                    session_token,
                    root_count = 1,
                    "TUI @ file search session started"
                );
                st.session = Some(session);
            }
            Err(err) => {
                tracing::warn!(
                    elapsed_ms = started_at.elapsed().as_millis(),
                    root_count = 1,
                    "file search session failed to start: {err}"
                );
                st.session = None;
            }
        }
    }
}

struct TuiSessionReporter {
    state: Arc<Mutex<SearchState>>,
    app_tx: AppEventSender,
    session_token: usize,
}

impl TuiSessionReporter {
    fn send_snapshot(&self, snapshot: &file_search::FileSearchSnapshot) {
        #[expect(clippy::unwrap_used)]
        let st = self.state.lock().unwrap();
        let elapsed_ms = st
            .latest_query_started_at
            .map(|started_at| started_at.elapsed().as_millis());
        if st.session_token != self.session_token
            || st.latest_query.is_empty()
            || snapshot.query.is_empty()
        {
            return;
        }
        let query = snapshot.query.clone();
        let match_count = snapshot.matches.len();
        drop(st);
        tracing::info!(
            elapsed_ms = ?elapsed_ms,
            query_len = query.len(),
            match_count,
            "TUI @ file search snapshot delivered"
        );
        self.app_tx.send(AppEvent::FileSearchResult {
            query,
            matches: snapshot.matches.clone(),
        });
    }
}

impl file_search::SessionReporter for TuiSessionReporter {
    fn on_update(&self, snapshot: &file_search::FileSearchSnapshot) {
        self.send_snapshot(snapshot);
    }

    fn on_complete(&self) {}
}
