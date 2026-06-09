use crate::synthetic_chat::DEFAULT_RECIPIENT;
use crate::synthetic_chat::SyntheticMessage;
use crate::synthetic_chat::compact_json;
use crate::synthetic_chat::developer_tools_content;
use crate::synthetic_chat::message_channel;
use crate::synthetic_chat::message_end_turn;
use crate::synthetic_chat::push_synthetic_message;
use crate::synthetic_chat::record_call_recipient;
use crate::synthetic_chat::render_content;
use crate::synthetic_chat::required_str;
use crate::synthetic_chat::required_value;
use crate::synthetic_chat::stable_uuid;
use crate::synthetic_chat::text_content;
use crate::synthetic_chat::tool_recipient;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde_json::Value as JsonValue;
use serde_json::json;
use std::collections::HashMap;

pub(super) const SYNTHETIC_CONVERSATION_SOURCE: &str = "synthetic_local_responses_request";

pub(super) fn synthetic_conversation(
    session_id: &str,
    thread_id: &str,
    context_window_id: &str,
    request_json: &JsonValue,
    full_input: &[JsonValue],
) -> Result<JsonValue> {
    let conversation_seed = format!("{session_id}:{thread_id}:{context_window_id}");
    let mut messages = Vec::new();
    let mut call_recipients = HashMap::new();

    if let Some(tools) = request_json
        .get("tools")
        .and_then(JsonValue::as_array)
        .filter(|tools| !tools.is_empty())
    {
        push_synthetic_message(
            &mut messages,
            &conversation_seed,
            SyntheticMessage {
                role: "developer".to_string(),
                author_name: None,
                content: developer_tools_content(tools),
                recipient: DEFAULT_RECIPIENT.to_string(),
                channel: None,
                end_turn: None,
            },
        );
    }
    if let Some(instructions) = request_json
        .get("instructions")
        .and_then(JsonValue::as_str)
        .filter(|instructions| !instructions.is_empty())
    {
        push_synthetic_message(
            &mut messages,
            &conversation_seed,
            SyntheticMessage {
                role: "developer".to_string(),
                author_name: None,
                content: text_content(instructions.to_string()),
                recipient: DEFAULT_RECIPIENT.to_string(),
                channel: None,
                end_turn: None,
            },
        );
    }
    for item in full_input {
        append_response_item_messages(
            &mut messages,
            &mut call_recipients,
            &conversation_seed,
            item,
        )?;
    }

    Ok(json!({
        "id": stable_uuid(&conversation_seed),
        "messages": messages,
        "create_time": null,
        "update_time": null,
        "metadata": {
            "local_analytics": {
                "conversation_source": SYNTHETIC_CONVERSATION_SOURCE,
                "context_window_id": context_window_id,
                "is_synthetic": true
            }
        }
    }))
}

fn append_response_item_messages(
    messages: &mut Vec<JsonValue>,
    call_recipients: &mut HashMap<String, String>,
    conversation_seed: &str,
    item: &JsonValue,
) -> Result<()> {
    let item_type = required_str(item, "type")?;
    match item_type {
        "message" => append_message_item(messages, conversation_seed, item),
        "agent_message" => append_agent_message(messages, conversation_seed, item),
        "reasoning" => append_reasoning_item(messages, conversation_seed, item),
        "function_call" => append_function_call(messages, call_recipients, conversation_seed, item),
        "function_call_output" => {
            append_tool_output(messages, call_recipients, conversation_seed, item, "output")
        }
        "custom_tool_call" => {
            append_custom_tool_call(messages, call_recipients, conversation_seed, item)
        }
        "custom_tool_call_output" => {
            append_tool_output(messages, call_recipients, conversation_seed, item, "output")
        }
        "tool_search_call" => {
            append_tool_search_call(messages, call_recipients, conversation_seed, item)
        }
        "tool_search_output" => append_tool_search_output(messages, conversation_seed, item),
        "web_search_call" => append_web_search_call(messages, conversation_seed, item),
        "local_shell_call" => {
            append_raw_assistant_tool_call(messages, conversation_seed, item, "container.exec")
        }
        "image_generation_call" => {
            append_raw_assistant_tool_call(messages, conversation_seed, item, "image_generation")
        }
        "compaction" => append_compaction(messages, conversation_seed, item),
        "compaction_trigger" => append_compaction_trigger(messages, conversation_seed),
        "context_compaction" => append_context_compaction(messages, conversation_seed, item),
        _ => bail!("unsupported Responses input item type {item_type}"),
    }
}

