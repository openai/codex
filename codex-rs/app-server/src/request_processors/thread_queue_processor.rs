use std::path::PathBuf;
use std::sync::Arc;

use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::QueuedItem as ApiQueuedItem;
use codex_app_server_protocol::QueuedItemProvenance as ApiQueuedItemProvenance;
use codex_app_server_protocol::QueuedItemStatus as ApiQueuedItemStatus;
use codex_app_server_protocol::ThreadQueueAddParams;
use codex_app_server_protocol::ThreadQueueAddResponse;
use codex_app_server_protocol::ThreadQueueDeleteParams;
use codex_app_server_protocol::ThreadQueueDeleteResponse;
use codex_app_server_protocol::ThreadQueueListParams;
use codex_app_server_protocol::ThreadQueueListResponse;
use codex_app_server_protocol::ThreadQueueReorderParams;
use codex_app_server_protocol::ThreadQueueReorderResponse;
use codex_app_server_protocol::TurnSubmission;
use codex_core::ThreadManager;
use codex_features::Feature;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::UserSubmission;
use codex_queue_extension::QueueServiceError;
use codex_queue_extension::QueuedItem;
use codex_queue_extension::QueuedItemProvenance;
use codex_queue_extension::QueuedItemService;
use codex_queue_extension::QueuedItemStatus;
use codex_rollout::StateDbHandle;

use crate::config_manager::ConfigManager;
use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use crate::request_processors::TurnRequestProcessor;

const MAX_LIMIT: u32 = 25;
const DIRECT_INPUT_TO_UNLOADED_SUBAGENT_ERROR: &str =
    "direct app-server input is not allowed for unloaded spawned sub-agents";

pub(crate) struct ThreadQueueRequestProcessor {
    thread_manager: Arc<ThreadManager>,
    config_manager: ConfigManager,
    state_db: StateDbHandle,
    service: Arc<QueuedItemService>,
}

impl ThreadQueueRequestProcessor {
    pub(crate) fn new(
        thread_manager: Arc<ThreadManager>,
        config_manager: ConfigManager,
        state_db: StateDbHandle,
        service: Arc<QueuedItemService>,
    ) -> Self {
        Self {
            thread_manager,
            config_manager,
            state_db,
            service,
        }
    }

    pub(crate) async fn add(
        &self,
        params: ThreadQueueAddParams,
    ) -> Result<ThreadQueueAddResponse, JSONRPCErrorError> {
        TurnRequestProcessor::validate_v2_input_limit(&params.submission.input)?;
        let (thread_id, loaded_thread, source) = self.require_thread(&params.thread_id).await?;
        if let Some(thread) = loaded_thread {
            TurnRequestProcessor::validate_direct_input_allowed(thread.as_ref()).await?;
        } else if matches!(
            source,
            SessionSource::SubAgent(SubAgentSource::ThreadSpawn { .. })
        ) {
            // Persisted metadata does not retain the multi-agent version, so an unloaded spawned
            // subagent cannot be proven safe for direct input.
            return Err(invalid_request(DIRECT_INPUT_TO_UNLOADED_SUBAGENT_ERROR));
        }
        let mut submission = UserSubmission::from(params.submission);
        // TODO: Support output schemas for queued submissions once turn-scoped configuration can
        // be passed directly into TurnContext without routing through SessionSettingsUpdate.
        submission.final_output_json_schema = None;
        let queued_item = self
            .service
            .enqueue(
                thread_id,
                submission,
                provenance_into_domain(params.provenance),
            )
            .await
            .map_err(queue_error)?;
        Ok(ThreadQueueAddResponse {
            queued_item: api_queued_item(queued_item),
        })
    }

    pub(crate) async fn list(
        &self,
        params: ThreadQueueListParams,
    ) -> Result<ThreadQueueListResponse, JSONRPCErrorError> {
        let (thread_id, _, _) = self.require_thread(&params.thread_id).await?;
        let offset = params
            .cursor
            .as_deref()
            .map(|cursor| {
                cursor
                    .parse::<usize>()
                    .ok()
                    .filter(|offset| i64::try_from(*offset).is_ok())
                    .ok_or(())
            })
            .transpose()
            .map_err(|_| invalid_request("invalid queue cursor"))?
            .unwrap_or_default();
        let limit = params.limit.unwrap_or(MAX_LIMIT).clamp(1, MAX_LIMIT) as usize;
        let mut items = self
            .service
            .list(thread_id, offset, limit.saturating_add(1))
            .await
            .map_err(queue_error)?;
        let next_cursor = (items.len() > limit).then(|| offset.saturating_add(limit).to_string());
        items.truncate(limit);
        Ok(ThreadQueueListResponse {
            data: items.into_iter().map(api_queued_item).collect(),
            next_cursor,
        })
    }

