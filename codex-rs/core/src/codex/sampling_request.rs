use super::*;

pub(super) struct SamplingRequestResult {
    pub(super) needs_follow_up: bool,
    pub(super) last_agent_message: Option<String>,
}

pub(crate) fn get_last_assistant_message_from_turn(responses: &[ResponseItem]) -> Option<String> {
    for item in responses.iter().rev() {
        if let Some(message) = last_assistant_message_from_item(item, /*plan_mode*/ false) {
            return Some(message);
        }
    }
    None
}

/// Ephemeral per-response state for streaming a single proposed plan.
/// This is intentionally not persisted or stored in session/state since it
/// only exists while a response is actively streaming. The final plan text
/// is extracted from the completed assistant message.
/// Tracks a single proposed plan item across a streaming response.
struct ProposedPlanItemState {
    item_id: String,
    started: bool,
    completed: bool,
}

/// Aggregated state used only while streaming a plan-mode response.
/// Includes per-item parsers, deferred agent message bookkeeping, and the plan item lifecycle.
struct PlanModeStreamState {
    /// Agent message items started by the model but deferred until we see non-plan text.
    pending_agent_message_items: HashMap<String, TurnItem>,
    /// Agent message items whose start notification has been emitted.
    started_agent_message_items: HashSet<String>,
    /// Leading whitespace buffered until we see non-whitespace text for an item.
    leading_whitespace_by_item: HashMap<String, String>,
    /// Tracks plan item lifecycle while streaming plan output.
    plan_item_state: ProposedPlanItemState,
}

#[derive(Debug, Default)]
pub(super) struct AssistantMessageStreamParsers {
    plan_mode: bool,
    parsers_by_item: HashMap<String, AssistantTextStreamParser>,
}

type ParsedAssistantTextDelta = AssistantTextChunk;

struct SamplingLoopState {
    in_flight: FuturesOrdered<BoxFuture<'static, CodexResult<ResponseInputItem>>>,
    needs_follow_up: bool,
    last_agent_message: Option<String>,
    active_item: Option<TurnItem>,
    should_emit_turn_diff: bool,
    server_model_warning_emitted: bool,
    assistant_message_stream_parsers: AssistantMessageStreamParsers,
    plan_mode_state: Option<PlanModeStreamState>,
}

impl SamplingLoopState {
    fn new(turn_context: &TurnContext, server_model_warning_emitted: bool) -> Self {
        let plan_mode = turn_context.collaboration_mode.mode == ModeKind::Plan;
        Self {
            in_flight: FuturesOrdered::new(),
            needs_follow_up: false,
            last_agent_message: None,
            active_item: None,
            should_emit_turn_diff: false,
            server_model_warning_emitted,
            assistant_message_stream_parsers: AssistantMessageStreamParsers::new(plan_mode),
            plan_mode_state: plan_mode.then(|| PlanModeStreamState::new(&turn_context.sub_id)),
        }
    }

