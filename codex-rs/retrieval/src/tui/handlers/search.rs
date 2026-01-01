//! Search view keyboard handler.

use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use tokio_util::sync::CancellationToken;

use crate::tui::app::App;
use crate::tui::app_event::AppEvent;
use crate::tui::constants::SEARCH_DEBOUNCE_MS;
use crate::tui::constants::SEARCH_TIMEOUT_SECS;

/// Search view keyboard handler trait.
pub trait SearchHandler {
    /// Handle keyboard events in the search view.
    fn handle_search_key(&mut self, key: KeyEvent);
}

impl SearchHandler for App {
    fn handle_search_key(&mut self, key: KeyEvent) {
        if self.search.focus_input {
            // Input is focused - handle text editing
            match key.code {
                // Ctrl+P/N for history navigation (must come before general Char handling)
                KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.search.input.prev_history();
                }
                KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.search.input.next_history();
                }
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
                    // Switch focus to results (if results exist)
                    if !self.search.results.results.is_empty() {
                        self.search.focus_input = false;
                        self.search.input.focused = false;
                        self.search.results.focused = true;
                    }
                }
                KeyCode::Down => {
                    // Switch focus to results (if results exist)
                    if !self.search.results.results.is_empty() {
                        self.search.focus_input = false;
                        self.search.input.focused = false;
                        self.search.results.focused = true;
                    }
                }
                KeyCode::Enter => {
                    self.execute_search();
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
                KeyCode::Char('r') => {
                    // Retry last search (re-focus input and press enter equivalent)
                    self.search.focus_input = true;
                    self.search.input.focused = true;
                    self.search.results.focused = false;
                    // Clear error and trigger search by simulating Enter
                    self.search.error = None;
                    self.error_banner = None;
                }
                KeyCode::Enter => {
                    // Open selected result in editor
                    if let Some(result) = self.search.results.selected() {
                        let filepath = result.filepath.clone();
                        let line = result.start_line;
                        self.open_file_in_editor(&filepath, line);
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
}

impl App {
    /// Execute a search with the current query.
    pub(crate) fn execute_search(&mut self) {
        // Trigger search if we have a service and query
        let query = self.search.input.query.clone();
        if query.is_empty() {
            return;
        }

        // Debounce: prevent rapid-fire searches
        if let Some(last_time) = self.last_search_time {
            if last_time.elapsed() < Duration::from_millis(SEARCH_DEBOUNCE_MS) {
                return;
            }
        }
        self.last_search_time = Some(Instant::now());

        let Some(ref service) = self.service else {
            self.error_banner = Some(
                "Service unavailable - index may be building or disabled. Press 'b' in Index view to build.".to_string(),
            );
            return;
        };

        // Cancel any existing search
        if let Some(token) = self.cancel_tokens.remove("search") {
            token.cancel();
        }

        // Create cancellation token for this search
        let cancel_token = CancellationToken::new();
        self.cancel_tokens
            .insert("search".to_string(), cancel_token.clone());

        // Push to history before searching
        self.search.input.push_history();

        let mode = self.search.input.mode;
        self.search.results.start_search();
        self.search.error = None;
        self.search_start_time = Some(Instant::now());
        self.status_message = Some(format!("Searching: {} (Ctrl+C to cancel)", query));

        let service = Arc::clone(service);
        let limit = self.config.search.n_final;
        let event_tx = self.event_tx.clone();
        let query_id = self.search.query_id.clone();

        // Spawn search task with timeout
        tokio::spawn(async move {
            let search_future = async {
                match mode {
                    crate::events::SearchMode::Hybrid | crate::events::SearchMode::Snippet => {
                        service.search_with_limit(&query, Some(limit)).await
                    }
                    crate::events::SearchMode::Bm25 => service.search_bm25(&query, limit).await,
                    crate::events::SearchMode::Vector => {
                        service.search_vector(&query, limit).await
                    }
                }
            };

            tokio::select! {
                _ = cancel_token.cancelled() => {
                    // Cancelled by user, do nothing (already handled in cancel_search)
                }
                result = tokio::time::timeout(
                    Duration::from_secs(SEARCH_TIMEOUT_SECS),
                    search_future
                ) => {
                    match result {
                        Ok(_) => {
                            // Results arrive via event emitter -> SearchCompleted/SearchError
                        }
                        Err(_) => {
                            // Timeout - send error event
                            let _ = event_tx.send(AppEvent::SearchError {
                                query_id: query_id.unwrap_or_default(),
                                error: format!("Search timed out after {}s. Try a more specific query.", SEARCH_TIMEOUT_SECS),
                            }).await;
                        }
                    }
                }
            }
        });
    }

    /// Cancel the current search.
    pub(crate) fn cancel_search(&mut self) {
        if let Some(token) = self.cancel_tokens.remove("search") {
            token.cancel();
        }
        self.search.results.searching = false;
        self.search.pipeline.stage = crate::tui::app_state::SearchStage::Idle;
        self.search.error = None;
        self.search_start_time = None;
        self.status_message = Some("Search cancelled".to_string());
    }
}
