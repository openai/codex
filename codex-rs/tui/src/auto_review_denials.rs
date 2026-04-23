use std::collections::VecDeque;

use codex_protocol::protocol::GuardianAssessmentAction;
use codex_protocol::protocol::GuardianAssessmentEvent;
use codex_protocol::protocol::GuardianAssessmentStatus;

const MAX_RECENT_DENIALS: usize = 10;

#[derive(Debug, Default)]
pub(crate) struct RecentAutoReviewDenials {
    entries: VecDeque<GuardianAssessmentEvent>,
}

impl RecentAutoReviewDenials {
    pub(crate) fn push(&mut self, event: GuardianAssessmentEvent) {
        if event.status != GuardianAssessmentStatus::Denied {
            return;
        }

        self.entries.retain(|entry| entry.id != event.id);
        self.entries.push_front(event);
        self.entries.truncate(MAX_RECENT_DENIALS);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn entries(&self) -> impl Iterator<Item = &GuardianAssessmentEvent> {
        self.entries.iter()
    }

    pub(crate) fn take(&mut self, id: &str) -> Option<GuardianAssessmentEvent> {
        let idx = self.entries.iter().position(|entry| entry.id == id)?;
        self.entries.remove(idx)
    }
}

pub(crate) fn action_summary(action: &GuardianAssessmentAction) -> String {
    match action {
        GuardianAssessmentAction::Command { command, .. } => command.clone(),
        GuardianAssessmentAction::Execve { program, argv, .. } => {
            let command = if argv.is_empty() {
                vec![program.clone()]
            } else {
                argv.clone()
            };
            shlex::try_join(command.iter().map(String::as_str))
                .unwrap_or_else(|_| command.join(" "))
        }
        GuardianAssessmentAction::ApplyPatch { files, .. } => {
            if files.len() == 1 {
                format!("apply_patch touching {}", files[0].display())
            } else {
                format!("apply_patch touching {} files", files.len())
            }
        }
        GuardianAssessmentAction::NetworkAccess { target, .. } => {
            format!("network access to {target}")
        }
        GuardianAssessmentAction::McpToolCall {
            server,
            tool_name,
            connector_name,
            ..
        } => {
            let label = connector_name.as_deref().unwrap_or(server.as_str());
            format!("MCP {tool_name} on {label}")
        }
        GuardianAssessmentAction::RequestPermissions { reason, .. } => reason
            .as_deref()
            .map(|reason| format!("permission request: {reason}"))
            .unwrap_or_else(|| "permission request".to_string()),
    }
}

pub(crate) fn action_detail(action: &GuardianAssessmentAction) -> String {
    match action {
        GuardianAssessmentAction::Command { command, cwd, .. } => {
            format!("Command: {command}\nCwd: {}", cwd.display())
        }
        GuardianAssessmentAction::Execve {
            program, argv, cwd, ..
        } => {
            format!(
                "Program: {program}\nArgv: {}\nCwd: {}",
                serde_json::to_string(argv).unwrap_or_else(|_| "[]".to_string()),
                cwd.display()
            )
        }
        GuardianAssessmentAction::ApplyPatch { cwd, files } => {
            let files = files
                .iter()
                .map(|file| file.display().to_string())
                .collect::<Vec<_>>()
                .join("\n");
            format!("Apply patch\nCwd: {}\nFiles:\n{files}", cwd.display())
        }
        GuardianAssessmentAction::NetworkAccess {
            target,
            host,
            protocol,
            port,
        } => {
            format!("Network: {target}\nHost: {host}\nProtocol: {protocol:?}\nPort: {port}")
        }
        GuardianAssessmentAction::McpToolCall {
            server,
            tool_name,
            connector_id,
            connector_name,
            tool_title,
        } => {
            format!(
                "MCP tool: {server}.{tool_name}\nConnector id: {}\nConnector name: {}\nTool title: {}",
                connector_id.as_deref().unwrap_or("none"),
                connector_name.as_deref().unwrap_or("none"),
                tool_title.as_deref().unwrap_or("none")
            )
        }
        GuardianAssessmentAction::RequestPermissions {
            reason,
            permissions,
        } => {
            let permissions =
                serde_json::to_string_pretty(permissions).unwrap_or_else(|_| "{}".to_string());
            format!(
                "Permission request\nReason: {}\nPermissions: {permissions}",
                reason.as_deref().unwrap_or("none")
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use codex_protocol::protocol::GuardianCommandSource;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use pretty_assertions::assert_eq;

    use super::*;

    fn denied_event(id: usize) -> GuardianAssessmentEvent {
        GuardianAssessmentEvent {
            id: format!("review-{id}"),
            target_item_id: None,
            turn_id: "turn-1".to_string(),
            status: GuardianAssessmentStatus::Denied,
            risk_level: None,
            user_authorization: None,
            rationale: Some(format!("rationale {id}")),
            decision_source: None,
            action: GuardianAssessmentAction::Command {
                source: GuardianCommandSource::Shell,
                command: format!("rm -rf /tmp/test-{id}"),
                cwd: test_path_buf("/tmp").abs(),
            },
        }
    }

    #[test]
    fn keeps_only_ten_most_recent_denials() {
        let mut denials = RecentAutoReviewDenials::default();
        for id in 0..12 {
            denials.push(denied_event(id));
        }

        let ids = denials
            .entries()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec![
                "review-11",
                "review-10",
                "review-9",
                "review-8",
                "review-7",
                "review-6",
                "review-5",
                "review-4",
                "review-3",
                "review-2",
            ]
        );
    }

    #[test]
    fn take_consumes_denial_once() {
        let mut denials = RecentAutoReviewDenials::default();
        denials.push(denied_event(/*id*/ 1));

        assert_eq!(
            denials.take("review-1").map(|event| event.id),
            Some("review-1".to_string())
        );
        assert_eq!(denials.take("review-1"), None);
        assert!(denials.is_empty());
    }
}
