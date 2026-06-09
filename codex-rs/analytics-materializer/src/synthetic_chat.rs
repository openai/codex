use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde_json::Value as JsonValue;
use serde_json::json;
use std::collections::HashMap;

pub(super) const DEFAULT_RECIPIENT: &str = "all";

pub(super) struct SyntheticMessage {
    pub(super) role: String,
    pub(super) author_name: Option<String>,
    pub(super) content: JsonValue,
    pub(super) recipient: String,
    pub(super) channel: Option<String>,
    pub(super) end_turn: Option<bool>,
}

pub(super) fn push_synthetic_message(
    messages: &mut Vec<JsonValue>,
    conversation_seed: &str,
    message: SyntheticMessage,
) {
    let message_seed = format!("{conversation_seed}:message:{}", messages.len());
    let weight = if message.role == "user" { 0.0 } else { 1.0 };
    messages.push(json!({
        "id": stable_uuid(&message_seed),
        "author": {
            "role": message.role,
            "name": message.author_name,
            "metadata": {}
        },
        "create_time": null,
        "update_time": null,
        "content": message.content,
        "status": "finished_successfully",
        "end_turn": message.end_turn,
        "weight": weight,
        "metadata": {},
        "recipient": message.recipient,
        "channel": message.channel
    }));
}

pub(super) fn developer_tools_content(tools: &[JsonValue]) -> JsonValue {
    json!({
        "content_type": "developer_content",
        "instructions": [""],
        "settings": null,
        "function_namespaces": [{
            "name": "functions",
            "description": "",
            "functions": tools
        }],
        "response_formats": []
    })
}

pub(super) fn text_content(text: String) -> JsonValue {
    json!({
        "content_type": "text",
        "parts": [text]
    })
}

pub(super) fn render_content(value: &JsonValue) -> Result<String> {
    match value {
        JsonValue::Array(parts) => parts
            .iter()
            .map(render_content_part)
            .collect::<Result<Vec<_>>>()
            .map(|parts| {
                parts
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n")
            }),
        JsonValue::String(text) => Ok(text.clone()),
        JsonValue::Null => Ok(String::new()),
        _ => render_content_part(value),
    }
}

pub(super) fn message_channel(role: &str, phase: Option<&str>) -> Option<&'static str> {
    if role != "assistant" {
        return None;
    }
    match phase {
        Some("final_answer") => Some("final"),
        Some("commentary") | None => Some("commentary"),
        Some(_) => Some("commentary"),
    }
}

pub(super) fn message_end_turn(role: &str, phase: Option<&str>) -> Option<bool> {
    (role == "assistant" && phase == Some("final_answer")).then_some(true)
}

pub(super) fn tool_recipient(namespace: Option<&str>, name: &str) -> String {
    format!("{}.{}", namespace.unwrap_or("functions"), name)
}

pub(super) fn record_call_recipient(
    call_recipients: &mut HashMap<String, String>,
    call_id: &str,
    recipient: &str,
) -> Result<()> {
    if call_recipients
        .insert(call_id.to_string(), recipient.to_string())
        .is_some()
    {
        bail!("duplicate Responses tool call_id {call_id}");
    }
    Ok(())
}

pub(super) fn required_value<'a>(value: &'a JsonValue, field: &str) -> Result<&'a JsonValue> {
    value
        .get(field)
        .with_context(|| format!("Responses item has no {field} field"))
}

pub(super) fn required_str<'a>(value: &'a JsonValue, field: &str) -> Result<&'a str> {
    required_value(value, field)?
        .as_str()
        .with_context(|| format!("Responses item field {field} is not a string"))
}

pub(super) fn compact_json(value: &JsonValue) -> Result<String> {
    Ok(serde_json::to_string(value)?)
}

pub(super) fn stable_uuid(seed: &str) -> String {
    let mut value =
        (u128::from(fnv1a_64(seed)) << 64) | u128::from(fnv1a_64(&format!("{seed}:uuid")));
    value &= !(0xf_u128 << 76);
    value |= 0x5_u128 << 76;
    value &= !(0x3_u128 << 62);
    value |= 0x2_u128 << 62;
    let value = format!("{value:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &value[..8],
        &value[8..12],
        &value[12..16],
        &value[16..20],
        &value[20..]
    )
}

fn render_content_part(value: &JsonValue) -> Result<String> {
    match value.get("type").and_then(JsonValue::as_str) {
        Some("input_text" | "output_text" | "summary_text" | "reasoning_text" | "text") => {
            Ok(required_str(value, "text")?.to_string())
        }
        Some("encrypted_content") => Ok(required_str(value, "encrypted_content")?.to_string()),
        _ => compact_json(value),
    }
}

fn fnv1a_64(value: &str) -> u64 {
    value.bytes().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}
