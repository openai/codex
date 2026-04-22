//! Normalization from Responses-shaped JSON items into conversation item data.

use anyhow::Result;
use anyhow::bail;
use serde_json::Value;

use crate::model::ConversationBody;
use crate::model::ConversationChannel;
use crate::model::ConversationItemKind;
use crate::model::ConversationPart;
use crate::model::ConversationRole;
use crate::model::TokenUsage;
use crate::payload::RawPayloadRef;

/// Conversation fields parsed from one Responses item before trace identity.
///
/// IDs and provenance are assigned after positional reconciliation. Keeping the
/// normalized data separate from `ConversationItem` makes reuse vs insertion a
/// single reducer decision instead of something the parser has to know about.
#[derive(Clone)]
pub(super) struct NormalizedConversationItem {
    pub(super) role: ConversationRole,
    pub(super) channel: Option<ConversationChannel>,
    pub(super) kind: ConversationItemKind,
    pub(super) body: ConversationBody,
    pub(super) call_id: Option<String>,
}

pub(super) fn normalize_model_items(
    items: &[Value],
    raw_payload: &RawPayloadRef,
) -> Result<Vec<NormalizedConversationItem>> {
    let mut normalized_items = Vec::new();
    for item in items {
        normalized_items.push(normalize_model_item(item, raw_payload)?);
    }
    Ok(normalized_items)
}

pub(super) fn token_usage_from_value(value: &Value) -> Option<TokenUsage> {
    Some(TokenUsage {
        input_tokens: u64_field(value, "input_tokens")?,
        cached_input_tokens: u64_field(value, "cached_input_tokens")?,
        output_tokens: u64_field(value, "output_tokens")?,
        reasoning_output_tokens: u64_field(value, "reasoning_output_tokens")?,
    })
}

fn normalize_model_item(
    item: &Value,
    raw_payload: &RawPayloadRef,
) -> Result<NormalizedConversationItem> {
    let Some(item_type) = item.get("type").and_then(Value::as_str) else {
        bail!(
            "model item in payload {} did not contain a string type",
            raw_payload.raw_payload_id
        );
    };
    match item_type {
        "message" => normalize_message_item(item, raw_payload),
        "reasoning" => normalize_reasoning_item(item, raw_payload),
        "function_call" => Ok(NormalizedConversationItem {
            role: ConversationRole::Assistant,
            channel: Some(ConversationChannel::Commentary),
            kind: ConversationItemKind::FunctionCall,
            body: raw_text_or_json_body(item.get("arguments"), raw_payload),
            call_id: item
                .get("call_id")
                .and_then(Value::as_str)
                .map(ToString::to_string),
        }),
        "function_call_output" => Ok(NormalizedConversationItem {
            role: ConversationRole::Tool,
            channel: Some(ConversationChannel::Commentary),
            kind: ConversationItemKind::FunctionCallOutput,
            body: tool_output_body(item.get("output"), raw_payload),
            call_id: item
                .get("call_id")
                .and_then(Value::as_str)
                .map(ToString::to_string),
        }),
        "custom_tool_call" => Ok(NormalizedConversationItem {
            role: ConversationRole::Assistant,
            channel: Some(ConversationChannel::Commentary),
            kind: ConversationItemKind::CustomToolCall,
            body: custom_tool_call_body(item, raw_payload),
            call_id: item
                .get("call_id")
                .and_then(Value::as_str)
                .map(ToString::to_string),
        }),
        "custom_tool_call_output" => Ok(NormalizedConversationItem {
            role: ConversationRole::Tool,
            channel: Some(ConversationChannel::Commentary),
            kind: ConversationItemKind::CustomToolCallOutput,
            body: tool_output_body(item.get("output"), raw_payload),
            call_id: item
                .get("call_id")
                .and_then(Value::as_str)
                .map(ToString::to_string),
        }),
        "tool_search_call" | "web_search_call" | "image_generation_call" | "local_shell_call" => {
            Ok(NormalizedConversationItem {
                role: ConversationRole::Assistant,
                channel: Some(ConversationChannel::Commentary),
                kind: ConversationItemKind::FunctionCall,
                body: json_body(item, raw_payload),
                call_id: item
                    .get("call_id")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
            })
        }
        "tool_search_output" | "mcp_tool_call_output" => Ok(NormalizedConversationItem {
            role: ConversationRole::Tool,
            channel: Some(ConversationChannel::Commentary),
            kind: ConversationItemKind::FunctionCallOutput,
            body: json_body(item, raw_payload),
            call_id: item
                .get("call_id")
                .and_then(Value::as_str)
                .map(ToString::to_string),
        }),
        "compaction" | "compaction_summary" => Ok(NormalizedConversationItem {
            role: ConversationRole::Assistant,
            channel: Some(ConversationChannel::Summary),
            kind: ConversationItemKind::Message,
            body: compaction_body(item, raw_payload)?,
            call_id: None,
        }),
        _ => bail!(
            "unsupported model item type {item_type} in payload {}",
            raw_payload.raw_payload_id
        ),
    }
}