    async fn handle_event(
        &mut self,
        tool_runtime: &ToolCallRuntime,
        sess: &Arc<Session>,
        turn_context: &Arc<TurnContext>,
        cancellation_token: &CancellationToken,
        handle_responses: &tracing::Span,
        event: ResponseEvent,
    ) -> CodexResult<Option<SamplingRequestResult>> {
        match event {
            ResponseEvent::Created => Ok(None),
            ResponseEvent::OutputItemDone(item) => {
                self.handle_output_item_done(
                    tool_runtime,
                    sess,
                    turn_context,
                    cancellation_token,
                    handle_responses,
                    item,
                )
                .await?;
                Ok(None)
            }
            ResponseEvent::OutputItemAdded(item) => {
                self.handle_output_item_added(sess, turn_context, item)
                    .await;
                Ok(None)
            }
            ResponseEvent::ServerModel(server_model) => {
                self.handle_server_model(sess, turn_context, server_model)
                    .await;
                Ok(None)
            }
            ResponseEvent::ServerReasoningIncluded(included) => {
                sess.set_server_reasoning_included(included).await;
                Ok(None)
            }
            ResponseEvent::RateLimits(snapshot) => {
                sess.update_rate_limits(turn_context, snapshot).await;
                Ok(None)
            }
            ResponseEvent::ModelsEtag(etag) => {
                sess.services.models_manager.refresh_if_new_etag(etag).await;
                Ok(None)
            }
            ResponseEvent::Completed {
                response_id: _,
                token_usage,
            } => {
                sess.update_token_usage_info(turn_context, token_usage.as_ref())
                    .await;
                self.should_emit_turn_diff = true;
                self.needs_follow_up |= sess.has_pending_input().await;
                Ok(Some(SamplingRequestResult {
                    needs_follow_up: self.needs_follow_up,
                    last_agent_message: self.last_agent_message.clone(),
                }))
            }
            ResponseEvent::OutputTextDelta(delta) => {
                self.handle_output_text_delta(sess, turn_context, delta)
                    .await;
                Ok(None)
            }
            ResponseEvent::ReasoningSummaryDelta {
                delta,
                summary_index,
            } => {
                send_reasoning_summary_delta(
                    sess,
                    turn_context,
                    self.active_item_id("ReasoningSummaryDelta"),
                    delta,
                    summary_index,
                )
                .await;
                Ok(None)
            }
            ResponseEvent::ReasoningSummaryPartAdded { summary_index } => {
                send_reasoning_summary_part_added(
                    sess,
                    turn_context,
                    self.active_item_id("ReasoningSummaryPartAdded"),
                    summary_index,
                )
                .await;
                Ok(None)
            }
            ResponseEvent::ReasoningContentDelta {
                delta,
                content_index,
            } => {
                send_reasoning_content_delta(
                    sess,
                    turn_context,
                    self.active_item_id("ReasoningRawContentDelta"),
                    delta,
                    content_index,
                )
                .await;
                Ok(None)
            }
        }
    }

    async fn handle_output_item_done(
        &mut self,
        tool_runtime: &ToolCallRuntime,
        sess: &Arc<Session>,
        turn_context: &Arc<TurnContext>,
        cancellation_token: &CancellationToken,
        handle_responses: &tracing::Span,
        item: ResponseItem,
    ) -> CodexResult<()> {
        let previously_active_item = self.active_item.take();
        if let Some(previous) = previously_active_item.as_ref()
            && matches!(previous, TurnItem::AgentMessage(_))
        {
            let item_id = previous.id();
            flush_assistant_text_segments_for_item(
                sess,
                turn_context,
                self.plan_mode_state.as_mut(),
                &mut self.assistant_message_stream_parsers,
                &item_id,
            )
            .await;
        }
        if let Some(state) = self.plan_mode_state.as_mut()
            && handle_assistant_item_done_in_plan_mode(
                sess,
                turn_context,
                &item,
                state,
                previously_active_item.as_ref(),
                &mut self.last_agent_message,
            )
            .await
        {
            return Ok(());
        }

        let mut ctx = HandleOutputCtx {
            sess: sess.clone(),
            turn_context: turn_context.clone(),
            tool_runtime: tool_runtime.clone(),
            cancellation_token: cancellation_token.child_token(),
        };
        let output_result = handle_output_item_done(&mut ctx, item, previously_active_item)
            .instrument(handle_responses.clone())
            .await?;
        if let Some(tool_future) = output_result.tool_future {
            self.in_flight.push_back(tool_future);
        }
        if let Some(agent_message) = output_result.last_agent_message {
            self.last_agent_message = Some(agent_message);
        }
        self.needs_follow_up |= output_result.needs_follow_up;
        Ok(())
    }

