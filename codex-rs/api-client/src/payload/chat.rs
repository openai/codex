use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;

use crate::client::PayloadBuilder;
use crate::error::Result;
use crate::prompt::Prompt;

use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ResponseItem;

pub struct ChatPayloadBuilder {
    model: String,
}

impl ChatPayloadBuilder {
    pub fn new(model: String) -> Self {
        Self { model }
    }
}

impl PayloadBuilder for ChatPayloadBuilder {
    fn build(&self, prompt: &Prompt) -> Result<Value> {
        let mut messages = Vec::<Value>::new();
        messages.push(json!({ "role": "system", "content": prompt.instructions }));

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
                            ResponseItem::FunctionCall { .. }
                            | ResponseItem::LocalShellCall { .. } => {
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
                                items.push(
                                    json!({"type":"image_url","image_url": {"url": image_url}}),
                                );
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

                    let mut message = json!({
                        "role": role,
                        "content": content_value,
                    });

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
                            "function": {
                                "name": name,
                                "arguments": arguments,
                            },
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
                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": call_id,
                        "content": content_value,
                    }));
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
                            "function": {
                                "name": name,
                                "arguments": input,
                            },
                        }],
                    }));
                }
                ResponseItem::CustomToolCallOutput { call_id, output } => {
                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": call_id,
                        "content": output,
                    }));
                }
                ResponseItem::WebSearchCall { .. }
                | ResponseItem::Reasoning { .. }
                | ResponseItem::Other
                | ResponseItem::GhostSnapshot { .. } => {}
            }
        }

        let tools_json = create_tools_json_for_chat_completions_api(&prompt.tools)?;
        let payload = json!({
            "model": self.model,
            "messages": messages,
            "stream": true,
            "tools": tools_json,
        });
        Ok(payload)
    }
}

fn create_tools_json_for_chat_completions_api(
    tools: &[serde_json::Value],
) -> Result<Vec<serde_json::Value>> {
    let tools_json = tools
        .iter()
        .filter_map(|tool| {
            if tool.get("type") != Some(&serde_json::Value::String("function".to_string())) {
                return None;
            }

            let function_value = if let Some(function) = tool.get("function") {
                function.clone()
            } else if let Some(map) = tool.as_object() {
                let mut function = map.clone();
                function.remove("type");
                Value::Object(function)
            } else {
                return None;
            };

            Some(json!({
                "type": "function",
                "function": function_value,
            }))
        })
        .collect::<Vec<serde_json::Value>>();
    Ok(tools_json)
}
