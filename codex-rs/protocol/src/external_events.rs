use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ExternalEventSeverity {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

impl ExternalEventSeverity {
    pub fn as_label(self) -> &'static str {
        match self {
            ExternalEventSeverity::Debug => "debug",
            ExternalEventSeverity::Info => "info",
            ExternalEventSeverity::Warning => "warning",
            ExternalEventSeverity::Error => "error",
            ExternalEventSeverity::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema, TS)]
pub struct ExternalEvent {
    pub schema_version: u32,
    pub event_id: String,
    pub time_unix_ms: i64,
    #[serde(rename = "type")]
    pub ty: String,
    pub severity: ExternalEventSeverity,
    pub title: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

/// Wrapper type for the `EventMsg::ExternalEvent` payload.
///
/// This wrapper is required because `EventMsg` is serde-tagged with `type`, and `ExternalEvent`
/// itself includes a field named `type`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema, TS)]
pub struct ExternalEventEvent {
    pub event: ExternalEvent,
}
