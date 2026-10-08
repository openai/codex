use crate::context::UserGoalUpdate;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use crate::Prompt;
use crate::client::ModelClientSession;
use crate::client_common::ResponseEvent;
use crate::context::BaseInstructionsFragment;
use crate::context::CompactionSummary;
use crate::context::ContextualUserFragment;
use crate::context::world_state::WorldState;
use crate::context::world_state::WorldStateSnapshot;
use crate::context::world_state::split_prefix_updates;
use crate::context_manager::updates::merge_world_state_updates;
use crate::hook_runtime::PostCompactHookOutcome;
use crate::hook_runtime::PreCompactHookOutcome;
use crate::hook_runtime::run_post_compact_hooks;
use crate::hook_runtime::run_pre_compact_hooks;
use crate::responses_metadata::CodexResponsesMetadata;
use crate::responses_metadata::CompactionTurnMetadata;
use crate::session::RequestEffortUsage;
use crate::session::session::Session;
use crate::session::step_context::StepContext;
use crate::session::turn::get_last_assistant_message_from_turn;
use crate::session::turn_context::TurnContext;
use crate::state::AutoCompactWindowIds;
use crate::util::backoff;
use codex_analytics::CodexCompactionEvent;
use codex_analytics::CompactionImplementation;
use codex_analytics::CompactionPhase;
use codex_analytics::CompactionReason;
use codex_analytics::CompactionStatus;
use codex_analytics::CompactionStrategy;
use codex_analytics::CompactionTrigger;
use codex_analytics::now_unix_seconds;
use codex_history::CodexHarnessMetadata;
use codex_history::ResponseItemEnvelope;
use codex_protocol::ResponseItemId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::items::ContextCompactionItem;
use codex_protocol::items::TurnItem;
use codex_protocol::models::AgentMessageInputContent;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::WarningEvent;
use codex_protocol::user_input::UserInput;
use codex_rollout_trace::InferenceTraceContext;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::approx_token_count;
use codex_utils_output_truncation::truncate_text;
use futures::prelude::*;
use tracing::error;

pub use codex_prompts::SUMMARIZATION_PROMPT;
pub use codex_prompts::SUMMARY_PREFIX;
const COMPACT_USER_MESSAGE_MAX_TOKENS: usize = 20_000;

/// Metadata for a new compaction checkpoint, kept separate from its replacement history.
///
/// `Session::replace_compacted_history` assigns missing item IDs before constructing the persisted
/// `CompactedItem`, ensuring the live and persisted histories remain identical.
pub(crate) struct CompactedHistoryMetadata {
    pub(crate) input_goal_ids: HashSet<ResponseItemId>,
    pub(crate) message: String,
    pub(crate) window_number: u64,
    pub(crate) window_ids: AutoCompactWindowIds,
    pub(crate) compaction_response_id: Option<String>,
    pub(crate) compaction_model_hash: Option<String>,
    pub(crate) reviewer_compaction_hash: Option<String>,
}

/// Renders the window prefix and ordinary context with their comparison baseline.
pub(crate) async fn build_compaction_replacement_history(
    sess: &Session,
    step_context: &StepContext,
    world_state: &WorldState,
    compacted_history: Vec<ResponseItemEnvelope>,
) -> (Vec<ResponseItemEnvelope>, WorldStateSnapshot) {
    let (updates, snapshot) = sess
        .build_initial_context_with_world_state(step_context, world_state)
        .await;
    let (prefix, context) = split_prefix_updates(updates);
    let context = merge_world_state_updates(context);
    (
        assemble_compaction_history(compacted_history, prefix, context),
        snapshot,
    )
}

fn assemble_compaction_history(
    compacted_history: Vec<ResponseItemEnvelope>,
    prefix: Vec<ResponseItem>,
    context: Vec<ResponseItem>,
) -> Vec<ResponseItemEnvelope> {
    let history = insert_initial_context_before_last_real_user_or_summary(
        compacted_history,
        context.into_iter().map(ResponseItemEnvelope::new).collect(),
    );
    prefix
        .into_iter()
        .map(ResponseItemEnvelope::new)
        .chain(history)
        .collect()
}

