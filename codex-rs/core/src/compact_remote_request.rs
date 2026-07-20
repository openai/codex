use std::sync::Arc;
use std::sync::OnceLock;

use super::trim_function_call_history_to_fit_context_window;
use crate::Prompt;
use crate::client::CompactConversationRequestSettings;
use crate::compact::CompactionAnalyticsDetails;
use crate::responses_metadata::CodexResponsesRequestKind;
use crate::responses_metadata::CompactionTurnMetadata;
use crate::session::session::Session;
use crate::session::step_context::StepContext;
use crate::session::turn::built_tools;
use codex_protocol::auth::AuthMode;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::ResponseItem;
use codex_rollout_trace::CompactionTraceContext;
use tokio_util::sync::CancellationToken;
use tracing::info;

pub(super) struct RemoteCompactAttempt {
    pub(super) new_history: Vec<ResponseItem>,
    pub(super) trace_input_history: Option<Vec<ResponseItem>>,
}

pub(super) async fn run_remote_compact_attempt(
    sess: &Arc<Session>,
    step_context: &Arc<StepContext>,
    turn_state: Option<Arc<OnceLock<String>>>,
    compaction_trace: &CompactionTraceContext,
    compaction_metadata: CompactionTurnMetadata,
    analytics_details: &mut CompactionAnalyticsDetails,
) -> CodexResult<RemoteCompactAttempt> {
    let turn_context = &step_context.turn;
    let mut history = sess.clone_history().await;
    let base_instructions = sess.get_base_instructions().await;
    let (rewritten_outputs, estimated_deleted_tokens) =
        trim_function_call_history_to_fit_context_window(
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
    if estimated_deleted_tokens > 0 {
        let max_local_deleted_tokens = sess
            .estimated_tokens_after_last_model_generated_item()
            .await;
        analytics_details.active_context_tokens_before = analytics_details
            .active_context_tokens_before
            .map(|active_context_tokens_before| {
                active_context_tokens_before
                    .saturating_sub(estimated_deleted_tokens.min(max_local_deleted_tokens))
            });
    }
    let trace_input_history = compaction_trace
        .is_enabled()
        .then(|| history.raw_items().to_vec());
    let prompt_input = history.for_prompt(&turn_context.model_info.input_modalities);
    let tool_router = built_tools(
        sess.as_ref(),
        step_context.as_ref(),
        &CancellationToken::new(),
    )
    .await?;
    let prompt = Prompt {
        input: prompt_input,
        tools: tool_router.model_visible_specs(),
        parallel_tool_calls: turn_context.model_info.supports_parallel_tool_calls,
        base_instructions,
        output_schema: None,
        output_schema_strict: true,
    };
    let window_id = sess.current_window_id().await;
    let responses_metadata = turn_context.turn_metadata_state.to_responses_metadata(
        sess.installation_id.clone(),
        window_id,
        CodexResponsesRequestKind::Compaction(compaction_metadata),
    );
    let new_history = sess
        .services
        .model_client
        .compact_conversation_history(
            &prompt,
            &turn_context.model_info,
            turn_state,
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
            compaction_trace,
            &responses_metadata,
        )
        .await?;
    Ok(RemoteCompactAttempt {
        new_history,
        trace_input_history,
    })
}
