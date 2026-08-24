use std::sync::Arc;

use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use crate::outgoing_message::ConnectionRequestId;
use crate::outgoing_message::OutgoingMessageSender;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::QueuedSubmission;
use codex_app_server_protocol::ThreadQueueAddParams;
use codex_app_server_protocol::ThreadQueueAddResponse;
use codex_app_server_protocol::ThreadQueueDeleteParams;
use codex_app_server_protocol::ThreadQueueDeleteResponse;
use codex_app_server_protocol::ThreadQueueListParams;
use codex_app_server_protocol::ThreadQueueListResponse;
use codex_app_server_protocol::ThreadQueueReorderParams;
use codex_app_server_protocol::ThreadQueueReorderResponse;
use codex_app_server_protocol::ThreadQueueStartParams;
use codex_app_server_protocol::ThreadQueueStartResponse;
use codex_app_server_protocol::ThreadQueueUpdateParams;
use codex_app_server_protocol::ThreadQueueUpdateResponse;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnItemsView;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_core::CodexThread;
use codex_core::NotSubmittedReason;
use codex_core::StartIfIdleSubmission;
use codex_core::ThreadManager;
use codex_core::TurnInput;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_queue_extension::QueueServiceError;
use codex_queue_extension::QueuedItem;
use codex_queue_extension::QueuedItemService;
use codex_thread_store::ReadThreadParams;
use codex_thread_store::ThreadStore;
use codex_thread_store::ThreadStoreError;

use super::TurnRequestProcessor;
use super::thread_input::DIRECT_INPUT_TO_MULTI_AGENT_V2_SUBAGENT_ERROR;
use super::thread_input::can_accept_direct_input;
use super::thread_processor::THREAD_LIST_DEFAULT_LIMIT;
use super::thread_processor::THREAD_LIST_MAX_LIMIT;
use super::turn_processor::validate_user_input_image_urls;

const DIRECT_INPUT_TO_UNLOADED_SUBAGENT_ERROR: &str =
    "direct app-server input is not allowed for unloaded spawned sub-agents";

pub(crate) struct ThreadQueueRequestProcessor {
    thread_manager: Arc<ThreadManager>,
    thread_store: Arc<dyn ThreadStore>,
    outgoing: Arc<OutgoingMessageSender>,
    service: Option<Arc<QueuedItemService>>,
}

impl ThreadQueueRequestProcessor {
    pub(crate) fn new(
        thread_manager: Arc<ThreadManager>,
        thread_store: Arc<dyn ThreadStore>,
        outgoing: Arc<OutgoingMessageSender>,
        service: Option<Arc<QueuedItemService>>,
    ) -> Self {
        Self {
            thread_manager,
            thread_store,
            outgoing,
            service,
        }
    }

