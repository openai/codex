//! Orders retained inputs at acceptance, assistant messages at stream start,
//! and Code Mode messages at confirmed delivery.
//! Explicit goal authorization becomes live only after its persistence barrier succeeds.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::task::Context;
use std::task::Poll;

use crate::context::ContextualUserFragment;
use crate::context::UserGoalUpdate;
use codex_history::CodexHarnessMetadata;
use codex_history::ResponseItemEnvelope;
use codex_history::RetainedContextEvent;
use codex_history::RetainedUserMessage;
use codex_history::RolloutItem;
use codex_protocol::models::ResponseItem;
use codex_thread_store::PersistContext;
use tokio::sync::Semaphore;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio_util::task::TaskTracker;
use tokio_util::task::task_tracker::TaskTrackerToken;

use super::Session;
use super::TurnContext;
use super::thread_settings;

/// Only unrecorded starts need a reservation; abandoned entries expire with the turn.
#[derive(Default)]
pub(super) struct PendingAssistantMessageOrders(pub(super) Mutex<HashMap<String, u64>>);

pub(super) struct CodeModeMessageTasks {
    tasks: Mutex<TaskTracker>,
    pending_persistence: Mutex<Vec<watch::Receiver<()>>>,
    pub(super) communication_boundary: Arc<Semaphore>,
}

impl Default for CodeModeMessageTasks {
    fn default() -> Self {
        Self {
            tasks: Mutex::default(),
            pending_persistence: Mutex::default(),
            communication_boundary: Arc::new(Semaphore::new(1)),
        }
    }
}

impl Session {
    /// Reserve before deriving display items, including plans, from the source message.
    /// Completion-only responses use the same path before publishing their text.
    pub(super) async fn reserve_assistant_message_order(
        &self,
        turn_context: &TurnContext,
        item: &ResponseItem,
    ) {
        if let ResponseItem::Message {
            id: Some(id), role, ..
        } = item
            && role == "assistant"
        {
            let mut state = self.state.lock().await;
            if !state
                .history
                .raw_items()
                .any(|item| item.id().is_some_and(|recorded_id| recorded_id == id))
            {
                turn_context
                    .extension_data
                    .get_or_init(PendingAssistantMessageOrders::default)
                    .0
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .entry(id.to_string())
                    .or_insert_with(|| state.history.reserve_input_order());
            }
        }
    }

    /// Records authorization without adding pending input or reopening an active turn.
    pub(crate) async fn record_user_goal_update(
        &self,
        update: UserGoalUpdate,
    ) -> std::io::Result<()> {
        // Goal metadata must not initialize a model step, even on a goal-first thread.
        // Keep context construction off callers' stacks, including the TUI RPC dispatcher.
        let context = Box::pin(self.new_inject_items_context()).await;
        // Earlier Code Mode messages may need the persistence lock before this can return.
        let user_input_order = self.reserve_user_input_order().await;
        let _guard = thread_settings::acquire_persistence_lock(self).await;
        let mut item = ContextualUserFragment::into(update);
        Self::stamp_response_item_for_history(&mut item, &context.sub_id);
        Self::assign_missing_response_item_id(&mut item);
        let mut item = ResponseItemEnvelope {
            item,
            metadata: Some(CodexHarnessMetadata {
                user_input_order: Some(user_input_order),
                ..Default::default()
            }),
        };
        if let Some(live_thread) = self.live_thread() {
            // Capture settings under the same permit as the instruction, including when a
            // goal creates the rollout. No fallible settings catch-up runs after publication.
            live_thread
                .append_items(&[
                    RolloutItem::EventMsg(thread_settings::applied_event(self).await),
                    RolloutItem::ResponseItem(item.clone()),
                ])
                .await
                .map_err(std::io::Error::other)?;
        }
        // A failed append or checkpoint must not change live authorization or its revision.
        self.try_ensure_rollout_materialized(PersistContext::ThreadPreparation)
            .await?;
        self.state.lock().await.history.record_annotated_items(
            std::slice::from_mut(&mut item),
            context.model_info().truncation_policy.into(),
        );
        self.send_raw_response_items(&context, std::slice::from_ref(&item.item))
            .await;
        Ok(())
    }

