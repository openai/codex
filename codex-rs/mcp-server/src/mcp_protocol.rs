use codex_core::config_types::SandboxMode;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

use mcp_types::RequestId;

// Introduce a dedicated ConversationId type to allow future flexibility
// on the underlying representation (e.g. switching from UUID to u32).
pub type ConversationId = Uuid;

// Requests
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "name", content = "arguments", rename_all = "snake_case")]
pub enum ToolCallRequestParams {
    ConversationCreate(ConversationCreateArgs),
    ConversationConnect(ConversationConnectArgs),
    ConversationSendMessage(ConversationSendMessageArgs),
    ConversationsList(ConversationsListArgs),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationCreateArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
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
pub struct ConversationConnectArgs {
    pub conversation_id: ConversationId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationSendMessageArgs {
    pub conversation_id: ConversationId,
    pub content: Vec<MessageInputItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(flatten)]
    pub conversation_overrides: Option<ConversationOverrides>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageInputItem {
    /// Following OpenAI's Responses API: https://platform.openai.com/docs/api-reference/responses
    Text { text: String },
    Image {
        #[serde(flatten)]
        source: ImageSource,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<ImageDetail>,
    },
    File {
        #[serde(flatten)]
        source: FileSource,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ImageSource {
    /// Following OpenAI's API: https://platform.openai.com/docs/guides/images-vision#giving-a-model-images-as-input
    ImageUrl {
        image_url: String,
    },
    FileId {
        file_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FileSource {
    /// Following OpenAI's Responses API: https://platform.openai.com/docs/guides/pdf-files?api-mode=responses#uploading-files
    Url {
        file_url: String,
    },
    Id {
        file_id: String,
    },
    Base64 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        filename: Option<String>,
        /// Base64-encoded file contents.
        file_data: String,
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
pub struct ConversationsListArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

// Responses

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallResponseEnvelope {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<ToolCallResponseContent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<ToolCallResponseData>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ToolCallResponseData {
    ConversationCreate(ConversationCreateResult),
    ConversationConnect(ConversationConnectResult),
    ConversationSendMessage(ConversationSendMessageAccepted),
    ConversationsList(ConversationsListResult),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationCreateResult {
    pub conversation_id: ConversationId,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationConnectResult {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationSendMessageAccepted {
    pub accepted: bool,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolCallResponseContent {
    Text { text: String },
}

// Notifications
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "snake_case")]
pub enum ConversationNotificationParams {
    InitialState(InitialStateNotificationParams),
    // sent when a second client connects to the same conversation
    ConnectionRevoked(ConnectionRevokedNotificationParams),
    CodexEvent(CodexEventNotificationParams),
    Cancelled(CancellNotificationParams),
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CancellNotificationParams {
    pub request_id: RequestId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
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

        let observed = serde_json::to_value(&params)
            .expect("failed to serialize InitialStateNotificationParams");
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
        assert_eq!(observed, expected);
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
        let observed = serde_json::to_value(&params)
            .expect("failed to serialize ConnectionRevokedNotificationParams");
        let expected = json!({
            "_meta": { "conversationId": "67e55044-10b1-426f-9247-bb680e5fe0c8" },
            "reason": "New connect() took over"
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_new_conversation_result() {
        let result = ConversationCreateResult {
            conversation_id: uuid!("d0f6ecbe-84a2-41c1-b23d-b20473b25eab"),
            model: "o3".into(),
        };
        let observed =
            serde_json::to_value(&result).expect("failed to serialize ConversationCreateResult");
        let expected = json!({
            "conversation_id": "d0f6ecbe-84a2-41c1-b23d-b20473b25eab",
            "model": "o3",
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_get_conversations_result() {
        let result = ConversationsListResult {
            conversations: vec![ConversationSummary {
                conversation_id: uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
                title: "Refactor config loader".into(),
            }],
            next_cursor: Some("eyJsb2dpZF9vZmZzZXQiOjIwfQ==".into()),
        };
        let observed =
            serde_json::to_value(&result).expect("failed to serialize ConversationsListResult");
        let expected = json!({
            "conversations": [
                {"conversation_id": "67e55044-10b1-426f-9247-bb680e5fe0c8", "title": "Refactor config loader"}
            ],
            "next_cursor": "eyJsb2dpZF9vZmZzZXQiOjIwfQ=="
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_tool_call_request_params_send_user_message() {
        let req = ToolCallRequestParams::ConversationSendMessage(ConversationSendMessageArgs {
            conversation_id: uuid!("d0f6ecbe-84a2-41c1-b23d-b20473b25eab"),
            content: vec![MessageInputItem::Text {
                text: "Hello".into(),
            }],
            message_id: Some("client-uuid-123".into()),
            conversation_overrides: None,
        });
        let observed = serde_json::to_value(&req)
            .expect("failed to serialize ToolCallRequestParams::SendUserMessage");
        let expected = json!({
            "name": "conversation_send_message",
            "arguments": {
                "conversation_id": "d0f6ecbe-84a2-41c1-b23d-b20473b25eab",
                "content": [ { "type": "text", "text": "Hello" } ],
                "message_id": "client-uuid-123"
            }
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_tool_call_response_data_new_conversation() {
        let resp = ToolCallResponseData::ConversationCreate(ConversationCreateResult {
            conversation_id: uuid!("d0f6ecbe-84a2-41c1-b23d-b20473b25eab"),
            model: "o3".into(),
        });
        let observed = serde_json::to_value(&resp)
            .expect("failed to serialize ToolCallResponseData::ConversationCreate");
        let expected = json!({
            "type": "conversation_create",
            "data": {
                "conversation_id": "d0f6ecbe-84a2-41c1-b23d-b20473b25eab",
                "model": "o3",
            }
        });
        assert_eq!(observed, expected);
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
        let observed = serde_json::to_value(&params)
            .expect("failed to serialize ConversationNotificationParams::CodexEvent");
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
        assert_eq!(observed, expected);
    }
}
