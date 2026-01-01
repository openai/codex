//! Index view keyboard handler.

use std::sync::Arc;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use tokio_util::sync::CancellationToken;

use crate::indexing::RebuildMode;
use crate::tui::app::App;
use crate::tui::app_event::AppEvent;

/// Index view keyboard handler trait.
pub trait IndexHandler {
    /// Handle keyboard events in the index view.
    fn handle_index_key(&mut self, key: KeyEvent);
}

impl IndexHandler for App {
    fn handle_index_key(&mut self, key: KeyEvent) {
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
}

impl App {
    /// Start an index build operation.
    pub(crate) fn start_index_build(&mut self, clean: bool) {
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
    pub(crate) fn cancel_index_build(&mut self) {
        if let Some(token) = self.cancel_tokens.remove("build") {
            token.cancel();
            self.index.progress.fail("Stopped by user".to_string());
        }
    }
}
