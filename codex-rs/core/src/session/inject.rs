use super::TurnInput as PendingTurnInput;
use super::session::Session;
use super::turn_context::TurnContext;
use codex_features::Feature;
use codex_history::CodexHarnessMetadata;
use codex_history::ResponseItemEnvelope;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelInfo;

impl Session {
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
        match active.as_mut() {
            Some(active_turn) => {
                self.input_queue
                    .extend_pending_input_and_accept_mailbox_delivery_for_turn_state(
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
            .extend_pending_input_and_accept_mailbox_delivery_for_turn_state(
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

    /// Preserves trusted client provenance while items wait for an active turn.
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "active turn checks and turn state updates must remain atomic"
    )]
    pub(crate) async fn inject_client_response_items(
        &self,
        items: Vec<ResponseItem>,
        turn_context: &TurnContext,
    ) {
        let items = items
            .into_iter()
            .map(|item| self.annotate_client_response_item(item))
            .collect::<Vec<_>>();
        let mut active = self.active_turn.lock().await;
        if let Some(active_turn) = active.as_mut() {
            self.input_queue
                .extend_pending_input_and_accept_mailbox_delivery_for_turn_state(
                    active_turn.turn_state.as_ref(),
                    items
                        .into_iter()
                        .map(PendingTurnInput::ResponseItem)
                        .collect(),
                )
                .await;
            return;
        }
        drop(active);
        self.record_annotated_conversation_items(turn_context, turn_context.model_info(), items)
            .await;
    }

    pub(crate) fn annotate_client_response_item(&self, item: ResponseItem) -> ResponseItemEnvelope {
        let metadata = (self.enabled(Feature::RetainClientDeveloperMessages)
            && matches!(&item, ResponseItem::Message { role, .. } if role == "developer"))
        .then_some(CodexHarnessMetadata {
            client_authored: true,
            ..Default::default()
        });

        ResponseItemEnvelope { item, metadata }
    }

    pub(crate) async fn record_annotated_conversation_items(
        &self,
        turn_context: &TurnContext,
        model_info: &ModelInfo,
        items: Vec<ResponseItemEnvelope>,
    ) {
        if items.iter().all(|item| item.metadata.is_none()) {
            let items = items
                .into_iter()
                .map(ResponseItemEnvelope::into_item)
                .collect::<Vec<_>>();
            self.record_conversation_items(turn_context, model_info, &items)
                .await;
            return;
        }

        let mut annotated_items = Vec::with_capacity(items.len());
        let mut image_preparations = Vec::new();
        for envelope in items {
            let (prepared_items, prepared_images) = self.prepare_conversation_items_for_history(
                turn_context,
                model_info,
                std::slice::from_ref(&envelope.item),
            );
            image_preparations.extend(prepared_images);

            let mut metadata = envelope.metadata;
            annotated_items.extend(prepared_items.into_owned().into_iter().map(|item| {
                ResponseItemEnvelope {
                    item,
                    metadata: metadata.take(),
                }
            }));
        }
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
