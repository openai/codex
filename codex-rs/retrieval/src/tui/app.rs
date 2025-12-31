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
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Tabs;
use ratatui::widgets::Widget;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;

use crate::config::RetrievalConfig;
use crate::event_emitter;
use crate::events::RetrievalEvent;
use crate::indexing::RebuildMode;
use crate::repomap::RepoMapRequest;
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
use super::terminal::Tui;
use super::views::DebugView;
use super::views::IndexView;
use super::views::RepoMapView;
use super::views::SearchView;
use super::views::WatchView;
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
    event_tx: mpsc::Sender<AppEvent>,

    /// App event receiver.
    event_rx: mpsc::Receiver<AppEvent>,

    /// Start time for elapsed tracking.
    start_time: Instant,

    /// Whether watcher task is running (prevents starting multiple).
    watcher_running: bool,

    /// Whether index build task is running (prevents starting multiple).
    build_running: bool,

    /// Cancellation tokens for background tasks.
    cancel_tokens: HashMap<String, CancellationToken>,

    /// Last search time for debouncing.
    last_search_time: Option<Instant>,

    /// Status message (displayed in status bar, auto-clears).
    status_message: Option<String>,
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
                    self.error_banner = Some(format!("Search failed: {}", error));
                    self.status_message = None;
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

    /// Handle search view key events.
    fn handle_search_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyModifiers;

        if self.search.focus_input {
            // Input is focused - handle text editing
            match key.code {
                KeyCode::Char(c) => {
                    self.search.input.reset_history_navigation();
                    self.search.input.insert(c);
                }
                KeyCode::Backspace => {
                    self.search.input.reset_history_navigation();
                    self.search.input.backspace();
                }
                KeyCode::Delete => {
                    self.search.input.reset_history_navigation();
                    self.search.input.delete();
                }
                KeyCode::Left => {
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        // Ctrl+Left: toggle search mode left
                        self.search.input.prev_mode();
                    } else {
                        self.search.input.move_left();
                    }
                }
                KeyCode::Right => {
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        // Ctrl+Right: toggle search mode right
                        self.search.input.next_mode();
                    } else {
                        self.search.input.move_right();
                    }
                }
                KeyCode::Home => {
                    self.search.input.move_start();
                }
                KeyCode::End => {
                    self.search.input.move_end();
                }
                KeyCode::Up => {
                    // Navigate history or switch to results
                    if self.search.input.history.is_empty()
                        && !self.search.results.results.is_empty()
                    {
                        // No history, switch to results
                        self.search.focus_input = false;
                        self.search.input.focused = false;
                        self.search.results.focused = true;
                    } else {
                        // Navigate to previous (older) history entry
                        self.search.input.prev_history();
                    }
                }
                KeyCode::Down => {
                    // Navigate history or switch to results
                    if self.search.input.is_navigating_history() {
                        // Navigate to next (newer) history entry
                        self.search.input.next_history();
                    } else if !self.search.results.results.is_empty() {
                        // Not navigating history, switch to results
                        self.search.focus_input = false;
                        self.search.input.focused = false;
                        self.search.results.focused = true;
                    }
                }
                KeyCode::Enter => {
                    // Trigger search if we have a service and query
                    let query = self.search.input.query.clone();
                    if query.is_empty() {
                        return;
                    }

                    // Debounce: prevent rapid-fire searches (200ms minimum interval)
                    const DEBOUNCE_MS: u64 = 200;
                    if let Some(last_time) = self.last_search_time {
                        if last_time.elapsed() < Duration::from_millis(DEBOUNCE_MS) {
                            return;
                        }
                    }
                    self.last_search_time = Some(Instant::now());

                    let Some(ref service) = self.service else {
                        self.error_banner = Some(
                            "Service unavailable - index may be building or disabled".to_string(),
                        );
                        return;
                    };

                    // Push to history before searching
                    self.search.input.push_history();

                    let mode = self.search.input.mode;
                    self.search.results.start_search();
                    self.search.error = None;
                    self.status_message = Some(format!("Searching: {}", query));

                    let service = Arc::clone(service);
                    let limit = self.config.search.n_final;

                    // Spawn search task - results come via event emitter
                    tokio::spawn(async move {
                        let _ = match mode {
                            crate::events::SearchMode::Hybrid
                            | crate::events::SearchMode::Snippet => {
                                // Hybrid includes snippet search
                                service.search_with_limit(&query, Some(limit)).await
                            }
                            crate::events::SearchMode::Bm25 => {
                                service.search_bm25(&query, limit).await
                            }
                            crate::events::SearchMode::Vector => {
                                service.search_vector(&query, limit).await
                            }
                        };
                        // Results arrive via event emitter -> SearchCompleted/SearchError
                    });
                }
                KeyCode::Esc => {
                    self.search.input.reset_history_navigation();
                    self.search.input.clear();
                }
                _ => {}
            }
        } else {
            // Results are focused - handle list navigation
            match key.code {
                KeyCode::Up => {
                    self.search.results.select_previous();
                }
                KeyCode::Down => {
                    self.search.results.select_next();
                }
                KeyCode::PageUp => {
                    self.search.results.page_up();
                }
                KeyCode::PageDown => {
                    self.search.results.page_down();
                }
                KeyCode::Home => {
                    self.search.results.select_first();
                }
                KeyCode::End => {
                    self.search.results.select_last();
                }
                KeyCode::Char('/') | KeyCode::Char('i') => {
                    // Switch focus back to input
                    self.search.focus_input = true;
                    self.search.input.focused = true;
                    self.search.results.focused = false;
                }
                KeyCode::Enter => {
                    // Open selected result in editor
                    if let Some(result) = self.search.results.selected() {
                        let filepath = &result.filepath;
                        let line = result.start_line;
                        self.open_file_in_editor(filepath, line);
                    }
                }
                KeyCode::Esc => {
                    // Switch focus back to input
                    self.search.focus_input = true;
                    self.search.input.focused = true;
                    self.search.results.focused = false;
                }
                _ => {}
            }
        }
    }

    /// Handle index view key events.
    fn handle_index_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Char('b') => {
                // Start incremental build
                self.start_index_build(false);
            }
            KeyCode::Char('c') => {
                // Start clean build
                self.start_index_build(true);
            }
            KeyCode::Char('w') => {
                // Toggle watch mode
                self.toggle_file_watcher();
            }
            KeyCode::Char('s') => {
                // Stop build - cancel via token
                self.cancel_index_build();
            }
            _ => {}
        }
    }

    /// Start an index build operation.
    fn start_index_build(&mut self, clean: bool) {
        if self.index.progress.in_progress || self.build_running {
            return; // Already building
        }

        let Some(ref service) = self.service else {
            self.error_banner = Some("Service unavailable".to_string());
            return;
        };

        // Cancel any existing build first
        if let Some(token) = self.cancel_tokens.remove("build") {
            token.cancel();
        }

        // Create cancellation token for this build
        let cancel_token = CancellationToken::new();
        self.cancel_tokens
            .insert("build".to_string(), cancel_token.clone());

        self.build_running = true;
        self.index.progress.start(0);
        self.status_message = Some(if clean {
            "Starting clean rebuild...".to_string()
        } else {
            "Starting incremental build...".to_string()
        });

        let service = Arc::clone(service);
        let event_tx = self.event_tx.clone();
        let mode = if clean {
            RebuildMode::Clean
        } else {
            RebuildMode::Incremental
        };

        tokio::spawn(async move {
            match service.build_index(mode, cancel_token).await {
                Ok(mut rx) => {
                    // Progress updates are emitted via event_emitter and handled
                    // by the TUI's handle_retrieval_event()
                    // Just drain the progress receiver
                    while let Some(_progress) = rx.recv().await {
                        // Progress events arrive via event_emitter -> AppEvent::RetrievalEvent
                    }
                }
                Err(e) => {
                    tracing::error!("Index build failed: {}", e);
                    let _ = event_tx
                        .send(AppEvent::BuildError {
                            error: e.to_string(),
                        })
                        .await;
                }
            }
        });
    }

    /// Cancel the current index build.
    fn cancel_index_build(&mut self) {
        if let Some(token) = self.cancel_tokens.remove("build") {
            token.cancel();
            self.index.progress.fail("Stopped by user".to_string());
        }
    }

    /// Handle repomap view key events.
    fn handle_repomap_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Char('g') => {
                // Generate repomap
                self.generate_repomap();
            }
            KeyCode::Char('r') => {
                // Refresh (regenerate) repomap
                self.generate_repomap();
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.repomap.max_tokens = (self.repomap.max_tokens + 256).min(8192);
            }
            KeyCode::Char('-') => {
                self.repomap.max_tokens = (self.repomap.max_tokens - 256).max(256);
            }
            KeyCode::Up => {
                self.repomap.scroll_offset = (self.repomap.scroll_offset - 1).max(0);
            }
            KeyCode::Down => {
                self.repomap.scroll_offset += 1;
            }
            KeyCode::PageUp => {
                self.repomap.scroll_offset = (self.repomap.scroll_offset - 10).max(0);
            }
            KeyCode::PageDown => {
                self.repomap.scroll_offset += 10;
            }
            KeyCode::Home => {
                self.repomap.scroll_offset = 0;
            }
            _ => {}
        }
    }

    /// Generate a repomap.
    fn generate_repomap(&mut self) {
        if self.repomap.generating {
            return; // Already generating
        }

        let Some(ref service) = self.service else {
            self.error_banner = Some("Service unavailable".to_string());
            return;
        };

        self.repomap.generating = true;
        self.repomap.content = None;
        self.status_message = Some(format!(
            "Generating repomap ({} tokens)...",
            self.repomap.max_tokens
        ));

        let service = Arc::clone(service);
        let max_tokens = self.repomap.max_tokens;
        let event_tx = self.event_tx.clone();

        tokio::spawn(async move {
            let request = RepoMapRequest {
                chat_files: vec![],
                max_tokens,
                ..Default::default()
            };

            match service.generate_repomap(request).await {
                Ok(result) => {
                    let _ = event_tx
                        .send(AppEvent::RepoMapGenerated {
                            content: result.content,
                            tokens: result.tokens,
                            files: result.files_included,
                            duration_ms: result.generation_time_ms,
                        })
                        .await;
                }
                Err(e) => {
                    tracing::error!("Repomap generation failed: {}", e);
                    let _ = event_tx
                        .send(AppEvent::RepoMapError {
                            error: e.to_string(),
                        })
                        .await;
                }
            }
        });
    }

    /// Open a file in the default editor.
    fn open_file_in_editor(&self, filepath: &str, line: i32) {
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

    /// Handle watch view key events.
    fn handle_watch_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Char('w') => {
                // Toggle watch mode (same as index view)
                self.toggle_file_watcher();
            }
            KeyCode::Char('c') => {
                // Clear event log
                self.event_log.clear();
            }
            _ => {}
        }
    }

    /// Handle debug view key events.
    fn handle_debug_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::PageUp => {
                self.event_log.scroll_up(10);
            }
            KeyCode::PageDown => {
                self.event_log.scroll_down(10);
            }
            KeyCode::Up => {
                self.event_log.scroll_up(1);
            }
            KeyCode::Down => {
                self.event_log.scroll_down(1);
            }
            KeyCode::Home => {
                self.event_log.scroll_to_top();
            }
            KeyCode::End => {
                self.event_log.scroll_to_bottom();
            }
            KeyCode::Char('c') => {
                // Clear event log
                self.event_log.clear();
            }
            KeyCode::Char('a') => {
                // Toggle auto-scroll
                self.event_log.toggle_auto_scroll();
            }
            _ => {}
        }
    }

    /// Toggle file watching on/off.
    fn toggle_file_watcher(&mut self) {
        if self.watcher_running {
            self.stop_file_watcher();
        } else {
            self.start_file_watcher();
        }
    }

    /// Start file watching.
    fn start_file_watcher(&mut self) {
        if self.watcher_running {
            return; // Already running
        }

        let Some(ref service) = self.service else {
            self.error_banner = Some("Service unavailable".to_string());
            return;
        };

        // Create cancellation token for this watcher
        let cancel_token = CancellationToken::new();
        self.cancel_tokens
            .insert("watcher".to_string(), cancel_token.clone());
        self.watcher_running = true;
        self.index.watching = true;
        self.status_message = Some("Starting file watcher...".to_string());

        let service = Arc::clone(service);
        let event_tx = self.event_tx.clone();

        tokio::spawn(async move {
            match service.start_watch(cancel_token).await {
                Ok(mut rx) => {
                    // Watch events and incremental reindexing are handled by the service
                    // Events are emitted via event_emitter and received by TUI's handle_retrieval_event()
                    // Just drain the watch event receiver
                    while let Some(_event) = rx.recv().await {
                        // Watch events arrive via event_emitter -> AppEvent::RetrievalEvent
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to start watcher: {}", e);
                    let _ = event_tx
                        .send(AppEvent::WatchError {
                            error: e.to_string(),
                        })
                        .await;
                }
            }
        });
    }

    /// Stop file watching.
    fn stop_file_watcher(&mut self) {
        // Cancel the watcher task via the stored token
        if let Some(token) = self.cancel_tokens.remove("watcher") {
            token.cancel();
        }
        // The task will emit WatchStopped when it exits
        self.watcher_running = false;
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

    /// Render the error banner.
    fn render_error_banner(&self, area: Rect, buf: &mut Buffer) {
        if let Some(ref error) = self.error_banner {
            let error_text = format!(" Error: {} ", error);
            let banner = Paragraph::new(error_text).style(Style::default().red().bold().reversed());
            banner.render(area, buf);
        }
    }

    /// Render the tab bar.
    fn render_tabs(&self, area: Rect, buf: &mut Buffer) {
        let titles: Vec<Line> = ViewMode::all()
            .iter()
            .enumerate()
            .map(|(i, mode)| {
                let num = format!("{}", i + 1);
                Line::from(vec![
                    Span::raw("["),
                    Span::raw(num).cyan(),
                    Span::raw("] "),
                    Span::raw(mode.name()),
                ])
            })
            .collect();

        let tabs = Tabs::new(titles)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Retrieval TUI "),
            )
            .select(self.view_mode.index())
            .highlight_style(ratatui::style::Style::default().bold().reversed());

        tabs.render(area, buf);
    }

    /// Render the search view.
    fn render_search_view(&self, area: Rect, buf: &mut Buffer) {
        let view = SearchView::new(&self.search, &self.event_log);
        view.render(area, buf);
    }

    /// Render the index view.
    fn render_index_view(&self, area: Rect, buf: &mut Buffer) {
        let view = IndexView::new(&self.index, &self.event_log);
        view.render(area, buf);
    }

    /// Render the repomap view.
    fn render_repomap_view(&self, area: Rect, buf: &mut Buffer) {
        let view = RepoMapView::new(&self.repomap);
        view.render(area, buf);
    }

    /// Render the watch view.
    fn render_watch_view(&self, area: Rect, buf: &mut Buffer) {
        let view = WatchView::new(self.index.watching, &self.event_log);
        view.render(area, buf);
    }

    /// Render the debug view.
    fn render_debug_view(&self, area: Rect, buf: &mut Buffer) {
        let view = DebugView::new(&self.event_log);
        view.render(area, buf);
    }

    /// Render the status bar.
    fn render_status_bar(&self, area: Rect, buf: &mut Buffer) {
        let elapsed = self.start_time.elapsed();

        // Show status message if present, otherwise show keybindings
        let status = if let Some(ref msg) = self.status_message {
            format!(" {} | Elapsed: {:.1}s ", msg, elapsed.as_secs_f64())
        } else {
            format!(
                " Tab/Shift+Tab: navigate | 1-5: switch view | ?: help | q: quit | Elapsed: {:.1}s ",
                elapsed.as_secs_f64()
            )
        };

        let style = if self.status_message.is_some() {
            ratatui::style::Style::default().yellow().reversed()
        } else {
            ratatui::style::Style::default().reversed()
        };

        let status_bar = Paragraph::new(status).style(style);
        status_bar.render(area, buf);
    }

    /// Render the help overlay.
    fn render_help_overlay(&self, area: Rect, buf: &mut Buffer) {
        let help_text = r#"
 Keyboard Shortcuts
 ==================

 Global:
   q, Ctrl+C    Quit
   ?            Toggle help
   Tab          Next view
   Shift+Tab    Previous view
   1-5          Switch to view

 Search View (Input):
   Type         Enter query
   Enter        Execute search
   Up/Down      Switch to results
   Ctrl+←/→     Change search mode
   Home/End     Cursor start/end
   Esc          Clear query

 Search View (Results):
   Up/Down      Navigate results
   Enter        Open file in editor
   PgUp/PgDn    Page through results
   / or i       Focus input

 Index View:
   b            Build index
   c            Clean rebuild
   w            Toggle watch mode
   s            Stop current build

 RepoMap View:
   g            Generate map
   r            Refresh
   +/-          Adjust token budget

 Watch View:
   w            Toggle watch mode
   c            Clear event log

 Debug View:
   Up/Down      Scroll events
   PgUp/PgDn    Scroll page
   Home/End     Jump to start/end
   c            Clear event log
   a            Toggle auto-scroll

 Press Escape or Enter to close
"#;

        // Center the help overlay
        let help_width = 42;
        let help_height = 46;
        let x = (area.width.saturating_sub(help_width)) / 2;
        let y = (area.height.saturating_sub(help_height)) / 2;
        let help_area = Rect::new(
            x,
            y,
            help_width.min(area.width),
            help_height.min(area.height),
        );

        // Clear background with reversed style (theme-aware)
        let bg_style = Style::default().reversed();
        for y in help_area.y..help_area.y + help_area.height {
            for x in help_area.x..help_area.x + help_area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                    cell.set_style(bg_style);
                }
            }
        }

        let help = Paragraph::new(help_text)
            .block(Block::default().borders(Borders::ALL).title(" Help "))
            .style(bg_style);
        help.render(help_area, buf);
    }
}
