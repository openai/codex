use std::collections::BTreeMap;

use crate::compact_remote_v2::should_retain_client_item;
use crate::context::ContextualUserFragment;
use crate::context::ImageResizeNotice;
use crate::context_manager::ContextManager;
use crate::context_manager::estimate_item_token_count_with_audio;
use crate::session::FileAudioDurations;
use crate::session::turn_context::TurnContext;
use crate::tools::is_matching_tool_output;
use codex_history::ResponseItemEnvelope;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_utils_output_truncation::approx_token_count;

const CONTEXT_WINDOW_TRUNCATED_OUTPUT_MESSAGE: &str =
    "Output exceeded the available model context and was truncated";

pub(crate) struct HistoryItemGroup<'a> {
    pub(crate) source_position: usize,
    pub(crate) source: &'a ResponseItemEnvelope,
    pub(crate) attached_notice: Option<&'a ResponseItemEnvelope>,
    pub(crate) tool_call: Option<&'a ResponseItemEnvelope>,
}

impl HistoryItemGroup<'_> {
    pub(crate) fn keep(
        &self,
        selected: &mut BTreeMap<usize, ResponseItemEnvelope>,
        source: ResponseItemEnvelope,
    ) {
        if let Some(call) = self.tool_call {
            selected.insert(self.source_position - 1, call.clone());
        }
        selected.insert(self.source_position, source);
        if let Some(notice) = self.attached_notice {
            selected.insert(self.source_position + 1, notice.clone());
        }
    }

    pub(crate) fn estimated_token_count(&self, audio_durations: &FileAudioDurations) -> i128 {
        self.tool_call
            .into_iter()
            .chain(std::iter::once(self.source))
            .chain(self.attached_notice)
            .map(|item| i128::from(estimate_item_token_count_with_audio(item, audio_durations)))
            .fold(0, i128::saturating_add)
    }
}

pub(crate) fn newest_history_item_groups(
    mut items: &[ResponseItemEnvelope],
) -> impl Iterator<Item = HistoryItemGroup<'_>> {
    std::iter::from_fn(move || {
        let (last, preceding) = items.split_last()?;
        let (source, attached_notice, rest) = match preceding.split_last() {
            Some((source, rest)) if is_attached_notice(source, last) => (source, Some(last), rest),
            _ => (last, None, preceding),
        };
        let source_position = rest.len();
        // Injection requires explicitly retained exchanges to be adjacent in the request.
        let (tool_call, rest) = match rest.split_last() {
            Some((call, rest))
                if source.is_always_retained()
                    && call.is_always_retained()
                    && is_matching_tool_output(&call.item, &source.item) =>
            {
                (Some(call), rest)
            }
            _ => (None, rest),
        };
        items = rest;
        Some(HistoryItemGroup {
            source_position,
            source,
            attached_notice,
            tool_call,
        })
    })
}

fn is_attached_notice(source: &ResponseItemEnvelope, notice: &ResponseItemEnvelope) -> bool {
    let can_have_resize_notice = match &source.item {
        ResponseItem::Message { role, .. } => role == "user",
        ResponseItem::FunctionCallOutput { .. } | ResponseItem::CustomToolCallOutput { .. } => true,
        _ => false,
    };
    // A client may send the same text as a resize notice with its own policy.
    if !can_have_resize_notice
        || notice.metadata.as_ref().is_some_and(|metadata| {
            metadata.client_authored || metadata.retain_across_windows.is_some()
        })
    {
        return false;
    }
    matches!(
        &notice.item,
        ResponseItem::Message { role, content, .. }
            if role == "developer"
                && matches!(
                    content.as_slice(),
                    [ContentItem::InputText { text }]
                        if ImageResizeNotice::matches_text(text)
                )
    )
}

