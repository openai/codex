use codex_protocol::ThreadId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::SearchToolCallParams;
use codex_protocol::openai_models::InputModality;
use codex_tools::INVALID_TOOL_SEARCH_QUERY;
use serde::Deserialize;
use std::collections::HashSet;
use tracing::info;
use tracing::warn;
use uuid::Uuid;

use crate::util::error_or_panic;

const IMAGE_CONTENT_OMITTED_PLACEHOLDER: &str =
    "image content omitted because you do not support image input";
// Changing this value would change model-visible IDs and invalidate prompt caches.
const SYNTHETIC_OUTPUT_ID_NAMESPACE: Uuid = Uuid::from_u128(0x90d38d3e_6a5b_4d52_bfe2_2f1e634bfac4);

/// Returns a canonical durable-history view when an item must differ from its raw event.
///
/// Client-executed tool search is the only case handled here because its arguments have a fixed
/// schema that can be safely deserialized and reserialized. Function-call arguments and custom
/// tool input are intentionally preserved as schema-specific or freeform strings.
pub(crate) fn canonicalize_for_durable_history(
    thread_id: &ThreadId,
    items: &[ResponseItem],
) -> Option<Vec<ResponseItem>> {
    if !items.iter().any(|item| {
        matches!(
            item,
            ResponseItem::ToolSearchCall { execution, .. } if execution == "client"
        )
    }) {
        return None;
    }

    Some(
        items
            .iter()
            .filter_map(|item| {
                let ResponseItem::ToolSearchCall {
                    id,
                    call_id,
                    status,
                    execution,
                    arguments,
                    internal_chat_message_metadata_passthrough,
                } = item
                else {
                    return Some(item.clone());
                };
                if execution != "client" {
                    return Some(item.clone());
                }

                let Some(call_id) = call_id.as_deref().filter(|call_id| !call_id.is_empty()) else {
                    warn!(
                        %thread_id,
                        "dropping client tool_search call with missing call_id from durable history"
                    );
                    return None;
                };

                let canonical_arguments = match SearchToolCallParams::deserialize(arguments) {
                    Ok(params) => match serde_json::to_value(params) {
                        Ok(canonical_arguments) => canonical_arguments,
                        Err(err) => {
                            warn!(
                                %thread_id,
                                call_id,
                                %err,
                                "failed to serialize canonical client tool_search arguments"
                            );
                            invalid_tool_search_arguments()
                        }
                    },
                    Err(err) => {
                        warn!(
                            %thread_id,
                            call_id,
                            error_category = ?err.classify(),
                            error_line = err.line(),
                            error_column = err.column(),
                            "replacing malformed client tool_search arguments in durable history"
                        );
                        invalid_tool_search_arguments()
                    }
                };

                Some(ResponseItem::ToolSearchCall {
                    id: id.clone(),
                    call_id: Some(call_id.to_string()),
                    status: status.clone(),
                    execution: execution.clone(),
                    arguments: canonical_arguments,
                    internal_chat_message_metadata_passthrough:
                        internal_chat_message_metadata_passthrough.clone(),
                })
            })
            .collect(),
    )
}

fn invalid_tool_search_arguments() -> serde_json::Value {
    serde_json::json!({ "query": INVALID_TOOL_SEARCH_QUERY })
}

