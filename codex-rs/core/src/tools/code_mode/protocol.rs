use std::collections::HashMap;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;

use super::CODE_MODE_BRIDGE_SOURCE;
use super::PUBLIC_TOOL_NAME;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum CodeModeToolKind {
    Function,
    Freeform,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct EnabledTool {
    pub(super) tool_name: String,
    pub(super) global_name: String,
    #[serde(rename = "module")]
    pub(super) module_path: String,
    pub(super) namespace: Vec<String>,
    pub(super) name: String,
    pub(super) description: String,
    pub(super) kind: CodeModeToolKind,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct CodeModeToolCall {
    pub(super) request_id: String,
    pub(super) id: String,
    pub(super) name: String,
    #[serde(default)]
    pub(super) input: Option<JsonValue>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct CodeModeNotify {
    pub(super) cell_id: String,
    pub(super) text: String,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum HostToNodeMessage {
    Start {
        request_id: String,
        cell_id: String,
        default_yield_time_ms: u64,
        enabled_tools: Vec<EnabledTool>,
        stored_values: HashMap<String, JsonValue>,
        source: String,
        yield_time_ms: Option<u64>,
        max_output_tokens: Option<usize>,
    },
    Poll {
        request_id: String,
        cell_id: String,
        yield_time_ms: u64,
    },
    Terminate {
        request_id: String,
        cell_id: String,
    },
    Response {
        request_id: String,
        id: String,
        code_mode_result: JsonValue,
    },
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum NodeToHostMessage {
    ToolCall {
        #[serde(flatten)]
        tool_call: CodeModeToolCall,
    },
    Yielded {
        request_id: String,
        content_items: Vec<JsonValue>,
    },
    Terminated {
        request_id: String,
        content_items: Vec<JsonValue>,
    },
    Notify {
        #[serde(flatten)]
        notify: CodeModeNotify,
    },
    Result {
        request_id: String,
        content_items: Vec<JsonValue>,
        stored_values: HashMap<String, JsonValue>,
        #[serde(default)]
        error_text: Option<String>,
        #[serde(default)]
        max_output_tokens_per_exec_call: Option<usize>,
    },
}

pub(super) fn build_source(
    user_code: &str,
    enabled_tools: &[EnabledTool],
) -> Result<String, String> {
    let enabled_tools_json = serde_json::to_string(enabled_tools)
        .map_err(|err| format!("failed to serialize enabled tools: {err}"))?;
    Ok(CODE_MODE_BRIDGE_SOURCE
        .replace(
            "__CODE_MODE_ENABLED_TOOLS_PLACEHOLDER__",
            &enabled_tools_json,
        )
        .replace("__CODE_MODE_USER_CODE_PLACEHOLDER__", user_code))
}

pub(super) fn message_request_id(message: &NodeToHostMessage) -> Option<&str> {
    match message {
        NodeToHostMessage::ToolCall { .. } => None,
        NodeToHostMessage::Yielded { request_id, .. }
        | NodeToHostMessage::Terminated { request_id, .. }
        | NodeToHostMessage::Result { request_id, .. } => Some(request_id),
        NodeToHostMessage::Notify { .. } => None,
    }
}

pub(super) fn unexpected_tool_call_error() -> String {
    format!("{PUBLIC_TOOL_NAME} received an unexpected tool call response")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::CodeModeNotify;
    use super::NodeToHostMessage;
    use super::message_request_id;

    #[test]
    fn message_request_id_absent_for_notify() {
        let message = NodeToHostMessage::Notify {
            notify: CodeModeNotify {
                cell_id: "1".to_string(),
                text: "hello".to_string(),
            },
        };

        assert_eq!(None, message_request_id(&message));
    }

    #[test]
    fn message_request_id_present_for_result() {
        let message = NodeToHostMessage::Result {
            request_id: "req-1".to_string(),
            content_items: Vec::new(),
            stored_values: HashMap::new(),
            error_text: None,
            max_output_tokens_per_exec_call: None,
        };

        assert_eq!(Some("req-1"), message_request_id(&message));
    }
}