    async fn handle_output_item_added(
        &mut self,
        sess: &Arc<Session>,
        turn_context: &Arc<TurnContext>,
        item: ResponseItem,
    ) {
        let plan_mode = self.plan_mode_state.is_some();
        if let Some(turn_item) =
            handle_non_tool_response_item(sess.as_ref(), turn_context.as_ref(), &item, plan_mode)
                .await
        {
            let mut turn_item = turn_item;
            let mut seeded_parsed: Option<ParsedAssistantTextDelta> = None;
            let mut seeded_item_id: Option<String> = None;
            if matches!(turn_item, TurnItem::AgentMessage(_))
                && let Some(raw_text) = raw_assistant_output_text_from_item(&item)
            {
                let item_id = turn_item.id();
                let mut seeded = self
                    .assistant_message_stream_parsers
                    .seed_item_text(&item_id, &raw_text);
                if let TurnItem::AgentMessage(agent_message) = &mut turn_item {
                    agent_message.content =
                        vec![codex_protocol::items::AgentMessageContent::Text {
                            text: if plan_mode {
                                String::new()
                            } else {
                                std::mem::take(&mut seeded.visible_text)
                            },
                        }];
                }
                seeded_parsed = plan_mode.then_some(seeded);
                seeded_item_id = Some(item_id);
            }
            if let Some(state) = self.plan_mode_state.as_mut()
                && matches!(turn_item, TurnItem::AgentMessage(_))
            {
                let item_id = turn_item.id();
                state
                    .pending_agent_message_items
                    .insert(item_id, turn_item.clone());
            } else {
                sess.emit_turn_item_started(turn_context, &turn_item).await;
            }
            if let (Some(state), Some(item_id), Some(parsed)) = (
                self.plan_mode_state.as_mut(),
                seeded_item_id.as_deref(),
                seeded_parsed,
            ) {
                emit_streamed_assistant_text_delta(
                    sess,
                    turn_context,
                    Some(state),
                    item_id,
                    parsed,
                )
                .await;
            }
            self.active_item = Some(turn_item);
        }
    }

    async fn handle_server_model(
        &mut self,
        sess: &Arc<Session>,
        turn_context: &Arc<TurnContext>,
        server_model: String,
    ) {
        if !self.server_model_warning_emitted
            && sess
                .maybe_warn_on_server_model_mismatch(turn_context, server_model)
                .await
        {
            self.server_model_warning_emitted = true;
        }
    }

    async fn handle_output_text_delta(
        &mut self,
        sess: &Arc<Session>,
        turn_context: &Arc<TurnContext>,
        delta: String,
    ) {
        if let Some(active) = self.active_item.as_ref() {
            let item_id = active.id();
            if matches!(active, TurnItem::AgentMessage(_)) {
                let parsed = self
                    .assistant_message_stream_parsers
                    .parse_delta(&item_id, &delta);
                emit_streamed_assistant_text_delta(
                    sess,
                    turn_context,
                    self.plan_mode_state.as_mut(),
                    &item_id,
                    parsed,
                )
                .await;
            } else {
                let event = AgentMessageContentDeltaEvent {
                    thread_id: sess.conversation_id.to_string(),
                    turn_id: turn_context.sub_id.clone(),
                    item_id,
                    delta,
                };
                sess.send_event(turn_context, EventMsg::AgentMessageContentDelta(event))
                    .await;
            }
        } else {
            error_or_panic("OutputTextDelta without active item".to_string());
        }
    }

    fn active_item_id(&self, event_name: &str) -> Option<String> {
        if let Some(active) = self.active_item.as_ref() {
            Some(active.id())
        } else {
            error_or_panic(format!("{event_name} without active item"));
            None
        }
    }

    async fn finalize(
        &mut self,
        sess: Arc<Session>,
        turn_context: Arc<TurnContext>,
        turn_diff_tracker: SharedTurnDiffTracker,
        cancellation_token: CancellationToken,
    ) -> CodexResult<()> {
        flush_assistant_text_segments_all(
            &sess,
            &turn_context,
            self.plan_mode_state.as_mut(),
            &mut self.assistant_message_stream_parsers,
        )
        .await;

        drain_in_flight(&mut self.in_flight, sess.clone(), turn_context.clone()).await?;

        if cancellation_token.is_cancelled() {
            return Err(CodexErr::TurnAborted);
        }

        if self.should_emit_turn_diff {
            let unified_diff = {
                let mut tracker = turn_diff_tracker.lock().await;
                tracker.get_unified_diff()
            };
            if let Ok(Some(unified_diff)) = unified_diff {
                let msg = EventMsg::TurnDiff(TurnDiffEvent { unified_diff });
                sess.send_event(&turn_context, msg).await;
            }
        }

        Ok(())
    }
}

