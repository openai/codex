//! Routes realtime input through ordinary admission and publishes its durable history projection.

use super::session::Session;
use super::turn_input;
use crate::realtime_conversation::RealtimeConversationManager;
use crate::realtime_history::handoff::HandoffSettlement;
use crate::realtime_history::handoff::PreparedRealtimeInput;
use crate::state::SessionState;
use codex_history::RealtimeHandoffMetadata;
use codex_history::ResponseItemEnvelope;
use codex_history::RolloutItem;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RealtimeConversationRealtimeEvent;
use codex_protocol::protocol::RealtimeEvent;
use codex_protocol::realtime::RealtimeHandoffMode;
use codex_protocol::realtime::RealtimeItem;
use codex_protocol::realtime::RealtimeItemContent;
use codex_protocol::turn_input::NotSubmittedReason;
use codex_protocol::turn_input::TurnInputMode;
use codex_protocol::turn_input::TurnInputRequest;
use codex_protocol::turn_input::TurnInputSubmission;
use codex_protocol::turn_input::TurnStartOptions;
use codex_user_input::ResolvedUserInput;
use codex_utils_output_truncation::TruncationPolicy;
use std::sync::Arc;
use uuid::Uuid;

/// A canceled seal has an unknown durable outcome and cannot leave the live thread usable.
struct RealtimeSealGuard {
    conversation: Option<Arc<RealtimeConversationManager>>,
}

impl RealtimeSealGuard {
    fn new(conversation: Arc<RealtimeConversationManager>) -> Self {
        Self {
            conversation: Some(conversation),
        }
    }

    fn disarm(&mut self) {
        self.conversation = None;
    }
}

impl Drop for RealtimeSealGuard {
    fn drop(&mut self) {
        if let Some(conversation) = self.conversation.take() {
            conversation.mark_history_recovery_required();
            drop(tokio::spawn(async move {
                conversation.stop_for_history_recovery().await;
            }));
        }
    }
}

impl Session {
    pub(super) async fn record_realtime_handoff_input(
        &self,
        state: &mut SessionState,
        turn_id: &str,
        mut envelope: ResponseItemEnvelope,
        prepared: &PreparedRealtimeInput,
        settings_event: Option<EventMsg>,
        policy: TruncationPolicy,
    ) -> std::io::Result<ResponseItemEnvelope> {
        if self.conversation.history_recovery_required() {
            return Err(std::io::Error::other(
                "realtime history recovery is required",
            ));
        }
        let replay = prepared.cached_envelope.is_some();
        if !replay {
            envelope.metadata.get_or_insert_default().realtime_handoff = Some(Box::new(
                self.realtime_handoff_metadata(prepared, turn_id).await?,
            ));
        }
        if envelope.item.turn_id() != Some(turn_id) || envelope.realtime_handoff_item().is_none() {
            return Err(std::io::Error::other("invalid realtime accepted input"));
        }
        // Reserve sequence and order without adding model-facing content. A failed
        // seal may leave a gap, but siblings cannot reuse an uncertain input's ID.
        state.history.prepare_item_sampling(&mut envelope);
        let staged_retained_context = state.history.stage_realtime_input(&mut envelope);
        let live = self
            .live_thread_for_persistence("accept realtime handoff")
            .map_err(std::io::Error::other)?;
        if let Some(event) = settings_event {
            live.append_items(&[RolloutItem::EventMsg(event)])
                .await
                .map_err(std::io::Error::other)?;
        }
        let prepared = self
            .cache_realtime_handoff(prepared.id, envelope.clone())
            .await?;
        let mut seal_guard = RealtimeSealGuard::new(Arc::clone(&self.conversation));
        // Confirm an uncertain write before another input can advance history. The store
        // contract makes this retry idempotent even when the first write actually landed.
        if let Err(first_error) = live
            .seal_realtime_input(prepared.new_items.clone(), envelope.clone())
            .await
            && let Err(error) = live
                .seal_realtime_input(prepared.new_items.clone(), envelope.clone())
                .await
        {
            tracing::warn!(%first_error, %error, "could not confirm realtime input seal");
            self.conversation.mark_history_recovery_required();
            seal_guard.disarm();
            return Err(std::io::Error::other(error));
        }
        // Fresh recording exposes the source only after capturing the review backup;
        // a cached retry replays its already persisted provenance.
        let fresh_source = if replay {
            None
        } else {
            envelope
                .metadata
                .as_mut()
                .and_then(|metadata| metadata.retained_source.take())
        };
        state
            .history
            .record_sealed_realtime_input(&envelope, staged_retained_context, policy);
        seal_guard.disarm();
        if let Some(source) = fresh_source {
            envelope.metadata.get_or_insert_default().retained_source = Some(source);
        }
        Ok(envelope)
    }

