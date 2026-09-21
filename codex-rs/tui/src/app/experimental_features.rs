//! Server-backed feature persistence. Configured readback never replaces task state,
//! and an accepted save finishes even if its popup closes or the user navigates.

use super::config_persistence::overridden_write_message;
use super::*;
use crate::experimental_features::FeatureWriteResult;
use tokio::sync::oneshot;

impl App {
    pub(super) async fn enable_feature_for_new_threads(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &AppServerSession,
        feature: Feature,
    ) {
        let label = match feature {
            Feature::Collab => "Subagents",
            Feature::MemoryTool => "Memories",
            _ => return,
        };
        let mut edits = vec![crate::config_update::build_feature_enabled_edit(
            feature.key(),
            /*enabled*/ true,
        )];
        if feature == Feature::MemoryTool {
            // Older app servers still use the legacy key for this feature.
            edits.push(crate::config_update::build_feature_enabled_edit(
                "memory_tool",
                /*enabled*/ true,
            ));
        }
        let notice: Box<dyn HistoryCell> = match crate::config_update::write_config_batch(
            app_server.request_handle(),
            edits,
        )
        .await
        {
            Ok(response) if response.status == WriteStatus::Ok => {
                Box::new(history_cell::new_warning_event(format!(
                    "{label} setting saved on the server for new threads. This thread is unchanged. Project or task settings may override it."
                )))
            }
            Ok(response) => {
                self.transcript_view.jump_to_latest();
                Box::new(history_cell::new_error_event(format!(
                    "{label} setting was saved but is overridden: {}",
                    overridden_write_message(&response)
                )))
            }
            Err(err) => {
                self.transcript_view.jump_to_latest();
                Box::new(history_cell::new_error_event(format!(
                    "Failed to save {label} setting: {}",
                    crate::config_update::format_config_error(&err)
                )))
            }
        };
        self.insert_history_cell(tui, notice);
    }

    pub(super) fn fetch_experimental_features(
        &self,
        app_server: &AppServerSession,
        thread_id: ThreadId,
        mut response_tx: oneshot::Sender<
            Result<Vec<codex_app_server_protocol::ExperimentalFeature>, String>,
        >,
    ) {
        let lock = self.feature_write_lock.clone();
        let handle = app_server.request_handle();
        tokio::spawn(async move {
            // A reopened popup must discover values after the outstanding save.
            tokio::select! {
                _ = response_tx.closed() => {},
                _guard = lock.lock() => crate::experimental_features::fetch(
                    handle, Some(thread_id), "tui-experimental-features", response_tx,
                ),
            }
        });
    }

    pub(super) fn save_experimental_features(
        &mut self,
        app_server: &AppServerSession,
        thread_id: ThreadId,
        updates: Vec<(String, bool)>,
        response_tx: oneshot::Sender<Result<FeatureWriteResult, String>>,
    ) {
        let Ok(guard) = self.feature_write_lock.clone().try_lock_owned() else {
            let error =
                "An experimental feature save is still in progress. Retry after it finishes.";
            self.chat_widget.add_warning_message(error.to_string());
            let _ = response_tx.send(Err(error.to_string()));
            return;
        };
        let request_handle = app_server.request_handle();
        let tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result =
                crate::experimental_features::write(request_handle, thread_id, updates).await;
            drop(guard);
            // Report unresolved outcomes even if the popup closes before its next draw.
            let warning = match &result {
                Ok(result) => result.warning.as_ref(),
                Err(error) => Some(error),
            };
            if let Some(warning) = warning {
                tx.send(AppEvent::InsertHistoryCell(Box::new(
                    history_cell::new_warning_event(warning.clone()),
                )));
            }
            let _ = response_tx.send(result);
        });
    }
}

#[cfg(test)]
#[path = "experimental_features_tests.rs"]
mod tests;