async fn send_reasoning_summary_delta(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    item_id: Option<String>,
    delta: String,
    summary_index: i64,
) {
    let Some(item_id) = item_id else {
        return;
    };
    let event = ReasoningContentDeltaEvent {
        thread_id: sess.conversation_id.to_string(),
        turn_id: turn_context.sub_id.clone(),
        item_id,
        delta,
        summary_index,
    };
    sess.send_event(turn_context, EventMsg::ReasoningContentDelta(event))
        .await;
}

async fn send_reasoning_summary_part_added(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    item_id: Option<String>,
    summary_index: i64,
) {
    let Some(item_id) = item_id else {
        return;
    };
    let event = EventMsg::AgentReasoningSectionBreak(AgentReasoningSectionBreakEvent {
        item_id,
        summary_index,
    });
    sess.send_event(turn_context, event).await;
}

async fn send_reasoning_content_delta(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    item_id: Option<String>,
    delta: String,
    content_index: i64,
) {
    let Some(item_id) = item_id else {
        return;
    };
    let event = ReasoningRawContentDeltaEvent {
        thread_id: sess.conversation_id.to_string(),
        turn_id: turn_context.sub_id.clone(),
        item_id,
        delta,
        content_index,
    };
    sess.send_event(turn_context, EventMsg::ReasoningRawContentDelta(event))
        .await;
}

impl PlanModeStreamState {
    fn new(turn_id: &str) -> Self {
        Self {
            pending_agent_message_items: HashMap::new(),
            started_agent_message_items: HashSet::new(),
            leading_whitespace_by_item: HashMap::new(),
            plan_item_state: ProposedPlanItemState::new(turn_id),
        }
    }
}

impl AssistantMessageStreamParsers {
    pub(super) fn new(plan_mode: bool) -> Self {
        Self {
            plan_mode,
            parsers_by_item: HashMap::new(),
        }
    }

    fn parser_mut(&mut self, item_id: &str) -> &mut AssistantTextStreamParser {
        let plan_mode = self.plan_mode;
        self.parsers_by_item
            .entry(item_id.to_string())
            .or_insert_with(|| AssistantTextStreamParser::new(plan_mode))
    }

    pub(super) fn seed_item_text(&mut self, item_id: &str, text: &str) -> ParsedAssistantTextDelta {
        if text.is_empty() {
            return ParsedAssistantTextDelta::default();
        }
        self.parser_mut(item_id).push_str(text)
    }

    pub(super) fn parse_delta(&mut self, item_id: &str, delta: &str) -> ParsedAssistantTextDelta {
        self.parser_mut(item_id).push_str(delta)
    }

    pub(super) fn finish_item(&mut self, item_id: &str) -> ParsedAssistantTextDelta {
        let Some(mut parser) = self.parsers_by_item.remove(item_id) else {
            return ParsedAssistantTextDelta::default();
        };
        parser.finish()
    }

    fn drain_finished(&mut self) -> Vec<(String, ParsedAssistantTextDelta)> {
        let parsers_by_item = std::mem::take(&mut self.parsers_by_item);
        parsers_by_item
            .into_iter()
            .map(|(item_id, mut parser)| (item_id, parser.finish()))
            .collect()
    }
}

impl ProposedPlanItemState {
    fn new(turn_id: &str) -> Self {
        Self {
            item_id: format!("{turn_id}-plan"),
            started: false,
            completed: false,
        }
    }

    async fn start(&mut self, sess: &Session, turn_context: &TurnContext) {
        if self.started || self.completed {
            return;
        }
        self.started = true;
        let item = TurnItem::Plan(PlanItem {
            id: self.item_id.clone(),
            text: String::new(),
        });
        sess.emit_turn_item_started(turn_context, &item).await;
    }

    async fn push_delta(&mut self, sess: &Session, turn_context: &TurnContext, delta: &str) {
        if self.completed || delta.is_empty() {
            return;
        }
        let event = PlanDeltaEvent {
            thread_id: sess.conversation_id.to_string(),
            turn_id: turn_context.sub_id.clone(),
            item_id: self.item_id.clone(),
            delta: delta.to_string(),
        };
        sess.send_event(turn_context, EventMsg::PlanDelta(event))
            .await;
    }

    async fn complete_with_text(
        &mut self,
        sess: &Session,
        turn_context: &TurnContext,
        text: String,
    ) {
        if self.completed || !self.started {
            return;
        }
        self.completed = true;
        let item = TurnItem::Plan(PlanItem {
            id: self.item_id.clone(),
            text,
        });
        sess.emit_turn_item_completed(turn_context, item).await;
    }
}

