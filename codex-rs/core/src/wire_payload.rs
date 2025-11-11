use codex_protocol::ConversationId;
use codex_protocol::models::ResponseItem;
use serde_json::Value;
use serde_json::json;

use crate::client_common::Prompt;
use crate::tools::spec::create_tools_json_for_responses_api;

pub fn build_responses_payload(
    prompt: &Prompt,
    model: &str,
    conversation_id: ConversationId,
    azure_workaround: bool,
    reasoning: Option<codex_api_client::Reasoning>,
    text_controls: Option<codex_api_client::TextControls>,
    instructions: String,
) -> Value {
    let tools =
        create_tools_json_for_responses_api(&prompt.tools).unwrap_or_else(|_| Vec::<Value>::new());

    let mut payload = json!({
        "model": model,
        "instructions": instructions,
        "input": prompt.get_formatted_input(),
        "tools": tools,
        "tool_choice": "auto",
        "parallel_tool_calls": prompt.parallel_tool_calls,
        "store": azure_workaround,
        "stream": true,
        "prompt_cache_key": conversation_id.to_string(),
    });

    if let Some(reasoning) = reasoning
        && let Some(obj) = payload.as_object_mut()
    {
        obj.insert(
            "reasoning".to_string(),
            serde_json::to_value(reasoning).unwrap_or(Value::Null),
        );
    }

    if let Some(text) = text_controls
        && let Some(obj) = payload.as_object_mut()
    {
        obj.insert(
            "text".to_string(),
            serde_json::to_value(text).unwrap_or(Value::Null),
        );
    }

    let include = if prompt
        .get_formatted_input()
        .iter()
        .any(|it| matches!(it, ResponseItem::Reasoning { .. }))
    {
        vec!["reasoning.encrypted_content".to_string()]
    } else {
        Vec::new()
    };
    if let Some(obj) = payload.as_object_mut() {
        obj.insert(
            "include".to_string(),
            Value::Array(include.into_iter().map(Value::String).collect()),
        );
    }

    // Azure Responses requires ids attached to input items
    if azure_workaround
        && let Some(input_value) = payload.get_mut("input")
        && let Some(array) = input_value.as_array_mut()
    {
        attach_item_ids_array(array, &prompt.get_formatted_input());
    }

    payload
}

fn attach_item_ids_array(json_array: &mut [Value], prompt_input: &[ResponseItem]) {
    for (json_item, item) in json_array.iter_mut().zip(prompt_input.iter()) {
        let Some(obj) = json_item.as_object_mut() else {
            continue;
        };
        let mut set_id_if_absent = |id: &str| match obj.get("id") {
            Some(Value::String(s)) if !s.is_empty() => {}
            Some(Value::Null) | None => {
                obj.insert("id".to_string(), Value::String(id.to_string()));
            }
            _ => {}
        };
        match item {
            ResponseItem::Reasoning { id, .. } => set_id_if_absent(id),
            ResponseItem::Message { id, .. } => {
                if let Some(id) = id.as_ref() {
                    set_id_if_absent(id);
                }
            }
            ResponseItem::WebSearchCall { id, .. }
            | ResponseItem::FunctionCall { id, .. }
            | ResponseItem::LocalShellCall { id, .. }
            | ResponseItem::CustomToolCall { id, .. } => {
                if let Some(id) = id.as_ref() {
                    set_id_if_absent(id);
                }
            }
            _ => {}
        }
    }
}