fn normalize_message_item(
    item: &Value,
    raw_payload: &RawPayloadRef,
) -> Result<NormalizedConversationItem> {
    let Some(role) = item.get("role").and_then(Value::as_str) else {
        bail!(
            "message item in payload {} did not contain a string role",
            raw_payload.raw_payload_id
        );
    };
    let Some(role) = role_from_str(role) else {
        bail!(
            "unsupported message role {role} in payload {}",
            raw_payload.raw_payload_id
        );
    };
    Ok(NormalizedConversationItem {
        role,
        channel: item
            .get("phase")
            .and_then(Value::as_str)
            .and_then(channel_from_phase),
        kind: ConversationItemKind::Message,
        body: ConversationBody {
            parts: content_parts(item.get("content"), raw_payload),
        },
        call_id: None,
    })
}

fn normalize_reasoning_item(
    item: &Value,
    raw_payload: &RawPayloadRef,
) -> Result<NormalizedConversationItem> {
    let mut parts = Vec::new();
    append_reasoning_parts(
        item,
        "content",
        ReasoningPartKind::Content,
        raw_payload,
        &mut parts,
    )?;
    append_reasoning_parts(
        item,
        "summary",
        ReasoningPartKind::Summary,
        raw_payload,
        &mut parts,
    )?;

    if let Some(encrypted_content) = item.get("encrypted_content") {
        let encrypted_content = match encrypted_content {
            Value::Null => None,
            Value::String(encrypted_content) => Some(encrypted_content),
            _ => {
                bail!(
                    "reasoning item in payload {} had non-string encrypted_content",
                    raw_payload.raw_payload_id
                );
            }
        };
        if let Some(encrypted_content) = encrypted_content {
            parts.push(ConversationPart::Encoded {
                label: "encrypted_content".to_string(),
                value: encrypted_content.to_string(),
            });
        }
    }

    if parts.is_empty() {
        bail!(
            "reasoning item in payload {} contained no content, summary, or encrypted_content",
            raw_payload.raw_payload_id
        );
    }

    Ok(NormalizedConversationItem {
        role: ConversationRole::Assistant,
        channel: Some(ConversationChannel::Analysis),
        kind: ConversationItemKind::Reasoning,
        body: ConversationBody { parts },
        call_id: None,
    })
}

#[derive(Clone, Copy)]
enum ReasoningPartKind {
    Content,
    Summary,
}

fn append_reasoning_parts(
    item: &Value,
    key: &str,
    kind: ReasoningPartKind,
    raw_payload: &RawPayloadRef,
    parts: &mut Vec<ConversationPart>,
) -> Result<()> {
    let Some(items) = item.get(key) else {
        return Ok(());
    };
    if matches!((kind, items), (ReasoningPartKind::Content, Value::Null)) {
        return Ok(());
    }
    let Some(items) = items.as_array() else {
        bail!(
            "reasoning item in payload {} had non-array {key}",
            raw_payload.raw_payload_id
        );
    };

    for content_item in items {
        let Some(item_type) = content_item.get("type").and_then(Value::as_str) else {
            bail!(
                "reasoning item in payload {} had {key} entry without string type",
                raw_payload.raw_payload_id
            );
        };
        let expected_type = match kind {
            ReasoningPartKind::Content => {
                if !matches!(item_type, "reasoning_text" | "text") {
                    bail!(
                        "reasoning item in payload {} had unsupported content type {item_type}",
                        raw_payload.raw_payload_id
                    );
                }
                "content"
            }
            ReasoningPartKind::Summary => {
                if item_type != "summary_text" {
                    bail!(
                        "reasoning item in payload {} had unsupported summary type {item_type}",
                        raw_payload.raw_payload_id
                    );
                }
                "summary"
            }
        };

        let Some(text) = content_item.get("text").and_then(Value::as_str) else {
            bail!(
                "reasoning item in payload {} had {expected_type} entry without string text",
                raw_payload.raw_payload_id
            );
        };
        match kind {
            ReasoningPartKind::Content => parts.push(ConversationPart::Text {
                text: text.to_string(),
            }),
            ReasoningPartKind::Summary => parts.push(ConversationPart::Summary {
                text: text.to_string(),
            }),
        }
    }

    Ok(())
}

fn role_from_str(role: &str) -> Option<ConversationRole> {
    match role {
        "system" => Some(ConversationRole::System),
        "developer" => Some(ConversationRole::Developer),
        "user" => Some(ConversationRole::User),
        "assistant" => Some(ConversationRole::Assistant),
        "tool" => Some(ConversationRole::Tool),
        _ => None,
    }
}

