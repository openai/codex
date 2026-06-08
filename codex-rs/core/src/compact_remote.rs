use std::sync::Arc;

use crate::Prompt;
use crate::client::CompactConversationRequestSettings;
use crate::compact::CompactionRunOptions;
use crate::compact::InitialContextInjection;
#[cfg(test)]
use crate::compact::apply_initial_context_injection;
use crate::compact::run_compaction_with_history_builder;
use crate::context_manager::ContextManager;
use crate::context_manager::TotalTokenUsageBreakdown;
use crate::context_manager::estimate_response_item_model_visible_bytes;
use crate::session::session::Session;
use crate::session::turn::built_tools;
use crate::session::turn_context::TurnContext;
use crate::turn_metadata::CompactionTurnMetadata;
use codex_analytics::CompactionImplementation;
use codex_analytics::CompactionPhase;
use codex_analytics::CompactionReason;
use codex_analytics::CompactionTrigger;
use codex_app_server_protocol::AuthMode;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::items::TurnItem;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use futures::TryFutureExt;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing::info;

const CONTEXT_WINDOW_TRUNCATED_OUTPUT_MESSAGE: &str =
    "Output exceeded the available model context and was truncated";

pub(crate) async fn run_inline_remote_auto_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    initial_context_injection: InitialContextInjection,
    reason: CompactionReason,
    phase: CompactionPhase,
) -> CodexResult<()> {
    let sess_for_builder = Arc::clone(&sess);
    let turn_context_for_builder = Arc::clone(&turn_context);
    run_compaction_with_history_builder(
        &sess,
        &turn_context,
        CompactionRunOptions {
            initial_context_injection,
            trigger: CompactionTrigger::Auto,
            reason,
            implementation: CompactionImplementation::ResponsesCompact,
            phase,
            error_message_prefix: Some("Error running remote compact task"),
            emit_accuracy_warning: false,
        },
        |metadata, item_id| {
            Box::pin(async move {
                build_remote_compacted_history(
                    &sess_for_builder,
                    &turn_context_for_builder,
                    metadata,
                    item_id,
                )
                .await
            })
        },
    )
    .await
}

pub(crate) async fn run_remote_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
) -> CodexResult<()> {
    let sess_for_builder = Arc::clone(&sess);
    let turn_context_for_builder = Arc::clone(&turn_context);
    run_compaction_with_history_builder(
        &sess,
        &turn_context,
        CompactionRunOptions {
            initial_context_injection: InitialContextInjection::DoNotInject,
            trigger: CompactionTrigger::Manual,
            reason: CompactionReason::UserRequested,
            implementation: CompactionImplementation::ResponsesCompact,
            phase: CompactionPhase::StandaloneTurn,
            error_message_prefix: Some("Error running remote compact task"),
            emit_accuracy_warning: false,
        },
        |metadata, item_id| {
            Box::pin(async move {
                build_remote_compacted_history(
                    &sess_for_builder,
                    &turn_context_for_builder,
                    metadata,
                    item_id,
                )
                .await
            })
        },
    )
    .await
}

