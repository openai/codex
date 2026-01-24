//! RepoMap view keyboard handler.

use std::sync::Arc;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;

use crate::repomap::RepoMapRequest;
use crate::tui::app::App;
use crate::tui::app_event::AppEvent;

/// RepoMap view keyboard handler trait.
pub trait RepoMapHandler {
    /// Handle keyboard events in the repomap view.
    fn handle_repomap_key(&mut self, key: KeyEvent);
}

impl RepoMapHandler for App {
    fn handle_repomap_key(&mut self, key: KeyEvent) {
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
}

impl App {
    /// Generate a repomap.
    pub(crate) fn generate_repomap(&mut self) {
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
}
