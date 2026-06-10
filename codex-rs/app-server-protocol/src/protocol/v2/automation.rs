use codex_protocol::openai_models::ReasoningEffort;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", tag = "type")]
#[ts(export_to = "v2/", rename_all = "camelCase", tag = "type")]
/// EXPERIMENTAL - typed automation target for recurring scheduled work.
pub enum AutomationTarget {
    Cron {
        cwds: Vec<PathBuf>,
    },
    Heartbeat {
        #[serde(rename = "threadId")]
        #[ts(rename = "threadId")]
        thread_id: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[ts(export_to = "v2/")]
pub enum AutomationStatus {
    #[serde(rename = "ACTIVE")]
    #[ts(rename = "ACTIVE")]
    Active,
    #[serde(rename = "PAUSED")]
    #[ts(rename = "PAUSED")]
    Paused,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// EXPERIMENTAL - durable recurring automation or heartbeat.
pub struct Automation {
    pub id: String,
    pub name: String,
    pub prompt: String,
    pub status: AutomationStatus,
    pub rrule: String,
    pub next_run_at: Option<i64>,
    pub last_run_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub model: Option<String>,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub target: AutomationTarget,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// EXPERIMENTAL - list recurring automations.
pub struct AutomationListParams {
    /// Opaque pagination cursor returned by a previous call.
    #[ts(optional = nullable)]
    pub cursor: Option<String>,
    /// Optional page size; defaults to a reasonable server-side value.
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// EXPERIMENTAL - paginated automation list response.
pub struct AutomationListResponse {
    pub data: Vec<Automation>,
    pub next_cursor: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// EXPERIMENTAL - read one automation by id.
pub struct AutomationReadParams {
    pub automation_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AutomationReadResponse {
    pub automation: Option<Automation>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// EXPERIMENTAL - create a recurring automation or heartbeat.
pub struct AutomationCreateParams {
    pub name: String,
    pub prompt: String,
    #[ts(optional = nullable)]
    pub rrule: Option<String>,
    #[ts(optional = nullable)]
    pub model: Option<String>,
    #[ts(optional = nullable)]
    pub reasoning_effort: Option<ReasoningEffort>,
    #[ts(optional = nullable)]
    pub status: Option<AutomationStatus>,
    pub target: AutomationTarget,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AutomationCreateResponse {
    pub automation: Automation,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// EXPERIMENTAL - update an existing automation.
pub struct AutomationUpdateParams {
    pub automation_id: String,
    #[ts(optional = nullable)]
    pub name: Option<String>,
    #[ts(optional = nullable)]
    pub prompt: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional = nullable)]
    pub rrule: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional = nullable)]
    pub model: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional = nullable)]
    pub reasoning_effort: Option<Option<ReasoningEffort>>,
    #[ts(optional = nullable)]
    pub status: Option<AutomationStatus>,
    #[ts(optional = nullable)]
    pub target: Option<AutomationTarget>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AutomationUpdateResponse {
    pub automation: Option<Automation>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// EXPERIMENTAL - delete an automation by id.
pub struct AutomationDeleteParams {
    pub automation_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AutomationDeleteResponse {
    pub deleted: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
/// EXPERIMENTAL - start an immediate run for an automation.
pub struct AutomationRunNowParams {
    pub automation_id: String,
    #[ts(optional = nullable)]
    pub thread_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AutomationRunNowResponse {
    pub found: bool,
    pub started_count: u32,
}

#[cfg(test)]
#[path = "automation_tests.rs"]
mod tests;