async fn build_remote_compacted_history(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    compaction_metadata: CompactionTurnMetadata,
    item_id: String,
) -> CodexResult<Vec<ResponseItem>> {
    // Use the UI compaction item ID as the trace compaction ID so protocol lifecycle events and
    // endpoint attempts have one join key.
    let compaction_trace = sess.services.rollout_thread_trace.compaction_trace_context(
        turn_context.sub_id.as_str(),
        item_id.as_str(),
        turn_context.model_info.slug.as_str(),
        turn_context.provider.info().name.as_str(),
    );
    let mut history = sess.clone_history().await;
    let base_instructions = sess.get_base_instructions().await;
    let (rewritten_outputs, _) = trim_function_call_history_to_fit_context_window(
        &mut history,
        turn_context.as_ref(),
        &base_instructions,
    );
    if rewritten_outputs > 0 {
        info!(
            turn_id = %turn_context.sub_id,
            rewritten_outputs,
            "rewrote history outputs before remote compaction"
        );
    }
    let prompt_input = history.for_prompt(&turn_context.model_info.input_modalities);
    let tool_router = built_tools(
        sess.as_ref(),
        turn_context.as_ref(),
        &CancellationToken::new(),
    )
    .await?;
    let prompt = Prompt {
        input: prompt_input,
        tools: tool_router.model_visible_specs(),
        parallel_tool_calls: turn_context.model_info.supports_parallel_tool_calls,
        base_instructions,
        personality: turn_context.personality,
        output_schema: None,
        output_schema_strict: true,
    };
    let window_id = sess.services.model_client.current_window_id();
    let turn_metadata_header = turn_context
        .turn_metadata_state
        .current_header_value_for_compaction(&window_id, compaction_metadata);
    let new_history = sess
        .services
        .model_client
        .compact_conversation_history(
            &prompt,
            &turn_context.model_info,
            CompactConversationRequestSettings {
                effort: turn_context.reasoning_effort.clone(),
                summary: turn_context.reasoning_summary,
                service_tier: if sess.services.auth_manager.auth_mode() == Some(AuthMode::ApiKey) {
                    None
                } else {
                    turn_context.config.service_tier.clone()
                },
            },
            &turn_context.session_telemetry,
            &compaction_trace,
            turn_metadata_header.as_deref(),
        )
        .or_else(|err| async {
            let total_usage_breakdown = sess.get_total_token_usage_breakdown().await;
            let compact_request_log_data =
                build_compact_request_log_data(&prompt.input, &prompt.base_instructions.text);
            log_remote_compact_failure(
                turn_context,
                &compact_request_log_data,
                total_usage_breakdown,
                &err,
            );
            Err(err)
        })
        .await?;
    Ok(filter_compacted_history(new_history))
}

#[cfg(test)]
pub(crate) async fn process_compacted_history(
    sess: &Session,
    turn_context: &TurnContext,
    mut compacted_history: Vec<ResponseItem>,
    initial_context_injection: InitialContextInjection,
) -> Vec<ResponseItem> {
    compacted_history = filter_compacted_history(compacted_history);
    apply_initial_context_injection(
        sess,
        turn_context,
        compacted_history,
        initial_context_injection,
    )
    .await
}

fn filter_compacted_history(mut compacted_history: Vec<ResponseItem>) -> Vec<ResponseItem> {
    compacted_history.retain(should_keep_compacted_history_item);
    compacted_history
}

/// Returns whether an item from remote compaction output should be preserved.
///
/// Called while processing the model-provided compacted transcript, before we
/// append fresh canonical context from the current session.
///
/// We drop:
/// - `developer` messages because remote output can include stale/duplicated
///   instruction content.
/// - non-user-content `user` messages (session prefix/instruction wrappers),
///   while preserving real user messages and persisted hook prompts.
///
/// This intentionally keeps:
/// - `assistant` messages (future remote compaction models may emit them)
/// - `user`-role warnings that parse as `TurnItem::UserMessage` and compaction-generated summary
///   messages. Legacy warning fragments are filtered by `parse_turn_item` before they reach this
///   check.
pub(crate) fn should_keep_compacted_history_item(item: &ResponseItem) -> bool {
    match item {
        ResponseItem::Message { role, .. } if role == "developer" => false,
        ResponseItem::Message { role, .. } if role == "user" => {
            matches!(
                crate::event_mapping::parse_turn_item(item),
                Some(TurnItem::UserMessage(_) | TurnItem::HookPrompt(_))
            )
        }
        ResponseItem::Message { role, .. } if role == "assistant" => true,
        ResponseItem::Message { .. } => false,
        ResponseItem::AgentMessage { .. } => true,
        ResponseItem::Compaction { .. } | ResponseItem::ContextCompaction { .. } => true,
        ResponseItem::CompactionTrigger => false,
        ResponseItem::Reasoning { .. }
        | ResponseItem::LocalShellCall { .. }
        | ResponseItem::FunctionCall { .. }
        | ResponseItem::ToolSearchCall { .. }
        | ResponseItem::FunctionCallOutput { .. }
        | ResponseItem::ToolSearchOutput { .. }
        | ResponseItem::CustomToolCall { .. }
        | ResponseItem::CustomToolCallOutput { .. }
        | ResponseItem::WebSearchCall { .. }
        | ResponseItem::ImageGenerationCall { .. }
        | ResponseItem::Other => false,
    }
}

