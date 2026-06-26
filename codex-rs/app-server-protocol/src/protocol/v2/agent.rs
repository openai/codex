use codex_protocol::protocol::AgentCommunicationKind as CoreAgentCommunicationKind;
use codex_protocol::protocol::AgentCommunicationRecord as CoreAgentCommunicationRecord;
use codex_protocol::protocol::AgentCommunicationState as CoreAgentCommunicationState;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum AgentCommunicationKind {
    InitialTask,
    Message,
    Followup,
    Result,
}

impl From<CoreAgentCommunicationKind> for AgentCommunicationKind {
    fn from(value: CoreAgentCommunicationKind) -> Self {
        match value {
            CoreAgentCommunicationKind::InitialTask => Self::InitialTask,
            CoreAgentCommunicationKind::Message => Self::Message,
            CoreAgentCommunicationKind::Followup => Self::Followup,
            CoreAgentCommunicationKind::Result => Self::Result,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum AgentCommunicationState {
    Created,
    Enqueued,
}

impl From<CoreAgentCommunicationState> for AgentCommunicationState {
    fn from(value: CoreAgentCommunicationState) -> Self {
        match value {
            CoreAgentCommunicationState::Created => Self::Created,
            CoreAgentCommunicationState::Enqueued => Self::Enqueued,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AgentCommunication {
    pub id: String,
    pub kind: AgentCommunicationKind,
    pub state: AgentCommunicationState,
    pub sender_thread_id: String,
    pub receiver_thread_id: String,
    pub content: String,
    #[schemars(required, schema_with = "nullable_string_schema")]
    pub source_call_id: Option<String>,
    pub occurred_at_ms: i64,
}

fn nullable_string_schema(
    generator: &mut schemars::r#gen::SchemaGenerator,
) -> schemars::schema::Schema {
    generator.subschema_for::<Option<String>>()
}

impl From<CoreAgentCommunicationRecord> for AgentCommunication {
    fn from(value: CoreAgentCommunicationRecord) -> Self {
        Self {
            id: value.id,
            kind: value.kind.into(),
            state: value.state.into(),
            sender_thread_id: value.sender_thread_id.to_string(),
            receiver_thread_id: value.receiver_thread_id.to_string(),
            content: value.content,
            source_call_id: value.source_call_id,
            occurred_at_ms: value.occurred_at_ms,
        }
    }
}
