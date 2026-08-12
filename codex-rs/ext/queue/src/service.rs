use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::Weak;

use codex_core::CodexThread;
use codex_core::ThreadManager;
use codex_core::TurnInput;
use codex_core::UserMessageAdmission;
use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ThreadIdleCause;
use codex_extension_api::ThreadIdleInput;
use codex_extension_api::ThreadLifecycleContributor;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::models::snapshot_local_user_input;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadQueueChangedEvent;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::protocol::W3cTraceContext;
use codex_protocol::user_input::MAX_USER_INPUT_TEXT_CHARS;
use codex_protocol::user_input::UserInput;
use codex_thread_store::MAX_QUEUE_ITEMS;
use codex_thread_store::QueueStore;
use codex_thread_store::QueuedUserSubmissionRecord;
use codex_thread_store::ThreadStoreError;
use thiserror::Error;
use tokio::sync::Mutex;
use tokio::sync::OwnedMutexGuard;
use uuid::Uuid;

/// One user message waiting to start on its thread.
#[derive(Clone, Debug, PartialEq)]
pub struct QueuedItem {
    pub id: String,
    pub input: TurnInput,
}

#[derive(Debug, Error)]
pub enum QueueServiceError {
    #[error("queue storage failed: {0}")]
    Storage(#[from] ThreadStoreError),
    #[error("queued item payload is invalid: {0}")]
    InvalidPayload(#[from] serde_json::Error),
    #[error("local queued attachment is invalid: {0}")]
    InvalidAttachment(#[from] std::io::Error),
    #[error("queued user message could not be admitted: {0}")]
    Admission(#[from] CodexErr),
    #[error("only user input can be added to the user-message queue")]
    InvalidInput,
    #[error(
        "queued user input exceeds the maximum length of {MAX_USER_INPUT_TEXT_CHARS} characters ({actual_chars} provided)"
    )]
    InputTooLarge { actual_chars: usize },
}

#[derive(Clone)]
pub struct QueuedItemService {
    queue: Arc<dyn QueueStore>,
    thread_manager: Weak<ThreadManager>,
    event_sink: Arc<dyn ExtensionEventSink>,
    dispatch_locks: Arc<StdMutex<HashMap<ThreadId, Weak<Mutex<()>>>>>,
}

impl QueuedItemService {
    pub fn new(
        queue: Arc<dyn QueueStore>,
        thread_manager: Weak<ThreadManager>,
        event_sink: Arc<dyn ExtensionEventSink>,
    ) -> Self {
        Self {
            queue,
            thread_manager,
            event_sink,
            dispatch_locks: Arc::new(StdMutex::new(HashMap::new())),
        }
    }

    fn dispatch_lock(&self, thread_id: ThreadId) -> Arc<Mutex<()>> {
        let mut locks = self
            .dispatch_locks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        locks.retain(|_, lock| lock.strong_count() != 0);
        if let Some(lock) = locks.get(&thread_id).and_then(Weak::upgrade) {
            return lock;
        }
        let lock = Arc::new(Mutex::new(()));
        locks.insert(thread_id, Arc::downgrade(&lock));
        lock
    }

    async fn dispatch_guard(&self, thread_id: ThreadId) -> OwnedMutexGuard<()> {
        self.dispatch_lock(thread_id).lock_owned().await
    }

    pub async fn enqueue(
        &self,
        thread_id: ThreadId,
        input: TurnInput,
    ) -> Result<QueuedItem, QueueServiceError> {
        let input = prepare_queued_user_input(input).await?;
        let payload = serde_json::to_string(&input)?;
        let item = queued_item_from_record(self.queue.enqueue(thread_id, payload).await?)?;
        self.emit_changed(thread_id);
        self.wake_if_loaded(thread_id).await;
        Ok(item)
    }

    pub async fn list(&self, thread_id: ThreadId) -> Result<Vec<QueuedItem>, QueueServiceError> {
        self.queue
            .list_page(thread_id, /*offset*/ 0, MAX_QUEUE_ITEMS)
            .await?
            .into_iter()
            .map(queued_item_from_record)
            .collect()
    }

    pub async fn update(
        &self,
        thread_id: ThreadId,
        queued_item_id: String,
        mut input: TurnInput,
    ) -> Result<Option<QueuedItem>, QueueServiceError> {
        let _dispatch_guard = self.dispatch_guard(thread_id).await;
        if let TurnInput::UserInput { client_id, .. } = &mut input
            && client_id.is_none()
        {
            *client_id = self
                .list(thread_id)
                .await?
                .into_iter()
                .find_map(|item| match item {
                    QueuedItem {
                        id,
                        input: TurnInput::UserInput { client_id, .. },
                    } if id == queued_item_id => client_id,
                    _ => None,
                });
        }
        let input = prepare_queued_user_input(input).await?;
        let payload = serde_json::to_string(&input)?;
        let item = self
            .queue
            .update(thread_id, queued_item_id, payload)
            .await?
            .map(queued_item_from_record)
            .transpose()?;
        if item.is_some() {
            self.emit_changed(thread_id);
        }
        Ok(item)
    }

    pub async fn delete(
        &self,
        thread_id: ThreadId,
        queued_item_id: String,
    ) -> Result<bool, QueueServiceError> {
        let _dispatch_guard = self.dispatch_guard(thread_id).await;
        self.delete_locked(thread_id, queued_item_id).await
    }

    async fn delete_locked(
        &self,
        thread_id: ThreadId,
        queued_item_id: String,
    ) -> Result<bool, QueueServiceError> {
        let deleted = self.queue.delete(thread_id, queued_item_id).await?;
        if deleted {
            self.emit_changed(thread_id);
        }
        Ok(deleted)
    }

    pub async fn reorder(
        &self,
        thread_id: ThreadId,
        ordered_ids: Vec<String>,
    ) -> Result<(), QueueServiceError> {
        let _dispatch_guard = self.dispatch_guard(thread_id).await;
        self.queue.reorder(thread_id, ordered_ids).await?;
        self.emit_changed(thread_id);
        Ok(())
    }

    /// Starts or steers the exact queued message and removes it after Core
    /// accepts the input for turn processing.
    pub async fn start(
        &self,
        thread: &CodexThread,
        queued_item_id: String,
        trace: Option<W3cTraceContext>,
    ) -> Result<UserMessageAdmission, QueueServiceError> {
        let thread_id = thread.session_configured().thread_id;
        let _dispatch_guard = self.dispatch_guard(thread_id).await;
        let item = self
            .list(thread_id)
            .await?
            .into_iter()
            .find(|item| item.id == queued_item_id)
            .ok_or_else(|| ThreadStoreError::InvalidRequest {
                message: format!("queued item not found: {queued_item_id}"),
            })?;
        let TurnInput::UserInput { content, client_id } = item.input else {
            return Err(QueueServiceError::InvalidInput);
        };
        let admission = thread
            .submit_user_input_and_wait_for_admission(
                Op::UserInput {
                    items: content,
                    final_output_json_schema: None,
                    responsesapi_client_metadata: None,
                    additional_context: BTreeMap::new(),
                    thread_settings: ThreadSettingsOverrides::default(),
                },
                trace,
                client_id,
            )
            .await?;
        self.delete_locked(thread_id, queued_item_id).await?;
        Ok(admission)
    }

    async fn dispatch_if_idle(&self, thread_id: ThreadId) -> Result<(), QueueServiceError> {
        let Some(manager) = self.thread_manager.upgrade() else {
            return Ok(());
        };
        let Ok(thread) = manager.get_thread(thread_id).await else {
            return Ok(());
        };

        loop {
            let Some(record) = self
                .queue
                .list_page(thread_id, /*offset*/ 0, /*limit*/ 1)
                .await?
                .into_iter()
                .next()
            else {
                return Ok(());
            };
            let queued_item_id = record.id.clone();

            let input = match serde_json::from_str::<TurnInput>(&record.payload) {
                Ok(input) => input,
                Err(error) => {
                    tracing::warn!(%queued_item_id, %error, "discarding invalid queued item");
                    self.delete_locked(thread_id, queued_item_id).await?;
                    continue;
                }
            };
            if !matches!(input, TurnInput::UserInput { .. }) {
                tracing::warn!(%queued_item_id, "discarding non-user queued input");
                self.delete_locked(thread_id, queued_item_id).await?;
                continue;
            }

            if let Err(error) = thread.try_start_turn_if_idle(vec![input]).await {
                tracing::warn!(
                    %thread_id,
                    %queued_item_id,
                    reason = ?error.reason(),
                    "core could not start queued user input"
                );
                return Ok(());
            }

            self.delete_locked(thread_id, queued_item_id).await?;
            return Ok(());
        }
    }

    async fn wake_if_loaded(&self, thread_id: ThreadId) {
        let Some(manager) = self.thread_manager.upgrade() else {
            return;
        };
        if let Ok(thread) = manager.get_thread(thread_id).await
            && !matches!(thread.agent_status().await, AgentStatus::Interrupted)
        {
            thread
                .emit_thread_idle_lifecycle_if_idle(ThreadIdleCause::Completed)
                .await;
        }
    }

    fn emit_changed(&self, thread_id: ThreadId) {
        self.event_sink.emit(Event {
            id: Uuid::now_v7().to_string(),
            msg: EventMsg::ThreadQueueChanged(ThreadQueueChangedEvent { thread_id }),
        });
    }
}

async fn prepare_queued_user_input(mut input: TurnInput) -> Result<TurnInput, QueueServiceError> {
    validate_queued_user_input(&input)?;
    let TurnInput::UserInput { content, client_id } = &mut input else {
        return Err(QueueServiceError::InvalidInput);
    };
    client_id.get_or_insert_with(|| Uuid::now_v7().to_string());
    if !content.iter().any(|item| {
        matches!(
            item,
            UserInput::LocalImage { .. } | UserInput::LocalAudio { .. }
        )
    }) {
        return Ok(input);
    }

    tokio::task::spawn_blocking(move || {
        let mut input = input;
        if let TurnInput::UserInput { content, .. } = &mut input {
            for item in content {
                snapshot_local_user_input(item)?;
            }
        }
        Ok::<TurnInput, std::io::Error>(input)
    })
    .await
    .map_err(|error| QueueServiceError::InvalidAttachment(std::io::Error::other(error)))?
    .map_err(QueueServiceError::InvalidAttachment)
}

fn validate_queued_user_input(input: &TurnInput) -> Result<(), QueueServiceError> {
    let TurnInput::UserInput { content, .. } = input else {
        return Err(QueueServiceError::InvalidInput);
    };
    if content.is_empty() {
        return Err(QueueServiceError::InvalidInput);
    }
    let actual_chars: usize = content
        .iter()
        .filter_map(|item| match item {
            UserInput::Text { text, .. } => Some(text.chars().count()),
            _ => None,
        })
        .sum();
    if actual_chars > MAX_USER_INPUT_TEXT_CHARS {
        return Err(QueueServiceError::InputTooLarge { actual_chars });
    }
    Ok(())
}

impl<C> ThreadLifecycleContributor<C> for QueuedItemService
where
    C: Send + Sync + 'static,
{
    fn on_thread_idle<'a>(&'a self, input: ThreadIdleInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if input.cause != ThreadIdleCause::Completed {
                return;
            }
            let Ok(thread_id) = ThreadId::from_string(input.thread_store.level_id()) else {
                tracing::warn!(
                    level_id = input.thread_store.level_id(),
                    "queue extension received an invalid thread id"
                );
                return;
            };
            let _guard = self.dispatch_guard(thread_id).await;
            if let Err(error) = self.dispatch_if_idle(thread_id).await {
                tracing::warn!(%thread_id, %error, "failed to dispatch queued user input");
            }
        })
    }
}

fn queued_item_from_record(
    record: QueuedUserSubmissionRecord,
) -> Result<QueuedItem, QueueServiceError> {
    Ok(QueuedItem {
        id: record.id,
        input: serde_json::from_str::<TurnInput>(&record.payload)?,
    })
}
