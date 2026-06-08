use std::sync::Arc;

use crate::Prompt;
use crate::ResponseStream;
use crate::client::ModelClientSession;
use crate::client_common::ResponseEvent;
use crate::compact::CompactionRunOptions;
use crate::compact::InitialContextInjection;
use crate::compact::run_compaction_with_history_builder;
use crate::compact_remote::build_compact_request_log_data;
use crate::compact_remote::log_remote_compact_failure;
use crate::compact_remote::should_keep_compacted_history_item;
use crate::compact_remote::trim_function_call_history_to_fit_context_window;
use crate::responses_retry::ResponsesStreamRequest;
use crate::responses_retry::handle_retryable_response_stream_error;
use crate::session::session::Session;
use crate::session::turn::built_tools;
use crate::session::turn_context::TurnContext;
use crate::turn_metadata::CompactionTurnMetadata;
use codex_analytics::CompactionImplementation;
use codex_analytics::CompactionPhase;
use codex_analytics::CompactionReason;
use codex_analytics::CompactionTrigger;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TruncationPolicy;
use codex_rollout_trace::InferenceTraceContext;
use codex_utils_output_truncation::approx_token_count;
use codex_utils_output_truncation::truncate_text;
use futures::StreamExt;
use tokio_util::sync::CancellationToken;
use tracing::info;

// Mirror the current /responses/compact retained-message default while the
// server-side path remains the reference implementation.
const RETAINED_MESSAGE_TOKEN_BUDGET: usize = 64_000;
// Compact attempts can run much longer than normal turns, so keep the per-transport
// retry budget smaller than the general Responses stream retry budget.
const MAX_REMOTE_COMPACTION_V2_STREAM_RETRIES: u64 = 2;

pub(crate) async fn run_inline_remote_auto_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    client_session: &mut ModelClientSession,
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
            implementation: CompactionImplementation::ResponsesCompactionV2,
            phase,
            error_message_prefix: Some("Error running remote compact task"),
            emit_accuracy_warning: false,
        },
        |metadata, item_id| {
            Box::pin(async move {
                build_remote_v2_compacted_history(
                    &sess_for_builder,
                    &turn_context_for_builder,
                    Some(client_session),
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
            implementation: CompactionImplementation::ResponsesCompactionV2,
            phase: CompactionPhase::StandaloneTurn,
            error_message_prefix: Some("Error running remote compact task"),
            emit_accuracy_warning: false,
        },
        |metadata, item_id| {
            Box::pin(async move {
                build_remote_v2_compacted_history(
                    &sess_for_builder,
                    &turn_context_for_builder,
                    /*client_session*/ None,
                    metadata,
                    item_id,
                )
                .await
            })
        },
    )
    .await
}

async fn build_remote_v2_compacted_history(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    client_session: Option<&mut ModelClientSession>,
    compaction_metadata: CompactionTurnMetadata,
    item_id: String,
) -> CodexResult<Vec<ResponseItem>> {
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
            "rewrote history outputs before remote compaction v2"
        );
    }

    let prompt_input = history.for_prompt(&turn_context.model_info.input_modalities);
    let tool_router = built_tools(
        sess.as_ref(),
        turn_context.as_ref(),
        &CancellationToken::new(),
    )
    .await?;
    let mut input = prompt_input.clone();
    input.push(ResponseItem::CompactionTrigger);
    let prompt = Prompt {
        input,
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
    let trace_attempt = compaction_trace.start_attempt(&serde_json::json!({
        "model": turn_context.model_info.slug.as_str(),
        "instructions": prompt.base_instructions.text.as_str(),
        "input": &prompt.input,
        "parallel_tool_calls": prompt.parallel_tool_calls,
    }));

    let mut owned_client_session;
    let client_session = match client_session {
        Some(client_session) => client_session,
        None => {
            owned_client_session = sess.services.model_client.new_session();
            &mut owned_client_session
        }
    };
    let compaction_output_result = run_remote_compaction_request_v2(
        sess,
        turn_context,
        client_session,
        &prompt,
        turn_metadata_header.as_deref(),
    )
    .await;

    trace_attempt.record_result(compaction_output_result.as_ref().map(std::slice::from_ref));
    let compaction_output = compaction_output_result?;
    Ok(build_v2_compacted_history(&prompt_input, compaction_output))
}