pub(crate) fn ensure_call_outputs_present(items: &mut Vec<ResponseItem>) {
    let mut function_output_ids = HashSet::new();
    let mut tool_search_output_ids = HashSet::new();
    let mut custom_tool_output_ids = HashSet::new();
    for item in items.iter() {
        match item {
            ResponseItem::FunctionCallOutput { call_id, .. } => {
                function_output_ids.insert(call_id.as_str());
            }
            ResponseItem::ToolSearchOutput {
                call_id: Some(call_id),
                ..
            } => {
                tool_search_output_ids.insert(call_id.as_str());
            }
            ResponseItem::CustomToolCallOutput { call_id, .. } => {
                custom_tool_output_ids.insert(call_id.as_str());
            }
            _ => {}
        }
    }

    // Collect synthetic outputs to insert immediately after their calls.
    // Store the insertion position (index of call) alongside the item so
    // we can insert in reverse order and avoid index shifting.
    let mut missing_outputs_to_insert: Vec<(usize, ResponseItem)> = Vec::new();

    for (idx, item) in items.iter().enumerate() {
        match item {
            ResponseItem::FunctionCall { id, call_id, .. }
                if !function_output_ids.contains(call_id.as_str()) =>
            {
                info!("Function call output is missing for call id: {call_id}");
                missing_outputs_to_insert.push((
                    idx,
                    ResponseItem::FunctionCallOutput {
                        id: synthetic_output_id("fco", id.as_deref()),
                        call_id: call_id.clone(),
                        output: FunctionCallOutputPayload::from_text("aborted".to_string()),
                        internal_chat_message_metadata_passthrough: None,
                    },
                ));
            }
            ResponseItem::ToolSearchCall {
                id,
                call_id: Some(call_id),
                ..
            } if !tool_search_output_ids.contains(call_id.as_str()) => {
                info!("Tool search output is missing for call id: {call_id}");
                missing_outputs_to_insert.push((
                    idx,
                    ResponseItem::ToolSearchOutput {
                        id: synthetic_output_id("tso", id.as_deref()),
                        call_id: Some(call_id.clone()),
                        status: "completed".to_string(),
                        execution: "client".to_string(),
                        tools: Vec::new(),
                        internal_chat_message_metadata_passthrough: None,
                    },
                ));
            }
            ResponseItem::CustomToolCall { id, call_id, .. }
                if !custom_tool_output_ids.contains(call_id.as_str()) =>
            {
                error_or_panic(format!(
                    "Custom tool call output is missing for call id: {call_id}"
                ));
                missing_outputs_to_insert.push((
                    idx,
                    ResponseItem::CustomToolCallOutput {
                        id: synthetic_output_id("ctco", id.as_deref()),
                        call_id: call_id.clone(),
                        name: None,
                        output: FunctionCallOutputPayload::from_text("aborted".to_string()),
                        internal_chat_message_metadata_passthrough: None,
                    },
                ));
            }
            // LocalShellCall is represented in upstream streams by a FunctionCallOutput
            ResponseItem::LocalShellCall {
                id,
                call_id: Some(call_id),
                ..
            } if !function_output_ids.contains(call_id.as_str()) => {
                error_or_panic(format!(
                    "Local shell call output is missing for call id: {call_id}"
                ));
                missing_outputs_to_insert.push((
                    idx,
                    ResponseItem::FunctionCallOutput {
                        id: synthetic_output_id("fco", id.as_deref()),
                        call_id: call_id.clone(),
                        output: FunctionCallOutputPayload::from_text("aborted".to_string()),
                        internal_chat_message_metadata_passthrough: None,
                    },
                ));
            }
            _ => {}
        }
    }
    drop((
        function_output_ids,
        tool_search_output_ids,
        custom_tool_output_ids,
    ));

    // Insert synthetic outputs in reverse index order to avoid re-indexing.
    for (idx, output_item) in missing_outputs_to_insert.into_iter().rev() {
        items.insert(idx + 1, output_item);
    }
}

/// Derives a stable ID for a prompt-only output from its source call's item ID.
///
/// Prompt normalization can run repeatedly without persisting its synthetic
/// outputs, so the namespace and name format must remain stable across retries
/// and resumes to preserve prompt-cache reuse. Returning `None` when the source
/// call has no ID preserves the legacy behavior for older history items.
fn synthetic_output_id(prefix: &str, item_id: Option<&str>) -> Option<String> {
    let source_id = item_id.filter(|id| !id.is_empty())?;
    let name = format!("{prefix}:{source_id}");
    Some(format!(
        "{prefix}_{}",
        Uuid::new_v5(&SYNTHETIC_OUTPUT_ID_NAMESPACE, name.as_bytes())
    ))
}

pub(crate) fn remove_orphan_outputs(items: &mut Vec<ResponseItem>) {
    let function_call_ids: HashSet<String> = items
        .iter()
        .filter_map(|i| match i {
            ResponseItem::FunctionCall { call_id, .. } => Some(call_id.clone()),
            _ => None,
        })
        .collect();

    let tool_search_call_ids: HashSet<String> = items
        .iter()
        .filter_map(|i| match i {
            ResponseItem::ToolSearchCall {
                call_id: Some(call_id),
                ..
            } => Some(call_id.clone()),
            _ => None,
        })
        .collect();

    let local_shell_call_ids: HashSet<String> = items
        .iter()
        .filter_map(|i| match i {
            ResponseItem::LocalShellCall {
                call_id: Some(call_id),
                ..
            } => Some(call_id.clone()),
            _ => None,
        })
        .collect();

    let custom_tool_call_ids: HashSet<String> = items
        .iter()
        .filter_map(|i| match i {
            ResponseItem::CustomToolCall { call_id, .. } => Some(call_id.clone()),
            _ => None,
        })
        .collect();

    items.retain(|item| match item {
        ResponseItem::FunctionCallOutput { call_id, .. } => {
            let has_match =
                function_call_ids.contains(call_id) || local_shell_call_ids.contains(call_id);
            if !has_match {
                error_or_panic(format!(
                    "Orphan function call output for call id: {call_id}"
                ));
            }
            has_match
        }
        ResponseItem::CustomToolCallOutput { call_id, .. } => {
            let has_match = custom_tool_call_ids.contains(call_id);
            if !has_match {
                error_or_panic(format!(
                    "Orphan custom tool call output for call id: {call_id}"
                ));
            }
            has_match
        }
        ResponseItem::ToolSearchOutput { execution, .. } if execution == "server" => true,
        ResponseItem::ToolSearchOutput {
            call_id: Some(call_id),
            ..
        } => {
            let has_match = tool_search_call_ids.contains(call_id);
            if !has_match {
                error_or_panic(format!("Orphan tool search output for call id: {call_id}"));
            }
            has_match
        }
        ResponseItem::ToolSearchOutput { call_id: None, .. } => true,
        _ => true,
    });
}