    pub(crate) async fn add(
        &self,
        params: ThreadQueueAddParams,
    ) -> Result<ThreadQueueAddResponse, JSONRPCErrorError> {
        validate_user_input_image_urls(&params.input)?;
        let (thread_id, loaded_thread, source) = self.require_thread(&params.thread_id).await?;
        ensure_direct_input_allowed(loaded_thread.as_deref(), &source)?;
        let queued_item = self
            .service()?
            .enqueue(
                thread_id,
                submission_into_turn_input(params.input, Some(params.client_user_message_id)),
            )
            .await
            .map_err(queue_error)?;
        Ok(ThreadQueueAddResponse {
            queued_submission: api_queued_submission(queued_item)?,
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
            .map(str::parse::<usize>)
            .transpose()
            .map_err(|error| invalid_request(format!("invalid queue pagination cursor: {error}")))?
            .unwrap_or_default();
        let limit = params
            .limit
            .map(|value| value as usize)
            .unwrap_or(THREAD_LIST_DEFAULT_LIMIT)
            .clamp(1, THREAD_LIST_MAX_LIMIT);
        let mut items = self
            .service()?
            .list_page(thread_id, offset, limit.saturating_add(1))
            .await
            .map_err(queue_error)?;
        let next_cursor = if items.len() > limit {
            items.truncate(limit);
            Some(offset.saturating_add(limit).to_string())
        } else {
            None
        };
        Ok(ThreadQueueListResponse {
            data: items
                .into_iter()
                .map(api_queued_submission)
                .collect::<Result<Vec<_>, _>>()?,
            next_cursor,
        })
    }

    pub(crate) async fn update(
        &self,
        params: ThreadQueueUpdateParams,
    ) -> Result<ThreadQueueUpdateResponse, JSONRPCErrorError> {
        validate_user_input_image_urls(&params.input)?;
        let (thread_id, loaded_thread, source) = self.require_thread(&params.thread_id).await?;
        ensure_direct_input_allowed(loaded_thread.as_deref(), &source)?;
        let queued_item = self
            .service()?
            .update(
                thread_id,
                params.queued_submission_id.clone(),
                submission_into_turn_input(params.input, /*client_user_message_id*/ None),
            )
            .await
            .map_err(queue_error)?
            .ok_or_else(|| {
                invalid_request(format!(
                    "queued submission not found: {}",
                    params.queued_submission_id
                ))
            })?;
        Ok(ThreadQueueUpdateResponse {
            queued_submission: api_queued_submission(queued_item)?,
        })
    }

    pub(crate) async fn delete(
        &self,
        params: ThreadQueueDeleteParams,
    ) -> Result<ThreadQueueDeleteResponse, JSONRPCErrorError> {
        let (thread_id, _, _) = self.require_thread(&params.thread_id).await?;
        let deleted = self
            .service()?
            .delete(thread_id, params.queued_submission_id)
            .await
            .map_err(queue_error)?;
        Ok(ThreadQueueDeleteResponse { deleted })
    }

    pub(crate) async fn reorder(
        &self,
        params: ThreadQueueReorderParams,
    ) -> Result<ThreadQueueReorderResponse, JSONRPCErrorError> {
        let (thread_id, _, _) = self.require_thread(&params.thread_id).await?;
        self.service()?
            .reorder(thread_id, params.queued_submission_ids)
            .await
            .map_err(queue_error)?;
        Ok(ThreadQueueReorderResponse {})
    }

    pub(crate) async fn start(
        &self,
        request_id: &ConnectionRequestId,
        params: ThreadQueueStartParams,
    ) -> Result<ThreadQueueStartResponse, JSONRPCErrorError> {
        let (_, loaded_thread, source) = self.require_thread(&params.thread_id).await?;
        ensure_direct_input_allowed(loaded_thread.as_deref(), &source)?;
        let thread = loaded_thread
            .ok_or_else(|| invalid_request("resume the thread before starting a queued message"))?;
        let submission = self
            .service()?
            .start(
                thread.as_ref(),
                params.queued_submission_id,
                self.outgoing.request_trace_context(request_id).await,
            )
            .await
            .map_err(queue_error)?;
        let turn_id = match submission {
            StartIfIdleSubmission::Started { turn_id } => turn_id,
            StartIfIdleSubmission::NotSubmitted {
                reason: NotSubmittedReason::NotIdle | NotSubmittedReason::PendingTriggerTurn,
            } => {
                return Err(invalid_request(
                    "thread already has an active or pending turn",
                ));
            }
            StartIfIdleSubmission::NotSubmitted { reason } => {
                return Err(internal_error(format!(
                    "Core declined to start queued user message: {reason:?}"
                )));
            }
        };
        self.outgoing
            .record_request_turn_id(request_id, &turn_id)
            .await;
        Ok(ThreadQueueStartResponse {
            turn: Turn {
                id: turn_id,
                items: vec![],
                items_view: TurnItemsView::NotLoaded,
                error: None,
                status: TurnStatus::InProgress,
                started_at: None,
                completed_at: None,
                duration_ms: None,
            },
        })
    }

    fn service(&self) -> Result<&QueuedItemService, JSONRPCErrorError> {
        self.service
            .as_deref()
            .ok_or_else(|| invalid_request("user message queue is unavailable"))
    }

    async fn require_thread(
        &self,
        raw_thread_id: &str,
    ) -> Result<(ThreadId, Option<Arc<CodexThread>>, SessionSource), JSONRPCErrorError> {
        let thread_id = ThreadId::from_string(raw_thread_id)
            .map_err(|error| invalid_request(format!("invalid thread id: {error}")))?;
        let (loaded_thread, source) = if let Ok(thread) =
            self.thread_manager.get_thread(thread_id).await
        {
            let snapshot = thread.config_snapshot().await;
            if snapshot.ephemeral {
                return Err(invalid_request(format!(
                    "ephemeral thread does not support queued submissions: {thread_id}"
                )));
            }
            (Some(thread), snapshot.session_source)
        } else {
            let stored = self
                .thread_store
                .read_thread(ReadThreadParams {
                    thread_id,
                    include_archived: true,
                    include_history: false,
                })
                .await
                .map_err(|error| match error {
                    ThreadStoreError::ThreadNotFound { .. } => {
                        invalid_request(format!("thread not found: {thread_id}"))
                    }
                    error => internal_error(format!("failed to read thread: {error}")),
                })?;
            if stored.archived_at.is_some() {
                return Err(invalid_request(format!(
                    "session {thread_id} is archived. Run `codex unarchive {thread_id}` to unarchive it first."
                )));
            }
            (None, stored.source)
        };

        Ok((thread_id, loaded_thread, source))
    }
}

fn ensure_direct_input_allowed(
    loaded_thread: Option<&CodexThread>,
    source: &SessionSource,
) -> Result<(), JSONRPCErrorError> {
    match loaded_thread {
        Some(thread) if !can_accept_direct_input(thread.multi_agent_version(), source) => Err(
            invalid_request(DIRECT_INPUT_TO_MULTI_AGENT_V2_SUBAGENT_ERROR),
        ),
        None if matches!(
            source,
            SessionSource::SubAgent(SubAgentSource::ThreadSpawn { .. })
        ) =>
        {
            Err(invalid_request(DIRECT_INPUT_TO_UNLOADED_SUBAGENT_ERROR))
        }
        _ => Ok(()),
    }
}

fn submission_into_turn_input(
    input: Vec<UserInput>,
    client_user_message_id: Option<String>,
) -> TurnInput {
    TurnInput::UserInput {
        content: input.into_iter().map(UserInput::into_core).collect(),
        client_id: client_user_message_id,
    }
}

pub(super) fn queue_error(error: QueueServiceError) -> JSONRPCErrorError {
    match error {
        QueueServiceError::InputTooLarge { actual_chars } => {
            TurnRequestProcessor::input_too_large_error(actual_chars)
        }
        error @ (QueueServiceError::InvalidInput | QueueServiceError::InvalidAttachment(_)) => {
            invalid_request(error.to_string())
        }
        QueueServiceError::Storage(ThreadStoreError::InvalidRequest { message }) => {
            invalid_request(message)
        }
        error => internal_error(format!("queued submission operation failed: {error}")),
    }
}

fn api_queued_submission(value: QueuedItem) -> Result<QueuedSubmission, JSONRPCErrorError> {
    let TurnInput::UserInput { content, client_id } = value.input else {
        return Err(internal_error(
            "queued submission does not contain user input",
        ));
    };
    Ok(QueuedSubmission {
        id: value.id,
        input: content.into_iter().map(Into::into).collect(),
        client_user_message_id: client_id
            .ok_or_else(|| internal_error("queued submission is missing its client message id"))?,
    })
}