async fn run_remote_compaction_request_v2(
    sess: &Session,
    turn_context: &TurnContext,
    client_session: &mut ModelClientSession,
    prompt: &Prompt,
    turn_metadata_header: Option<&str>,
) -> CodexResult<ResponseItem> {
    let max_retries = turn_context
        .provider
        .info()
        .stream_max_retries()
        .min(MAX_REMOTE_COMPACTION_V2_STREAM_RETRIES);
    let mut retries = 0;
    loop {
        let result = match client_session
            .stream(
                prompt,
                &turn_context.model_info,
                &turn_context.session_telemetry,
                turn_context.reasoning_effort.clone(),
                turn_context.reasoning_summary,
                turn_context.config.service_tier.clone(),
                turn_metadata_header,
                &InferenceTraceContext::disabled(),
            )
            .await
        {
            Ok(stream) => collect_compaction_output(stream).await,
            Err(err) => Err(err),
        };

        match result {
            Ok(compaction_output) => return Ok(compaction_output),
            Err(err) if !err.is_retryable() => {
                log_remote_compaction_request_failure(sess, turn_context, prompt, &err).await;
                return Err(err);
            }
            Err(err) => {
                if let Err(err) = handle_retryable_response_stream_error(
                    &mut retries,
                    max_retries,
                    err,
                    client_session,
                    sess,
                    turn_context,
                    ResponsesStreamRequest::RemoteCompactionV2,
                )
                .await
                {
                    log_remote_compaction_request_failure(sess, turn_context, prompt, &err).await;
                    return Err(err);
                }
            }
        }
    }
}

async fn log_remote_compaction_request_failure(
    sess: &Session,
    turn_context: &TurnContext,
    prompt: &Prompt,
    err: &CodexErr,
) {
    let total_usage_breakdown = sess.get_total_token_usage_breakdown().await;
    let compact_request_log_data =
        build_compact_request_log_data(&prompt.input, &prompt.base_instructions.text);
    log_remote_compact_failure(
        turn_context,
        &compact_request_log_data,
        total_usage_breakdown,
        err,
    );
}

async fn collect_compaction_output(mut stream: ResponseStream) -> CodexResult<ResponseItem> {
    let mut output_item_count = 0usize;
    let mut compaction_count = 0usize;
    let mut compaction_output = None;
    let mut saw_completed = false;
    while let Some(event) = stream.next().await {
        match event? {
            ResponseEvent::OutputItemDone(item) => {
                output_item_count += 1;
                if let ResponseItem::Compaction { .. } = item {
                    compaction_count += 1;
                    if compaction_output.is_none() {
                        compaction_output = Some(item);
                    }
                }
            }
            ResponseEvent::Completed { .. } => {
                saw_completed = true;
                break;
            }
            _ => {}
        }
    }

    if !saw_completed {
        return Err(CodexErr::Stream(
            "remote compaction v2 stream closed before response.completed".to_string(),
            None,
        ));
    }

    if compaction_count != 1 {
        return Err(CodexErr::Fatal(format!(
            "remote compaction v2 expected exactly one compaction output item, got {compaction_count} from {output_item_count} output items"
        )));
    }

    let Some(compaction_output) = compaction_output else {
        unreachable!("compaction output must exist when count is exactly one");
    };
    Ok(compaction_output)
}

fn build_v2_compacted_history(
    prompt_input: &[ResponseItem],
    compaction_output: ResponseItem,
) -> Vec<ResponseItem> {
    let retained = prompt_input
        .iter()
        .filter(|item| is_retained_for_remote_compaction_v2(item))
        .filter(|item| should_keep_compacted_history_item(item))
        .cloned()
        .collect::<Vec<_>>();
    let mut retained =
        truncate_retained_messages_for_remote_compaction(retained, RETAINED_MESSAGE_TOKEN_BUDGET);
    retained.push(compaction_output);
    retained
}

fn is_retained_for_remote_compaction_v2(item: &ResponseItem) -> bool {
    let ResponseItem::Message { role, .. } = item else {
        return false;
    };

    matches!(role.as_str(), "user" | "developer" | "system")
}