pub(crate) async fn run_inline_auto_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    replacement_step_context: Arc<StepContext>,
    world_state: Arc<WorldState>,
    reason: CompactionReason,
    phase: CompactionPhase,
) -> CodexResult<()> {
    let prompt = turn_context
        .config
        .compact_prompt
        .as_deref()
        .unwrap_or(SUMMARIZATION_PROMPT)
        .to_string();
    let input = vec![UserInput::Text {
        text: prompt,
        // Compaction prompt is synthesized; no UI element ranges to preserve.
        text_elements: Vec::new(),
    }];

    run_compact_task_inner(
        sess,
        turn_context,
        replacement_step_context,
        input,
        world_state,
        CompactionTurnMetadata::new(
            CompactionTrigger::Auto,
            reason,
            CompactionImplementation::Responses,
            phase,
        ),
    )
    .await?;
    Ok(())
}

pub(crate) async fn run_compact_task(
    sess: Arc<Session>,
    step_context: Arc<StepContext>,
    world_state: Arc<WorldState>,
    input: Vec<UserInput>,
) -> CodexResult<()> {
    run_compact_task_inner(
        sess.clone(),
        Arc::clone(&step_context.turn),
        step_context,
        input,
        world_state,
        CompactionTurnMetadata::new(
            CompactionTrigger::Manual,
            CompactionReason::UserRequested,
            CompactionImplementation::Responses,
            CompactionPhase::StandaloneTurn,
        ),
    )
    .await?;
    Ok(())
}

async fn run_compact_task_inner(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    replacement_step_context: Arc<StepContext>,
    input: Vec<UserInput>,
    world_state: Arc<WorldState>,
    compaction_metadata: CompactionTurnMetadata,
) -> CodexResult<()> {
    let trigger = compaction_metadata.trigger();
    let reason = compaction_metadata.reason();
    let phase = compaction_metadata.phase();
    let attempt = CompactionAnalyticsAttempt::begin(
        sess.as_ref(),
        turn_context.as_ref(),
        trigger,
        reason,
        CompactionImplementation::Responses,
        phase,
    )
    .await;
    let pre_compact_outcome = run_pre_compact_hooks(&sess, &turn_context, trigger).await;
    match pre_compact_outcome {
        PreCompactHookOutcome::Continue => {}
        PreCompactHookOutcome::Stopped => {
            let error = CodexErr::TurnAborted;
            attempt
                .track(
                    sess.as_ref(),
                    CompactionStatus::Interrupted,
                    Some(&error),
                    CompactionAnalyticsDetails::default(),
                )
                .await;
            return Err(error);
        }
    }
    let result = run_compact_task_inner_impl(
        Arc::clone(&sess),
        Arc::clone(&turn_context),
        replacement_step_context,
        input,
        world_state,
        compaction_metadata,
    )
    .await;
    let status = compaction_status_from_result(&result);
    let codex_error = result.as_ref().err();
    if result.is_ok() {
        let post_compact_outcome = run_post_compact_hooks(&sess, &turn_context, trigger).await;
        if let PostCompactHookOutcome::Stopped = post_compact_outcome {
            attempt
                .track(
                    sess.as_ref(),
                    status,
                    codex_error,
                    CompactionAnalyticsDetails::default(),
                )
                .await;
            return Err(CodexErr::TurnAborted);
        }
    }
    attempt
        .track(
            sess.as_ref(),
            status,
            codex_error,
            CompactionAnalyticsDetails::default(),
        )
        .await;
    if let Err(err) = &result
        && !matches!(phase, CompactionPhase::PostTurn)
        && !matches!(
            err.details(),
            CodexErrorDetails::Interrupted | CodexErrorDetails::TurnAborted
        )
    {
        sess.track_turn_codex_error(turn_context.as_ref(), err);
        // Pre-turn failures are reported after preserving the incoming prompt.
        if !matches!(phase, CompactionPhase::PreTurn) {
            let event = EventMsg::Error(err.to_error_event(/*message_prefix*/ None));
            sess.send_event(&turn_context, event).await;
        }
    }
    result.map(|_| ())
}

