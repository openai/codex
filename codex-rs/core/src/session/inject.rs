use super::TurnInput as PendingTurnInput;
use super::session::Session;
use super::turn_context::TurnContext;
use codex_features::Feature;
use codex_history::ResponseItemEnvelope;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelInfo;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Uploaded files need the admission checks on typed turn input before entering history.
pub(crate) fn validate_injected_response_item(item: &ResponseItem) -> CodexResult<()> {
    if item.file_audio_ids().next().is_some() {
        return Err(CodexErr::InvalidRequest(
            "Uploaded audio files must be submitted as turn input.".to_string(),
        ));
    }
    Ok(())
}

impl Session {
    /// History injection can precede the first turn. Capture full initial context
    /// only when the thread has no reference context, including after compaction.
    pub(crate) async fn history_injection_context(
        self: &Arc<Self>,
    ) -> CodexResult<Arc<TurnContext>> {
        if self.reference_context_item().await.is_some() {
            let turn = self.new_inject_items_context().await;
            // Compaction can clear the baseline while we build this context.
            // Rebuilding initial context requires a full skills snapshot.
            if self.reference_context_item().await.is_some() {
                return Ok(turn);
            }
        }
        let turn = self.new_default_turn().await;
        if self.reference_context_item().await.is_none() {
            let step = self
                .capture_step_context(Arc::clone(&turn), &CancellationToken::new())
                .await?;
            self.record_context_updates_and_set_reference_context_item(step.as_ref())
                .await?;
        }
        Ok(turn)
    }

    pub(crate) async fn record_inherited_items(
        self: &Arc<Self>,
        items: Vec<ResponseItemEnvelope>,
    ) -> CodexResult<()> {
        if items.is_empty() {
            return Ok(());
        }
        let turn = self.history_injection_context().await?;
        // The parent already prepared the images and their dimension notices.
        self.record_prepared_conversation_items(&turn, turn.model_info(), items, Vec::new())
            .await;
        self.flush_rollout().await?;
        Ok(())
    }

    /// Returns the input if there is no active turn to inject into.
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "active turn checks and turn state updates must remain atomic"
    )]
    pub(crate) async fn inject_if_running<T: Into<ResponseItemEnvelope>>(
        &self,
        input: Vec<T>,
    ) -> Result<(), Vec<T>> {
        let mut active = self.active_turn.lock().await;
        match active.as_mut().filter(|turn| turn.finishing.is_none()) {
            Some(active_turn) => {
                self.input_queue
                    .extend_pending_input_and_notify_for_turn_state(
                        active_turn.turn_state.as_ref(),
                        input
                            .into_iter()
                            .map(Into::into)
                            .map(PendingTurnInput::ResponseItem)
                            .collect(),
                    )
                    .await;
                Ok(())
            }
            None => Err(input),
        }
    }

    /// Injects hook context into the running turn atomically.
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "active turn provenance and turn state updates must remain atomic"
    )]
    pub(crate) async fn inject_hook_context_if_running(
        &self,
        input: Vec<ResponseItem>,
    ) -> Result<(), Vec<ResponseItem>> {
        let mut active = self.active_turn.lock().await;
        let Some(active_turn) = active.as_mut() else {
            return Err(input);
        };
        if active_turn.task.is_none() {
            return Err(input);
        }
        self.input_queue
            .extend_pending_input_and_notify_for_turn_state(
                active_turn.turn_state.as_ref(),
                input
                    .into_iter()
                    .map(ResponseItemEnvelope::new)
                    .map(PendingTurnInput::ResponseItem)
                    .collect(),
            )
            .await;
        Ok(())
    }

    /// Applies client developer-message retention before queueing or recording items.
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "active turn checks and turn state updates must remain atomic"
    )]
    pub(crate) async fn inject_client_response_items(
        &self,
        items: Vec<impl Into<ResponseItemEnvelope>>,
        turn_context: &TurnContext,
    ) {
        let items = items
            .into_iter()
            .map(Into::into)
            .map(|item| self.annotate_client_response_item(item))
            .collect::<Vec<_>>();
        let mut active = self.active_turn.lock().await;
        if let Some(active_turn) = active.as_mut().filter(|turn| turn.finishing.is_none()) {
            self.input_queue
                .extend_pending_input_and_notify_for_turn_state(
                    active_turn.turn_state.as_ref(),
                    items
                        .into_iter()
                        .map(PendingTurnInput::ResponseItem)
                        .collect(),
                )
                .await;
            return;
        }
        let finishing = active.as_ref().and_then(|turn| turn.finishing.clone());
        drop(active);
        if let Some(finishing) = finishing {
            finishing.cancelled().await;
        }
        self.record_annotated_conversation_items(turn_context, turn_context.model_info(), items)
            .await;
    }

    pub(crate) fn annotate_client_response_item(
        &self,
        mut envelope: ResponseItemEnvelope,
    ) -> ResponseItemEnvelope {
        if self.enabled(Feature::RetainClientDeveloperMessages)
            && matches!(&envelope.item, ResponseItem::Message { role, .. } if role == "developer")
        {
            envelope
                .metadata
                .get_or_insert_default()
                .retain_across_windows
                .get_or_insert(true);
        }
        envelope
    }

    #[tracing::instrument(
        name = "record_conversation_items",
        level = "trace",
        skip_all,
        fields(item_count = items.len())
    )]
    pub(crate) async fn record_annotated_conversation_items(
        &self,
        turn_context: &TurnContext,
        model_info: &ModelInfo,
        items: Vec<ResponseItemEnvelope>,
    ) {
        let (annotated_items, image_preparations) = self
            .prepare_conversation_items_for_history(turn_context, model_info, items)
            .await;
        self.record_prepared_conversation_items(
            turn_context,
            model_info,
            annotated_items,
            image_preparations,
        )
        .await;
    }

    /// Injects items into active work, or records them without starting a turn.
    pub(crate) async fn inject_no_new_turn(
        &self,
        items: Vec<ResponseItem>,
        current_turn_context: Option<&TurnContext>,
    ) {
        let Err(items) = self.inject_if_running(items).await else {
            return;
        };
        let finishing = self
            .active_turn
            .lock()
            .await
            .as_ref()
            .and_then(|turn| turn.finishing.clone());
        if let Some(finishing) = finishing {
            finishing.cancelled().await;
        }
        let default_turn_context;
        let turn_context = match current_turn_context {
            Some(turn_context) => turn_context,
            None => {
                default_turn_context = self.new_default_turn().await;
                default_turn_context.as_ref()
            }
        };
        self.record_conversation_items(turn_context, turn_context.model_info(), &items)
            .await;
    }
}

#[cfg(test)]
#[path = "inject_tests.rs"]
mod tests;