fn truncate_retained_messages_for_remote_compaction(
    items: Vec<ResponseItem>,
    max_tokens: usize,
) -> Vec<ResponseItem> {
    let mut remaining = max_tokens;
    let mut truncated_reversed = Vec::with_capacity(items.len());
    for item in items.into_iter().rev() {
        if remaining == 0 {
            continue;
        }

        let token_count = message_text_token_count(&item).max(1);
        if token_count <= remaining {
            truncated_reversed.push(item);
            remaining = remaining.saturating_sub(token_count);
        } else if let Some(truncated_item) =
            truncate_message_text_to_token_budget(item, /*max_tokens*/ remaining)
        {
            truncated_reversed.push(truncated_item);
            remaining = 0;
        }
    }
    truncated_reversed.reverse();
    truncated_reversed
}

fn message_text_token_count(item: &ResponseItem) -> usize {
    let ResponseItem::Message { content, .. } = item else {
        return 0;
    };

    content
        .iter()
        .map(|item| match item {
            ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                approx_token_count(text)
            }
            ContentItem::InputImage { .. } => 0,
        })
        .sum()
}

fn truncate_message_text_to_token_budget(
    item: ResponseItem,
    max_tokens: usize,
) -> Option<ResponseItem> {
    let ResponseItem::Message {
        id,
        role,
        content,
        phase,
    } = item
    else {
        return Some(item);
    };

    let mut remaining = max_tokens;
    let mut truncated_content = Vec::with_capacity(content.len());
    for mut content_item in content {
        match &mut content_item {
            ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                if remaining == 0 {
                    continue;
                }

                let token_count = approx_token_count(text);
                if token_count <= remaining {
                    remaining = remaining.saturating_sub(token_count);
                } else {
                    *text = truncate_text(text, TruncationPolicy::Tokens(remaining));
                    remaining = 0;
                }
                if !text.is_empty() {
                    truncated_content.push(content_item);
                }
            }
            ContentItem::InputImage { .. } => truncated_content.push(content_item),
        }
    }

    if truncated_content.is_empty() {
        return None;
    }

    Some(ResponseItem::Message {
        id,
        role,
        content: truncated_content,
        phase,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::MessagePhase;
    use codex_protocol::protocol::TokenUsage;
    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    fn message(role: &str, text: &str, phase: Option<MessagePhase>) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: role.to_string(),
            content: vec![ContentItem::InputText {
                text: text.to_string(),
            }],
            phase,
        }
    }

    fn response_stream(events: Vec<CodexResult<ResponseEvent>>) -> ResponseStream {
        let (tx_event, rx_event) = mpsc::channel(events.len().max(1));
        for event in events {
            tx_event
                .try_send(event)
                .expect("response stream test channel should have capacity");
        }
        drop(tx_event);
        ResponseStream {
            rx_event,
            consumer_dropped: CancellationToken::new(),
        }
    }

    #[test]
    fn build_v2_compacted_history_filters_to_installed_retention_shape() {
        let input = vec![
            message("developer", "dev", /*phase*/ None),
            message("system", "sys", /*phase*/ None),
            message("user", "user", /*phase*/ None),
            message("assistant", "commentary", Some(MessagePhase::Commentary)),
            message("assistant", "final", Some(MessagePhase::FinalAnswer)),
            ResponseItem::FunctionCall {
                id: None,
                name: "shell_command".to_string(),
                namespace: None,
                arguments: "{}".to_string(),
                call_id: "call_1".to_string(),
            },
            ResponseItem::Compaction {
                encrypted_content: "old".to_string(),
            },
        ];
        let output = ResponseItem::Compaction {
            encrypted_content: "new".to_string(),
        };

        let history = build_v2_compacted_history(&input, output.clone());

        assert_eq!(
            history,
            vec![message("user", "user", /*phase*/ None), output]
        );
    }

    #[test]
    fn build_v2_compacted_history_discards_messages_before_truncating() {
        let old = message("user", "old", /*phase*/ None);
        let new = message("user", "new", /*phase*/ None);
        let huge_developer_message = "d".repeat((RETAINED_MESSAGE_TOKEN_BUDGET + 1) * 4);
        let huge_contextual_message = format!(
            "<environment_context>\n{}\n</environment_context>",
            "c".repeat((RETAINED_MESSAGE_TOKEN_BUDGET + 1) * 4)
        );
        let input = vec![
            old.clone(),
            message("developer", &huge_developer_message, /*phase*/ None),
            message("user", &huge_contextual_message, /*phase*/ None),
            new.clone(),
        ];
        let output = ResponseItem::Compaction {
            encrypted_content: "new".to_string(),
        };

        let history = build_v2_compacted_history(&input, output.clone());

        assert_eq!(history, vec![old, new, output]);
    }

    #[test]
    fn retained_history_truncation_keeps_newest_messages_first() {
        let middle = message("user", "middle1234", /*phase*/ None);
        let new = message("user", "new", /*phase*/ None);
        let retained = vec![
            message("user", "old-old", /*phase*/ None),
            middle,
            new.clone(),
        ];

        let truncated =
            truncate_retained_messages_for_remote_compaction(retained, /*max_tokens*/ 3);

        assert_eq!(
            truncated,
            vec![
                message("user", "midd…1 tokens truncated…1234", /*phase*/ None),
                new,
            ]
        );
    }

    #[test]
    fn retained_history_truncation_preserves_images_and_truncates_later_text_parts() {
        let item = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![
                ContentItem::InputText {
                    text: "abcdef".to_string(),
                },
                ContentItem::InputImage {
                    image_url: "data:image/png;base64,abc".to_string(),
                    detail: None,
                },
                ContentItem::OutputText {
                    text: "uvwxyz".to_string(),
                },
            ],
            phase: None,
        };

        let truncated =
            truncate_retained_messages_for_remote_compaction(vec![item], /*max_tokens*/ 3);

        assert_eq!(
            truncated,
            vec![ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![
                    ContentItem::InputText {
                        text: "abcdef".to_string(),
                    },
                    ContentItem::InputImage {
                        image_url: "data:image/png;base64,abc".to_string(),
                        detail: None,
                    },
                    ContentItem::OutputText {
                        text: "uv…1 tokens truncated…yz".to_string(),
                    },
                ],
                phase: None,
            }]
        );
    }

    #[test]
    fn retained_history_truncation_charges_image_only_messages() {
        let image_only_message = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputImage {
                image_url: "data:image/png;base64,abc".to_string(),
                detail: None,
            }],
            phase: None,
        };
        let newest = message("user", "new", /*phase*/ None);
        let retained = vec![
            message("user", "old", /*phase*/ None),
            image_only_message.clone(),
            newest.clone(),
        ];

        let truncated =
            truncate_retained_messages_for_remote_compaction(retained, /*max_tokens*/ 2);

        assert_eq!(truncated, vec![image_only_message, newest]);
    }

    #[test]
    fn retained_history_truncation_drops_image_only_messages_after_budget_is_spent() {
        let image_only_message = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputImage {
                image_url: "data:image/png;base64,abc".to_string(),
                detail: None,
            }],
            phase: None,
        };
        let newest = message("user", "new", /*phase*/ None);
        let retained = vec![image_only_message, newest.clone()];

        let truncated =
            truncate_retained_messages_for_remote_compaction(retained, /*max_tokens*/ 1);

        assert_eq!(truncated, vec![newest]);
    }

    #[tokio::test]
    async fn collect_compaction_output_accepts_additional_output_items() {
        let compaction = ResponseItem::Compaction {
            encrypted_content: "encrypted".to_string(),
        };
        let stream = response_stream(vec![
            Ok(ResponseEvent::OutputItemDone(message(
                "assistant",
                "IGNORED_COMPACT_REPLY",
                Some(MessagePhase::FinalAnswer),
            ))),
            Ok(ResponseEvent::OutputItemDone(compaction.clone())),
            Ok(ResponseEvent::Completed {
                response_id: "resp-compact".to_string(),
                token_usage: Some(TokenUsage {
                    input_tokens: 123_456,
                    cached_input_tokens: 7_890,
                    output_tokens: 42,
                    reasoning_output_tokens: 5,
                    total_tokens: 123_498,
                }),
                end_turn: Some(true),
            }),
        ]);

        let output = collect_compaction_output(stream)
            .await
            .expect("compaction should be collected");

        assert_eq!(output, compaction);
    }
}