fn append_message_item(
    messages: &mut Vec<JsonValue>,
    conversation_seed: &str,
    item: &JsonValue,
) -> Result<()> {
    let role = required_str(item, "role")?;
    let phase = item.get("phase").and_then(JsonValue::as_str);
    push_synthetic_message(
        messages,
        conversation_seed,
        SyntheticMessage {
            role: role.to_string(),
            author_name: None,
            content: text_content(render_content(required_value(item, "content")?)?),
            recipient: DEFAULT_RECIPIENT.to_string(),
            channel: message_channel(role, phase).map(str::to_string),
            end_turn: message_end_turn(role, phase),
        },
    );
    Ok(())
}

fn append_agent_message(
    messages: &mut Vec<JsonValue>,
    conversation_seed: &str,
    item: &JsonValue,
) -> Result<()> {
    push_synthetic_message(
        messages,
        conversation_seed,
        SyntheticMessage {
            role: "assistant".to_string(),
            author_name: Some(required_str(item, "author")?.to_string()),
            content: text_content(render_content(required_value(item, "content")?)?),
            recipient: required_str(item, "recipient")?.to_string(),
            channel: Some("commentary".to_string()),
            end_turn: None,
        },
    );
    Ok(())
}

fn append_reasoning_item(
    messages: &mut Vec<JsonValue>,
    conversation_seed: &str,
    item: &JsonValue,
) -> Result<()> {
    let summary = render_content(required_value(item, "summary")?)?;
    let content = if summary.is_empty() {
        item.get("encrypted_content")
            .and_then(JsonValue::as_str)
            .unwrap_or_default()
            .to_string()
    } else {
        summary
    };
    push_synthetic_message(
        messages,
        conversation_seed,
        SyntheticMessage {
            role: "assistant".to_string(),
            author_name: None,
            content: text_content(content),
            recipient: DEFAULT_RECIPIENT.to_string(),
            channel: Some("analysis".to_string()),
            end_turn: None,
        },
    );
    Ok(())
}

fn append_function_call(
    messages: &mut Vec<JsonValue>,
    call_recipients: &mut HashMap<String, String>,
    conversation_seed: &str,
    item: &JsonValue,
) -> Result<()> {
    let recipient = tool_recipient(
        item.get("namespace").and_then(JsonValue::as_str),
        required_str(item, "name")?,
    );
    record_call_recipient(call_recipients, required_str(item, "call_id")?, &recipient)?;
    append_assistant_tool_call(
        messages,
        conversation_seed,
        recipient,
        required_str(item, "arguments")?.to_string(),
    );
    Ok(())
}

fn append_custom_tool_call(
    messages: &mut Vec<JsonValue>,
    call_recipients: &mut HashMap<String, String>,
    conversation_seed: &str,
    item: &JsonValue,
) -> Result<()> {
    let recipient = tool_recipient(None, required_str(item, "name")?);
    record_call_recipient(call_recipients, required_str(item, "call_id")?, &recipient)?;
    append_assistant_tool_call(
        messages,
        conversation_seed,
        recipient,
        required_str(item, "input")?.to_string(),
    );
    Ok(())
}

fn append_tool_output(
    messages: &mut Vec<JsonValue>,
    call_recipients: &HashMap<String, String>,
    conversation_seed: &str,
    item: &JsonValue,
    output_field: &str,
) -> Result<()> {
    let call_id = required_str(item, "call_id")?;
    let recipient = call_recipients
        .get(call_id)
        .cloned()
        .or_else(|| {
            item.get("name")
                .and_then(JsonValue::as_str)
                .map(|name| tool_recipient(None, name))
        })
        .unwrap_or_else(|| tool_recipient(None, call_id));
    push_synthetic_message(
        messages,
        conversation_seed,
        SyntheticMessage {
            role: "tool".to_string(),
            author_name: Some(recipient),
            content: text_content(render_content(required_value(item, output_field)?)?),
            recipient: DEFAULT_RECIPIENT.to_string(),
            channel: Some("commentary".to_string()),
            end_turn: None,
        },
    );
    Ok(())
}

