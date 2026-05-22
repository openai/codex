use super::ChatWidget;
use crate::app_event::AppEvent;
use codex_app_server_protocol::UsageContributorKind;
use codex_app_server_protocol::UsageEntry;
use codex_app_server_protocol::UsageRange;
use codex_app_server_protocol::UsageReadResponse;
use codex_app_server_protocol::UsageReport;

impl ChatWidget {
    pub(crate) fn add_usage_output(&mut self) {
        self.request_usage(UsageRange::Day);
    }

    fn request_usage(&mut self, range: UsageRange) {
        let request_id = self.next_usage_request_id;
        self.next_usage_request_id = self.next_usage_request_id.saturating_add(/*rhs*/ 1);
        self.active_usage_request_id = Some(request_id);
        self.app_event_tx
            .send(AppEvent::FetchUsage { request_id, range });
    }

    pub(crate) fn on_usage_loaded(
        &mut self,
        request_id: u64,
        result: Result<UsageReadResponse, String>,
    ) {
        if self.active_usage_request_id != Some(request_id) {
            return;
        }
        self.active_usage_request_id = None;
        let message = match result {
            Ok(response) => render_usage_report(&response.report),
            Err(err) => format!("Usage\n\nFailed to load usage: {err}"),
        };
        self.add_info_message(message, /*hint*/ None);
    }
}

fn render_usage_report(report: &UsageReport) -> String {
    let mut lines = vec!["Usage".to_string()];
    if let Some(headline) = report.headline.as_ref() {
        lines.push(String::new());
        lines.push(format!(
            "{}% of your usage came from {} \"{}\"",
            headline.entry.percent_of_usage,
            contributor_kind_label(headline.entry.kind),
            headline.entry.label
        ));
        if let Some(note) = headline.note.as_ref() {
            lines.push(note.clone());
        }
    }

    if report.total_tokens == 0 {
        lines.push(String::new());
        lines.push("No tracked usage in this range yet.".to_string());
        return lines.join("\n");
    }

    let sections = [
        ("Skills", report.skills.as_slice()),
        ("Subagents", report.subagents.as_slice()),
        ("Apps", report.apps.as_slice()),
        ("MCP servers", report.mcp_servers.as_slice()),
        ("Plugins", report.plugins.as_slice()),
    ];
    if sections.iter().all(|(_, entries)| entries.is_empty()) {
        lines.push(String::new());
        lines.push(
            "No attributed skills, subagents, apps, MCP servers, or plugins in this range."
                .to_string(),
        );
        return lines.join("\n");
    }

    for (label, entries) in sections {
        push_section(&mut lines, label, entries);
    }
    lines.join("\n")
}

fn push_section(lines: &mut Vec<String>, label: &str, entries: &[UsageEntry]) {
    if entries.is_empty() {
        return;
    }
    lines.push(String::new());
    lines.push(label.to_string());
    for entry in entries {
        lines.push(format!(
            "{:<24} {:>3}%",
            entry.label, entry.percent_of_usage
        ));
    }
}

fn contributor_kind_label(kind: UsageContributorKind) -> &'static str {
    match kind {
        UsageContributorKind::Skill => "skill",
        UsageContributorKind::Subagent => "subagent",
        UsageContributorKind::App => "app",
        UsageContributorKind::McpServer => "MCP server",
        UsageContributorKind::Plugin => "plugin",
    }
}
