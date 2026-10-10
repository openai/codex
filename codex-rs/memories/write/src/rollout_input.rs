//! Selects v2 extraction evidence by provenance, then restores chronological order.
//! Human replies retain their questions; message media is replaced with placeholders.
//! Budgets use the same token estimate as the rest of the memory pipeline.
//! Extraction chunks preserve selected evidence in order within bounded messages.

use codex_core::context::ContextualUserFragment;
use codex_core::context::MemoryContextFragment;
use codex_protocol::ToolName;
use codex_protocol::models::ContentItem;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::InterAgentCommunication;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_rollout::RolloutItem;
use codex_rollout::should_persist_response_item_for_memories;
use codex_secrets::redact_secrets;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::approx_bytes_for_tokens;
use codex_utils_output_truncation::truncate_text;
use std::collections::HashMap;

const OMITTED: &str = "[... response items omitted ...]\n";
const TRUNCATION_RESERVE_BYTES: usize = 96;
const TOOL_OUTPUT_TOKENS: usize = 2_000;
const MAX_ROW_BYTES: usize = 10_000;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Tier {
    Human,
    Final,
    OtherAgent,
    Commentary,
    Context,
    Tool,
}

/// Select newest evidence first within each tier, but render in source order.
pub(crate) fn serialize_tiered_input(
    items: &[RolloutItem],
    token_limit: usize,
) -> anyhow::Result<String> {
    let mut rows = Vec::new();
    let mut user_input_calls = HashMap::new();
    for item in items {
        let item = match item {
            RolloutItem::ResponseItem(item) => sanitize_response_item_for_memories(&item.item),
            RolloutItem::InterAgentCommunication(message) => Some(message.to_model_input_item()),
            RolloutItem::SessionMeta(_)
            | RolloutItem::InterAgentCommunicationMetadata { .. }
            | RolloutItem::Compacted(_)
            | RolloutItem::TurnContext(_)
            | RolloutItem::RealtimeItem(_)
            | RolloutItem::WorldState(_)
            | RolloutItem::SecurityRiskScore(_)
            | RolloutItem::TokenUsageRecord(_)
            | RolloutItem::RetainedContext(_)
            | RolloutItem::EventMsg(_) => None,
        };
        let Some(mut item) = item else { continue };
        if let ResponseItem::Message { content, .. } = &mut item {
            for part in content {
                match part {
                    ContentItem::InputImage { .. } => {
                        *part = ContentItem::InputText {
                            text: "[image omitted]".into(),
                        };
                    }
                    ContentItem::InputAudio { .. } => {
                        *part = ContentItem::InputText {
                            text: "[audio omitted]".into(),
                        };
                    }
                    ContentItem::InputText { .. } | ContentItem::OutputText { .. } => {}
                }
            }
        }
        let question = match &item {
            ResponseItem::FunctionCall {
                name,
                namespace,
                call_id,
                arguments,
                ..
            } if name == "request_user_input"
                && ToolName::new(namespace.clone(), name).is_default_namespace() =>
            {
                user_input_calls.insert(call_id.clone(), arguments.clone());
                None
            }
            ResponseItem::FunctionCallOutput {
                call_id: Some(call_id),
                output,
                ..
            } if user_input_calls.contains_key(call_id)
                && output
                    .text_content()
                    .and_then(|text| serde_json::from_str::<RequestUserInputResponse>(text).ok())
                    .is_some_and(|reply| {
                        reply
                            .answers
                            .values()
                            .any(|answer| answer.answers.iter().any(|text| !text.trim().is_empty()))
                    }) =>
            {
                user_input_calls.remove(call_id)
            }
            _ => None,
        };
        let tier = match &item {
            ResponseItem::FunctionCallOutput { .. } if question.is_some() => Tier::Human,
            ResponseItem::AgentMessage { .. } => Tier::OtherAgent,
            ResponseItem::Message {
                role,
                content,
                phase,
                internal_chat_message_metadata_passthrough,
                ..
            } => {
                let agent_content = internal_chat_message_metadata_passthrough
                    .as_ref()
                    .and_then(|metadata| metadata.content_item_kinds.as_ref())
                    .is_some_and(|kinds| {
                        kinds.iter().any(|kind| kind.0.starts_with("multi_agent."))
                    })
                    || InterAgentCommunication::is_message_content(content)
                    || content.iter().any(|part| match part {
                        ContentItem::InputText { text } | ContentItem::OutputText { text, .. } => {
                            let text = text.trim_start();
                            text.starts_with("<subagent_notification>")
                                || (text.starts_with("Message Type:")
                                    && text
                                        .lines()
                                        .nth(1)
                                        .is_some_and(|line| line.starts_with("Task name:"))
                                    && text
                                        .lines()
                                        .nth(2)
                                        .is_some_and(|line| line.starts_with("Sender:"))
                                    && text.lines().nth(3) == Some("Payload:"))
                        }
                        _ => false,
                    });
                if agent_content {
                    Tier::OtherAgent
                } else if role == "user" {
                    let contextual = internal_chat_message_metadata_passthrough.as_ref()
                        .and_then(|metadata| metadata.content_item_kinds.as_ref())
                        .is_some_and(|kinds| !kinds.is_empty() && kinds.iter().all(|kind| !kind.0.starts_with("user.")))
                        || content.iter().any(|part| matches!(part, ContentItem::InputText { text }
                            if matches_marked_fragment(text, "<environment_context>", "</environment_context>")));
                    if contextual {
                        Tier::Context
                    } else {
                        Tier::Human
                    }
                } else if matches!(phase, Some(MessagePhase::Commentary)) {
                    Tier::Commentary
                } else {
                    Tier::Final
                }
            }
            _ => Tier::Tool,
        };
        let label = match tier {
            Tier::Human => "human user",
            Tier::Final => "assistant final",
            Tier::Commentary => "assistant commentary",
            Tier::OtherAgent => "other agent",
            Tier::Context => "harness context",
            Tier::Tool => "tool",
        };
        let text = serde_json::to_string(&item)?;
        let mut text = redact_secrets(match question {
            Some(question) => format!("Assistant question: {question}\nHuman reply: {text}"),
            None => text,
        });
        if tier == Tier::Tool
            && matches!(
                item,
                ResponseItem::FunctionCallOutput { .. }
                    | ResponseItem::CustomToolCallOutput { .. }
                    | ResponseItem::ToolSearchOutput { .. }
            )
        {
            text = truncate_text(
                &text,
                TruncationPolicy::Bytes(
                    approx_bytes_for_tokens(TOOL_OUTPUT_TOKENS)
                        .saturating_sub(TRUNCATION_RESERVE_BYTES),
                ),
            );
        }
        text = truncate_text(
            &text,
            TruncationPolicy::Bytes(MAX_ROW_BYTES - TRUNCATION_RESERVE_BYTES),
        );
        rows.push((tier, format!("[{label}]\n{text}\n")));
    }
    let mut remaining = approx_bytes_for_tokens(token_limit).saturating_sub(OMITTED.len());
    let mut selected = vec![None; rows.len()];
    for tier in [
        Tier::Human,
        Tier::Final,
        Tier::OtherAgent,
        Tier::Commentary,
        Tier::Context,
        Tier::Tool,
    ] {
        for (index, (row_tier, text)) in rows.iter().enumerate().rev() {
            if *row_tier != tier || remaining <= OMITTED.len() + TRUNCATION_RESERVE_BYTES {
                continue;
            }
            let budget = remaining - OMITTED.len();
            let text = if text.len() <= budget {
                text.clone()
            } else {
                truncate_text(
                    text,
                    TruncationPolicy::Bytes(budget - TRUNCATION_RESERVE_BYTES),
                )
            };
            remaining = remaining.saturating_sub(text.len() + OMITTED.len());
            selected[index] = Some(text);
        }
    }
    let mut rendered = String::new();
    let mut gap = false;
    for row in selected {
        match row {
            Some(text) => {
                rendered.push_str(&text);
                gap = false;
            }
            None if !gap => {
                rendered.push_str(OMITTED);
                gap = true;
            }
            None => {}
        }
    }
    if rendered.len() > approx_bytes_for_tokens(token_limit) {
        return Ok(String::new());
    }
    Ok(rendered)
}

