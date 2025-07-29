use codex_core::config_types::SandboxMode;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

use mcp_types::RequestId;

pub type ConversationId = Uuid;

// Requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequestEnvelope {
    #[serde(rename = "jsonrpc")]
    pub jsonrpc: &'static str,
    pub id: u64,
    pub method: &'static str,
    pub params: ToolCallRequestParams,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "name", content = "arguments", rename_all = "snake_case")]
pub enum ToolCallRequestParams {
    ConversationCreate(ConversationCreateArgs),
    ConversationConnect(ConversationConnectArgs),
    ConversationSendMessage(ConversationSendMessageArgs),
    ConversationsList(ConversationsListArgs),
}

impl ToolCallRequestParams {
    /// Wrap this request in a JSON-RPC envelope.
    #[allow(dead_code)]
    pub fn into_envelope(self, id: u64) -> ToolCallRequestEnvelope {
        ToolCallRequestEnvelope {
            jsonrpc: "2.0",
            id,
            method: "tools/call",
            params: self,
        }
    }
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
    pub request_id: RequestId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<ToolCallResponseData>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
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

#[derive(Debug, Clone, Deserialize)]
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
    #[serde(rename = "requestId")]
    pub request_id: RequestId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Strongly-typed notification envelope (no unwraps/expect, no serde_json::Value payloads).
#[derive(Debug, Clone)]
pub enum NotificationEnvelope {
    InitialState(InitialStateNotificationParams),
    ConnectionRevoked(ConnectionRevokedNotificationParams),
    Cancelled(CancellNotificationParams),
    CodexEvent(CodexEventNotificationParams),
}

impl From<ConversationNotificationParams> for NotificationEnvelope {
    fn from(n: ConversationNotificationParams) -> Self {
        match n {
            ConversationNotificationParams::InitialState(p) => {
                NotificationEnvelope::InitialState(p)
            }
            ConversationNotificationParams::ConnectionRevoked(p) => {
                NotificationEnvelope::ConnectionRevoked(p)
            }
            ConversationNotificationParams::Cancelled(p) => NotificationEnvelope::Cancelled(p),
            ConversationNotificationParams::CodexEvent(p) => NotificationEnvelope::CodexEvent(p),
        }
    }
}

impl Serialize for NotificationEnvelope {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;

        fn event_type(msg: &EventMsg) -> &'static str {
            // Keep in sync with EventMsg variants/serde renames used by codex_core.
            match msg {
                EventMsg::TaskStarted => "task_started",
                EventMsg::AgentMessageDelta(_) => "agent_message_delta",
                EventMsg::AgentMessage(_) => "agent_message",
                _ => "unknown",
            }
        }