async fn run_compact_task_inner_impl(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    replacement_step_context: Arc<StepContext>,
    input: Vec<UserInput>,
    world_state: Arc<WorldState>,
    compaction_metadata: CompactionTurnMetadata,
) -> CodexResult<String> {
    let compaction_item = TurnItem::ContextCompaction(ContextCompactionItem::new());
    sess.emit_turn_item_started(&turn_context, &compaction_item)
        .await;
    let initial_input_for_turn: ResponseInputItem = ResponseInputItem::from(input);

    let mut history = sess.clone_history().await;
    let input_goal_ids = UserGoalUpdate::message_ids(history.raw_items());
    history.record_items(
        &[initial_input_for_turn.into()],
        turn_context.model_info().truncation_policy.into(),
    );

    let max_retries = turn_context.provider.info().stream_max_retries();
    let mut retries = 0;
    // Reuse one client session so turn-scoped state (sticky routing and websocket incremental
    // request tracking) survives retries within this compact turn.
    let mut client_session = sess.services.model_client.new_session();
    let compaction_response = loop {
        // Clone is required because of the loop
        let mut turn_input = history
            .clone()
            .for_prompt(&turn_context.model_info().input_modalities);
        sess.services
            .executed_tool_calls
            .attach_to_compaction_prompt(&mut turn_input);
        let turn_input_len = turn_input.len();
        let base_instructions = if turn_input
            .iter()
            .any(BaseInstructionsFragment::matches_item)
        {
            BaseInstructions {
                text: String::new(),
                provenance: None,
            }
        } else {
            sess.get_prompt_base_instructions().await
        };
        let prompt = Prompt {
            input: turn_input,
            base_instructions,
            cyber_access_program: turn_context.cyber_access_program,
            ..Default::default()
        };
        let responses_metadata = sess
            .compaction_responses_metadata(turn_context.as_ref(), compaction_metadata)
            .await;
        let attempt_result = drain_to_completed(
            &sess,
            turn_context.as_ref(),
            &mut client_session,
            &responses_metadata,
            &prompt,
            compaction_metadata.phase(),
        )
        .await;

        match attempt_result {
            Ok(response) => {
                break response;
            }
            Err(err)
                if matches!(
                    err.details(),
                    CodexErrorDetails::Interrupted | CodexErrorDetails::TurnAborted
                ) =>
            {
                return Err(err);
            }
            Err(e) if matches!(e.details(), CodexErrorDetails::SessionBudgetExceeded) => {
                return Err(e);
            }
            Err(e) if matches!(e.details(), CodexErrorDetails::ContextWindowExceeded) => {
                if turn_input_len > 1 {
                    // Trim from the beginning to preserve cache (prefix-based) and keep recent messages intact.
                    error!(
                        "Context window exceeded while compacting; removing oldest history item. Error: {e}"
                    );
                    history.remove_first_item();
                    retries = 0;
                    continue;
                }
                sess.set_total_tokens_full(turn_context.as_ref()).await;
                return Err(e);
            }
            Err(e) => {
                if retries < max_retries {
                    retries += 1;
                    let delay = backoff(retries);
                    sess.notify_stream_error(
                        turn_context.as_ref(),
                        format!("Reconnecting... {retries}/{max_retries}"),
                        e,
                    )
                    .await;
                    tokio::time::sleep(delay).await;
                    continue;
                } else {
                    return Err(e);
                }
            }
        }
    };

    let history_snapshot = sess.clone_history().await;
    let history_items = history_snapshot.annotated_items();
    let summary_suffix = if matches!(compaction_metadata.phase(), CompactionPhase::PostTurn) {
        get_last_assistant_message_from_turn(compaction_response.output.iter())
            .filter(|summary| !summary.trim().is_empty())
            .ok_or_else(|| {
                CodexErr::Stream(
                    "Post-turn compaction completed without an assistant summary".to_string(),
                )
            })?
    } else {
        get_last_assistant_message_from_turn(history_snapshot.raw_items()).unwrap_or_default()
    };
    let summary_text = format!("{SUMMARY_PREFIX}\n{summary_suffix}");
    let user_messages = collect_annotated_user_messages(history_items);

    let mut new_history = build_compacted_history(Vec::new(), &user_messages, &summary_text);
    if let Some(summary_item) = new_history.last_mut() {
        // This replacement history skips `record_conversation_items`; only the appended summary
        // belongs to this compaction turn.
        summary_item.set_turn_id_if_missing(&turn_context.sub_id);
    }
    let (window_number, window_ids) = sess.advance_auto_compact_window().await;

    let (new_history, world_state_baseline) = build_compaction_replacement_history(
        sess.as_ref(),
        &replacement_step_context,
        &world_state,
        new_history,
    )
    .await;
    sess.replace_compacted_history(
        new_history,
        replacement_step_context.to_turn_context_item(),
        world_state_baseline,
        CompactedHistoryMetadata {
            input_goal_ids,
            message: summary_text,
            window_number,
            window_ids,
            compaction_response_id: Some(compaction_response.response_id),
            compaction_model_hash: turn_context.model_info().comp_hash.clone(),
            reviewer_compaction_hash: None,
        },
    )
    .await;
    sess.recompute_token_usage(&replacement_step_context.turn)
        .await;

    sess.emit_turn_item_completed(&turn_context, compaction_item)
        .await;
    let warning = EventMsg::Warning(WarningEvent {
        message: "Heads up: Long threads and multiple compactions can cause the model to be less accurate. Start a new thread when possible to keep threads small and targeted.".to_string(),
    });
    sess.send_event(&turn_context, warning).await;
    Ok(summary_suffix)
}

