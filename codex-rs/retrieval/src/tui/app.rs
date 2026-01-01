//! TUI application state machine for retrieval.
//!
//! Manages the overall application state, view navigation, and event handling.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use crossterm::event::Event as CrosstermEvent;
use crossterm::event::EventStream;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;

use crate::config::RetrievalConfig;
use crate::event_emitter;
use crate::events::RetrievalEvent;
use crate::service::RetrievalService;

use super::app_event::AppEvent;
use super::app_event::ViewMode;
use super::app_event::keybindings;
// Re-export state types for backward compatibility
pub use super::app_state::IndexState;
pub use super::app_state::RepoMapState;
pub use super::app_state::SearchPipelineState;
pub use super::app_state::SearchStage;
pub use super::app_state::SearchState;
use super::handlers::DebugHandler;
use super::handlers::IndexHandler;
use super::handlers::RepoMapHandler;
use super::handlers::SearchHandler;
use super::handlers::WatchHandler;
use super::render::AppRenderer;
use super::terminal::Tui;
use super::widgets::EventLogState;

/// Main TUI application state.
pub struct App {
    /// Current view mode.
    pub view_mode: ViewMode,

    /// Configuration.
    pub config: RetrievalConfig,

    /// Retrieval service (lazy initialized).
    pub service: Option<Arc<RetrievalService>>,

    /// Event log widget state.
    pub event_log: EventLogState,

    /// Search state.
    pub search: SearchState,

    /// Index state.
    pub index: IndexState,

    /// RepoMap state.
    pub repomap: RepoMapState,

    /// Should quit.
    pub should_quit: bool,

    /// Show help overlay.
    pub show_help: bool,

    /// Error banner message (displayed at top of content area).
    pub error_banner: Option<String>,

    /// App event sender.
    pub(crate) event_tx: mpsc::Sender<AppEvent>,

    /// App event receiver.
    event_rx: mpsc::Receiver<AppEvent>,

    /// Start time for elapsed tracking.
    pub(crate) start_time: Instant,

    /// Whether watcher task is running (prevents starting multiple).
    pub(crate) watcher_running: bool,

    /// Whether index build task is running (prevents starting multiple).
    pub(crate) build_running: bool,

    /// Cancellation tokens for background tasks.
    pub(crate) cancel_tokens: HashMap<String, CancellationToken>,

    /// Last search time for debouncing.
    pub(crate) last_search_time: Option<Instant>,

    /// Status message (displayed in status bar, auto-clears).
    pub(crate) status_message: Option<String>,

    /// Search start time for elapsed display.
    pub(crate) search_start_time: Option<Instant>,

    /// Number of watched paths (for display).
    pub(crate) watched_path_count: i32,
}

impl App {
    /// Create a new app with the given configuration and optional service.
    ///
    /// # Arguments
    /// * `config` - Retrieval configuration
    /// * `service` - Optional RetrievalService for performing operations.
    ///   If None, the TUI will be display-only.
    pub fn new(config: RetrievalConfig, service: Option<Arc<RetrievalService>>) -> Self {
        let (event_tx, event_rx) = mpsc::channel(256);

        // Initialize search state with focus on input
        let mut search = SearchState::default();
        search.focus_input = true;
        search.input.focused = true;

        Self {
            view_mode: ViewMode::Search,
            config,
            service,
            event_log: EventLogState::new(),
            search,
            index: IndexState::default(),
            repomap: RepoMapState {
                max_tokens: 1024,
                ..Default::default()
            },
            should_quit: false,
            show_help: false,
            error_banner: None,
            event_tx,
            event_rx,
            start_time: Instant::now(),
            watcher_running: false,
            build_running: false,
            cancel_tokens: HashMap::new(),
            last_search_time: None,
            status_message: None,
            search_start_time: None,
            watched_path_count: 0,
        }
    }