    pub(crate) async fn delete(
        &self,
        params: ThreadQueueDeleteParams,
    ) -> Result<ThreadQueueDeleteResponse, JSONRPCErrorError> {
        let (thread_id, _, _) = self.require_thread(&params.thread_id).await?;
        let deleted = self
            .service
            .delete(thread_id, &params.queued_item_id)
            .await
            .map_err(queue_error)?;
        Ok(ThreadQueueDeleteResponse { deleted })
    }

    pub(crate) async fn reorder(
        &self,
        params: ThreadQueueReorderParams,
    ) -> Result<ThreadQueueReorderResponse, JSONRPCErrorError> {
        let (thread_id, _, _) = self.require_thread(&params.thread_id).await?;
        self.service
            .reorder(thread_id, &params.queued_item_ids)
            .await
            .map_err(queue_error)?;
        Ok(ThreadQueueReorderResponse {})
    }

    async fn require_thread(
        &self,
        raw_thread_id: &str,
    ) -> Result<
        (
            ThreadId,
            Option<Arc<codex_core::CodexThread>>,
            SessionSource,
        ),
        JSONRPCErrorError,
    > {
        let thread_id = ThreadId::from_string(raw_thread_id)
            .map_err(|err| invalid_request(format!("invalid thread id: {err}")))?;
        let (loaded_thread, source, stored_cwd) = if let Ok(thread) =
            self.thread_manager.get_thread(thread_id).await
        {
            let snapshot = thread.config_snapshot().await;
            if snapshot.ephemeral {
                return Err(invalid_request(format!(
                    "ephemeral thread does not support queued items: {thread_id}"
                )));
            }
            (Some(thread), snapshot.session_source, None)
        } else if let Some(stored_thread) = self
            .state_db
            .get_thread(thread_id)
            .await
            .map_err(|err| internal_error(format!("failed to read thread: {err}")))?
        {
            if stored_thread.archived_at.is_some() {
                return Err(invalid_request(format!(
                    "session {thread_id} is archived. Run `codex unarchive {thread_id}` to unarchive it first."
                )));
            }
            let source = serde_json::from_str(&stored_thread.source)
                .or_else(|_| {
                    serde_json::from_value::<SessionSource>(serde_json::Value::String(
                        stored_thread.source.clone(),
                    ))
                })
                .map_err(|err| internal_error(format!("failed to decode thread source: {err}")))?;
            (None, source, Some(stored_thread.cwd))
        } else {
            return Err(invalid_request(format!("thread not found: {thread_id}")));
        };

        self.require_enabled_for_thread(loaded_thread.as_deref(), stored_cwd)
            .await?;
        Ok((thread_id, loaded_thread, source))
    }

    async fn require_enabled_for_thread(
        &self,
        loaded_thread: Option<&codex_core::CodexThread>,
        stored_cwd: Option<PathBuf>,
    ) -> Result<(), JSONRPCErrorError> {
        let config = match loaded_thread {
            Some(thread) => {
                let thread_config = thread.config().await;
                self.config_manager
                    .load_latest_config_for_thread(thread_config.as_ref())
                    .await
            }
            None => self.config_manager.load_latest_config(stored_cwd).await,
        }
        .map_err(|err| internal_error(format!("failed to load queue feature state: {err}")))?;
        if !config.features.enabled(Feature::UserMessageQueue) {
            return Err(invalid_request("user message queue is unavailable"));
        }
        Ok(())
    }
}

fn queue_error(error: QueueServiceError) -> JSONRPCErrorError {
    match error {
        error @ QueueServiceError::InvalidReorder => invalid_request(error.to_string()),
        error => internal_error(format!("queued item operation failed: {error}")),
    }
}

fn provenance_into_domain(value: ApiQueuedItemProvenance) -> QueuedItemProvenance {
    match value {
        ApiQueuedItemProvenance::User => QueuedItemProvenance::User,
        ApiQueuedItemProvenance::ExternalEvent { source, metadata } => {
            QueuedItemProvenance::ExternalEvent { source, metadata }
        }
    }
}

fn api_queued_item(value: QueuedItem) -> ApiQueuedItem {
    ApiQueuedItem {
        id: value.id,
        submission: TurnSubmission::from(value.submission),
        provenance: match value.provenance {
            QueuedItemProvenance::User => ApiQueuedItemProvenance::User,
            QueuedItemProvenance::ExternalEvent { source, metadata } => {
                ApiQueuedItemProvenance::ExternalEvent { source, metadata }
            }
        },
        status: match value.status {
            QueuedItemStatus::Pending => ApiQueuedItemStatus::Pending,
            QueuedItemStatus::Failed { error } => ApiQueuedItemStatus::Failed { error },
        },
    }
}