pub(crate) struct CompactionAnalyticsAttempt {
    thread_id: String,
    turn_id: String,
    trigger: CompactionTrigger,
    reason: CompactionReason,
    implementation: CompactionImplementation,
    phase: CompactionPhase,
    active_context_tokens_before: i64,
    started_at: u64,
    start_instant: Instant,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct CompactionAnalyticsDetails {
    pub(crate) active_context_tokens_before: Option<i64>,
    pub(crate) retained_image_count: Option<usize>,
    pub(crate) compaction_summary_tokens: Option<i64>,
    pub(crate) cached_input_tokens: Option<i64>,
    pub(crate) cache_write_input_tokens: Option<i64>,
}

impl CompactionAnalyticsAttempt {
    pub(crate) async fn begin(
        sess: &Session,
        turn_context: &TurnContext,
        trigger: CompactionTrigger,
        reason: CompactionReason,
        implementation: CompactionImplementation,
        phase: CompactionPhase,
    ) -> Self {
        let active_context_tokens_before = sess.get_total_token_usage().await;
        Self {
            thread_id: sess.thread_id.to_string(),
            turn_id: turn_context.sub_id.clone(),
            trigger,
            reason,
            implementation,
            phase,
            active_context_tokens_before,
            started_at: now_unix_seconds(),
            start_instant: Instant::now(),
        }
    }

