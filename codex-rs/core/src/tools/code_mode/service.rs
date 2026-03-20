use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value as JsonValue;
use tokio::sync::Mutex;
use tracing::warn;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::tools::ToolRouter;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::js_repl::resolve_compatible_node;
use crate::tools::parallel::ToolCallRuntime;
use codex_features::Feature;

use super::ExecContext;
use super::PUBLIC_TOOL_NAME;
use super::process::CodeModeProcess;
use super::process::spawn_code_mode_process;
use super::worker::CodeModeWorker;

pub(crate) struct CodeModeService {
    js_repl_node_path: Option<PathBuf>,
    stored_values: Mutex<HashMap<String, JsonValue>>,
    process: Arc<Mutex<Option<CodeModeProcess>>>,
    next_cell_id: Mutex<u64>,
}

impl CodeModeService {
    pub(crate) fn new(js_repl_node_path: Option<PathBuf>) -> Self {
        Self {
            js_repl_node_path,
            stored_values: Mutex::new(HashMap::new()),
            process: Arc::new(Mutex::new(None)),
            next_cell_id: Mutex::new(1),
        }
    }

    pub(crate) async fn stored_values(&self) -> HashMap<String, JsonValue> {
        self.stored_values.lock().await.clone()
    }

    pub(crate) async fn replace_stored_values(&self, values: HashMap<String, JsonValue>) {
        *self.stored_values.lock().await = values;
    }

    pub(super) async fn ensure_started(
        &self,
    ) -> Result<tokio::sync::OwnedMutexGuard<Option<CodeModeProcess>>, std::io::Error> {
        let mut process_slot = self.process.lock().await;
        let needs_spawn = match process_slot.as_mut() {
            Some(process) => !matches!(process.has_exited(), Ok(false)),
            None => true,
        };
        if needs_spawn {
            let node_path = resolve_compatible_node(self.js_repl_node_path.as_deref())
                .await
                .map_err(std::io::Error::other)?;
            *process_slot = Some(spawn_code_mode_process(&node_path).await?);
        }
        drop(process_slot);
        Ok(self.process.clone().lock_owned().await)
    }

    pub(crate) async fn start_turn_worker(
        &self,
        session: &Arc<Session>,
        turn: &Arc<TurnContext>,
        router: Arc<ToolRouter>,
        tracker: SharedTurnDiffTracker,
    ) -> Option<CodeModeWorker> {
        if !turn.features.enabled(Feature::CodeMode) {
            return None;
        }
        let exec = ExecContext {
            session: Arc::clone(session),
            turn: Arc::clone(turn),
        };
        let tool_runtime =
            ToolCallRuntime::new(router, Arc::clone(session), Arc::clone(turn), tracker);
        let mut process_slot = match self.ensure_started().await {
            Ok(process_slot) => process_slot,
            Err(err) => {
                warn!("failed to start {PUBLIC_TOOL_NAME} worker for turn: {err}");
                return None;
            }
        };
        let Some(process) = process_slot.as_mut() else {
            warn!(
                "failed to start {PUBLIC_TOOL_NAME} worker for turn: {PUBLIC_TOOL_NAME} runner failed to start"
            );
            return None;
        };
        Some(process.worker(exec, tool_runtime))
    }

    pub(crate) async fn allocate_cell_id(&self) -> String {
        let mut next_cell_id = self.next_cell_id.lock().await;
        let cell_id = *next_cell_id;
        *next_cell_id = next_cell_id.saturating_add(1);
        cell_id.to_string()
    }

    pub(crate) async fn allocate_request_id(&self) -> String {
        uuid::Uuid::new_v4().to_string()
    }
}
