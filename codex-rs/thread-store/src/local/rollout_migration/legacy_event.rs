//! Converts legacy persisted events into modern paginated turn items.
//!
//! Legacy rollouts persisted events like `ExecCommandEnd`, `PatchApplyEnd`, and
//! `McpToolCallEnd`. Paginated history instead persists `ItemCompleted(item)` records containing
//! canonical `TurnItem`s.
//!
//! This module owns that old-event -> new-item mapping, including stable synthesized item IDs.
//! It intentionally stays separate from live thread-history reduction because migration needs a
//! frozen adapter for historical rollout payloads.

use codex_extension_items::ExtensionItem;
use codex_extension_items::image_generation::ImageGenerationItem;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::AgentMessageItem;
use codex_protocol::items::CommandExecutionItem;
use codex_protocol::items::ContextCompactionItem;
use codex_protocol::items::DynamicToolCallItem;
use codex_protocol::items::DynamicToolCallStatus;
use codex_protocol::items::EnteredReviewModeItem;
use codex_protocol::items::ExitedReviewModeItem;
use codex_protocol::items::FileChangeItem;
use codex_protocol::items::McpToolCallError;
use codex_protocol::items::McpToolCallItem;
use codex_protocol::items::McpToolCallStatus;
use codex_protocol::items::SubAgentActivityItem;
use codex_protocol::items::TurnItem;
use codex_protocol::items::UserMessageItem;
use codex_protocol::items::WebSearchItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::UserMessageEvent;
use codex_protocol::user_input::UserInput;

use crate::ThreadStoreResult;

pub(super) fn user_message_item(
    event: UserMessageEvent,
    next_item_id: &mut impl FnMut() -> ThreadStoreResult<String>,
) -> ThreadStoreResult<TurnItem> {
    let mut content = Vec::new();
    if !event.message.trim().is_empty() {
        content.push(UserInput::Text {
            text: event.message,
            text_elements: event.text_elements,
        });
    }
    if let Some(images) = event.images {
        for (index, image_url) in images.into_iter().enumerate() {
            content.push(UserInput::Image {
                image_url,
                detail: event.image_details.get(index).copied().flatten(),
            });
        }
    }
    for (index, path) in event.local_images.into_iter().enumerate() {
        content.push(UserInput::LocalImage {
            path,
            detail: event.local_image_details.get(index).copied().flatten(),
        });
    }
    if let Some(audio) = event.audio {
        content.extend(
            audio
                .into_iter()
                .map(|audio_url| UserInput::Audio { audio_url }),
        );
    }
    content.extend(
        event
            .local_audio
            .into_iter()
            .map(|path| UserInput::LocalAudio { path }),
    );
    Ok(TurnItem::UserMessage(UserMessageItem {
        id: next_item_id()?,
        client_id: event.client_id,
        content,
    }))
}