#[allow(clippy::too_many_arguments)]
#[instrument(level = "trace",
    skip_all,
    fields(
        turn_id = %turn_context.sub_id,
        model = %turn_context.model_info.slug
    )
)]
pub(super) async fn try_run_sampling_request(
    tool_runtime: ToolCallRuntime,
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    client_session: &mut ModelClientSession,
    turn_metadata_header: Option<&str>,
    turn_diff_tracker: SharedTurnDiffTracker,
    server_model_warning_emitted_for_turn: &mut bool,
    prompt: &Prompt,
    cancellation_token: CancellationToken,
) -> CodexResult<SamplingRequestResult> {
    feedback_tags!(
        model = turn_context.model_info.slug.clone(),
        approval_policy = turn_context.approval_policy.value(),
        sandbox_policy = turn_context.sandbox_policy.get(),
        effort = turn_context.reasoning_effort,
        auth_mode = sess.services.auth_manager.auth_mode(),
        features = sess.features.enabled_features(),
    );
    let mut stream = client_session
        .stream(
            prompt,
            &turn_context.model_info,
            &turn_context.session_telemetry,
            turn_context.reasoning_effort,
            turn_context.reasoning_summary,
            turn_context.config.service_tier,
            turn_metadata_header,
        )
        .instrument(trace_span!("stream_request"))
        .or_cancel(&cancellation_token)
        .await??;
    let receiving_span = trace_span!("receiving_stream");
    let mut state = SamplingLoopState::new(
        turn_context.as_ref(),
        *server_model_warning_emitted_for_turn,
    );
    let outcome: CodexResult<SamplingRequestResult> = loop {
        let handle_responses = trace_span!(
            parent: &receiving_span,
            "handle_responses",
            otel.name = field::Empty,
            tool_name = field::Empty,
            from = field::Empty,
        );

        let event = match stream
            .next()
            .instrument(trace_span!(parent: &handle_responses, "receiving"))
            .or_cancel(&cancellation_token)
            .await
        {
            Ok(event) => event,
            Err(codex_async_utils::CancelErr::Cancelled) => break Err(CodexErr::TurnAborted),
        };

        let event = match event {
            Some(res) => res?,
            None => {
                break Err(CodexErr::Stream(
                    "stream closed before response.completed".into(),
                    None,
                ));
            }
        };

        sess.services
            .session_telemetry
            .record_responses(&handle_responses, &event);
        record_turn_ttft_metric(&turn_context, &event).await;

        if let Some(result) = state
            .handle_event(
                &tool_runtime,
                &sess,
                &turn_context,
                &cancellation_token,
                &handle_responses,
                event,
            )
            .await?
        {
            break Ok(result);
        }
    };

    state
        .finalize(
            sess.clone(),
            turn_context.clone(),
            turn_diff_tracker,
            cancellation_token,
        )
        .await?;
    *server_model_warning_emitted_for_turn = state.server_model_warning_emitted;

    outcome
}

/// In plan mode we defer agent message starts until the parser emits non-plan
/// text. The parser buffers each line until it can rule out a tag prefix, so
/// plan-only outputs never show up as empty assistant messages.
async fn maybe_emit_pending_agent_message_start(
    sess: &Session,
    turn_context: &TurnContext,
    state: &mut PlanModeStreamState,
    item_id: &str,
) {
    if state.started_agent_message_items.contains(item_id) {
        return;
    }
    if let Some(item) = state.pending_agent_message_items.remove(item_id) {
        sess.emit_turn_item_started(turn_context, &item).await;
        state
            .started_agent_message_items
            .insert(item_id.to_string());
    }
}

/// Agent messages are text-only today; concatenate all text entries.
fn agent_message_text(item: &codex_protocol::items::AgentMessageItem) -> String {
    item.content
        .iter()
        .map(|entry| match entry {
            codex_protocol::items::AgentMessageContent::Text { text } => text.as_str(),
        })
        .collect()
}

