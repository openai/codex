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

const DEFAULT_LIMIT: usize = 25;
const MAX_LIMIT: usize = 25;

#[derive(Clone)]
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
        self.require_enabled_for_thread(loaded_thread.as_deref())
            .await?;
        if let Some(thread) = loaded_thread {
            TurnRequestProcessor::validate_direct_input_allowed(thread.as_ref()).await?;
        } else {
            TurnRequestProcessor::validate_unloaded_direct_input_allowed(&source)?;
        }
        let queued_item = self
            .service
            .enqueue(
                thread_id,
                params.submission.into(),
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
        let (thread_id, loaded_thread, _) = self.require_thread(&params.thread_id).await?;
        self.require_enabled_for_thread(loaded_thread.as_deref())
            .await?;
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
        let limit = params
            .limit
            .unwrap_or(DEFAULT_LIMIT as u32)
            .clamp(1, MAX_LIMIT as u32) as usize;
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
        let (thread_id, loaded_thread, _) = self.require_thread(&params.thread_id).await?;
        self.require_enabled_for_thread(loaded_thread.as_deref())
            .await?;
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
        let (thread_id, loaded_thread, _) = self.require_thread(&params.thread_id).await?;
        self.require_enabled_for_thread(loaded_thread.as_deref())
            .await?;
        let current_items = self
            .service
            .list(
                thread_id,
                /*offset*/ 0,
                params.queued_item_ids.len().saturating_add(1),
            )
            .await
            .map_err(queue_error)?;
        let mut current_ids = current_items
            .into_iter()
            .map(|item| item.id)
            .collect::<Vec<_>>();
        current_ids.sort();
        let mut requested_ids = params.queued_item_ids.clone();
        requested_ids.sort();
        if current_ids != requested_ids {
            return Err(invalid_request(
                "queue reorder must include every visible queued item exactly once",
            ));
        }
        let queued_items = self
            .service
            .reorder(thread_id, &params.queued_item_ids)
            .await
            .map_err(queue_error)?;
        Ok(ThreadQueueReorderResponse {
            queued_items: queued_items.into_iter().map(api_queued_item).collect(),
        })
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
        if let Ok(thread) = self.thread_manager.get_thread(thread_id).await {
            let snapshot = thread.config_snapshot().await;
            if snapshot.ephemeral {
                return Err(invalid_request(format!(
                    "ephemeral thread does not support queued items: {thread_id}"
                )));
            }
            return Ok((thread_id, Some(thread), snapshot.session_source));
        }
        if let Some(stored_thread) = self
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
            return Ok((thread_id, None, source));
        }
        Err(invalid_request(format!("thread not found: {thread_id}")))
    }

    async fn require_enabled_for_thread(
        &self,
        loaded_thread: Option<&codex_core::CodexThread>,
    ) -> Result<(), JSONRPCErrorError> {
        let config = match loaded_thread {
            Some(thread) => {
                let thread_config = thread.config().await;
                self.config_manager
                    .load_latest_config_for_thread(thread_config.as_ref())
                    .await
            }
            None => {
                self.config_manager
                    .load_latest_config(/*fallback_cwd*/ None)
                    .await
            }
        }
        .map_err(|err| internal_error(format!("failed to load queue feature state: {err}")))?;
        if !config.features.enabled(Feature::UserMessageQueue) {
            return Err(invalid_request("user message queue is unavailable"));
        }
        Ok(())
    }
}

fn queue_error(error: QueueServiceError) -> JSONRPCErrorError {
    internal_error(format!("queued item operation failed: {error}"))
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
