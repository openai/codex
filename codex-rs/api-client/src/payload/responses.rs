use serde_json::Value;
use serde_json::json;

use crate::client::PayloadBuilder;
use crate::error::Result;
use crate::prompt::Prompt;

use codex_protocol::ConversationId;
use codex_protocol::models::ResponseItem;

pub struct ResponsesPayloadBuilder {
    model: String,
    conversation_id: ConversationId,
    azure_workaround: bool,
}

impl ResponsesPayloadBuilder {
    pub fn new(model: String, conversation_id: ConversationId, azure_workaround: bool) -> Self {
        Self {
            model,
            conversation_id,
            azure_workaround,
        }
    }
}

impl PayloadBuilder for ResponsesPayloadBuilder {
    fn build(&self, prompt: &Prompt) -> Result<Value> {
        let azure = self.azure_workaround;
        let mut payload = json!({
            "model": self.model,
            "instructions": prompt.instructions,
            "input": prompt.input,
            "tools": prompt.tools,
            "tool_choice": "auto",
            "parallel_tool_calls": prompt.parallel_tool_calls,
            "store": azure,
            "stream": true,
            "prompt_cache_key": prompt
                .prompt_cache_key
                .clone()
                .unwrap_or_else(|| self.conversation_id.to_string()),
        });

        if let Some(reasoning) = prompt.reasoning.as_ref()
            && let Some(obj) = payload.as_object_mut()
        {
            obj.insert("reasoning".to_string(), serde_json::to_value(reasoning)?);
        }

        if let Some(text) = prompt.text_controls.as_ref()
            && let Some(obj) = payload.as_object_mut()
        {
            obj.insert("text".to_string(), serde_json::to_value(text)?);
        }

        let include = if prompt.reasoning.is_some() {
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
        if azure
            && let Some(input_value) = payload.get_mut("input")
            && let Some(array) = input_value.as_array_mut()
        {
            attach_item_ids_array(array, &prompt.input);
        }

        Ok(payload)
    }
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
            ResponseItem::WebSearchCall { id, .. } => {
                if let Some(id) = id.as_ref() {
                    set_id_if_absent(id);
                }
            }
            ResponseItem::FunctionCall { id, .. } => {
                if let Some(id) = id.as_ref() {
                    set_id_if_absent(id);
                }
            }
            ResponseItem::LocalShellCall { id, .. } => {
                if let Some(id) = id.as_ref() {
                    set_id_if_absent(id);
                }
            }
            ResponseItem::CustomToolCall { id, .. } => {
                if let Some(id) = id.as_ref() {
                    set_id_if_absent(id);
                }
            }
            _ => {}
        }
    }
}