pub(super) fn realtime_text_for_event(msg: &EventMsg) -> Option<String> {
    match msg {
        EventMsg::AgentMessage(event) => Some(event.message.clone()),
        EventMsg::ItemCompleted(event) => match &event.item {
            TurnItem::AgentMessage(item) => Some(agent_message_text(item)),
            _ => None,
        },
        EventMsg::Error(_)
        | EventMsg::Warning(_)
        | EventMsg::RealtimeConversationStarted(_)
        | EventMsg::RealtimeConversationRealtime(_)
        | EventMsg::RealtimeConversationClosed(_)
        | EventMsg::ModelReroute(_)
        | EventMsg::ContextCompacted(_)
        | EventMsg::ThreadRolledBack(_)
        | EventMsg::TurnStarted(_)
        | EventMsg::TurnComplete(_)
        | EventMsg::TokenCount(_)
        | EventMsg::UserMessage(_)
        | EventMsg::AgentMessageDelta(_)
        | EventMsg::AgentReasoning(_)
        | EventMsg::AgentReasoningDelta(_)
        | EventMsg::AgentReasoningRawContent(_)
        | EventMsg::AgentReasoningRawContentDelta(_)
        | EventMsg::AgentReasoningSectionBreak(_)
        | EventMsg::SessionConfigured(_)
        | EventMsg::ThreadNameUpdated(_)
        | EventMsg::McpStartupUpdate(_)
        | EventMsg::McpStartupComplete(_)
        | EventMsg::McpToolCallBegin(_)
        | EventMsg::McpToolCallEnd(_)
        | EventMsg::WebSearchBegin(_)
        | EventMsg::WebSearchEnd(_)
        | EventMsg::ExecCommandBegin(_)
        | EventMsg::ExecCommandOutputDelta(_)
        | EventMsg::TerminalInteraction(_)
        | EventMsg::ExecCommandEnd(_)
        | EventMsg::PatchApplyBegin(_)
        | EventMsg::PatchApplyEnd(_)
        | EventMsg::ViewImageToolCall(_)
        | EventMsg::ImageGenerationBegin(_)
        | EventMsg::ImageGenerationEnd(_)
        | EventMsg::ExecApprovalRequest(_)
        | EventMsg::RequestPermissions(_)
        | EventMsg::RequestUserInput(_)
        | EventMsg::DynamicToolCallRequest(_)
        | EventMsg::DynamicToolCallResponse(_)
        | EventMsg::GuardianAssessment(_)
        | EventMsg::ElicitationRequest(_)
        | EventMsg::ApplyPatchApprovalRequest(_)
        | EventMsg::DeprecationNotice(_)
        | EventMsg::BackgroundEvent(_)
        | EventMsg::UndoStarted(_)
        | EventMsg::UndoCompleted(_)
        | EventMsg::StreamError(_)
        | EventMsg::TurnDiff(_)
        | EventMsg::GetHistoryEntryResponse(_)
        | EventMsg::McpListToolsResponse(_)
        | EventMsg::ListSkillsResponse(_)
        | EventMsg::SkillsUpdateAvailable
        | EventMsg::PlanUpdate(_)
        | EventMsg::TurnAborted(_)
        | EventMsg::ShutdownComplete
        | EventMsg::EnteredReviewMode(_)
        | EventMsg::ExitedReviewMode(_)
        | EventMsg::RawResponseItem(_)
        | EventMsg::ItemStarted(_)
        | EventMsg::HookStarted(_)
        | EventMsg::HookCompleted(_)
        | EventMsg::AgentMessageContentDelta(_)
        | EventMsg::PlanDelta(_)
        | EventMsg::ReasoningContentDelta(_)
        | EventMsg::ReasoningRawContentDelta(_)
        | EventMsg::CollabAgentSpawnBegin(_)
        | EventMsg::CollabAgentSpawnEnd(_)
        | EventMsg::CollabAgentInteractionBegin(_)
        | EventMsg::CollabAgentInteractionEnd(_)
        | EventMsg::CollabWaitingBegin(_)
        | EventMsg::CollabWaitingEnd(_)
        | EventMsg::CollabCloseBegin(_)
        | EventMsg::CollabCloseEnd(_)
        | EventMsg::CollabResumeBegin(_)
        | EventMsg::CollabResumeEnd(_) => None,
    }
}

