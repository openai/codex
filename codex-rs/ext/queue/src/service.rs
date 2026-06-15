use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Weak;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codex_core::IdleTurnInput;
use codex_core::ThreadManager;
use codex_core::TryStartTurnIfIdleRejectionReason;
use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ThreadIdleInput;
use codex_extension_api::ThreadLifecycleContributor;
use codex_protocol::ThreadId;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadQueueChangedEvent;
use codex_protocol::protocol::UserSubmission;
use codex_state::QueuedItemRecord;
use codex_state::QueuedItemState;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;
use tokio::sync::Mutex;

use crate::types::QueuedItem;
use crate::types::QueuedItemProvenance;
use crate::types::QueuedItemStatus;
use crate::types::StoredQueuedItemPayload;

const INTERRUPTED_CLAIM_MESSAGE: &str = "queue claim was interrupted before core accepted it";
const CLAIM_STALE_AFTER: Duration = Duration::from_secs(5 * 60);
const IDLE_RETRY_AFTER: Duration = Duration::from_secs(1);

#[derive(Debug, Error)]
pub enum QueueServiceError {
    #[error("queue storage failed: {0}")]
    Storage(#[from] anyhow::Error),
    #[error("queued item payload is invalid: {0}")]
    InvalidPayload(#[from] serde_json::Error),
    #[error("queue reorder must include every visible queued item exactly once")]
    InvalidReorder,
}

#[derive(Clone)]
pub struct QueuedItemService {
    state_dbs: Arc<codex_state::StateRuntime>,
    thread_manager: Weak<ThreadManager>,
    event_sink: Arc<dyn ExtensionEventSink>,
    scheduled_recoveries: Arc<Mutex<HashSet<ThreadId>>>,
}

impl QueuedItemService {
    pub fn new(
        state_dbs: Arc<codex_state::StateRuntime>,
        thread_manager: Weak<ThreadManager>,
        event_sink: Arc<dyn ExtensionEventSink>,
    ) -> Self {
        Self {
            state_dbs,
            thread_manager,
            event_sink,
            scheduled_recoveries: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub async fn enqueue(
        &self,
        thread_id: ThreadId,
        submission: UserSubmission,
        provenance: QueuedItemProvenance,
    ) -> Result<QueuedItem, QueueServiceError> {
        self.recover_stale_claims(thread_id).await?;
        let payload = serde_json::to_vec(&StoredQueuedItemPayload::V1 {
            submission,
            provenance,
        })?;
        let record = self
            .state_dbs
            .thread_queue()
            .enqueue(thread_id, &payload)
            .await?;
        let item = queued_item_from_record(record)?;
        self.emit_changed(thread_id);
        self.wake_if_loaded(thread_id).await;
        Ok(item)
    }

    pub async fn list(
        &self,
        thread_id: ThreadId,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<QueuedItem>, QueueServiceError> {
        self.recover_stale_claims(thread_id).await?;
        self.state_dbs
            .thread_queue()
            .list_page(thread_id, offset, limit)
            .await?
            .into_iter()
            .map(queued_item_from_record)
            .collect()
    }

    pub async fn delete(
        &self,
        thread_id: ThreadId,
        queued_item_id: &str,
    ) -> Result<bool, QueueServiceError> {
        self.recover_stale_claims(thread_id).await?;
        let deleted = self
            .state_dbs
            .thread_queue()
            .delete(thread_id, queued_item_id)
            .await?;
        if deleted {
            self.emit_changed(thread_id);
            self.wake_if_loaded(thread_id).await;
        }
        Ok(deleted)
    }

    pub async fn reorder(
        &self,
        thread_id: ThreadId,
        ordered_ids: &[String],
    ) -> Result<(), QueueServiceError> {
        self.recover_stale_claims(thread_id).await?;
        if !self
            .state_dbs
            .thread_queue()
            .reorder(thread_id, ordered_ids)
            .await?
        {
            return Err(QueueServiceError::InvalidReorder);
        }
        self.emit_changed(thread_id);
        self.wake_if_loaded(thread_id).await;
        Ok(())
    }

    async fn dispatch_if_idle(&self, thread_id: ThreadId) -> Result<(), QueueServiceError> {
        self.recover_stale_claims(thread_id).await?;
        let (record, claim_token, payload) = loop {
            let Some(claim) = self.state_dbs.thread_queue().claim_next(thread_id).await? else {
                if self
                    .state_dbs
                    .thread_queue()
                    .has_claimed_item(thread_id)
                    .await?
                {
                    self.schedule_recovery(thread_id).await;
                }
                return Ok(());
            };
            let claim_token = claim.claim_token;
            let record = claim.item;
            match serde_json::from_slice::<StoredQueuedItemPayload>(&record.payload_jsonb) {
                Ok(payload) => break (record, claim_token, payload),
                Err(err) => {
                    let queued_item_id = &record.queued_item_id;
                    if !self
                        .state_dbs
                        .thread_queue()
                        .complete_claim(queued_item_id, &claim_token)
                        .await?
                    {
                        tracing::warn!(%queued_item_id, "invalid queued item lost its storage claim");
                    }
                    tracing::warn!(%queued_item_id, %err, "discarding invalid queued item payload");
                    self.emit_changed(thread_id);
                }
            }
        };
        let queued_item_id = record.queued_item_id.clone();
        let (submission, _) = payload.into_parts();
        let thread = match self.thread_manager.upgrade() {
            Some(thread_manager) => thread_manager.get_thread(thread_id).await.ok(),
            None => None,
        };
        let Some(thread) = thread else {
            self.state_dbs
                .thread_queue()
                .release_claim(&queued_item_id, &claim_token)
                .await?;
            return Ok(());
        };

        match thread
            .try_start_turn_if_idle(IdleTurnInput::UserSubmission(submission))
            .await
        {
            Ok(()) => {
                if !self
                    .state_dbs
                    .thread_queue()
                    .complete_claim(&queued_item_id, &claim_token)
                    .await?
                {
                    tracing::warn!(%queued_item_id, "accepted queued item lost its storage claim");
                }
                self.emit_changed(thread_id);
                Ok(())
            }
            Err(err) => match err.reason() {
                TryStartTurnIfIdleRejectionReason::Busy => {
                    self.state_dbs
                        .thread_queue()
                        .release_claim(&queued_item_id, &claim_token)
                        .await?;
                    let service = self.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(IDLE_RETRY_AFTER).await;
                        service.wake_if_loaded(thread_id).await;
                    });
                    Ok(())
                }
                TryStartTurnIfIdleRejectionReason::PendingTriggerTurn => {
                    self.state_dbs
                        .thread_queue()
                        .release_claim(&queued_item_id, &claim_token)
                        .await?;
                    Ok(())
                }
                TryStartTurnIfIdleRejectionReason::PlanMode => {
                    let failure = failure_json(&format!(
                        "core rejected queued user input: {:?}",
                        err.reason()
                    ))?;
                    self.state_dbs
                        .thread_queue()
                        .fail_claim(&queued_item_id, &claim_token, &failure)
                        .await?;
                    self.emit_changed(thread_id);
                    Ok(())
                }
            },
        }
    }

    async fn recover_stale_claims(&self, thread_id: ThreadId) -> Result<(), QueueServiceError> {
        let stale_before_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .saturating_sub(CLAIM_STALE_AFTER)
            .as_millis()
            .try_into()
            .unwrap_or(i64::MAX);
        let failure = failure_json(INTERRUPTED_CLAIM_MESSAGE)?;
        let recovered = self
            .state_dbs
            .thread_queue()
            .recover_claims_as_failed_before(thread_id, stale_before_ms, &failure)
            .await?;
        if recovered > 0 {
            self.emit_changed(thread_id);
        }
        Ok(())
    }

    async fn schedule_recovery(&self, thread_id: ThreadId) {
        if !self.scheduled_recoveries.lock().await.insert(thread_id) {
            return;
        }
        let service = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(CLAIM_STALE_AFTER).await;
            service.scheduled_recoveries.lock().await.remove(&thread_id);
            if let Err(err) = service.recover_stale_claims(thread_id).await {
                tracing::warn!(%thread_id, %err, "failed to recover stale queue claim");
            }
        });
    }

    async fn wake_if_loaded(&self, thread_id: ThreadId) {
        let Some(thread_manager) = self.thread_manager.upgrade() else {
            return;
        };
        let Ok(thread) = thread_manager.get_thread(thread_id).await else {
            return;
        };
        thread.emit_thread_idle_lifecycle_if_idle().await;
    }

    fn emit_changed(&self, thread_id: ThreadId) {
        self.event_sink.emit(Event {
            id: thread_id.to_string(),
            msg: EventMsg::ThreadQueueChanged(ThreadQueueChangedEvent { thread_id }),
        });
    }
}

impl<C> ThreadLifecycleContributor<C> for QueuedItemService
where
    C: Send + Sync + 'static,
{
    fn on_thread_idle<'a>(&'a self, input: ThreadIdleInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let Ok(thread_id) = ThreadId::from_string(input.thread_store.level_id()) else {
                tracing::warn!(
                    level_id = input.thread_store.level_id(),
                    "queue extension received an invalid thread id"
                );
                return;
            };
            if let Err(err) = self.dispatch_if_idle(thread_id).await {
                tracing::warn!(%thread_id, "failed to dispatch queued item: {err}");
            }
        })
    }
}

