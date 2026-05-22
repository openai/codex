use super::*;
use crate::error_code::internal_error;
use chrono::Utc;
use codex_app_server_protocol::UsageEntry;
use codex_app_server_protocol::UsageHeadline;
use codex_app_server_protocol::UsageRange;
use codex_app_server_protocol::UsageReadParams;
use codex_app_server_protocol::UsageReadResponse;
use codex_app_server_protocol::UsageReport;
use codex_rollout::StateDbHandle;

#[derive(Clone)]
pub(crate) struct UsageRequestProcessor {
    state_db: Option<StateDbHandle>,
}

impl UsageRequestProcessor {
    pub(crate) fn new(state_db: Option<StateDbHandle>) -> Self {
        Self { state_db }
    }

    pub(crate) async fn usage_read(
        &self,
        params: UsageReadParams,
    ) -> Result<UsageReadResponse, JSONRPCErrorError> {
        let state_db = self
            .state_db
            .as_ref()
            .ok_or_else(|| internal_error("sqlite state db unavailable for usage"))?;
        let report = state_db
            .read_usage_report(state_usage_range(params.range), Utc::now().timestamp())
            .await
            .map_err(|err| internal_error(format!("failed to read usage report: {err}")))?;
        Ok(UsageReadResponse {
            report: UsageReport {
                range: api_usage_range(report.range),
                generated_at: report.generated_at,
                tracked_from: report.tracked_from,
                total_tokens: report.total_tokens,
                headline: report.headline.map(|headline| UsageHeadline {
                    entry: usage_entry(headline.entry),
                    note: headline.note,
                }),
                skills: report.skills.into_iter().map(usage_entry).collect(),
                subagents: report.subagents.into_iter().map(usage_entry).collect(),
                agent_tasks: report.agent_tasks.into_iter().map(usage_entry).collect(),
                apps: report.apps.into_iter().map(usage_entry).collect(),
                mcp_servers: report.mcp_servers.into_iter().map(usage_entry).collect(),
                plugins: report.plugins.into_iter().map(usage_entry).collect(),
            },
        })
    }
}

fn usage_entry(entry: codex_state::UsageEntry) -> UsageEntry {
    UsageEntry {
        kind: entry.kind.into(),
        id: entry.id,
        label: entry.label,
        attributed_tokens: entry.attributed_tokens,
        percent_of_usage: entry.percent_of_usage,
    }
}

fn state_usage_range(value: UsageRange) -> codex_state::UsageRange {
    match value {
        UsageRange::Day => codex_state::UsageRange::Day,
        UsageRange::Week => codex_state::UsageRange::Week,
    }
}

fn api_usage_range(value: codex_state::UsageRange) -> UsageRange {
    match value {
        codex_state::UsageRange::Day => UsageRange::Day,
        codex_state::UsageRange::Week => UsageRange::Week,
    }
}