pub(crate) fn trim_function_call_history_to_fit_context_window(
    history: &mut ContextManager,
    turn_context: &TurnContext,
    context_window: Option<i64>,
    base_instructions: &BaseInstructions,
) -> (usize, i64) {
    let Some(context_window) = context_window else {
        return (0, 0);
    };
    // Keep the unclamped total so replacing an item cannot lose an overflow hidden by i64
    // saturation in the normal history estimator.
    let base_tokens =
        i128::try_from(approx_token_count(&base_instructions.text)).unwrap_or(i128::MAX);
    let audio_durations = &history.file_audio_durations;
    let original_items = history.annotated_items();
    let mut estimated_tokens = original_items
        .iter()
        .map(|item| i128::from(estimate_item_token_count_with_audio(item, audio_durations)))
        .fold(base_tokens, i128::saturating_add);
    let initial_estimated_tokens = i64::try_from(estimated_tokens).unwrap_or(i64::MAX);
    if initial_estimated_tokens <= context_window {
        return (0, 0);
    }
    let mut rewritten_items = Vec::new();
    let mut retained_len = original_items.len();
    for group in newest_history_item_groups(original_items) {
        if i64::try_from(estimated_tokens).unwrap_or(i64::MAX) <= context_window {
            break;
        }
        if should_retain_client_item(group.source, &turn_context.config.features) {
            break;
        }
        let Some(rewritten_item) = rewritten_output_for_context_window(group.source) else {
            break;
        };
        estimated_tokens = estimated_tokens
            .saturating_sub(group.estimated_token_count(audio_durations))
            .saturating_add(i128::from(estimate_item_token_count_with_audio(
                &rewritten_item.item,
                audio_durations,
            )));
        retained_len = group.source_position;
        rewritten_items.push(rewritten_item);
    }

    let rewritten_outputs = rewritten_items.len();
    if rewritten_outputs > 0 {
        let mut items = original_items[..retained_len].to_vec();
        items.extend(rewritten_items.into_iter().rev());
        history.replace_annotated(items);
    }

    let final_estimated_tokens = i64::try_from(estimated_tokens).unwrap_or(i64::MAX);
    let estimated_deleted_tokens = initial_estimated_tokens.saturating_sub(final_estimated_tokens);
    (rewritten_outputs, estimated_deleted_tokens)
}

fn rewritten_output_for_context_window(
    envelope: &ResponseItemEnvelope,
) -> Option<ResponseItemEnvelope> {
    let item = match &envelope.item {
        ResponseItem::FunctionCallOutput {
            id,
            call_id,
            name,
            namespace,
            output,
            internal_chat_message_metadata_passthrough: metadata,
        } => ResponseItem::FunctionCallOutput {
            id: id.clone(),
            call_id: call_id.clone(),
            name: name.clone(),
            namespace: namespace.clone(),
            output: truncated_output_payload(output),
            internal_chat_message_metadata_passthrough: metadata.clone(),
        },
        ResponseItem::CustomToolCallOutput {
            id,
            call_id,
            name,
            output,
            internal_chat_message_metadata_passthrough: metadata,
        } => ResponseItem::CustomToolCallOutput {
            id: id.clone(),
            call_id: call_id.clone(),
            name: name.clone(),
            output: truncated_output_payload(output),
            internal_chat_message_metadata_passthrough: metadata.clone(),
        },
        ResponseItem::ToolSearchOutput {
            id,
            call_id,
            status,
            execution,
            internal_chat_message_metadata_passthrough: metadata,
            ..
        } => ResponseItem::ToolSearchOutput {
            id: id.clone(),
            call_id: call_id.clone(),
            status: status.clone(),
            execution: execution.clone(),
            tools: Vec::new(),
            internal_chat_message_metadata_passthrough: metadata.clone(),
        },
        _ => return None,
    };
    Some(ResponseItemEnvelope {
        item,
        metadata: envelope.metadata.clone(),
    })
}

fn truncated_output_payload(output: &FunctionCallOutputPayload) -> FunctionCallOutputPayload {
    FunctionCallOutputPayload {
        body: FunctionCallOutputBody::Text(CONTEXT_WINDOW_TRUNCATED_OUTPUT_MESSAGE.to_string()),
        success: output.success,
    }
}

#[cfg(test)]
#[path = "compact_remote_history_tests.rs"]
mod metadata_tests;
