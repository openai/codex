use codex_core::config_types::SandboxMode;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use serde::Deserialize;
use serde::Serialize;
use strum_macros::Display;
use uuid::Uuid;

use mcp_types::CallToolResult;
use mcp_types::ContentBlock;
use mcp_types::RequestId;
use mcp_types::TextContent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConversationId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MessageId(pub Uuid);

// Requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
    #[serde(rename = "jsonrpc")]
    pub jsonrpc: &'static str,
    pub id: RequestId,
    pub method: &'static str,
    pub params: ToolCallRequestParams,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "name", content = "arguments", rename_all = "camelCase")]
pub enum ToolCallRequestParams {
    ConversationCreate(ConversationCreateArgs),
    ConversationStream(ConversationStreamArgs),
    ConversationSendMessage(ConversationSendMessageArgs),
    ConversationsList(ConversationsListArgs),
}

impl ToolCallRequestParams {
    /// Wrap this request in a JSON-RPC request.
    #[allow(dead_code)]
    pub fn into_request(self, id: RequestId) -> ToolCallRequest {
        ToolCallRequest {
            jsonrpc: "2.0",
            id,
            method: "tools/call",
            params: self,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationCreateArgs {
    pub prompt: String,
    pub model: String,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<AskForApproval>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_instructions: Option<String>,
}

/// Optional overrides for an existing conversation's execution context when sending a message.
/// Fields left as `None` inherit the current conversation/session settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationOverrides {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<AskForApproval>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_instructions: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationStreamArgs {
    pub conversation_id: ConversationId,
}

/// If omitted, the message continues from the latest turn.
/// Set to resume/edit from an earlier parent message in the thread.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationSendMessageArgs {
    pub conversation_id: ConversationId,
    pub content: Vec<InputItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_message_id: Option<MessageId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(flatten)]
    pub conversation_overrides: Option<ConversationOverrides>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationsListArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

// Responses
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallResponse {
    pub request_id: RequestId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", flatten)]
    pub result: Option<ToolCallResponseResult>,
}

impl From<ToolCallResponse> for CallToolResult {
    fn from(val: ToolCallResponse) -> Self {
        let ToolCallResponse {
            request_id: _request_id,
            is_error,
            result,
        } = val;
        match result {
            Some(res) => match serde_json::to_value(&res) {
                Ok(v) => CallToolResult {
                    content: vec![ContentBlock::TextContent(TextContent {
                        r#type: "text".to_string(),
                        text: v.to_string(),
                        annotations: None,
                    })],
                    is_error,
                    structured_content: Some(v),
                },
                Err(e) => CallToolResult {
                    content: vec![ContentBlock::TextContent(TextContent {
                        r#type: "text".to_string(),
                        text: format!("Failed to serialize tool result: {e}"),
                        annotations: None,
                    })],
                    is_error: Some(true),
                    structured_content: None,
                },
            },
            None => CallToolResult {
                content: vec![],
                is_error,
                structured_content: None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolCallResponseResult {
    ConversationCreate(ConversationCreateResult),
    ConversationStream(ConversationStreamResult),
    ConversationSendMessage(ConversationSendMessageResult),
    ConversationsList(ConversationsListResult),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConversationCreateResult {
    Ok {
        conversation_id: ConversationId,
        model: String,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationStreamResult {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
// TODO: remove this status because we have is_error field in the response.
#[serde(tag = "status", rename_all = "camelCase")]
pub enum ConversationSendMessageResult {
    Ok,
    Error { message: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationsListResult {
    pub conversations: Vec<ConversationSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub conversation_id: ConversationId,
    pub title: String,
}

// Notifications
#[derive(Debug, Clone, Deserialize, Display)]
pub enum ServerNotification {
    InitialState(InitialStateNotificationParams),
    StreamDisconnected(StreamDisconnectedNotificationParams),
    CodexEvent(Box<CodexEventNotificationParams>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<ConversationId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<RequestId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitialStateNotificationParams {
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<NotificationMeta>,
    pub initial_state: InitialStatePayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitialStatePayload {
    #[serde(default)]
    pub events: Vec<CodexEventNotificationParams>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamDisconnectedNotificationParams {
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<NotificationMeta>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexEventNotificationParams {
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<NotificationMeta>,
    pub msg: EventMsg,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelNotificationParams {
    pub request_id: RequestId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl Serialize for ServerNotification {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;

        let mut map = serializer.serialize_map(Some(2))?;
        match self {
            ServerNotification::CodexEvent(p) => {
                map.serialize_entry("method", &format!("notifications/{}", p.msg))?;
                map.serialize_entry("params", p)?;
            }
            ServerNotification::InitialState(p) => {
                map.serialize_entry("method", "notifications/initial_state")?;
                map.serialize_entry("params", p)?;
            }
            ServerNotification::StreamDisconnected(p) => {
                map.serialize_entry("method", "notifications/stream_disconnected")?;
                map.serialize_entry("params", p)?;
            }
        }
        map.end()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
pub enum ClientNotification {
    #[serde(rename = "notifications/cancelled")]
    Cancelled(CancelNotificationParams),
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use codex_core::protocol::McpInvocation;
    use codex_core::protocol::McpToolCallBeginEvent;
    use pretty_assertions::assert_eq;
    use serde::Serialize;
    use serde_json::Value;
    use serde_json::json;
    use uuid::uuid;

    fn to_val<T: Serialize>(v: &T) -> Value {
        serde_json::to_value(v).expect("serialize to Value")
    }

    // ----- Requests -----

    #[test]
    fn serialize_tool_call_request_params_conversation_create_minimal() {
        let req = ToolCallRequestParams::ConversationCreate(ConversationCreateArgs {
            prompt: "".into(),
            model: "o3".into(),
            cwd: "/repo".into(),
            approval_policy: None,
            sandbox: None,
            config: None,
            profile: None,
            base_instructions: None,
        });

        let observed = to_val(&req.into_request(mcp_types::RequestId::Integer(2)));
        let expected = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "conversationCreate",
                "arguments": {
                    "prompt": "",
                    "model": "o3",
                    "cwd": "/repo"
                }
            }
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_tool_call_request_params_conversation_send_message_with_overrides_and_parent_message_id()
     {
        let req = ToolCallRequestParams::ConversationSendMessage(ConversationSendMessageArgs {
            conversation_id: ConversationId(uuid!("d0f6ecbe-84a2-41c1-b23d-b20473b25eab")),
            content: vec![
                InputItem::Text { text: "Hi".into() },
                InputItem::Image {
                    image_url: "https://example.com/cat.jpg".into(),
                },
                InputItem::LocalImage {
                    path: "notes.txt".into(),
                },
            ],
            parent_message_id: Some(MessageId(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))),
            conversation_overrides: Some(ConversationOverrides {
                model: Some("o4-mini".into()),
                cwd: Some("/workdir".into()),
                approval_policy: None,
                sandbox: Some(SandboxMode::DangerFullAccess),
                config: Some(json!({"temp": 0.2})),
                profile: Some("eng".into()),
                base_instructions: Some("Be terse".into()),
            }),
        });

        let observed = to_val(&req.into_request(mcp_types::RequestId::Integer(2)));
        let expected = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "conversationSendMessage",
                "arguments": {
                    "conversation_id": "d0f6ecbe-84a2-41c1-b23d-b20473b25eab",
                    "content": [
                        { "type": "text", "text": "Hi" },
                        { "type": "image", "image_url": "https://example.com/cat.jpg" },
                        { "type": "local_image", "path": "notes.txt" }
                    ],
                    "parent_message_id": "67e55044-10b1-426f-9247-bb680e5fe0c8",
                    "model": "o4-mini",
                    "cwd": "/workdir",
                    "sandbox": "danger-full-access",
                    "config": { "temp": 0.2 },
                    "profile": "eng",
                    "base_instructions": "Be terse"
                }
            }
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_tool_call_request_params_conversations_list_with_opts() {
        let req = ToolCallRequestParams::ConversationsList(ConversationsListArgs {
            limit: Some(50),
            cursor: Some("abc".into()),
        });

        let observed = to_val(&req.into_request(RequestId::Integer(2)));
        let expected = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "conversationsList",
                "arguments": {
                    "limit": 50,
                    "cursor": "abc"
                }
            }
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_tool_call_request_params_conversation_stream() {
        let req = ToolCallRequestParams::ConversationStream(ConversationStreamArgs {
            conversation_id: ConversationId(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")),
        });

        let observed = to_val(&req.into_request(mcp_types::RequestId::Integer(2)));
        let expected = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "conversationStream",
                "arguments": {
                    "conversation_id": "67e55044-10b1-426f-9247-bb680e5fe0c8"
                }
            }
        });
        assert_eq!(observed, expected);
    }

    // ----- Message inputs / sources -----

    #[test]
    fn serialize_message_input_image_url() {
        let item = InputItem::Image {
            image_url: "https://example.com/x.png".into(),
        };
        let observed = to_val(&item);
        let expected = json!({
            "type": "image",
            "image_url": "https://example.com/x.png"
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_message_input_local_image_path() {
        let url = InputItem::LocalImage {
            path: PathBuf::from("https://example.com/a.pdf"),
        };
        let id = InputItem::LocalImage {
            path: PathBuf::from("file_456"),
        };
        let observed_url = to_val(&url);
        let expected_url = json!({"type":"local_image","path":"https://example.com/a.pdf"});
        assert_eq!(
            observed_url, expected_url,
            "LocalImage with URL path should serialize as image_url"
        );
        let observed_id = to_val(&id);
        let expected_id = json!({"type":"local_image","path":"file_456"});
        assert_eq!(
            observed_id, expected_id,
            "LocalImage with file id should serialize as image_url"
        );
    }

    #[test]
    fn serialize_message_input_image_url_without_detail() {
        let item = InputItem::Image {
            image_url: "https://example.com/x.png".into(),
        };
        let observed = to_val(&item);
        let expected = json!({
            "type": "image",
            "image_url": "https://example.com/x.png"
        });
        assert_eq!(observed, expected);
    }

    // ----- Responses -----

    #[test]
    fn response_success_conversation_create_full_schema() {
        let env = ToolCallResponse {
            request_id: RequestId::Integer(1),
            is_error: None,
            result: Some(ToolCallResponseResult::ConversationCreate(
                ConversationCreateResult::Ok {
                    conversation_id: ConversationId(uuid!("d0f6ecbe-84a2-41c1-b23d-b20473b25eab")),
                    model: "o3".into(),
                },
            )),
        };
        let req_id = env.request_id.clone();
        let observed = to_val(&CallToolResult::from(env));
        let expected = json!({
            "content": [
                { "type": "text", "text": "{\"conversation_id\":\"d0f6ecbe-84a2-41c1-b23d-b20473b25eab\",\"model\":\"o3\"}" }
            ],
            "structuredContent": {
                "conversation_id": "d0f6ecbe-84a2-41c1-b23d-b20473b25eab",
                "model": "o3"
            }
        });
        assert_eq!(
            observed, expected,
            "response (ConversationCreate) must match"
        );
        assert_eq!(req_id, RequestId::Integer(1));
    }

    #[test]
    fn response_error_conversation_create_full_schema() {
        let env = ToolCallResponse {
            request_id: RequestId::Integer(2),
            is_error: Some(true),
            result: Some(ToolCallResponseResult::ConversationCreate(
                ConversationCreateResult::Error {
                    message: "Failed to initialize session".into(),
                },
            )),
        };
        let req_id = env.request_id.clone();
        let observed = to_val(&CallToolResult::from(env));
        let expected = json!({
            "content": [
                { "type": "text", "text": "{\"message\":\"Failed to initialize session\"}" }
            ],
            "isError": true,
            "structuredContent": {
                "message": "Failed to initialize session"
            }
        });
        assert_eq!(
            observed, expected,
            "error response (ConversationCreate) must match"
        );
        assert_eq!(req_id, RequestId::Integer(2));
    }

    #[test]
    fn response_success_conversation_stream_empty_result_object() {
        let env = ToolCallResponse {
            request_id: RequestId::Integer(2),
            is_error: None,
            result: Some(ToolCallResponseResult::ConversationStream(
                ConversationStreamResult {},
            )),
        };
        let req_id = env.request_id.clone();
        let observed = to_val(&CallToolResult::from(env));
        let expected = json!({
            "content": [ { "type": "text", "text": "{}" } ],
            "structuredContent": {}
        });
        assert_eq!(
            observed, expected,
            "response (ConversationStream) must have empty object result"
        );
        assert_eq!(req_id, RequestId::Integer(2));
    }

    #[test]
    fn response_success_send_message_accepted_full_schema() {
        let env = ToolCallResponse {
            request_id: RequestId::Integer(3),
            is_error: None,
            result: Some(ToolCallResponseResult::ConversationSendMessage(
                ConversationSendMessageResult::Ok,
            )),
        };
        let req_id = env.request_id.clone();
        let observed = to_val(&CallToolResult::from(env));
        let expected = json!({
            "content": [ { "type": "text", "text": "{\"status\":\"ok\"}" } ],
            "structuredContent": { "status": "ok" }
        });
        assert_eq!(
            observed, expected,
            "response (ConversationSendMessageAccepted) must match"
        );
        assert_eq!(req_id, RequestId::Integer(3));
    }

    #[test]
    fn response_success_conversations_list_with_next_cursor_full_schema() {
        let env = ToolCallResponse {
            request_id: RequestId::Integer(4),
            is_error: None,
            result: Some(ToolCallResponseResult::ConversationsList(
                ConversationsListResult {
                    conversations: vec![ConversationSummary {
                        conversation_id: ConversationId(uuid!(
                            "67e55044-10b1-426f-9247-bb680e5fe0c8"
                        )),
                        title: "Refactor config loader".into(),
                    }],
                    next_cursor: Some("next123".into()),
                },
            )),
        };
        let req_id = env.request_id.clone();
        let observed = to_val(&CallToolResult::from(env));
        let expected = json!({
            "content": [
                { "type": "text", "text": "{\"conversations\":[{\"conversation_id\":\"67e55044-10b1-426f-9247-bb680e5fe0c8\",\"title\":\"Refactor config loader\"}],\"next_cursor\":\"next123\"}" }
            ],
            "structuredContent": {
                "conversations": [
                    {
                        "conversation_id": "67e55044-10b1-426f-9247-bb680e5fe0c8",
                        "title": "Refactor config loader"
                    }
                ],
                "next_cursor": "next123"
            }
        });
        assert_eq!(
            observed, expected,
            "response (ConversationsList with cursor) must match"
        );
        assert_eq!(req_id, RequestId::Integer(4));
    }

    #[test]
    fn response_error_only_is_error_and_request_id_string() {
        let env = ToolCallResponse {
            request_id: RequestId::Integer(4),
            is_error: Some(true),
            result: None,
        };
        let req_id = env.request_id.clone();
        let observed = to_val(&CallToolResult::from(env));
        let expected = json!({
            "content": [],
            "isError": true
        });
        assert_eq!(
            observed, expected,
            "error response must omit `result` and include `isError`"
        );
        assert_eq!(req_id, RequestId::Integer(4));
    }

    // ----- Notifications -----

    #[test]
    fn serialize_notification_initial_state_minimal() {
        let params = InitialStateNotificationParams {
            meta: Some(NotificationMeta {
                conversation_id: Some(ConversationId(uuid!(
                    "67e55044-10b1-426f-9247-bb680e5fe0c8"
                ))),
                request_id: Some(RequestId::Integer(44)),
            }),
            initial_state: InitialStatePayload {
                events: vec![
                    CodexEventNotificationParams {
                        meta: None,
                        msg: EventMsg::TaskStarted,
                    },
                    CodexEventNotificationParams {
                        meta: None,
                        msg: EventMsg::AgentMessageDelta(
                            codex_core::protocol::AgentMessageDeltaEvent {
                                delta: "Loading...".into(),
                            },
                        ),
                    },
                ],
            },
        };

        let observed = to_val(&ServerNotification::InitialState(params.clone()));
        let expected = json!({
            "method": "notifications/initial_state",
            "params": {
                "_meta": {
                    "conversationId": "67e55044-10b1-426f-9247-bb680e5fe0c8",
                    "requestId": 44
                },
                "initial_state": {
                    "events": [
                        { "msg": { "type": "task_started" } },
                        { "msg": { "type": "agent_message_delta", "delta": "Loading..." } }
                    ]
                }
            }
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_notification_initial_state_omits_empty_events_full_json() {
        let params = InitialStateNotificationParams {
            meta: None,
            initial_state: InitialStatePayload { events: vec![] },
        };

        let observed = to_val(&ServerNotification::InitialState(params));
        let expected = json!({
            "method": "notifications/initial_state",
            "params": {
                "initial_state": { "events": [] }
            }
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_notification_stream_disconnected() {
        let params = StreamDisconnectedNotificationParams {
            meta: Some(NotificationMeta {
                conversation_id: Some(ConversationId(uuid!(
                    "67e55044-10b1-426f-9247-bb680e5fe0c8"
                ))),
                request_id: None,
            }),
            reason: "New stream() took over".into(),
        };

        let observed = to_val(&ServerNotification::StreamDisconnected(params));
        let expected = json!({
            "method": "notifications/stream_disconnected",
            "params": {
                "_meta": { "conversationId": "67e55044-10b1-426f-9247-bb680e5fe0c8" },
                "reason": "New stream() took over"
            }
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_notification_codex_event_uses_eventmsg_type_in_method() {
        let params = CodexEventNotificationParams {
            meta: Some(NotificationMeta {
                conversation_id: Some(ConversationId(uuid!(
                    "67e55044-10b1-426f-9247-bb680e5fe0c8"
                ))),
                request_id: Some(RequestId::Integer(44)),
            }),
            msg: EventMsg::AgentMessage(codex_core::protocol::AgentMessageEvent {
                message: "hi".into(),
            }),
        };

        let observed = to_val(&ServerNotification::CodexEvent(Box::new(params)));
        let expected = json!({
            "method": "notifications/agent_message",
            "params": {
                "_meta": {
                    "conversationId": "67e55044-10b1-426f-9247-bb680e5fe0c8",
                    "requestId": 44
                },
                "msg": { "type": "agent_message", "message": "hi" }
            }
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_notification_codex_event_task_started_full_json() {
        let params = CodexEventNotificationParams {
            meta: Some(NotificationMeta {
                conversation_id: Some(ConversationId(uuid!(
                    "67e55044-10b1-426f-9247-bb680e5fe0c8"
                ))),
                request_id: Some(RequestId::Integer(7)),
            }),
            msg: EventMsg::TaskStarted,
        };

        let observed = to_val(&ServerNotification::CodexEvent(Box::new(params)));
        let expected = json!({
            "method": "notifications/task_started",
            "params": {
                "_meta": {
                    "conversationId": "67e55044-10b1-426f-9247-bb680e5fe0c8",
                    "requestId": 7
                },
                "msg": { "type": "task_started" }
            }
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_notification_codex_event_agent_message_delta_full_json() {
        let params = CodexEventNotificationParams {
            meta: None,
            msg: EventMsg::AgentMessageDelta(codex_core::protocol::AgentMessageDeltaEvent {
                delta: "stream...".into(),
            }),
        };

        let observed = to_val(&ServerNotification::CodexEvent(Box::new(params)));
        let expected = json!({
            "method": "notifications/agent_message_delta",
            "params": {
                "msg": { "type": "agent_message_delta", "delta": "stream..." }
            }
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_notification_codex_event_agent_message_full_json() {
        let params = CodexEventNotificationParams {
            meta: Some(NotificationMeta {
                conversation_id: Some(ConversationId(uuid!(
                    "67e55044-10b1-426f-9247-bb680e5fe0c8"
                ))),
                request_id: Some(RequestId::Integer(44)),
            }),
            msg: EventMsg::AgentMessage(codex_core::protocol::AgentMessageEvent {
                message: "hi".into(),
            }),
        };

        let observed = to_val(&ServerNotification::CodexEvent(Box::new(params)));
        let expected = json!({
            "method": "notifications/agent_message",
            "params": {
                "_meta": {
                    "conversationId": "67e55044-10b1-426f-9247-bb680e5fe0c8",
                    "requestId": 44
                },
                "msg": { "type": "agent_message", "message": "hi" }
            }
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_notification_codex_event_agent_reasoning_full_json() {
        let params = CodexEventNotificationParams {
            meta: None,
            msg: EventMsg::AgentReasoning(codex_core::protocol::AgentReasoningEvent {
                text: "thinking…".into(),
            }),
        };

        let observed = to_val(&ServerNotification::CodexEvent(Box::new(params)));
        let expected = json!({
            "method": "notifications/agent_reasoning",
            "params": {
                "msg": { "type": "agent_reasoning", "text": "thinking…" }
            }
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_notification_codex_event_token_count_full_json() {
        let usage = codex_core::protocol::TokenUsage {
            input_tokens: 10,
            cached_input_tokens: Some(2),
            output_tokens: 5,
            reasoning_output_tokens: Some(1),
            total_tokens: 16,
        };
        let params = CodexEventNotificationParams {
            meta: None,
            msg: EventMsg::TokenCount(usage),
        };

        let observed = to_val(&ServerNotification::CodexEvent(Box::new(params)));
        let expected = json!({
            "method": "notifications/token_count",
            "params": {
                "msg": {
                    "type": "token_count",
                    "input_tokens": 10,
                    "cached_input_tokens": 2,
                    "output_tokens": 5,
                    "reasoning_output_tokens": 1,
                    "total_tokens": 16
                }
            }
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_notification_codex_event_session_configured_full_json() {
        let params = CodexEventNotificationParams {
            meta: Some(NotificationMeta {
                conversation_id: Some(ConversationId(uuid!(
                    "67e55044-10b1-426f-9247-bb680e5fe0c8"
                ))),
                request_id: None,
            }),
            msg: EventMsg::SessionConfigured(codex_core::protocol::SessionConfiguredEvent {
                session_id: uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
                model: "codex-mini-latest".into(),
                history_log_id: 42,
                history_entry_count: 3,
            }),
        };

        let observed = to_val(&ServerNotification::CodexEvent(Box::new(params)));
        let expected = json!({
            "method": "notifications/session_configured",
            "params": {
                "_meta": { "conversationId": "67e55044-10b1-426f-9247-bb680e5fe0c8" },
                "msg": {
                    "type": "session_configured",
                    "session_id": "67e55044-10b1-426f-9247-bb680e5fe0c8",
                    "model": "codex-mini-latest",
                    "history_log_id": 42,
                    "history_entry_count": 3
                }
            }
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_notification_codex_event_exec_command_begin_full_json() {
        let params = CodexEventNotificationParams {
            meta: None,
            msg: EventMsg::ExecCommandBegin(codex_core::protocol::ExecCommandBeginEvent {
                call_id: "c1".into(),
                command: vec!["bash".into(), "-lc".into(), "echo hi".into()],
                cwd: std::path::PathBuf::from("/work"),
                parsed_cmd: vec![],
            }),
        };

        let observed = to_val(&ServerNotification::CodexEvent(Box::new(params)));
        let expected = json!({
            "method": "notifications/exec_command_begin",
            "params": {
                "msg": {
                    "type": "exec_command_begin",
                    "call_id": "c1",
                    "command": ["bash", "-lc", "echo hi"],
                    "cwd": "/work",
                    "parsed_cmd": []
                }
            }
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_notification_codex_event_mcp_tool_call_begin_full_json() {
        let params = CodexEventNotificationParams {
            meta: None,
            msg: EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
                call_id: "m1".into(),
                invocation: McpInvocation {
                    server: "calc".into(),
                    tool: "add".into(),
                    arguments: Some(json!({"a":1,"b":2})),
                },
            }),
        };

        let observed = to_val(&ServerNotification::CodexEvent(Box::new(params)));
        let expected = json!({
            "method": "notifications/mcp_tool_call_begin",
            "params": {
                "msg": {
                    "type": "mcp_tool_call_begin",
                    "call_id": "m1",
                    "invocation": {
                        "server": "calc",
                        "tool": "add",
                        "arguments": { "a": 1, "b": 2 }
                    }
                }
            }
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_notification_codex_event_patch_apply_end_full_json() {
        let params = CodexEventNotificationParams {
            meta: None,
            msg: EventMsg::PatchApplyEnd(codex_core::protocol::PatchApplyEndEvent {
                call_id: "p1".into(),
                stdout: "ok".into(),
                stderr: "".into(),
                success: true,
            }),
        };

        let observed = to_val(&ServerNotification::CodexEvent(Box::new(params)));
        let expected = json!({
            "method": "notifications/patch_apply_end",
            "params": {
                "msg": {
                    "type": "patch_apply_end",
                    "call_id": "p1",
                    "stdout": "ok",
                    "stderr": "",
                    "success": true
                }
            }
        });
        assert_eq!(observed, expected);
    }

    // ----- Cancelled notifications -----

    #[test]
    fn serialize_notification_cancelled_with_reason_full_json() {
        let params = CancelNotificationParams {
            request_id: RequestId::String("r-123".into()),
            reason: Some("user_cancelled".into()),
        };

        let observed = to_val(&ClientNotification::Cancelled(params));
        let expected = json!({
            "method": "notifications/cancelled",
            "params": {
                "requestId": "r-123",
                "reason": "user_cancelled"
            }
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_notification_cancelled_without_reason_full_json() {
        let params = CancelNotificationParams {
            request_id: RequestId::Integer(77),
            reason: None,
        };

        let observed = to_val(&ClientNotification::Cancelled(params));

        // Check exact structure: reason must be omitted.
        assert_eq!(observed["method"], "notifications/cancelled");
        assert_eq!(observed["params"]["requestId"], 77);
        assert!(
            observed["params"].get("reason").is_none(),
            "reason must be omitted when None"
        );
    }
}
