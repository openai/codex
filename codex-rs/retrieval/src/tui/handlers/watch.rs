//! Watch view keyboard handler.

use std::sync::Arc;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use tokio_util::sync::CancellationToken;

use crate::tui::app::App;
use crate::tui::app_event::AppEvent;

/// Watch view keyboard handler trait.
pub trait WatchHandler {
    /// Handle keyboard events in the watch view.
    fn handle_watch_key(&mut self, key: KeyEvent);
}

impl WatchHandler for App {
    fn handle_watch_key(&mut self, key: KeyEvent) {
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
}

impl App {
    /// Toggle file watching on/off.
    pub(crate) fn toggle_file_watcher(&mut self) {
        if self.watcher_running {
            self.stop_file_watcher();
        } else {
            self.start_file_watcher();
        }
    }

    /// Start file watching.
    pub(crate) fn start_file_watcher(&mut self) {
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
    pub(crate) fn stop_file_watcher(&mut self) {
        // Cancel the watcher task via the stored token
        if let Some(token) = self.cancel_tokens.remove("watcher") {
            token.cancel();
        }
        // The task will emit WatchStopped when it exits
        self.watcher_running = false;
    }
}