fn append_tool_search_call(
    messages: &mut Vec<JsonValue>,
    call_recipients: &mut HashMap<String, String>,
    conversation_seed: &str,
    item: &JsonValue,
) -> Result<()> {
    let recipient = "tool_search.tool_search_tool".to_string();
    if let Some(call_id) = item.get("call_id").and_then(JsonValue::as_str) {
        record_call_recipient(call_recipients, call_id, &recipient)?;
    }
    append_assistant_tool_call(
        messages,
        conversation_seed,
        recipient,
        compact_json(required_value(item, "arguments")?)?,
    );
    Ok(())
}

fn append_tool_search_output(
    messages: &mut Vec<JsonValue>,
    conversation_seed: &str,
    item: &JsonValue,
) -> Result<()> {
    let tools = item
        .get("tools")
        .and_then(JsonValue::as_array)
        .context("tool_search_output has no tools array")?;
    push_synthetic_message(
        messages,
        conversation_seed,
        SyntheticMessage {
            role: "developer".to_string(),
            author_name: None,
            content: developer_tools_content(tools),
            recipient: DEFAULT_RECIPIENT.to_string(),
            channel: None,
            end_turn: None,
        },
    );
    Ok(())
}

fn append_web_search_call(
    messages: &mut Vec<JsonValue>,
    conversation_seed: &str,
    item: &JsonValue,
) -> Result<()> {
    let content = item
        .get("action")
        .map(compact_json)
        .transpose()?
        .unwrap_or_default();
    append_assistant_tool_call(messages, conversation_seed, "web".to_string(), content);
    Ok(())
}

fn append_raw_assistant_tool_call(
    messages: &mut Vec<JsonValue>,
    conversation_seed: &str,
    item: &JsonValue,
    recipient: &str,
) -> Result<()> {
    append_assistant_tool_call(
        messages,
        conversation_seed,
        recipient.to_string(),
        compact_json(item)?,
    );
    Ok(())
}

fn append_compaction(
    messages: &mut Vec<JsonValue>,
    conversation_seed: &str,
    item: &JsonValue,
) -> Result<()> {
    append_summary_message(
        messages,
        conversation_seed,
        "assistant",
        required_str(item, "encrypted_content")?.to_string(),
    );
    Ok(())
}

fn append_compaction_trigger(messages: &mut Vec<JsonValue>, conversation_seed: &str) -> Result<()> {
    append_summary_message(
        messages,
        conversation_seed,
        "system",
        "Context compaction triggered. Summarize the current context.".to_string(),
    );
    Ok(())
}

fn append_context_compaction(
    messages: &mut Vec<JsonValue>,
    conversation_seed: &str,
    item: &JsonValue,
) -> Result<()> {
    append_summary_message(
        messages,
        conversation_seed,
        "assistant",
        item.get("encrypted_content")
            .and_then(JsonValue::as_str)
            .unwrap_or_default()
            .to_string(),
    );
    Ok(())
}

fn append_summary_message(
    messages: &mut Vec<JsonValue>,
    conversation_seed: &str,
    role: &str,
    content: String,
) {
    push_synthetic_message(
        messages,
        conversation_seed,
        SyntheticMessage {
            role: role.to_string(),
            author_name: None,
            content: text_content(content),
            recipient: DEFAULT_RECIPIENT.to_string(),
            channel: Some("summary".to_string()),
            end_turn: None,
        },
    );
}

fn append_assistant_tool_call(
    messages: &mut Vec<JsonValue>,
    conversation_seed: &str,
    recipient: String,
    content: String,
) {
    push_synthetic_message(
        messages,
        conversation_seed,
        SyntheticMessage {
            role: "assistant".to_string(),
            author_name: None,
            content: text_content(content),
            recipient,
            channel: Some("commentary".to_string()),
            end_turn: Some(false),
        },
    );
}
