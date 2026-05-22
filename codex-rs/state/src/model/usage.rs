use codex_protocol::protocol::UsageAttributionItem;
use codex_protocol::protocol::UsageContributorKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageRange {
    Day,
    Week,
}

impl UsageRange {
    pub(crate) fn seconds(self) -> i64 {
        match self {
            Self::Day => 24 * 60 * 60,
            Self::Week => 7 * 24 * 60 * 60,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageEntry {
    pub kind: UsageContributorKind,
    pub id: String,
    pub label: String,
    pub attributed_tokens: i64,
    pub percent_of_usage: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageHeadline {
    pub entry: UsageEntry,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageReport {
    pub range: UsageRange,
    pub generated_at: i64,
    pub tracked_from: Option<i64>,
    pub total_tokens: i64,
    pub headline: Option<UsageHeadline>,
    pub skills: Vec<UsageEntry>,
    pub subagents: Vec<UsageEntry>,
    pub agent_tasks: Vec<UsageEntry>,
    pub apps: Vec<UsageEntry>,
    pub mcp_servers: Vec<UsageEntry>,
    pub plugins: Vec<UsageEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageSample {
    pub thread_id: codex_protocol::ThreadId,
    pub attribution: UsageAttributionItem,
}