#[derive(Debug)]
pub(crate) struct CompactRequestLogData {
    failing_compaction_request_model_visible_bytes: i64,
}

pub(crate) fn build_compact_request_log_data(
    input: &[ResponseItem],
    instructions: &str,
) -> CompactRequestLogData {
    let failing_compaction_request_model_visible_bytes = input
        .iter()
        .map(estimate_response_item_model_visible_bytes)
        .fold(
            i64::try_from(instructions.len()).unwrap_or(i64::MAX),
            i64::saturating_add,
        );

    CompactRequestLogData {
        failing_compaction_request_model_visible_bytes,
    }
}

pub(crate) fn log_remote_compact_failure(
    turn_context: &TurnContext,
    log_data: &CompactRequestLogData,
    total_usage_breakdown: TotalTokenUsageBreakdown,
    err: &CodexErr,
) {
    error!(
        turn_id = %turn_context.sub_id,
        last_api_response_total_tokens = total_usage_breakdown.last_api_response_total_tokens,
        all_history_items_model_visible_bytes = total_usage_breakdown.all_history_items_model_visible_bytes,
        estimated_tokens_of_items_added_since_last_successful_api_response = total_usage_breakdown.estimated_tokens_of_items_added_since_last_successful_api_response,
        estimated_bytes_of_items_added_since_last_successful_api_response = total_usage_breakdown.estimated_bytes_of_items_added_since_last_successful_api_response,
        model_context_window_tokens = ?turn_context.model_context_window(),
        failing_compaction_request_model_visible_bytes = log_data.failing_compaction_request_model_visible_bytes,
        compact_error = %err,
        "remote compaction failed"
    );
}

pub(crate) fn trim_function_call_history_to_fit_context_window(
    history: &mut ContextManager,
    turn_context: &TurnContext,
    base_instructions: &BaseInstructions,
) -> (usize, i64) {
    let Some(context_window) = turn_context.model_context_window() else {
        return (0, 0);
    };
    let mut rewritten_outputs = 0usize;
    let mut estimated_deleted_tokens = 0i64;
    let item_count = history.raw_items().len();

    for index in (0..item_count).rev() {
        let Some(estimated_tokens_before) =
            history.estimate_token_count_with_base_instructions(base_instructions)
        else {
            break;
        };
        if estimated_tokens_before <= context_window {
            break;
        }
        let Some(rewritten_item) = history
            .raw_items()
            .get(index)
            .and_then(rewritten_output_for_context_window)
        else {
            break;
        };
        let mut items = history.raw_items().to_vec();
        items[index] = rewritten_item;
        history.replace(items);
        let estimated_tokens_after = history
            .estimate_token_count_with_base_instructions(base_instructions)
            .unwrap_or_default();
        rewritten_outputs += 1;
        estimated_deleted_tokens = estimated_deleted_tokens
            .saturating_add(estimated_tokens_before.saturating_sub(estimated_tokens_after));
    }

    (rewritten_outputs, estimated_deleted_tokens)
}

fn rewritten_output_for_context_window(item: &ResponseItem) -> Option<ResponseItem> {
    Some(match item {
        ResponseItem::FunctionCallOutput { call_id, output } => ResponseItem::FunctionCallOutput {
            call_id: call_id.clone(),
            output: truncated_output_payload(output),
        },
        ResponseItem::CustomToolCallOutput {
            call_id,
            name,
            output,
        } => ResponseItem::CustomToolCallOutput {
            call_id: call_id.clone(),
            name: name.clone(),
            output: truncated_output_payload(output),
        },
        ResponseItem::ToolSearchOutput {
            call_id,
            status,
            execution,
            ..
        } => ResponseItem::ToolSearchOutput {
            call_id: call_id.clone(),
            status: status.clone(),
            execution: execution.clone(),
            tools: Vec::new(),
        },
        _ => return None,
    })
}

fn truncated_output_payload(output: &FunctionCallOutputPayload) -> FunctionCallOutputPayload {
    FunctionCallOutputPayload {
        body: FunctionCallOutputBody::Text(CONTEXT_WINDOW_TRUNCATED_OUTPUT_MESSAGE.to_string()),
        success: output.success,
    }
}
