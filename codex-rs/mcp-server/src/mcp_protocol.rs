use codex_core::protocol::EventMsg;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

use mcp_types::RequestId;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "name", content = "arguments", rename_all = "snake_case")]
pub enum ToolCallRequestParams {
    NewConversation(NewConversationArgs),
    Connect(ConnectArgs),
    SendUserMessage(SendUserMessageArgs),
    GetConversations(GetConversationsArgs),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewConversationArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    pub model: String,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_instructions: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConnectArgs {
    pub conversation_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SendUserMessageArgs {
    pub conversation_id: Uuid,
    pub content: Vec<InputMessageContentPart>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputMessageContentPart {
    Text {
        text: String,
    },
    Image {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        image_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        file_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<ImageDetail>,
    },
    File {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        file_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        file_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        filename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        file_data: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageDetail {
    Low,
    High,
    Auto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GetConversationsArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ToolCallResponseData {
    NewConversation(NewConversationResult),
    Connect(ConnectResult),
    SendUserMessage(SendUserMessageAccepted),
    GetConversations(GetConversationsResult),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewConversationResult {
    pub conversation_id: Uuid,
    pub model: String,
    pub history_log_id: u64,
    pub history_entry_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConnectResult {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SendUserMessageAccepted {
    pub accepted: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GetConversationsResult {
    pub conversations: Vec<ConversationSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub conversation_id: Uuid,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ConversationNotificationParams {
    InitialState(InitialStateNotificationParams),
    ConnectionRevoked(ConnectionRevokedNotificationParams),
    CodexEvent(CodexEventNotificationParams),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<Uuid>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<CodexEventNotificationParams>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConnectionRevokedNotificationParams {
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use uuid::uuid;

    #[test]
    fn serialize_initial_state_params_minimal() {
        let params = InitialStateNotificationParams {
            meta: Some(NotificationMeta {
                conversation_id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")),
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

        let got = match serde_json::to_value(&params) {
            Ok(v) => v,
            Err(e) => panic!("failed to serialize InitialStateNotificationParams: {e}"),
        };
        let expected = json!({
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
        });
        assert_eq!(got, expected);
    }

    #[test]
    fn serialize_connection_revoked_params() {
        let params = ConnectionRevokedNotificationParams {
            meta: Some(NotificationMeta {
                conversation_id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")),
                request_id: None,
            }),
            reason: "New connect() took over".into(),
        };
        let got = match serde_json::to_value(&params) {
            Ok(v) => v,
            Err(e) => panic!("failed to serialize ConnectionRevokedNotificationParams: {e}"),
        };
        let expected = json!({
            "_meta": { "conversationId": "67e55044-10b1-426f-9247-bb680e5fe0c8" },
            "reason": "New connect() took over"
        });
        assert_eq!(got, expected);
    }

    #[test]
    fn serialize_new_conversation_result() {
        let result = NewConversationResult {
            conversation_id: uuid!("d0f6ecbe-84a2-41c1-b23d-b20473b25eab"),
            model: "o3".into(),
            history_log_id: 3874612938,
            history_entry_count: 0,
        };
        let got = match serde_json::to_value(&result) {
            Ok(v) => v,
            Err(e) => panic!("failed to serialize NewConversationResult: {e}"),
        };
        let expected = json!({
            "conversation_id": "d0f6ecbe-84a2-41c1-b23d-b20473b25eab",
            "model": "o3",
            "history_log_id": 3874612938u64,
            "history_entry_count": 0
        });
        assert_eq!(got, expected);
    }

    #[test]
    fn serialize_get_conversations_result() {
        let result = GetConversationsResult {
            conversations: vec![ConversationSummary {
                conversation_id: uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
                title: "Refactor config loader".into(),
            }],
            next_cursor: Some("eyJsb2dpZF9vZmZzZXQiOjIwfQ==".into()),
        };
        let got = match serde_json::to_value(&result) {
            Ok(v) => v,
            Err(e) => panic!("failed to serialize GetConversationsResult: {e}"),
        };
        let expected = json!({
            "conversations": [
                {"conversation_id": "67e55044-10b1-426f-9247-bb680e5fe0c8", "title": "Refactor config loader"}
            ],
            "next_cursor": "eyJsb2dpZF9vZmZzZXQiOjIwfQ=="
        });
        assert_eq!(got, expected);
    }

    #[test]
    fn serialize_tool_call_request_params_send_user_message() {
        let req = ToolCallRequestParams::SendUserMessage(SendUserMessageArgs {
            conversation_id: uuid!("d0f6ecbe-84a2-41c1-b23d-b20473b25eab"),
            content: vec![InputMessageContentPart::Text {
                text: "Hello".into(),
            }],
            message_id: Some("client-uuid-123".into()),
        });
        let got = match serde_json::to_value(&req) {
            Ok(v) => v,
            Err(e) => panic!("failed to serialize ToolCallRequestParams::SendUserMessage: {e}"),
        };
        let expected = json!({
            "name": "send_user_message",
            "arguments": {
                "conversation_id": "d0f6ecbe-84a2-41c1-b23d-b20473b25eab",
                "content": [ { "type": "text", "text": "Hello" } ],
                "message_id": "client-uuid-123"
            }
        });
        assert_eq!(got, expected);
    }

    #[test]
    fn serialize_tool_call_response_data_new_conversation() {
        let resp = ToolCallResponseData::NewConversation(NewConversationResult {
            conversation_id: uuid!("d0f6ecbe-84a2-41c1-b23d-b20473b25eab"),
            model: "o3".into(),
            history_log_id: 1,
            history_entry_count: 0,
        });
        let got = match serde_json::to_value(&resp) {
            Ok(v) => v,
            Err(e) => panic!("failed to serialize ToolCallResponseData::NewConversation: {e}"),
        };
        let expected = json!({
            "type": "new_conversation",
            "data": {
                "conversation_id": "d0f6ecbe-84a2-41c1-b23d-b20473b25eab",
                "model": "o3",
                "history_log_id": 1,
                "history_entry_count": 0
            }
        });
        assert_eq!(got, expected);
    }

    #[test]
    fn serialize_conversation_notification_params_codex_event() {
        let params = ConversationNotificationParams::CodexEvent(CodexEventNotificationParams {
            meta: Some(NotificationMeta {
                conversation_id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")),
                request_id: Some(RequestId::Integer(44)),
            }),
            msg: EventMsg::AgentMessage(codex_core::protocol::AgentMessageEvent {
                message: "hi".into(),
            }),
        });
        let got = match serde_json::to_value(&params) {
            Ok(v) => v,
            Err(e) => panic!("failed to serialize ConversationNotificationParams::CodexEvent: {e}"),
        };
        let expected = json!({
            "type": "codex_event",
            "data": {
                "_meta": {
                    "conversationId": "67e55044-10b1-426f-9247-bb680e5fe0c8",
                    "requestId": 44
                },
                "msg": { "type": "agent_message", "message": "hi" }
            }
        });
        assert_eq!(got, expected);
    }
}