    pub(crate) async fn track(
        self,
        sess: &Session,
        status: CompactionStatus,
        codex_error: Option<&CodexErr>,
        details: CompactionAnalyticsDetails,
    ) {
        let CompactionAnalyticsDetails {
            active_context_tokens_before,
            retained_image_count,
            compaction_summary_tokens,
            cached_input_tokens,
            cache_write_input_tokens,
        } = details;
        let active_context_tokens_before =
            active_context_tokens_before.unwrap_or(self.active_context_tokens_before);
        let active_context_tokens_after = sess.get_total_token_usage().await;
        sess.services
            .analytics_events_client
            .track_compaction(CodexCompactionEvent {
                thread_id: self.thread_id,
                turn_id: self.turn_id,
                trigger: self.trigger,
                reason: self.reason,
                implementation: self.implementation,
                phase: self.phase,
                strategy: CompactionStrategy::Memento,
                status,
                codex_error_kind: codex_error.map(Into::into),
                codex_error_http_status_code: codex_error
                    .and_then(CodexErr::http_status_code_value),
                usage_limit_window_minutes: codex_error.and_then(|error| match error.details() {
                    CodexErrorDetails::UsageLimitReached(error) => error.limit_window_minutes,
                    _ => None,
                }),
                active_context_tokens_before,
                active_context_tokens_after,
                retained_image_count,
                compaction_summary_tokens,
                cached_input_tokens,
                cache_write_input_tokens,
                started_at: self.started_at,
                completed_at: now_unix_seconds(),
                duration_ms: Some(
                    u64::try_from(self.start_instant.elapsed().as_millis()).unwrap_or(u64::MAX),
                ),
            });
    }
}

pub(crate) fn compaction_status_from_result<T>(result: &CodexResult<T>) -> CompactionStatus {
    match result {
        Ok(_) => CompactionStatus::Completed,
        Err(err)
            if matches!(
                err.details(),
                CodexErrorDetails::Interrupted | CodexErrorDetails::TurnAborted
            ) =>
        {
            CompactionStatus::Interrupted
        }
        Err(_) => CompactionStatus::Failed,
    }
}

pub fn content_items_to_text(content: &[ContentItem]) -> Option<String> {
    let mut pieces = Vec::new();
    for item in content {
        match item {
            ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                if !text.is_empty() {
                    pieces.push(text.as_str());
                }
            }
            ContentItem::InputImage { .. } | ContentItem::InputAudio { .. } => {}
        }
    }
    if pieces.is_empty() {
        None
    } else {
        Some(pieces.join("\n"))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CompactedUserMessage<'a> {
    // Flattened text is only for the existing budget and truncation policy.
    // Whole text messages retain their exact content parts and annotations.
    // Borrow from the history snapshot until selected output is materialized.
    message: String,
    original: &'a ResponseItem,
    harness_metadata: Option<&'a CodexHarnessMetadata>,
}

#[cfg(test)]
pub(crate) fn collect_user_messages(items: &[ResponseItem]) -> Vec<CompactedUserMessage<'_>> {
    items
        .iter()
        .filter_map(|item| compacted_user_message(item, /*harness_metadata*/ None))
        .collect()
}

pub(crate) fn collect_annotated_user_messages(
    items: &[ResponseItemEnvelope],
) -> Vec<CompactedUserMessage<'_>> {
    items
        .iter()
        .filter_map(|envelope| compacted_user_message(&envelope.item, envelope.metadata.as_ref()))
        .collect()
}

fn compacted_user_message<'a>(
    item: &'a ResponseItem,
    harness_metadata: Option<&'a CodexHarnessMetadata>,
) -> Option<CompactedUserMessage<'a>> {
    let Some(TurnItem::UserMessage(user)) = crate::event_mapping::parse_turn_item(item) else {
        return None;
    };
    let message = user.message();
    if is_summary_message(&message) {
        return None;
    }
    Some(CompactedUserMessage {
        message,
        original: item,
        harness_metadata,
    })
}

pub(crate) fn is_summary_message(message: &str) -> bool {
    message.starts_with(format!("{SUMMARY_PREFIX}\n").as_str())
}

