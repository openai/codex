use std::collections::HashMap;
use std::collections::HashSet;

use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::InputModality;

use crate::agent::AgentStatus;
use crate::session_prefix::is_session_prefix;
use crate::session_prefix::parse_subagent_notification;
use crate::tools::handlers::multi_agents::wait::parse_wait_output_statuses;
use crate::util::error_or_panic;
use tracing::info;

const IMAGE_CONTENT_OMITTED_PLACEHOLDER: &str =
    "image content omitted because you do not support image input";

pub(crate) fn ensure_call_outputs_present(items: &mut Vec<ResponseItem>) {
    // Collect synthetic outputs to insert immediately after their calls.
    // Store the insertion position (index of call) alongside the item so
    // we can insert in reverse order and avoid index shifting.
    let mut missing_outputs_to_insert: Vec<(usize, ResponseItem)> = Vec::new();

    for (idx, item) in items.iter().enumerate() {
        match item {
            ResponseItem::FunctionCall { call_id, .. } => {
                let has_output = items.iter().any(|i| match i {
                    ResponseItem::FunctionCallOutput {
                        call_id: existing, ..
                    } => existing == call_id,
                    _ => false,
                });

                if !has_output {
                    info!("Function call output is missing for call id: {call_id}");
                    missing_outputs_to_insert.push((
                        idx,
                        ResponseItem::FunctionCallOutput {
                            call_id: call_id.clone(),
                            output: FunctionCallOutputPayload {
                                body: FunctionCallOutputBody::Text("aborted".to_string()),
                                ..Default::default()
                            },
                        },
                    ));
                }
            }
            ResponseItem::CustomToolCall { call_id, .. } => {
                let has_output = items.iter().any(|i| match i {
                    ResponseItem::CustomToolCallOutput {
                        call_id: existing, ..
                    } => existing == call_id,
                    _ => false,
                });

                if !has_output {
                    error_or_panic(format!(
                        "Custom tool call output is missing for call id: {call_id}"
                    ));
                    missing_outputs_to_insert.push((
                        idx,
                        ResponseItem::CustomToolCallOutput {
                            call_id: call_id.clone(),
                            output: "aborted".to_string(),
                        },
                    ));
                }
            }
            // LocalShellCall is represented in upstream streams by a FunctionCallOutput
            ResponseItem::LocalShellCall { call_id, .. } => {
                if let Some(call_id) = call_id.as_ref() {
                    let has_output = items.iter().any(|i| match i {
                        ResponseItem::FunctionCallOutput {
                            call_id: existing, ..
                        } => existing == call_id,
                        _ => false,
                    });

                    if !has_output {
                        error_or_panic(format!(
                            "Local shell call output is missing for call id: {call_id}"
                        ));
                        missing_outputs_to_insert.push((
                            idx,
                            ResponseItem::FunctionCallOutput {
                                call_id: call_id.clone(),
                                output: FunctionCallOutputPayload {
                                    body: FunctionCallOutputBody::Text("aborted".to_string()),
                                    ..Default::default()
                                },
                            },
                        ));
                    }
                }
            }
            _ => {}
        }
    }

    // Insert synthetic outputs in reverse index order to avoid re-indexing.
    for (idx, output_item) in missing_outputs_to_insert.into_iter().rev() {
        items.insert(idx + 1, output_item);
    }
}

pub(crate) fn remove_orphan_outputs(items: &mut Vec<ResponseItem>) {
    let function_call_ids: HashSet<String> = items
        .iter()
        .filter_map(|i| match i {
            ResponseItem::FunctionCall { call_id, .. } => Some(call_id.clone()),
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

pub(crate) fn drop_subagent_notifications_covered_by_wait(items: &mut Vec<ResponseItem>) {
    // Track the most recent status emitted by `wait` for each sub-agent id.
    let mut latest_wait_status_by_agent: HashMap<String, (AgentStatus, usize)> = HashMap::new();
    let mut wait_call_ids = HashSet::new();
    let mut notification_indexes_to_drop = HashSet::new();
    // Index of the most recent item that is not a user session-prefix message.
    // We only dedupe when the `wait` output and notification are separated solely by
    // session-prefix user messages.
    let mut last_non_prefix_index: Option<usize> = None;

    for (index, item) in items.iter().enumerate() {
        let is_user_session_prefix_message = matches!(
            item,
            ResponseItem::Message { role, content, .. }
                if role == "user"
                    && content.iter().all(|content_item| match content_item {
                        ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                            is_session_prefix(text)
                        }
                        ContentItem::InputImage { .. } => false,
                    })
        );

        match item {
            ResponseItem::FunctionCall { name, call_id, .. } if name == "wait" => {
                wait_call_ids.insert(call_id.clone());
            }
            ResponseItem::FunctionCallOutput { call_id, output } => {
                // Deduping is only for outputs that correspond to explicit `wait` calls.
                if wait_call_ids.contains(call_id)
                    && let Some(statuses) = parse_wait_output_statuses(output)
                {
                    for (agent_id, status) in statuses {
                        latest_wait_status_by_agent.insert(agent_id.to_string(), (status, index));
                    }
                }
            }
            ResponseItem::Message { role, content, .. }
                if role == "user" && is_user_session_prefix_message =>
            {
                let notification = content.iter().find_map(|content_item| match content_item {
                    ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                        parse_subagent_notification(text)
                    }
                    ContentItem::InputImage { .. } => None,
                });
                if let Some(notification) = notification
                    && let Some((wait_status, wait_output_index)) =
                        latest_wait_status_by_agent.get(&notification.agent_id)
                {
                    let no_non_prefix_between = match last_non_prefix_index {
                        Some(last_non_prefix) => last_non_prefix <= *wait_output_index,
                        None => true,
                    };
                    if no_non_prefix_between && wait_status == &notification.status {
                        notification_indexes_to_drop.insert(index);
                    }
                }
            }
            _ => {}
        }

        if !is_user_session_prefix_message {
            last_non_prefix_index = Some(index);
        }
    }

    if notification_indexes_to_drop.is_empty() {
        return;
    }

    *items = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            (!notification_indexes_to_drop.contains(&index)).then_some(item.clone())
        })
        .collect();
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
            ResponseItem::FunctionCallOutput { output, .. } => {
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
            _ => {}
        }
    }
}