        let mut map = serializer.serialize_map(Some(2))?;
        match self {
            NotificationEnvelope::InitialState(p) => {
                map.serialize_entry("method", "notifications/initial_state")?;
                map.serialize_entry("params", p)?;
            }
            NotificationEnvelope::ConnectionRevoked(p) => {
                map.serialize_entry("method", "notifications/connection_revoked")?;
                map.serialize_entry("params", p)?;
            }
            NotificationEnvelope::Cancelled(p) => {
                map.serialize_entry("method", "notifications/cancelled")?;
                map.serialize_entry("params", p)?;
            }
            NotificationEnvelope::CodexEvent(p) => {
                let t = event_type(&p.msg);
                map.serialize_entry("method", &format!("notifications/{t}"))?;
                map.serialize_entry("params", p)?;
            }
        }
        map.end()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
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
            prompt: None,
            model: "o3".into(),
            cwd: "/repo".into(),
            approval_policy: None,
            sandbox: None,
            config: None,
            profile: None,
            base_instructions: None,
        });

        let observed = to_val(&req.into_envelope(2));
        let expected = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "conversation_create",
                "arguments": {
                    "model": "o3",
                    "cwd": "/repo"
                }
            }
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_tool_call_request_params_conversation_send_message_with_overrides_and_message_id()
    {
        let req = ToolCallRequestParams::ConversationSendMessage(ConversationSendMessageArgs {
            conversation_id: uuid!("d0f6ecbe-84a2-41c1-b23d-b20473b25eab"),
            content: vec![
                MessageInputItem::Text { text: "Hi".into() },
                MessageInputItem::Image {
                    source: ImageSource::ImageUrl {
                        image_url: "https://example.com/cat.jpg".into(),
                    },
                    detail: Some(ImageDetail::High),
                },
                MessageInputItem::File {
                    source: FileSource::Base64 {
                        filename: Some("notes.txt".into()),
                        file_data: "Zm9vYmFy".into(),
                    },
                },
            ],
            message_id: Some("client-uuid-123".into()),
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

        let observed = to_val(&req.into_envelope(2));
        let expected = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "conversation_send_message",
                "arguments": {
                    "conversation_id": "d0f6ecbe-84a2-41c1-b23d-b20473b25eab",
                    "content": [
                        { "type": "text", "text": "Hi" },
                        { "type": "image", "image_url": "https://example.com/cat.jpg", "detail": "high" },
                        { "type": "file", "filename": "notes.txt", "file_data": "Zm9vYmFy" }
                    ],
                    "message_id": "client-uuid-123",
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

        let observed = to_val(&req.into_envelope(2));
        let expected = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "conversations_list",
                "arguments": {
                    "limit": 50,
                    "cursor": "abc"
                }
            }
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_tool_call_request_params_conversation_connect() {
        let req = ToolCallRequestParams::ConversationConnect(ConversationConnectArgs {
            conversation_id: uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
        });

        let observed = to_val(&req.into_envelope(2));
        let expected = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "conversation_connect",
                "arguments": {
                    "conversation_id": "67e55044-10b1-426f-9247-bb680e5fe0c8"
                }
            }
        });
        assert_eq!(observed, expected);
    }

    // ----- Message inputs / sources -----

    #[test]
    fn serialize_message_input_image_file_id_auto_detail() {
        let item = MessageInputItem::Image {
            source: ImageSource::FileId {
                file_id: "file_123".into(),
            },
            detail: Some(ImageDetail::Auto),
        };
        let observed = to_val(&item);
        let expected = json!({
            "type": "image",
            "file_id": "file_123",
            "detail": "auto"
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_message_input_file_url_and_id_variants() {
        let url = MessageInputItem::File {
            source: FileSource::Url {
                file_url: "https://example.com/a.pdf".into(),
            },
        };
        let id = MessageInputItem::File {
            source: FileSource::Id {
                file_id: "file_456".into(),
            },
        };
        assert_eq!(
            to_val(&url),
            json!({"type":"file","file_url":"https://example.com/a.pdf"})
        );
        assert_eq!(to_val(&id), json!({"type":"file","file_id":"file_456"}));
    }

    #[test]
    fn serialize_message_input_image_url_without_detail() {
        let item = MessageInputItem::Image {
            source: ImageSource::ImageUrl {
                image_url: "https://example.com/x.png".into(),
            },
            detail: None,
        };
        let observed = to_val(&item);
        let expected = json!({
            "type": "image",
            "image_url": "https://example.com/x.png"
        });
        assert_eq!(observed, expected);
    }

    // ----- Responses (full envelope) -----

    #[test]
    fn envelope_success_conversation_create_full_schema() {
        let env = ToolCallResponseEnvelope {
            request_id: RequestId::Integer(1),
            is_error: None,
            result: Some(ToolCallResponseData::ConversationCreate(
                ConversationCreateResult {
                    conversation_id: uuid!("d0f6ecbe-84a2-41c1-b23d-b20473b25eab"),
                    model: "o3".into(),
                },
            )),
        };
        let observed = to_val(&env);
        let expected = json!({
            "requestId": 1,
            "result": {
                "conversation_id": "d0f6ecbe-84a2-41c1-b23d-b20473b25eab",
                "model": "o3"
            }
        });
        assert_eq!(
            observed, expected,
            "full envelope (ConversationCreate) must match"
        );
    }

    #[test]
    fn envelope_success_conversation_connect_empty_result_object() {
        let env = ToolCallResponseEnvelope {
            request_id: RequestId::Integer(2),
            is_error: None,
            result: Some(ToolCallResponseData::ConversationConnect(
                ConversationConnectResult {},
            )),
        };
        let observed = to_val(&env);
        let expected = json!({
            "requestId": 2,
            "result": {}
        });
        assert_eq!(
            observed, expected,
            "full envelope (ConversationConnect) must have empty object result"
        );
    }

    #[test]
    fn envelope_success_send_message_accepted_full_schema() {
        let env = ToolCallResponseEnvelope {
            request_id: RequestId::Integer(3),
            is_error: None,
            result: Some(ToolCallResponseData::ConversationSendMessage(
                ConversationSendMessageAccepted { accepted: true },
            )),
        };
        let observed = to_val(&env);
        let expected = json!({
            "requestId": 3,
            "result": { "accepted": true }
        });
        assert_eq!(
            observed, expected,
            "full envelope (ConversationSendMessageAccepted) must match"
        );
    }

    #[test]
    fn envelope_success_conversations_list_with_next_cursor_full_schema() {
        let env = ToolCallResponseEnvelope {
            request_id: RequestId::Integer(4),
            is_error: None,
            result: Some(ToolCallResponseData::ConversationsList(
                ConversationsListResult {
                    conversations: vec![ConversationSummary {
                        conversation_id: uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
                        title: "Refactor config loader".into(),
                    }],
                    next_cursor: Some("next123".into()),
                },
            )),
        };
        let observed = to_val(&env);
        let expected = json!({
            "requestId": 4,
            "result": {
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
            "full envelope (ConversationsList with cursor) must match"
        );
    }

    #[test]
    fn envelope_error_only_is_error_and_request_id_string() {
        let env = ToolCallResponseEnvelope {
            request_id: RequestId::Integer(4),
            is_error: Some(true),
            result: None,
        };
        let observed = to_val(&env);
        let expected = json!({
            "requestId": 4,
            "isError": true
        });
        assert_eq!(
            observed, expected,
            "error envelope must omit `result` and include `isError`"
        );
    }

    // ----- Notifications -----

    #[test]
    fn serialize_notification_initial_state_minimal() {
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

        let observed = to_val(&NotificationEnvelope::from(
            ConversationNotificationParams::InitialState(params.clone()),
        ));
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

        let observed = to_val(&NotificationEnvelope::from(
            ConversationNotificationParams::InitialState(params),
        ));
        let expected = json!({
            "method": "notifications/initial_state",
            "params": {
                "initial_state": {}
            }
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_notification_connection_revoked() {
        let params = ConnectionRevokedNotificationParams {
            meta: Some(NotificationMeta {
                conversation_id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")),
                request_id: None,
            }),
            reason: "New connect() took over".into(),
        };

        let observed = to_val(&NotificationEnvelope::from(
            ConversationNotificationParams::ConnectionRevoked(params),
        ));
        let expected = json!({
            "method": "notifications/connection_revoked",
            "params": {
                "_meta": { "conversationId": "67e55044-10b1-426f-9247-bb680e5fe0c8" },
                "reason": "New connect() took over"
            }
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_notification_codex_event_uses_eventmsg_type_in_method() {
        let params = CodexEventNotificationParams {
            meta: Some(NotificationMeta {
                conversation_id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")),
                request_id: Some(RequestId::Integer(44)),
            }),
            msg: EventMsg::AgentMessage(codex_core::protocol::AgentMessageEvent {
                message: "hi".into(),
            }),
        };

        let observed = to_val(&NotificationEnvelope::from(
            ConversationNotificationParams::CodexEvent(params),
        ));
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
                conversation_id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")),
                request_id: Some(RequestId::Integer(7)),
            }),
            msg: EventMsg::TaskStarted,
        };

        let observed = to_val(&NotificationEnvelope::from(
            ConversationNotificationParams::CodexEvent(params),
        ));
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

        let observed = to_val(&NotificationEnvelope::from(
            ConversationNotificationParams::CodexEvent(params),
        ));
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
                conversation_id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")),
                request_id: Some(RequestId::Integer(44)),
            }),
            msg: EventMsg::AgentMessage(codex_core::protocol::AgentMessageEvent {
                message: "hi".into(),
            }),
        };

        let observed = to_val(&NotificationEnvelope::from(
            ConversationNotificationParams::CodexEvent(params),
        ));
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

    // Fallback cases where method should be "notifications/unknown"
    #[test]
    fn serialize_notification_codex_event_agent_reasoning_full_json_unknown() {
        let params = CodexEventNotificationParams {
            meta: None,
            msg: EventMsg::AgentReasoning(codex_core::protocol::AgentReasoningEvent {
                text: "thinking…".into(),
            }),
        };

        let observed = to_val(&NotificationEnvelope::from(
            ConversationNotificationParams::CodexEvent(params),
        ));
        let expected = json!({
            "method": "notifications/unknown",
            "params": {
                "msg": { "type": "agent_reasoning", "text": "thinking…" }
            }
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_notification_codex_event_token_count_full_json_unknown() {
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

        let observed = to_val(&NotificationEnvelope::from(
            ConversationNotificationParams::CodexEvent(params),
        ));
        let expected = json!({
            "method": "notifications/unknown",
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
    fn serialize_notification_codex_event_session_configured_full_json_unknown() {
        let params = CodexEventNotificationParams {
            meta: Some(NotificationMeta {
                conversation_id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")),
                request_id: None,
            }),
            msg: EventMsg::SessionConfigured(codex_core::protocol::SessionConfiguredEvent {
                session_id: uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
                model: "codex-mini-latest".into(),
                history_log_id: 42,
                history_entry_count: 3,
            }),
        };

        let observed = to_val(&NotificationEnvelope::from(
            ConversationNotificationParams::CodexEvent(params),
        ));
        let expected = json!({
            "method": "notifications/unknown",
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
    fn serialize_notification_codex_event_exec_command_begin_full_json_unknown() {
        let params = CodexEventNotificationParams {
            meta: None,
            msg: EventMsg::ExecCommandBegin(codex_core::protocol::ExecCommandBeginEvent {
                call_id: "c1".into(),
                command: vec!["bash".into(), "-lc".into(), "echo hi".into()],
                cwd: std::path::PathBuf::from("/work"),
            }),
        };

        let observed = to_val(&NotificationEnvelope::from(
            ConversationNotificationParams::CodexEvent(params),
        ));
        let expected = json!({
            "method": "notifications/unknown",
            "params": {
                "msg": {
                    "type": "exec_command_begin",
                    "call_id": "c1",
                    "command": ["bash", "-lc", "echo hi"],
                    "cwd": "/work"
                }
            }
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_notification_codex_event_mcp_tool_call_begin_full_json_unknown() {
        let params = CodexEventNotificationParams {
            meta: None,
            msg: EventMsg::McpToolCallBegin(codex_core::protocol::McpToolCallBeginEvent {
                call_id: "m1".into(),
                server: "calc".into(),
                tool: "add".into(),
                arguments: Some(json!({"a":1,"b":2})),
            }),
        };

        let observed = to_val(&NotificationEnvelope::from(
            ConversationNotificationParams::CodexEvent(params),
        ));
        let expected = json!({
            "method": "notifications/unknown",
            "params": {
                "msg": {
                    "type": "mcp_tool_call_begin",
                    "call_id": "m1",
                    "server": "calc",
                    "tool": "add",
                    "arguments": { "a": 1, "b": 2 }
                }
            }
        });
        assert_eq!(observed, expected);
    }

    #[test]
    fn serialize_notification_codex_event_patch_apply_end_full_json_unknown() {
        let params = CodexEventNotificationParams {
            meta: None,
            msg: EventMsg::PatchApplyEnd(codex_core::protocol::PatchApplyEndEvent {
                call_id: "p1".into(),
                stdout: "ok".into(),
                stderr: "".into(),
                success: true,
            }),
        };

        let observed = to_val(&NotificationEnvelope::from(
            ConversationNotificationParams::CodexEvent(params),
        ));
        let expected = json!({
            "method": "notifications/unknown",
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
        let params = CancellNotificationParams {
            request_id: RequestId::String("r-123".into()),
            reason: Some("user_cancelled".into()),
        };

        let observed = to_val(&NotificationEnvelope::from(
            ConversationNotificationParams::Cancelled(params),
        ));
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
        let params = CancellNotificationParams {
            request_id: RequestId::Integer(77),
            reason: None,
        };

        let observed = to_val(&NotificationEnvelope::from(
            ConversationNotificationParams::Cancelled(params),
        ));

        // Check exact structure: reason must be omitted.
        assert_eq!(observed["method"], "notifications/cancelled");
        assert_eq!(observed["params"]["requestId"], 77);
        assert!(
            observed["params"].get("reason").is_none(),
            "reason must be omitted when None"
        );
    }
}
