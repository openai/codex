use crate::common::Reasoning;
use crate::common::ResponseCreateWsRequest;
use crate::common::ResponsesApiRequest;
use crate::common::TextControls;
use crate::error::ApiError;
use crate::provider::Provider;
use crate::requests::headers::build_conversation_headers;
use crate::requests::headers::insert_header;
use crate::requests::headers::subagent_header;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::SessionSource;
use http::HeaderMap;
use serde_json::Value;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Compression {
    #[default]
    None,
    Zstd,
}

/// Assembled request body plus headers for a Responses stream request.
pub struct ResponsesRawRequest {
    pub body: Value,
    pub headers: HeaderMap,
    pub compression: Compression,
}

pub struct ResponsesRequest {
    pub model: String,
    pub instructions: String,
    pub input: Vec<ResponseItem>,
    pub tools: Vec<Value>,
    pub parallel_tool_calls: bool,
    pub reasoning: Option<Reasoning>,
    pub include: Vec<String>,
    pub prompt_cache_key: Option<String>,
    pub text: Option<TextControls>,
    pub conversation_id: Option<String>,
    pub session_source: Option<SessionSource>,
    pub store_override: Option<bool>,
    pub extra_headers: HeaderMap,
    pub compression: Compression,
}

impl ResponsesRequest {
    pub fn into_raw_request(self, provider: &Provider) -> Result<ResponsesRawRequest, ApiError> {
        let store = self
            .store_override
            .unwrap_or_else(|| provider.is_azure_responses_endpoint());
        let req = ResponsesApiRequest {
            model: &self.model,
            instructions: &self.instructions,
            input: &self.input,
            tools: &self.tools,
            tool_choice: "auto",
            parallel_tool_calls: self.parallel_tool_calls,
            reasoning: self.reasoning,
            store,
            stream: true,
            include: self.include,
            prompt_cache_key: self.prompt_cache_key,
            text: self.text,
        };
        let mut body = serde_json::to_value(&req)
            .map_err(|e| ApiError::Stream(format!("failed to encode responses request: {e}")))?;

        if store && provider.is_azure_responses_endpoint() {
            attach_item_ids(&mut body, &self.input);
        }

        let mut headers = self.extra_headers;
        headers.extend(build_conversation_headers(self.conversation_id));
        if let Some(subagent) = subagent_header(&self.session_source) {
            insert_header(&mut headers, "x-openai-subagent", &subagent);
        }

        Ok(ResponsesRawRequest {
            body,
            headers,
            compression: self.compression,
        })
    }
}

impl TryFrom<&ResponsesRequest> for ResponseCreateWsRequest {
    type Error = ApiError;

    fn try_from(request: &ResponsesRequest) -> Result<Self, Self::Error> {
        let store = request.store_override.unwrap_or(false);

        Ok(Self {
            model: request.model.clone(),
            instructions: request.instructions.clone(),
            previous_response_id: None,
            input: request.input.clone(),
            tools: request.tools.clone(),
            tool_choice: "auto".to_string(),
            parallel_tool_calls: request.parallel_tool_calls,
            reasoning: request.reasoning.clone(),
            store,
            stream: true,
            include: request.include.clone(),
            prompt_cache_key: request.prompt_cache_key.clone(),
            text: request.text.clone(),
        })
    }
}

fn attach_item_ids(payload_json: &mut Value, original_items: &[ResponseItem]) {
    let Some(input_value) = payload_json.get_mut("input") else {
        return;
    };
    let Value::Array(items) = input_value else {
        return;
    };

    for (value, item) in items.iter_mut().zip(original_items.iter()) {
        if let ResponseItem::Reasoning { id, .. }
        | ResponseItem::Message { id: Some(id), .. }
        | ResponseItem::WebSearchCall { id: Some(id), .. }
        | ResponseItem::FunctionCall { id: Some(id), .. }
        | ResponseItem::LocalShellCall { id: Some(id), .. }
        | ResponseItem::CustomToolCall { id: Some(id), .. } = item
        {
            if id.is_empty() {
                continue;
            }

            if let Some(obj) = value.as_object_mut() {
                obj.insert("id".to_string(), Value::String(id.clone()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::RetryConfig;
    use codex_protocol::protocol::SubAgentSource;
    use http::HeaderValue;
    use pretty_assertions::assert_eq;
    use std::time::Duration;

    fn provider(name: &str, base_url: &str) -> Provider {
        Provider {
            name: name.to_string(),
            base_url: base_url.to_string(),
            query_params: None,
            headers: HeaderMap::new(),
            retry: RetryConfig {
                max_attempts: 1,
                base_delay: Duration::from_millis(50),
                retry_429: false,
                retry_5xx: true,
                retry_transport: true,
            },
            stream_idle_timeout: Duration::from_secs(5),
        }
    }

    #[test]
    fn azure_default_store_attaches_ids_and_headers() {
        let provider = provider("azure", "https://example.openai.azure.com/v1");
        let input = vec![
            ResponseItem::Message {
                id: Some("m1".into()),
                role: "assistant".into(),
                content: Vec::new(),
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "assistant".into(),
                content: Vec::new(),
                end_turn: None,
                phase: None,
            },
        ];

        let request = ResponsesRequest {
            model: "gpt-test".into(),
            instructions: "inst".into(),
            input,
            tools: Vec::new(),
            parallel_tool_calls: false,
            reasoning: None,
            include: Vec::new(),
            prompt_cache_key: None,
            text: None,
            conversation_id: Some("conv-1".into()),
            session_source: Some(SessionSource::SubAgent(SubAgentSource::Review)),
            store_override: None,
            extra_headers: HeaderMap::new(),
            compression: Compression::None,
        }
        .into_raw_request(&provider)
        .expect("request");

        assert_eq!(request.body.get("store"), Some(&Value::Bool(true)));

        let ids: Vec<Option<String>> = request
            .body
            .get("input")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .map(|item| item.get("id").and_then(|v| v.as_str().map(str::to_string)))
            .collect();
        assert_eq!(ids, vec![Some("m1".to_string()), None]);

        assert_eq!(
            request.headers.get("session_id"),
            Some(&HeaderValue::from_static("conv-1"))
        );
        assert_eq!(
            request.headers.get("x-openai-subagent"),
            Some(&HeaderValue::from_static("review"))
        );
    }
}