fn channel_from_phase(phase: &str) -> Option<ConversationChannel> {
    match phase {
        "commentary" => Some(ConversationChannel::Commentary),
        "final_answer" => Some(ConversationChannel::Final),
        "summary" => Some(ConversationChannel::Summary),
        _ => None,
    }
}

fn content_parts(content: Option<&Value>, raw_payload: &RawPayloadRef) -> Vec<ConversationPart> {
    let Some(content) = content.and_then(Value::as_array) else {
        return vec![payload_ref_part("content", raw_payload)];
    };

    let mut parts = Vec::new();
    for part in content {
        match part.get("type").and_then(Value::as_str) {
            Some("input_text" | "output_text" | "text") => {
                if let Some(text) = part.get("text").and_then(Value::as_str) {
                    parts.push(ConversationPart::Text {
                        text: text.to_string(),
                    });
                }
            }
            Some("input_image") => parts.push(payload_ref_part("input_image", raw_payload)),
            Some(other) => parts.push(payload_ref_part(other, raw_payload)),
            None => parts.push(payload_ref_part("content", raw_payload)),
        }
    }

    if parts.is_empty() {
        parts.push(payload_ref_part("empty_content", raw_payload));
    }
    parts
}

fn custom_tool_call_body(item: &Value, raw_payload: &RawPayloadRef) -> ConversationBody {
    let Some(input) = item.get("input").and_then(Value::as_str) else {
        return json_body(item, raw_payload);
    };
    if item.get("name").and_then(Value::as_str) == Some("exec") {
        ConversationBody {
            parts: vec![ConversationPart::Code {
                language: "javascript".to_string(),
                source: input.to_string(),
            }],
        }
    } else {
        ConversationBody {
            parts: vec![ConversationPart::Text {
                text: input.to_string(),
            }],
        }
    }
}

fn raw_text_or_json_body(value: Option<&Value>, raw_payload: &RawPayloadRef) -> ConversationBody {
    match value {
        Some(Value::String(text)) => {
            if let Ok(json) = serde_json::from_str::<Value>(text) {
                json_body(&json, raw_payload)
            } else {
                ConversationBody {
                    parts: vec![ConversationPart::Text { text: text.clone() }],
                }
            }
        }
        Some(value) => json_body(value, raw_payload),
        None => ConversationBody {
            parts: vec![payload_ref_part("payload", raw_payload)],
        },
    }
}

fn tool_output_body(output: Option<&Value>, raw_payload: &RawPayloadRef) -> ConversationBody {
    match output {
        Some(Value::String(text)) => ConversationBody {
            parts: vec![ConversationPart::Text { text: text.clone() }],
        },
        Some(Value::Array(_)) => ConversationBody {
            parts: content_parts(output, raw_payload),
        },
        Some(value) => json_body(value, raw_payload),
        None => ConversationBody {
            parts: vec![payload_ref_part("tool_output", raw_payload)],
        },
    }
}

fn compaction_body(item: &Value, raw_payload: &RawPayloadRef) -> Result<ConversationBody> {
    let Some(encrypted_content) = item.get("encrypted_content").and_then(Value::as_str) else {
        bail!(
            "compaction item in payload {} did not contain string encrypted_content",
            raw_payload.raw_payload_id
        );
    };
    // `type: "compaction"` is the remote-compaction summary that later re-enters model requests.
    // The structural "history was cut here" marker is inserted separately when the checkpoint is
    // installed; payload refs are observation-local, so the encoded summary itself is identity.
    Ok(ConversationBody {
        parts: vec![ConversationPart::Encoded {
            label: "encrypted_content".to_string(),
            value: encrypted_content.to_string(),
        }],
    })
}

fn json_body(value: &Value, raw_payload: &RawPayloadRef) -> ConversationBody {
    ConversationBody {
        parts: vec![ConversationPart::Json {
            summary: summarize_json(value),
            raw_payload_id: raw_payload.raw_payload_id.clone(),
        }],
    }
}

fn payload_ref_part(label: &str, raw_payload: &RawPayloadRef) -> ConversationPart {
    ConversationPart::PayloadRef {
        label: label.to_string(),
        raw_payload_id: raw_payload.raw_payload_id.clone(),
    }
}

fn summarize_json(value: &Value) -> String {
    const MAX_JSON_SUMMARY_LEN: usize = 240;
    let mut summary =
        serde_json::to_string(value).unwrap_or_else(|_| "<unserializable json>".to_string());
    if summary.len() > MAX_JSON_SUMMARY_LEN {
        summary.truncate(MAX_JSON_SUMMARY_LEN);
        summary.push_str("...");
    }
    summary
}

fn u64_field(value: &Value, field: &str) -> Option<u64> {
    value
        .get(field)
        .and_then(Value::as_i64)
        .map(|value| value.max(0) as u64)
}