    /// Get the event sender for sending events to the app.
    pub fn event_sender(&self) -> mpsc::Sender<AppEvent> {
        self.event_tx.clone()
    }

    /// Run the main application loop.
    pub async fn run(&mut self, terminal: &mut Tui) -> anyhow::Result<()> {
        // Subscribe to retrieval events
        let mut retrieval_rx = event_emitter::subscribe();
        let event_tx = self.event_tx.clone();

        // Spawn task to forward retrieval events
        tokio::spawn(async move {
            loop {
                match retrieval_rx.recv().await {
                    Ok(event) => {
                        if event_tx
                            .send(AppEvent::RetrievalEvent(event))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        });

        // Create crossterm event stream
        let mut event_stream = EventStream::new();

        // Spawn tick timer
        let tick_tx = self.event_tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(100));
            loop {
                interval.tick().await;
                if tick_tx.send(AppEvent::Tick).await.is_err() {
                    break;
                }
            }
        });

        // Main event loop
        loop {
            // Draw
            terminal.draw(|frame| self.render(frame.area(), frame.buffer_mut()))?;

            // Handle events
            tokio::select! {
                // Crossterm events (keyboard, mouse, resize)
                Some(event) = event_stream.next() => {
                    if let Ok(event) = event {
                        self.handle_crossterm_event(event);
                    }
                }
                // App events
                Some(event) = self.event_rx.recv() => {
                    self.handle_app_event(event).await;
                }
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    /// Handle a crossterm event.
    fn handle_crossterm_event(&mut self, event: CrosstermEvent) {
        match event {
            CrosstermEvent::Key(key) if key.kind == KeyEventKind::Press => {
                // Clear error banner on any key press
                self.error_banner = None;

                // Global keybindings
                if keybindings::is_quit(&key) {
                    // If search is running, Ctrl+C cancels it instead of quitting
                    if self.search.results.searching {
                        self.cancel_search();
                        return;
                    }
                    self.should_quit = true;
                    return;
                }

                if keybindings::is_help(&key) {
                    self.show_help = !self.show_help;
                    return;
                }

                if self.show_help {
                    if keybindings::is_escape(&key) || keybindings::is_enter(&key) {
                        self.show_help = false;
                    }
                    return;
                }

                // Tab navigation
                if keybindings::is_tab(&key) {
                    self.view_mode = self.view_mode.next();
                    return;
                }

                if keybindings::is_shift_tab(&key) {
                    self.view_mode = self.view_mode.prev();
                    return;
                }

                // Number key tab switching
                if let Some(idx) = keybindings::get_number_key(&key) {
                    if let Some(mode) = ViewMode::from_index(idx) {
                        self.view_mode = mode;
                    }
                    return;
                }

                // View-specific handling
                match self.view_mode {
                    ViewMode::Search => self.handle_search_key(key),
                    ViewMode::Index => self.handle_index_key(key),
                    ViewMode::RepoMap => self.handle_repomap_key(key),
                    ViewMode::Watch => self.handle_watch_key(key),
                    ViewMode::Debug => self.handle_debug_key(key),
                }
            }
            CrosstermEvent::Paste(text) => {
                if self.view_mode == ViewMode::Search && self.search.focus_input {
                    for c in text.chars() {
                        self.search.input.insert(c);
                    }
                }
            }
            CrosstermEvent::Resize(width, height) => {
                // Terminal will handle resize automatically
                let _ = (width, height);
            }
            _ => {}
        }
    }

    /// Handle an app event.
    async fn handle_app_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::RetrievalEvent(retrieval_event) => {
                self.event_log.push(retrieval_event.clone());
                self.handle_retrieval_event(retrieval_event);
            }
            AppEvent::SearchResults {
                query_id,
                results,
                duration_ms,
            } => {
                if self.search.query_id.as_ref() == Some(&query_id) {
                    // Convert SearchResult to SearchResultSummary
                    let summaries: Vec<_> = results
                        .into_iter()
                        .map(crate::events::SearchResultSummary::from)
                        .collect();
                    self.search.results.set_results(summaries, duration_ms);
                    self.search.error = None;
                }
            }
            AppEvent::SearchError { query_id, error } => {
                if self.search.query_id.as_ref() == Some(&query_id) {
                    self.search.error = Some(error);
                    self.search.results.searching = false;
                }
            }
            AppEvent::Tick => {
                // Handle periodic updates
            }
            AppEvent::Quit => {
                self.should_quit = true;
            }
            AppEvent::RepoMapGenerated {
                content,
                tokens,
                files,
                duration_ms,
            } => {
                self.repomap.content = Some(content);
                self.repomap.tokens = tokens;
                self.repomap.files = files;
                self.repomap.duration_ms = duration_ms;
                self.repomap.generating = false;
                self.repomap.scroll_offset = 0; // Reset scroll on new content
                self.status_message = None;
            }
            AppEvent::BuildError { error } => {
                self.index.progress.fail(error.clone());
                self.error_banner = Some(format!("Build failed: {}", error));
                self.build_running = false;
                self.status_message = None;
            }
            AppEvent::BuildCancelled => {
                self.index.progress.fail("Cancelled by user".to_string());
                self.build_running = false;
                self.status_message = None;
            }
            AppEvent::RepoMapError { error } => {
                self.error_banner = Some(format!("RepoMap failed: {}", error));
                self.repomap.generating = false;
                self.status_message = None;
            }
            AppEvent::WatchError { error } => {
                self.error_banner = Some(format!("Watch failed: {}", error));
                self.watcher_running = false;
                self.index.watching = false;
                self.status_message = None;
            }
            _ => {}
        }
    }

    /// Handle a retrieval system event.
    fn handle_retrieval_event(&mut self, event: RetrievalEvent) {
        match event {
            // Search pipeline events
            RetrievalEvent::SearchStarted {
                query_id, query, ..
            } => {
                self.search.query_id = Some(query_id);
                self.search.results.start_search();
                self.search.error = None;
                self.search.pipeline.start();
                self.search.pipeline.original_query = Some(query);
            }
            RetrievalEvent::QueryPreprocessed {
                query_id,
                duration_ms,
                ..
            } => {
                if self.search.query_id.as_ref() == Some(&query_id) {
                    self.search.pipeline.preprocess_duration_ms = Some(duration_ms);
                    self.search.pipeline.stage = SearchStage::QueryRewriting;
                }
            }
            RetrievalEvent::QueryRewritten {
                query_id,
                original,
                rewritten,
                expansions,
                duration_ms,
                ..
            } => {
                if self.search.query_id.as_ref() == Some(&query_id) {
                    self.search.pipeline.original_query = Some(original);
                    self.search.pipeline.rewritten_query = Some(rewritten);
                    self.search.pipeline.query_expansions = expansions;
                    self.search.pipeline.rewrite_duration_ms = Some(duration_ms);
                    self.search.pipeline.stage = SearchStage::Bm25Search;
                }
            }
            RetrievalEvent::Bm25SearchStarted { query_id, .. } => {
                if self.search.query_id.as_ref() == Some(&query_id) {
                    self.search.pipeline.stage = SearchStage::Bm25Search;
                }
            }
            RetrievalEvent::Bm25SearchCompleted {
                query_id,
                results,
                duration_ms,
            } => {
                if self.search.query_id.as_ref() == Some(&query_id) {
                    self.search.pipeline.bm25_count = Some(results.len() as i32);
                    self.search.pipeline.bm25_duration_ms = Some(duration_ms);
                    self.search.pipeline.stage = SearchStage::VectorSearch;
                }
            }
            RetrievalEvent::VectorSearchStarted { query_id, .. } => {
                if self.search.query_id.as_ref() == Some(&query_id) {
                    self.search.pipeline.stage = SearchStage::VectorSearch;
                }
            }
            RetrievalEvent::VectorSearchCompleted {
                query_id,
                results,
                duration_ms,
            } => {
                if self.search.query_id.as_ref() == Some(&query_id) {
                    self.search.pipeline.vector_count = Some(results.len() as i32);
                    self.search.pipeline.vector_duration_ms = Some(duration_ms);
                    self.search.pipeline.stage = SearchStage::SnippetSearch;
                }
            }
            RetrievalEvent::SnippetSearchStarted { query_id, .. } => {
                if self.search.query_id.as_ref() == Some(&query_id) {
                    self.search.pipeline.stage = SearchStage::SnippetSearch;
                }
            }
            RetrievalEvent::SnippetSearchCompleted {
                query_id,
                results,
                duration_ms,
            } => {
                if self.search.query_id.as_ref() == Some(&query_id) {
                    self.search.pipeline.snippet_count = Some(results.len() as i32);
                    self.search.pipeline.snippet_duration_ms = Some(duration_ms);
                    self.search.pipeline.stage = SearchStage::Fusion;
                }
            }
            RetrievalEvent::FusionStarted { query_id, .. } => {
                if self.search.query_id.as_ref() == Some(&query_id) {
                    self.search.pipeline.stage = SearchStage::Fusion;
                }
            }
            RetrievalEvent::FusionCompleted {
                query_id,
                merged_count,
                duration_ms,
            } => {
                if self.search.query_id.as_ref() == Some(&query_id) {
                    self.search.pipeline.fusion_count = Some(merged_count);
                    self.search.pipeline.fusion_duration_ms = Some(duration_ms);
                    self.search.pipeline.stage = SearchStage::Reranking;
                }
            }
            RetrievalEvent::RerankingStarted { query_id, .. } => {
                if self.search.query_id.as_ref() == Some(&query_id) {
                    self.search.pipeline.stage = SearchStage::Reranking;
                }
            }
            RetrievalEvent::RerankingCompleted {
                query_id,
                duration_ms,
                ..
            } => {
                if self.search.query_id.as_ref() == Some(&query_id) {
                    self.search.pipeline.rerank_duration_ms = Some(duration_ms);
                    self.search.pipeline.stage = SearchStage::Complete;
                }
            }
            RetrievalEvent::SearchCompleted {
                query_id,
                results,
                total_duration_ms,
                ..
            } => {
                if self.search.query_id.as_ref() == Some(&query_id) {
                    self.search.results.set_results(results, total_duration_ms);
                    self.search.pipeline.total_duration_ms = Some(total_duration_ms);
                    self.search.pipeline.stage = SearchStage::Complete;
                    self.status_message = None;
                    self.search_start_time = None;
                    // Remove the search cancellation token
                    self.cancel_tokens.remove("search");
                }
            }
            RetrievalEvent::SearchError {
                query_id, error, ..
            } => {
                if self.search.query_id.as_ref() == Some(&query_id) {
                    self.search.error = Some(error.clone());
                    self.search.results.searching = false;
                    self.search.pipeline.error = Some(error.clone());
                    self.search.pipeline.stage = SearchStage::Error;
                    self.error_banner = Some(format!("Search failed: {}. Press Enter to retry.", error));
                    self.status_message = None;
                    self.search_start_time = None;
                    // Remove the search cancellation token
                    self.cancel_tokens.remove("search");
                }
            }

            // Index events
            RetrievalEvent::IndexPhaseChanged {
                phase,
                progress,
                description,
                ..
            } => {
                self.index.progress.update(phase, progress, &description);
            }
            RetrievalEvent::IndexBuildStarted {
                estimated_files, ..
            } => {
                self.index.progress.start(estimated_files);
            }
            RetrievalEvent::IndexBuildCompleted {
                stats, duration_ms, ..
            } => {
                self.index.progress.complete(duration_ms);
                self.index
                    .stats
                    .set_stats(stats.file_count, stats.chunk_count, stats.symbol_count);
                self.build_running = false;
                self.status_message = None;
            }
            RetrievalEvent::IndexBuildFailed { error, .. } => {
                self.index.progress.fail(error.clone());
                self.error_banner = Some(format!("Index build failed: {}", error));
                self.build_running = false;
                self.status_message = None;
            }

            // Watch events
            RetrievalEvent::WatchStarted { .. } => {
                self.index.watching = true;
                self.index.stats.is_watching = true;
                self.watcher_running = true;
            }
            RetrievalEvent::WatchStopped { .. } => {
                self.index.watching = false;
                self.index.stats.is_watching = false;
                self.watcher_running = false;
            }

            // RepoMap events
            RetrievalEvent::RepoMapGenerated {
                tokens,
                files,
                duration_ms,
                ..
            } => {
                self.repomap.tokens = tokens;
                self.repomap.files = files;
                self.repomap.duration_ms = duration_ms;
                self.repomap.generating = false;
            }

            _ => {}
        }
    }

    /// Open a file in the default editor.
    pub(crate) fn open_file_in_editor(&self, filepath: &str, line: i32) {
        // Try $EDITOR first, fall back to common editors
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| {
            // Platform-specific defaults
            if cfg!(target_os = "macos") {
                "open".to_string()
            } else if cfg!(target_os = "windows") {
                "notepad".to_string()
            } else {
                "vi".to_string()
            }
        });

        let result = if editor == "open" {
            // macOS: use open command
            std::process::Command::new("open").arg(filepath).spawn()
        } else if editor.contains("vim") || editor.contains("vi") || editor.contains("nvim") {
            // vim-like editors: use +line syntax
            std::process::Command::new(&editor)
                .arg(format!("+{}", line))
                .arg(filepath)
                .spawn()
        } else if editor.contains("code") || editor.contains("vscode") {
            // VSCode: use -g syntax
            std::process::Command::new(&editor)
                .arg("-g")
                .arg(format!("{}:{}", filepath, line))
                .spawn()
        } else if editor.contains("emacs") {
            // Emacs: use +line syntax
            std::process::Command::new(&editor)
                .arg(format!("+{}", line))
                .arg(filepath)
                .spawn()
        } else {
            // Generic: just open the file
            std::process::Command::new(&editor).arg(filepath).spawn()
        };

        if let Err(e) = result {
            tracing::error!("Failed to open editor '{}': {}", editor, e);
        }
    }