pub(crate) fn remove_corresponding_for(items: &mut Vec<ResponseItem>, item: &ResponseItem) {
    match item {
        ResponseItem::FunctionCall { call_id, .. } => {
            remove_first_matching(items, |i| {
                matches!(
                    i,
                    ResponseItem::FunctionCallOutput {
                        call_id: existing, ..
                    } if existing == call_id
                )
            });
        }
        ResponseItem::FunctionCallOutput { call_id, .. } => {
            if let Some(pos) = items.iter().position(|i| {
                matches!(i, ResponseItem::FunctionCall { call_id: existing, .. } if existing == call_id)
            }) {
                items.remove(pos);
            } else if let Some(pos) = items.iter().position(|i| {
                matches!(i, ResponseItem::LocalShellCall { call_id: Some(existing), .. } if existing == call_id)
            }) {
                items.remove(pos);
            }
        }
        ResponseItem::ToolSearchCall {
            call_id: Some(call_id),
            ..
        } => {
            remove_first_matching(items, |i| {
                matches!(
                    i,
                    ResponseItem::ToolSearchOutput {
                        call_id: Some(existing),
                        ..
                    } if existing == call_id
                )
            });
        }
        ResponseItem::ToolSearchOutput {
            call_id: Some(call_id),
            ..
        } => {
            remove_first_matching(
                items,
                |i| {
                    matches!(
                        i,
                        ResponseItem::ToolSearchCall {
                            call_id: Some(existing),
                            ..
                        } if existing == call_id
                    )
                },
            );
        }
        ResponseItem::CustomToolCall { call_id, .. } => {
            remove_first_matching(items, |i| {
                matches!(
                    i,
                    ResponseItem::CustomToolCallOutput {
                        call_id: existing, ..
                    } if existing == call_id
                )
            });
        }
        ResponseItem::CustomToolCallOutput { call_id, .. } => {
            remove_first_matching(
                items,
                |i| matches!(i, ResponseItem::CustomToolCall { call_id: existing, .. } if existing == call_id),
            );
        }
        ResponseItem::LocalShellCall {
            call_id: Some(call_id),
            ..
        } => {
            remove_first_matching(items, |i| {
                matches!(
                    i,
                    ResponseItem::FunctionCallOutput {
                        call_id: existing, ..
                    } if existing == call_id
                )
            });
        }
        _ => {}
    }
}

fn remove_first_matching<F>(items: &mut Vec<ResponseItem>, predicate: F)
where
    F: Fn(&ResponseItem) -> bool,
{
    if let Some(pos) = items.iter().position(predicate) {
        items.remove(pos);
    }
}

/// Strip image content from messages and tool outputs when the model does not support images.
/// When `input_modalities` contains `InputModality::Image`, no stripping is performed.
pub(crate) fn strip_images_when_unsupported(
    input_modalities: &[InputModality],
    items: &mut [ResponseItem],
) {
    let supports_images = input_modalities.contains(&InputModality::Image);
    if supports_images {
        return;
    }

    for item in items.iter_mut() {
        match item {
            ResponseItem::Message { content, .. } => {
                let mut normalized_content = Vec::with_capacity(content.len());
                for content_item in content.iter() {
                    match content_item {
                        ContentItem::InputImage { .. } => {
                            normalized_content.push(ContentItem::InputText {
                                text: IMAGE_CONTENT_OMITTED_PLACEHOLDER.to_string(),
                            });
                        }
                        _ => normalized_content.push(content_item.clone()),
                    }
                }
                *content = normalized_content;
            }
            ResponseItem::FunctionCallOutput { output, .. }
            | ResponseItem::CustomToolCallOutput { output, .. } => {
                if let Some(content_items) = output.content_items_mut() {
                    let mut normalized_content_items = Vec::with_capacity(content_items.len());
                    for content_item in content_items.iter() {
                        match content_item {
                            FunctionCallOutputContentItem::InputImage { .. } => {
                                normalized_content_items.push(
                                    FunctionCallOutputContentItem::InputText {
                                        text: IMAGE_CONTENT_OMITTED_PLACEHOLDER.to_string(),
                                    },
                                );
                            }
                            _ => normalized_content_items.push(content_item.clone()),
                        }
                    }
                    *content_items = normalized_content_items;
                }
            }
            ResponseItem::ImageGenerationCall { result, .. } => {
                result.clear();
            }
            _ => {}
        }
    }
}