pub fn build_chat_payload(prompt: &Prompt, model: &str, instructions: String) -> Value {
    use crate::tools::spec::create_tools_json_for_chat_completions_api;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::FunctionCallOutputContentItem;
    use codex_protocol::models::ReasoningItemContent;
    use std::collections::HashMap;

    let mut messages = Vec::<Value>::new();
    messages.push(json!({ "role": "system", "content": instructions }));

    let mut reasoning_by_anchor_index: HashMap<usize, String> = HashMap::new();

    let mut last_emitted_role: Option<&str> = None;
    for item in &prompt.input {
        match item {
            ResponseItem::Message { role, .. } => last_emitted_role = Some(role.as_str()),
            ResponseItem::FunctionCall { .. } | ResponseItem::LocalShellCall { .. } => {
                last_emitted_role = Some("assistant");
            }
            ResponseItem::FunctionCallOutput { .. } => last_emitted_role = Some("tool"),
            ResponseItem::Reasoning { .. }
            | ResponseItem::Other
            | ResponseItem::CustomToolCall { .. }
            | ResponseItem::CustomToolCallOutput { .. }
            | ResponseItem::WebSearchCall { .. }
            | ResponseItem::GhostSnapshot { .. } => {}
        }
    }

    let mut last_user_index: Option<usize> = None;
    for (idx, item) in prompt.input.iter().enumerate() {
        if let ResponseItem::Message { role, .. } = item
            && role == "user"
        {
            last_user_index = Some(idx);
        }
    }

    if !matches!(last_emitted_role, Some("user")) {
        for (idx, item) in prompt.input.iter().enumerate() {
            if let Some(u_idx) = last_user_index
                && idx <= u_idx
            {
                continue;
            }

            if let ResponseItem::Reasoning {
                content: Some(items),
                ..
            } = item
            {
                let mut text = String::new();
                for entry in items {
                    match entry {
                        ReasoningItemContent::ReasoningText { text: segment }
                        | ReasoningItemContent::Text { text: segment } => {
                            text.push_str(segment);
                        }
                    }
                }
                if text.trim().is_empty() {
                    continue;
                }

                let mut attached = false;
                if idx > 0
                    && let ResponseItem::Message { role, .. } = &prompt.input[idx - 1]
                    && role == "assistant"
                {
                    reasoning_by_anchor_index
                        .entry(idx - 1)
                        .and_modify(|val| val.push_str(&text))
                        .or_insert(text.clone());
                    attached = true;
                }

                if !attached && idx + 1 < prompt.input.len() {
                    match &prompt.input[idx + 1] {
                        ResponseItem::FunctionCall { .. } | ResponseItem::LocalShellCall { .. } => {
                            reasoning_by_anchor_index
                                .entry(idx + 1)
                                .and_modify(|val| val.push_str(&text))
                                .or_insert(text.clone());
                        }
                        ResponseItem::Message { role, .. } if role == "assistant" => {
                            reasoning_by_anchor_index
                                .entry(idx + 1)
                                .and_modify(|val| val.push_str(&text))
                                .or_insert(text.clone());
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    let mut last_assistant_text: Option<String> = None;
    for (idx, item) in prompt.input.iter().enumerate() {
        match item {
            ResponseItem::Message { role, content, .. } => {
                let mut text = String::new();
                let mut items: Vec<Value> = Vec::new();
                let mut saw_image = false;

                for c in content {
                    match c {
                        ContentItem::InputText { text: t }
                        | ContentItem::OutputText { text: t } => {
                            text.push_str(t);
                            items.push(json!({"type":"text","text": t}));
                        }
                        ContentItem::InputImage { image_url } => {
                            saw_image = true;
                            items.push(json!({"type":"image_url","image_url": {"url": image_url}}));
                        }
                    }
                }

                if role == "assistant" {
                    if let Some(prev) = &last_assistant_text
                        && prev == &text
                    {
                        continue;
                    }
                    last_assistant_text = Some(text.clone());
                }

                let content_value = if role == "assistant" {
                    json!(text)
                } else if saw_image {
                    json!(items)
                } else {
                    json!(text)
                };

                let mut message = json!({ "role": role, "content": content_value });
                if let Some(reasoning) = reasoning_by_anchor_index.get(&idx)
                    && let Some(obj) = message.as_object_mut()
                {
                    obj.insert("reasoning".to_string(), json!({"text": reasoning}));
                }
                messages.push(message);
            }
            ResponseItem::FunctionCall {
                name,
                arguments,
                call_id,
                ..
            } => {
                messages.push(json!({
                    "role": "assistant",
                    "tool_calls": [{
                        "id": call_id,
                        "type": "function",
                        "function": { "name": name, "arguments": arguments },
                    }],
                }));
            }
            ResponseItem::FunctionCallOutput { call_id, output } => {
                let content_value = if let Some(items) = &output.content_items {
                    let mapped: Vec<Value> = items
                        .iter()
                        .map(|item| match item {
                            FunctionCallOutputContentItem::InputText { text } => {
                                json!({"type":"text","text": text})
                            }
                            FunctionCallOutputContentItem::InputImage { image_url } => {
                                json!({"type":"image_url","image_url": {"url": image_url}})
                            }
                        })
                        .collect();
                    json!(mapped)
                } else {
                    json!(output.content)
                };
                messages.push(
                    json!({ "role": "tool", "tool_call_id": call_id, "content": content_value }),
                );
            }
            ResponseItem::LocalShellCall {
                id,
                call_id,
                action,
                ..
            } => {
                let tool_id = call_id
                    .clone()
                    .filter(|value| !value.is_empty())
                    .or_else(|| id.clone())
                    .unwrap_or_default();
                messages.push(json!({
                    "role": "assistant",
                    "tool_calls": [{
                        "id": tool_id,
                        "type": "function",
                        "function": {
                            "name": "shell",
                            "arguments": serde_json::to_string(action).unwrap_or_default(),
                        },
                    }],
                }));
            }
            ResponseItem::CustomToolCall {
                call_id,
                name,
                input,
                ..
            } => {
                messages.push(json!({
                    "role": "assistant",
                    "tool_calls": [{
                        "id": call_id.clone(),
                        "type": "function",
                        "function": { "name": name, "arguments": input },
                    }],
                }));
            }
            ResponseItem::CustomToolCallOutput { call_id, output } => {
                messages
                    .push(json!({ "role": "tool", "tool_call_id": call_id, "content": output }));
            }
            ResponseItem::WebSearchCall { .. }
            | ResponseItem::Reasoning { .. }
            | ResponseItem::Other
            | ResponseItem::GhostSnapshot { .. } => {}
        }
    }

    let tools_json = create_tools_json_for_chat_completions_api(&prompt.tools)
        .unwrap_or_else(|_| Vec::<Value>::new());
    json!({ "model": model, "messages": messages, "stream": true, "tools": tools_json })
}