    /// Render the application.
    fn render(&self, area: Rect, buf: &mut Buffer) {
        // Layout: tabs at top, error banner (if any), content, status at bottom
        let has_error = self.error_banner.is_some();
        let constraints = if has_error {
            vec![
                Constraint::Length(3), // Tabs
                Constraint::Length(2), // Error banner
                Constraint::Min(10),   // Content
                Constraint::Length(1), // Status bar
            ]
        } else {
            vec![
                Constraint::Length(3), // Tabs
                Constraint::Min(10),   // Content
                Constraint::Length(1), // Status bar
            ]
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        // Render tabs
        self.render_tabs(chunks[0], buf);

        // Render error banner if present
        let content_idx = if has_error {
            self.render_error_banner(chunks[1], buf);
            2
        } else {
            1
        };

        // Render current view
        match self.view_mode {
            ViewMode::Search => self.render_search_view(chunks[content_idx], buf),
            ViewMode::Index => self.render_index_view(chunks[content_idx], buf),
            ViewMode::RepoMap => self.render_repomap_view(chunks[content_idx], buf),
            ViewMode::Watch => self.render_watch_view(chunks[content_idx], buf),
            ViewMode::Debug => self.render_debug_view(chunks[content_idx], buf),
        }

        // Render status bar
        let status_idx = if has_error { 3 } else { 2 };
        self.render_status_bar(chunks[status_idx], buf);

        // Render help overlay if active
        if self.show_help {
            self.render_help_overlay(area, buf);
        }
    }

}