/// Splits already-budgeted extraction input without dropping evidence.
pub(crate) fn extraction_messages(mut text: &str) -> Vec<ResponseItem> {
    let mut messages = Vec::new();
    while !text.is_empty() {
        let end = text.floor_char_boundary(text.len().min(8_900));
        messages.push(ContextualUserFragment::into(
            MemoryContextFragment::ExtractionEvidence(text[..end].to_string()),
        ));
        text = &text[end..];
    }
    messages
}

pub(crate) fn sanitize_response_item_for_memories(item: &ResponseItem) -> Option<ResponseItem> {
    if !should_persist_response_item_for_memories(item) {
        return None;
    }

    let mut item = item.clone();
    item.clear_encrypted_content();
    if let ResponseItem::Message {
        role,
        content,
        status,
        ..
    } = &mut item
        && role == "user"
    {
        *status = None;
        content.retain(|content_item| !is_memory_excluded_contextual_user_fragment(content_item));
        if content.is_empty() {
            return None;
        }
    }

    Some(item)
}

fn is_memory_excluded_contextual_user_fragment(content_item: &ContentItem) -> bool {
    let ContentItem::InputText { text } = content_item else {
        return false;
    };

    matches_marked_fragment(text, "# AGENTS.md instructions", "</INSTRUCTIONS>")
        || matches_marked_fragment(text, "<skill>", "</skill>")
}

fn matches_marked_fragment(text: &str, start_marker: &str, end_marker: &str) -> bool {
    let trimmed = text.trim_start();
    let starts_with_marker = trimmed
        .get(..start_marker.len())
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(start_marker));
    let trimmed = trimmed.trim_end();
    let ends_with_marker = trimmed
        .get(trimmed.len().saturating_sub(end_marker.len())..)
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(end_marker));
    starts_with_marker && ends_with_marker
}

#[cfg(test)]
#[path = "rollout_input_tests.rs"]
mod tests;