/// Convert historical completion events into the canonical completed-item payloads
/// that paginated rollouts persist. This deliberately remains a frozen migration
/// adapter rather than sharing the live thread-history reducer: migration must
/// preserve historical payloads while synthesizing stable item and turn IDs.
pub(super) fn completed_item(
    event: &EventMsg,
    next_item_id: &mut impl FnMut() -> ThreadStoreResult<String>,
) -> ThreadStoreResult<Option<(TurnItem, Option<String>)>> {
    let result = match event {
        EventMsg::AgentMessage(event) if !event.message.is_empty() => Some((
            TurnItem::AgentMessage(AgentMessageItem {
                id: next_item_id()?,
                content: vec![AgentMessageContent::Text {
                    text: event.message.clone(),
                }],
                phase: event.phase.clone(),
                memory_citation: event.memory_citation.clone(),
            }),
            None,
        )),
        EventMsg::PatchApplyEnd(event) => Some((
            TurnItem::FileChange(FileChangeItem {
                id: event.call_id.clone(),
                changes: event.changes.clone(),
                status: Some(event.status.clone()),
                auto_approved: None,
                stdout: (!event.stdout.is_empty()).then(|| event.stdout.clone()),
                stderr: (!event.stderr.is_empty()).then(|| event.stderr.clone()),
            }),
            (!event.turn_id.is_empty()).then(|| event.turn_id.clone()),
        )),
        EventMsg::McpToolCallEnd(event) => {
            let (result, error) = match &event.result {
                Ok(result) => (Some(result.clone()), None),
                Err(message) => (
                    None,
                    Some(McpToolCallError {
                        message: message.clone(),
                    }),
                ),
            };
            Some((
                TurnItem::McpToolCall(McpToolCallItem {
                    id: event.call_id.clone(),
                    server: event.invocation.server.clone(),
                    tool: event.invocation.tool.clone(),
                    arguments: event
                        .invocation
                        .arguments
                        .clone()
                        .unwrap_or(serde_json::Value::Null),
                    connector_id: event.connector_id.clone(),
                    mcp_app_resource_uri: event.mcp_app_resource_uri.clone(),
                    link_id: event.link_id.clone(),
                    app_name: event.app_name.clone(),
                    action_name: event.action_name.clone(),
                    plugin_id: event.plugin_id.clone(),
                    read_only_hint: event.read_only_hint,
                    status: if event.is_success() {
                        McpToolCallStatus::Completed
                    } else {
                        McpToolCallStatus::Failed
                    },
                    result,
                    error,
                    duration: Some(event.duration),
                }),
                None,
            ))
        }
        EventMsg::WebSearchEnd(event) => Some((
            TurnItem::WebSearch(WebSearchItem {
                id: event.call_id.clone(),
                query: event.query.clone(),
                action: event.action.clone(),
                results: event.results.clone(),
            }),
            None,
        )),
        EventMsg::ImageGenerationEnd(event) => Some((
            TurnItem::Extension(ExtensionItem::ImageGeneration(ImageGenerationItem {
                id: event.call_id.clone(),
                status: event.status.clone(),
                revised_prompt: event.revised_prompt.clone(),
                result: event.result.clone(),
                transparent_background: event.transparent_background,
                failure: event.failure.clone(),
                saved_path: event.saved_path.clone(),
            })),
            None,
        )),
        EventMsg::ContextCompacted(_) => Some((
            TurnItem::ContextCompaction(ContextCompactionItem {
                id: next_item_id()?,
            }),
            None,
        )),
        EventMsg::EnteredReviewMode(event) => Some((
            TurnItem::EnteredReviewMode(EnteredReviewModeItem {
                id: match event.item_id.clone() {
                    Some(id) => id,
                    None => next_item_id()?,
                },
                target: event.target.clone(),
                user_facing_hint: event
                    .user_facing_hint
                    .clone()
                    .unwrap_or_else(|| "Review requested.".to_string()),
            }),
            event.turn_id.clone(),
        )),
        EventMsg::ExitedReviewMode(event) => Some((
            TurnItem::ExitedReviewMode(ExitedReviewModeItem {
                id: match event.item_id.clone() {
                    Some(id) => id,
                    None => next_item_id()?,
                },
                review_output: event.review_output.clone(),
            }),
            event.turn_id.clone(),
        )),
        EventMsg::SubAgentActivity(event) => Some((
            TurnItem::SubAgentActivity(SubAgentActivityItem {
                id: event.event_id.clone(),
                kind: event.kind,
                agent_thread_id: event.agent_thread_id,
                agent_path: event.agent_path.clone(),
            }),
            None,
        )),
        EventMsg::ExecCommandEnd(event) => Some((
            TurnItem::CommandExecution(CommandExecutionItem {
                id: event.call_id.clone(),
                plugin_id: event.plugin_id.clone(),
                script_path: event.script_path.clone(),
                process_id: event.process_id.clone(),
                command: event.command.clone(),
                cwd: event.cwd.clone(),
                parsed_cmd: event.parsed_cmd.clone(),
                source: event.source,
                interaction_input: event.interaction_input.clone(),
                status: event.status.clone().into(),
                stdout: (!event.stdout.is_empty()).then(|| event.stdout.clone()),
                stderr: (!event.stderr.is_empty()).then(|| event.stderr.clone()),
                aggregated_output: (!event.aggregated_output.is_empty())
                    .then(|| event.aggregated_output.clone()),
                exit_code: Some(event.exit_code),
                duration: Some(event.duration),
                formatted_output: (!event.formatted_output.is_empty())
                    .then(|| event.formatted_output.clone()),
            }),
            Some(event.turn_id.clone()),
        )),
        EventMsg::DynamicToolCallResponse(event) => Some((
            TurnItem::DynamicToolCall(DynamicToolCallItem {
                id: event.call_id.clone(),
                namespace: event.namespace.clone(),
                tool: event.tool.clone(),
                arguments: event.arguments.clone(),
                status: if event.success {
                    DynamicToolCallStatus::Completed
                } else {
                    DynamicToolCallStatus::Failed
                },
                content_items: Some(event.content_items.clone()),
                success: Some(event.success),
                error: event.error.clone(),
                duration: Some(event.duration),
            }),
            Some(event.turn_id.clone()),
        )),
        _ => None,
    };
    Ok(result)
}