/// Split the stream into normal assistant text vs. proposed plan content.
/// Normal text becomes AgentMessage deltas; plan content becomes PlanDelta +
/// TurnItem::Plan.
async fn handle_plan_segments(
    sess: &Session,
    turn_context: &TurnContext,
    state: &mut PlanModeStreamState,
    item_id: &str,
    segments: Vec<ProposedPlanSegment>,
) {
    for segment in segments {
        match segment {
            ProposedPlanSegment::Normal(delta) => {
                if delta.is_empty() {
                    continue;
                }
                let has_non_whitespace = delta.chars().any(|ch| !ch.is_whitespace());
                if !has_non_whitespace && !state.started_agent_message_items.contains(item_id) {
                    let entry = state
                        .leading_whitespace_by_item
                        .entry(item_id.to_string())
                        .or_default();
                    entry.push_str(&delta);
                    continue;
                }
                let delta = if !state.started_agent_message_items.contains(item_id) {
                    if let Some(prefix) = state.leading_whitespace_by_item.remove(item_id) {
                        format!("{prefix}{delta}")
                    } else {
                        delta
                    }
                } else {
                    delta
                };
                maybe_emit_pending_agent_message_start(sess, turn_context, state, item_id).await;

                let event = AgentMessageContentDeltaEvent {
                    thread_id: sess.conversation_id.to_string(),
                    turn_id: turn_context.sub_id.clone(),
                    item_id: item_id.to_string(),
                    delta,
                };
                sess.send_event(turn_context, EventMsg::AgentMessageContentDelta(event))
                    .await;
            }
            ProposedPlanSegment::ProposedPlanStart => {
                if !state.plan_item_state.completed {
                    state.plan_item_state.start(sess, turn_context).await;
                }
            }
            ProposedPlanSegment::ProposedPlanDelta(delta) => {
                if !state.plan_item_state.completed {
                    if !state.plan_item_state.started {
                        state.plan_item_state.start(sess, turn_context).await;
                    }
                    state
                        .plan_item_state
                        .push_delta(sess, turn_context, &delta)
                        .await;
                }
            }
            ProposedPlanSegment::ProposedPlanEnd => {}
        }
    }
}

async fn emit_streamed_assistant_text_delta(
    sess: &Session,
    turn_context: &TurnContext,
    plan_mode_state: Option<&mut PlanModeStreamState>,
    item_id: &str,
    parsed: ParsedAssistantTextDelta,
) {
    if parsed.is_empty() {
        return;
    }
    if !parsed.citations.is_empty() {
        let _citations = parsed.citations;
    }
    if let Some(state) = plan_mode_state {
        if !parsed.plan_segments.is_empty() {
            handle_plan_segments(sess, turn_context, state, item_id, parsed.plan_segments).await;
        }
        return;
    }
    if parsed.visible_text.is_empty() {
        return;
    }
    let event = AgentMessageContentDeltaEvent {
        thread_id: sess.conversation_id.to_string(),
        turn_id: turn_context.sub_id.clone(),
        item_id: item_id.to_string(),
        delta: parsed.visible_text,
    };
    sess.send_event(turn_context, EventMsg::AgentMessageContentDelta(event))
        .await;
}

async fn flush_assistant_text_segments_for_item(
    sess: &Session,
    turn_context: &TurnContext,
    plan_mode_state: Option<&mut PlanModeStreamState>,
    parsers: &mut AssistantMessageStreamParsers,
    item_id: &str,
) {
    let parsed = parsers.finish_item(item_id);
    emit_streamed_assistant_text_delta(sess, turn_context, plan_mode_state, item_id, parsed).await;
}

async fn flush_assistant_text_segments_all(
    sess: &Session,
    turn_context: &TurnContext,
    mut plan_mode_state: Option<&mut PlanModeStreamState>,
    parsers: &mut AssistantMessageStreamParsers,
) {
    for (item_id, parsed) in parsers.drain_finished() {
        emit_streamed_assistant_text_delta(
            sess,
            turn_context,
            plan_mode_state.as_deref_mut(),
            &item_id,
            parsed,
        )
        .await;
    }
}