    pub(crate) async fn route_realtime_handoff(
        self: &Arc<Self>,
        handoff: &codex_protocol::protocol::RealtimeHandoffRequested,
        preserve_transcript: bool,
    ) -> Result<(), &'static str> {
        let history = match &self.realtime_history {
            Some(history) if history.lock().await.frames_enabled() => history,
            _ => {
                if let Some(text) = crate::realtime_conversation::realtime_delegation_from_handoff(
                    handoff,
                    preserve_transcript,
                ) {
                    return self.route_realtime_text_input(text).await;
                }
                return Ok(());
            }
        };
        let prepared = {
            let mut state = history.lock().await;
            let Some(prepared) = state.prepare_handoff(handoff) else {
                return Ok(());
            };
            if !state.reserve_handoff(prepared.id) {
                return Ok(());
            }
            prepared
        };
        let submission_id = prepared.id.to_string();
        let mode = if let Some(envelope) = &prepared.cached_envelope {
            let Some(target) = envelope.item.turn_id() else {
                self.reject_realtime_handoff(prepared.id).await;
                return Err("realtime handoff is missing its receiving turn");
            };
            if self.turn_context_for_sub_id(target).await.is_some() {
                TurnInputMode::Steer {
                    expected_turn_id: target.to_string(),
                }
            } else if target == submission_id
                && envelope
                    .metadata
                    .as_ref()
                    .and_then(|metadata| metadata.realtime_handoff.as_ref())
                    .is_some_and(|metadata| metadata.mode == RealtimeHandoffMode::Start)
            {
                TurnInputMode::StartIfIdle
            } else {
                self.reject_realtime_handoff(prepared.id).await;
                return Err("resume the receiving turn before retrying this realtime handoff");
            }
        } else {
            TurnInputMode::StartOrSteer
        };
        let mut request =
            TurnInputRequest::user_input(vec![ResolvedUserInput::text(prepared.input.clone())])
                .on_start(TurnStartOptions {
                    turn_trigger: Some("realtime".to_string()),
                    ..Default::default()
                });
        request.user_item_uuid = Some(prepared.id);
        match turn_input::handle_realtime_handoff(
            self,
            request,
            mode,
            submission_id.clone(),
            prepared.id,
        )
        .await
        {
            Ok(TurnInputSubmission::Accepted) => {
                unreachable!("realtime user input returns a turn outcome")
            }
            Ok(TurnInputSubmission::Started { .. } | TurnInputSubmission::Steered { .. }) => Ok(()),
            Ok(TurnInputSubmission::NotSubmitted {
                reason: NotSubmittedReason::ServerDraining,
            }) => {
                self.reject_realtime_handoff(prepared.id).await;
                Err("Server is draining; retry the turn after reconnecting")
            }
            Ok(TurnInputSubmission::NotSubmitted { reason }) => {
                self.reject_realtime_handoff(prepared.id).await;
                self.report_realtime_input_decline(submission_id, reason)
                    .await;
                Ok(())
            }
            Err(error) => {
                self.reject_realtime_handoff(prepared.id).await;
                tracing::warn!("failed to admit realtime handoff: {error}");
                Err("failed to admit realtime handoff")
            }
        }
    }

    pub(crate) async fn route_realtime_text_input(
        self: &Arc<Self>,
        text: String,
    ) -> Result<(), &'static str> {
        let submission_id = Uuid::now_v7().to_string();
        let submission = turn_input::handle(
            self,
            TurnInputRequest::user_input(vec![ResolvedUserInput::text(text)]).on_start(
                TurnStartOptions {
                    turn_trigger: Some("realtime".to_string()),
                    ..Default::default()
                },
            ),
            TurnInputMode::StartOrSteer,
            submission_id.clone(),
        )
        .await;
        match submission {
            Ok(TurnInputSubmission::Accepted) => {
                unreachable!("realtime user input returns a turn outcome")
            }
            Ok(TurnInputSubmission::Started { .. } | TurnInputSubmission::Steered { .. }) => {}
            Ok(TurnInputSubmission::NotSubmitted {
                reason: NotSubmittedReason::ServerDraining,
            }) => {
                return Err("Server is draining; retry the turn after reconnecting");
            }
            Ok(TurnInputSubmission::NotSubmitted { reason }) => {
                self.report_realtime_input_decline(submission_id, reason)
                    .await;
            }
            Err(error) => {
                self.send_event_raw(Event {
                    id: submission_id,
                    msg: EventMsg::Error(error.to_error_event(/*message_prefix*/ None)),
                })
                .await;
            }
        }
        Ok(())
    }

    async fn report_realtime_input_decline(
        &self,
        submission_id: String,
        reason: NotSubmittedReason,
    ) {
        self.send_event_raw(Event {
            id: submission_id,
            msg: EventMsg::Error(ErrorEvent {
                misalignment: None,
                message: format!("failed to submit turn input: {reason:?}"),
                codex_error_info: Some(CodexErrorInfo::BadRequest),
                rejected_audio_file_ids: Vec::new(),
            }),
        })
        .await;
    }

    /// UUID and exact text must both match a reserved internal input. Client UUIDs alone
    /// do not attach realtime provenance to an ordinary prompt.
    pub(crate) async fn pending_realtime_handoff(
        &self,
        id: Uuid,
        input: &[ResolvedUserInput],
    ) -> Option<PreparedRealtimeInput> {
        let [ResolvedUserInput::Text { text, .. }] = input else {
            return None;
        };
        let prepared = self
            .realtime_history
            .as_ref()?
            .lock()
            .await
            .pending_handoff(id)?;
        (prepared.reserved && prepared.input == *text).then_some(prepared)
    }

    pub(crate) async fn realtime_handoff_metadata(
        &self,
        prepared: &PreparedRealtimeInput,
        turn_id: &str,
    ) -> std::io::Result<RealtimeHandoffMetadata> {
        let history = self
            .realtime_history
            .as_ref()
            .ok_or_else(|| std::io::Error::other("realtime history is disabled"))?;
        let observed_at_ms = history.lock().await.observation_clock.now_ms();
        Ok(RealtimeHandoffMetadata {
            event_id: prepared.id.to_string(),
            realtime_session_id: prepared.session_id.clone(),
            observed_at_ms,
            handoff_id: prepared.handoff_id.clone(),
            mode: if turn_id == prepared.id.to_string() {
                RealtimeHandoffMode::Start
            } else {
                RealtimeHandoffMode::Steer
            },
            source_event_ids: prepared.sources.clone(),
        })
    }

    pub(crate) async fn cache_realtime_handoff(
        &self,
        id: Uuid,
        envelope: ResponseItemEnvelope,
    ) -> std::io::Result<PreparedRealtimeInput> {
        self.realtime_history
            .as_ref()
            .ok_or_else(|| std::io::Error::other("realtime history is disabled"))?
            .lock()
            .await
            .cache_handoff_input(id, envelope)
            .ok_or_else(|| std::io::Error::other("realtime handoff is no longer pending"))
    }

    pub(crate) async fn finish_realtime_handoff(
        &self,
        prepared: &PreparedRealtimeInput,
        envelope: &ResponseItemEnvelope,
    ) {
        if let Some(history) = &self.realtime_history {
            history
                .lock()
                .await
                .finish_handoff(prepared.id, envelope.item.turn_id());
        }
        self.publish_realtime_acceptance(&prepared.new_items).await;
        if let Some(receipt) = envelope.realtime_handoff_item() {
            self.publish_realtime_acceptance(std::slice::from_ref(&receipt))
                .await;
        }
    }

    pub(crate) async fn bind_realtime_handoffs_to_turn(
        &self,
        input: &[super::TurnInput],
        turn_id: &str,
    ) {
        let ids = input
            .iter()
            .filter_map(|input| match input {
                super::TurnInput::UserInput { metadata, .. } => metadata.realtime_handoff_id,
                _ => None,
            })
            .collect::<Vec<_>>();
        if ids.is_empty() {
            return;
        }
        if let Some(history) = &self.realtime_history {
            let mut state = history.lock().await;
            for id in ids {
                state.bind_handoff_to_turn(id, turn_id);
            }
        }
    }

    pub(crate) async fn reject_realtime_handoffs_for_turn(&self, turn_id: &str) {
        if let Some(history) = &self.realtime_history {
            history.lock().await.reject_handoffs_for_turn(turn_id);
        }
    }

    /// Wait for admitted inputs to be durable or rejected, not for their agent turns to finish.
    pub(crate) async fn wait_for_realtime_handoffs(&self) -> std::io::Result<()> {
        let Some(history) = &self.realtime_history else {
            return Ok(());
        };
        loop {
            let pending = history.lock().await.pending_handoff_settlement();
            match pending? {
                HandoffSettlement::Wait(settled) => settled.notified().await,
                HandoffSettlement::Settled => return Ok(()),
            }
        }
    }

    pub(crate) async fn reject_realtime_handoff(&self, id: Uuid) {
        if let Some(history) = &self.realtime_history {
            history.lock().await.reject_handoff(id);
        }
    }

    async fn publish_realtime_acceptance(&self, records: &[RealtimeItem]) {
        for item in records {
            let event = |payload| Event {
                id: item.id.clone(),
                msg: EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
                    payload,
                }),
            };
            if !matches!(item.content, RealtimeItemContent::Frame { .. }) {
                self.deliver_event_raw(event(RealtimeEvent::HistoryItemStarted(item.clone())))
                    .await;
            }
            self.deliver_event_raw(event(RealtimeEvent::HistoryItemCompleted(item.clone())))
                .await;
        }
    }
}