    pub(crate) async fn reserve_user_input_order(&self) -> u64 {
        let _boundary = self
            .code_mode_message_tasks
            .communication_boundary
            .acquire()
            .await
            .unwrap_or_else(|_| unreachable!("communication boundary remains open"));
        let (order, pending) = {
            let mut state = self.state.lock().await;
            let order = state.history.reserve_input_order();
            (order, self.pending_code_mode_message_recordings())
        };
        // Older readers assign assistant records to physical instruction boundaries.
        // Keep a later user steer from reaching the rollout ahead of an earlier send.
        for mut recording in pending {
            let _ = recording.changed().await;
        }
        order
    }

    /// Snapshot while holding `state` so earlier confirmed delivery reservations are included.
    pub(super) fn pending_code_mode_message_recordings(&self) -> Vec<watch::Receiver<()>> {
        let mut pending = self
            .code_mode_message_tasks
            .pending_persistence
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        pending.retain(|recording| recording.has_changed().is_ok());
        pending.clone()
    }

    pub(crate) async fn record_retained_context(&self, mut event: RetainedContextEvent) {
        event.bound();
        // Share the checkpoint persistence lock so a fact cannot land on the wrong side
        // of the checkpoint/suffix boundary. Ephemeral threads use the same live state.
        let _guard = thread_settings::acquire_persistence_lock(self).await;
        if self
            .state
            .lock()
            .await
            .history
            .record_retained_context(&event)
        {
            self.persist_rollout_items(&[RolloutItem::RetainedContext(event)])
                .await;
        }
    }

    pub(crate) fn track_code_mode_message(&self) -> Option<TaskTrackerToken> {
        let tasks = self
            .code_mode_message_tasks
            .tasks
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        (!tasks.is_closed()).then(|| tasks.token())
    }

    pub(super) async fn drain_code_mode_messages(&self) {
        let tasks = {
            let tasks = self
                .code_mode_message_tasks
                .tasks
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            tasks.close();
            tasks.clone()
        };
        tasks.wait().await;
    }

    /// Starts recording confirmed delivery synchronously while the tool's admission is held.
    pub(crate) fn record_delivered_assistant_message(
        self: &Arc<Self>,
        message: RetainedUserMessage,
    ) -> (JoinHandle<()>, watch::Receiver<()>) {
        let session = Arc::clone(self);
        let persistence_session = Arc::clone(self);
        let (recorded, recording) = watch::channel(());
        let pending_recording = recording.clone();
        let mut reserve = Box::pin(async move {
            let boundary = Arc::clone(&session.code_mode_message_tasks.communication_boundary)
                .acquire_owned()
                .await
                .unwrap_or_else(|_| unreachable!("communication boundary remains open"));
            let mut state = session.state.lock().await;
            let mut event = RetainedContextEvent::DeliveredAssistantMessage {
                message,
                acceptance_order: state.history.reserve_input_order(),
            };
            event.bound();
            state.history.record_retained_context(&event);
            let mut pending = session
                .code_mode_message_tasks
                .pending_persistence
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            pending.retain(|recording| recording.has_changed().is_ok());
            pending.push(pending_recording);
            (event, boundary)
        });
        // Poll while still at the confirmed MCP response boundary. A free state lock
        // reserves the order now; a busy lock queues this reservation ahead of a
        // later user reply before the detached task can be scheduled.
        let mut context = Context::from_waker(futures::task::noop_waker_ref());
        let reservation = reserve.as_mut().poll(&mut context);
        let task = self
            .code_mode_message_tasks
            .tasks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .spawn(async move {
                let (event, _boundary) = match reservation {
                    Poll::Ready(reservation) => reservation,
                    Poll::Pending => reserve.await,
                };
                let _guard = thread_settings::acquire_persistence_lock(&persistence_session).await;
                // A distinct delivery still belongs in the rollout when the live window is full.
                persistence_session
                    .persist_rollout_items(&[RolloutItem::RetainedContext(event)])
                    .await;
                drop(recorded);
            });
        (task, recording)
    }
}

#[cfg(test)]
#[path = "retained_context_tests.rs"]
mod tests;
