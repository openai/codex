use codex_protocol::protocol::UsageContributorKind as CoreUsageContributorKind;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub enum UsageRange {
    Day,
    Week,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct UsageReadParams {
    pub range: UsageRange,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct UsageEntry {
    pub kind: UsageContributorKind,
    pub id: String,
    pub label: String,
    #[ts(type = "number")]
    pub attributed_tokens: i64,
    pub percent_of_usage: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct UsageHeadline {
    pub entry: UsageEntry,
    pub note: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct UsageReport {
    pub range: UsageRange,
    #[ts(type = "number")]
    pub generated_at: i64,
    #[ts(type = "number | null")]
    pub tracked_from: Option<i64>,
    #[ts(type = "number")]
    pub total_tokens: i64,
    pub headline: Option<UsageHeadline>,
    pub skills: Vec<UsageEntry>,
    pub subagents: Vec<UsageEntry>,
    pub agent_tasks: Vec<UsageEntry>,
    pub apps: Vec<UsageEntry>,
    pub mcp_servers: Vec<UsageEntry>,
    pub plugins: Vec<UsageEntry>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct UsageReadResponse {
    pub report: UsageReport,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub enum UsageContributorKind {
    Skill,
    Subagent,
    AgentTask,
    App,
    McpServer,
    Plugin,
}

impl From<CoreUsageContributorKind> for UsageContributorKind {
    fn from(value: CoreUsageContributorKind) -> Self {
        match value {
            CoreUsageContributorKind::Skill => Self::Skill,
            CoreUsageContributorKind::Subagent => Self::Subagent,
            CoreUsageContributorKind::AgentTask => Self::AgentTask,
            CoreUsageContributorKind::App => Self::App,
            CoreUsageContributorKind::McpServer => Self::McpServer,
            CoreUsageContributorKind::Plugin => Self::Plugin,
        }
    }
}