/// Inserts canonical initial context into compacted replacement history at the
/// model-expected boundary.
///
/// Placement rules:
/// - Prefer immediately before the last real user or agent message.
/// - If no real user messages remain, insert before the compaction summary so
///   the summary stays last.
/// - If there are no user messages, insert before the last compaction item so
///   that item remains last (remote compaction may return only compaction items).
/// - If there are no user messages or compaction items, append the context.
pub(crate) fn insert_initial_context_before_last_real_user_or_summary(
    mut compacted_history: Vec<ResponseItemEnvelope>,
    initial_context: Vec<ResponseItemEnvelope>,
) -> Vec<ResponseItemEnvelope> {
    let mut last_user_or_summary_index = None;
    let mut last_real_user_index = None;
    for (i, item) in compacted_history.iter().enumerate().rev() {
        if let ResponseItem::AgentMessage { content, .. } = &item.item
            && !matches!(
                content.first(),
                Some(AgentMessageInputContent::InputText { text })
                    if text.starts_with("Message Type: FINAL_ANSWER\n")
            )
        {
            last_real_user_index = Some(i);
            break;
        }
        let Some(TurnItem::UserMessage(user)) = crate::event_mapping::parse_turn_item(&item.item)
        else {
            continue;
        };
        // Compaction summaries are encoded as user messages, so track both:
        // the last real user message (preferred insertion point) and the last
        // user-message-like item (fallback summary insertion point).
        last_user_or_summary_index.get_or_insert(i);
        if !is_summary_message(&user.message()) {
            last_real_user_index = Some(i);
            break;
        }
    }
    let last_compaction_index = compacted_history
        .iter()
        .enumerate()
        .rev()
        .find_map(|(i, item)| {
            matches!(
                &item.item,
                ResponseItem::Compaction { .. } | ResponseItem::ContextCompaction { .. }
            )
            .then_some(i)
        });
    let insertion_index = last_real_user_index
        .or(last_user_or_summary_index)
        .or(last_compaction_index);

    // Re-inject canonical context from the current session since we stripped it
    // from the pre-compaction history. Prefer placing it before the last real
    // user message; if there is no real user message left, place it before the
    // summary or compaction item so the compaction item remains last.
    if let Some(insertion_index) = insertion_index {
        compacted_history.splice(insertion_index..insertion_index, initial_context);
    } else {
        compacted_history.extend(initial_context);
    }

    compacted_history
}