#[derive(Deserialize, Serialize)]
struct QueuedItemFailure {
    message: String,
}

fn failure_json(message: &str) -> Result<Vec<u8>, QueueServiceError> {
    Ok(serde_json::to_vec(&QueuedItemFailure {
        message: message.to_string(),
    })?)
}

fn queued_item_from_record(record: QueuedItemRecord) -> Result<QueuedItem, QueueServiceError> {
    let payload: StoredQueuedItemPayload = serde_json::from_slice(&record.payload_jsonb)?;
    let (submission, provenance) = payload.into_parts();
    let status = match record.state {
        QueuedItemState::Pending => QueuedItemStatus::Pending,
        QueuedItemState::Failed => {
            let error = record
                .failure_jsonb
                .as_deref()
                .and_then(|json| serde_json::from_slice::<QueuedItemFailure>(json).ok())
                .map(|failure| failure.message)
                .unwrap_or_else(|| "queued item failed".to_string());
            QueuedItemStatus::Failed { error }
        }
        QueuedItemState::Claimed => {
            return Err(QueueServiceError::Storage(anyhow::anyhow!(
                "claimed queued items are not client-visible"
            )));
        }
    };
    Ok(QueuedItem {
        id: record.queued_item_id,
        submission,
        provenance,
        status,
    })
}