async fn maybe_complete_plan_item_from_message(
    sess: &Session,
    turn_context: &TurnContext,
    state: &mut PlanModeStreamState,
    item: &ResponseItem,
) {
    if let ResponseItem::Message { role, content, .. } = item
        && role == "assistant"
    {
        let mut text = String::new();
        for entry in content {
            if let ContentItem::OutputText { text: chunk } = entry {
                text.push_str(chunk);
            }
        }
        if let Some(plan_text) = extract_proposed_plan_text(&text) {
            let (plan_text, _citations) = strip_citations(&plan_text);
            if !state.plan_item_state.started {
                state.plan_item_state.start(sess, turn_context).await;
            }
            state
                .plan_item_state
                .complete_with_text(sess, turn_context, plan_text)
                .await;
        }
    }
}

async fn emit_agent_message_in_plan_mode(
    sess: &Session,
    turn_context: &TurnContext,
    agent_message: codex_protocol::items::AgentMessageItem,
    state: &mut PlanModeStreamState,
) {
    let agent_message_id = agent_message.id.clone();
    let text = agent_message_text(&agent_message);
    if text.trim().is_empty() {
        state.pending_agent_message_items.remove(&agent_message_id);
        state.started_agent_message_items.remove(&agent_message_id);
        return;
    }

    maybe_emit_pending_agent_message_start(sess, turn_context, state, &agent_message_id).await;

    if !state
        .started_agent_message_items
        .contains(&agent_message_id)
    {
        let start_item = state
            .pending_agent_message_items
            .remove(&agent_message_id)
            .unwrap_or_else(|| {
                TurnItem::AgentMessage(codex_protocol::items::AgentMessageItem {
                    id: agent_message_id.clone(),
                    content: Vec::new(),
                    phase: None,
                    memory_citation: None,
                })
            });
        sess.emit_turn_item_started(turn_context, &start_item).await;
        state
            .started_agent_message_items
            .insert(agent_message_id.clone());
    }

    sess.emit_turn_item_completed(turn_context, TurnItem::AgentMessage(agent_message))
        .await;
    state.started_agent_message_items.remove(&agent_message_id);
}

async fn emit_turn_item_in_plan_mode(
    sess: &Session,
    turn_context: &TurnContext,
    turn_item: TurnItem,
    previously_active_item: Option<&TurnItem>,
    state: &mut PlanModeStreamState,
) {
    match turn_item {
        TurnItem::AgentMessage(agent_message) => {
            emit_agent_message_in_plan_mode(sess, turn_context, agent_message, state).await;
        }
        _ => {
            if previously_active_item.is_none() {
                sess.emit_turn_item_started(turn_context, &turn_item).await;
            }
            sess.emit_turn_item_completed(turn_context, turn_item).await;
        }
    }
}

async fn handle_assistant_item_done_in_plan_mode(
    sess: &Session,
    turn_context: &TurnContext,
    item: &ResponseItem,
    state: &mut PlanModeStreamState,
    previously_active_item: Option<&TurnItem>,
    last_agent_message: &mut Option<String>,
) -> bool {
    if let ResponseItem::Message { role, .. } = item
        && role == "assistant"
    {
        maybe_complete_plan_item_from_message(sess, turn_context, state, item).await;

        if let Some(turn_item) =
            handle_non_tool_response_item(sess, turn_context, item, /*plan_mode*/ true).await
        {
            emit_turn_item_in_plan_mode(
                sess,
                turn_context,
                turn_item,
                previously_active_item,
                state,
            )
            .await;
        }

        record_completed_response_item(sess, turn_context, item).await;
        if let Some(agent_message) = last_assistant_message_from_item(item, /*plan_mode*/ true) {
            *last_agent_message = Some(agent_message);
        }
        return true;
    }
    false
}

async fn drain_in_flight(
    in_flight: &mut FuturesOrdered<BoxFuture<'static, CodexResult<ResponseInputItem>>>,
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
) -> CodexResult<()> {
    while let Some(res) = in_flight.next().await {
        match res {
            Ok(response_input) => {
                sess.record_conversation_items(&turn_context, &[response_input.into()])
                    .await;
            }
            Err(err) => {
                error_or_panic(format!("in-flight tool future failed during drain: {err}"));
            }
        }
    }
    Ok(())
}