pub(crate) fn build_compacted_history(
    initial_context: Vec<ResponseItemEnvelope>,
    user_messages: &[CompactedUserMessage<'_>],
    summary_text: &str,
) -> Vec<ResponseItemEnvelope> {
    build_compacted_history_with_limit(
        initial_context,
        user_messages,
        summary_text,
        COMPACT_USER_MESSAGE_MAX_TOKENS,
    )
}

fn build_compacted_history_with_limit(
    mut history: Vec<ResponseItemEnvelope>,
    user_messages: &[CompactedUserMessage<'_>],
    summary_text: &str,
    max_tokens: usize,
) -> Vec<ResponseItemEnvelope> {
    let mut selected_messages = Vec::new();
    if max_tokens > 0 {
        let mut remaining = max_tokens;
        for message in user_messages.iter().rev() {
            if remaining == 0 {
                break;
            }
            let tokens = approx_token_count(&message.message);
            let ResponseItem::Message {
                id,
                content,
                internal_chat_message_metadata_passthrough,
                ..
            } = message.original
            else {
                continue;
            };
            let mut passthrough = internal_chat_message_metadata_passthrough.clone();
            let mut harness_metadata = message.harness_metadata.cloned();
            let content = if tokens <= remaining
                && content
                    .iter()
                    .all(|part| matches!(part, ContentItem::InputText { .. }))
            {
                content.clone()
            } else {
                // Rebuild only the text fallback; never clone discarded media.
                if let Some(metadata) = &mut passthrough {
                    metadata.content_item_metadata = None;
                }
                if let Some(kinds) = passthrough
                    .as_mut()
                    .and_then(|metadata| metadata.content_item_kinds.as_mut())
                {
                    *kinds = vec![ContentItemKind("user.text".to_owned())];
                }
                vec![ContentItem::InputText {
                    text: truncate_text(&message.message, TruncationPolicy::Tokens(remaining)),
                }]
            };
            if tokens > remaining
                && let Some(metadata) = &mut harness_metadata
            {
                metadata.mark_retained_sources_incomplete();
            }
            selected_messages.push(ResponseItemEnvelope {
                item: ResponseItem::Message {
                    id: id.clone(),
                    role: "user".to_owned(),
                    content,
                    phase: None,
                    internal_chat_message_metadata_passthrough: passthrough,
                },
                metadata: harness_metadata,
            });
            if tokens > remaining {
                break;
            }
            remaining = remaining.saturating_sub(tokens);
        }
        selected_messages.reverse();
    }
    history.extend(selected_messages);

    let summary_text = if summary_text.is_empty() {
        "(no summary available)".to_string()
    } else {
        summary_text.to_string()
    };

    history.push(ResponseItemEnvelope::new(ContextualUserFragment::into(
        CompactionSummary::new(summary_text),
    )));

    history
}

struct CompactionResponse {
    response_id: String,
    output: Vec<ResponseItem>,
}

async fn drain_to_completed(
    sess: &Session,
    turn_context: &TurnContext,
    client_session: &mut ModelClientSession,
    responses_metadata: &CodexResponsesMetadata,
    prompt: &Prompt,
    phase: CompactionPhase,
) -> CodexResult<CompactionResponse> {
    let mut stream = client_session
        .stream(
            prompt,
            turn_context.model_info(),
            &turn_context.session_telemetry,
            sess.reasoning_effort_for_request(
                &turn_context.initial_settings,
                RequestEffortUsage::Compaction,
            )
            .await,
            turn_context.reasoning_summary(),
            turn_context.config.service_tier.clone(),
            responses_metadata,
            // Rollout tracing currently models remote compaction only; local compaction streams
            // are left untraced until the reducer has a first-class local compaction lifecycle.
            &InferenceTraceContext::disabled(),
        )
        .await?;
    let mut output = Vec::new();
    loop {
        let maybe_event = stream.next().await;
        let Some(event) = maybe_event else {
            return Err(CodexErr::Stream(
                "stream closed before response.completed".into(),
            ));
        };
        match event {
            Ok(ResponseEvent::OutputItemDone(item)) => {
                if matches!(phase, CompactionPhase::PostTurn) {
                    // Commit post-turn summaries only after success; failures must leave both
                    // the live history and persisted rollout intact.
                    output.push(item);
                } else {
                    sess.record_annotated_conversation_items(
                        turn_context,
                        turn_context.model_info(),
                        vec![ResponseItemEnvelope {
                            item,
                            metadata: Some(CodexHarnessMetadata {
                                compaction_output: true,
                                ..Default::default()
                            }),
                        }],
                    )
                    .await;
                }
            }
            Ok(ResponseEvent::ServerReasoningIncluded(included)) => {
                sess.set_server_reasoning_included(included).await;
            }
            Ok(ResponseEvent::RateLimits(snapshot)) => {
                sess.update_rate_limits(turn_context, snapshot).await;
            }
            Ok(ResponseEvent::Completed {
                response_id,
                token_usage,
                usage_metadata,
                ..
            }) => {
                sess.record_observed_response_completed(
                    turn_context,
                    &response_id,
                    token_usage.as_ref(),
                    usage_metadata.as_ref(),
                )
                .await;
                sess.update_token_usage_info(turn_context, token_usage.as_ref())
                    .await?;
                return Ok(CompactionResponse {
                    response_id,
                    output,
                });
            }
            Ok(_) => continue,
            Err(e) => return Err(e),
        }
    }
}

#[cfg(test)]
#[path = "compact_tests.rs"]
mod tests;
