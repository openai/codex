use std::collections::VecDeque;

use codex_protocol::approvals::GuardianAssessmentAction;
use codex_protocol::approvals::GuardianAssessmentEvent;
use codex_protocol::approvals::GuardianAssessmentStatus;

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

    pub(crate) fn take_soft(&mut self, id: &str) -> Option<GuardianAssessmentEvent> {
        let idx = self.entries.iter().position(|entry| entry.id == id)?;
        if !self.entries[idx].is_explicit_retry_eligible() {
            return None;
        }
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

#[cfg(test)]
mod tests {
    use codex_protocol::approvals::GuardianCommandSource;
    use codex_protocol::approvals::GuardianDenialKind;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use pretty_assertions::assert_eq;

    use super::*;

    fn denied_event(id: usize) -> GuardianAssessmentEvent {
        GuardianAssessmentEvent {
            id: format!("review-{id}"),
            target_item_id: None,
            turn_id: "turn-1".to_string(),
            started_at_ms: 0,
            completed_at_ms: Some(1),
            status: GuardianAssessmentStatus::Denied,
            risk_level: None,
            user_authorization: None,
            rationale: Some(format!("rationale {id}")),
            denial_kind: Some(GuardianDenialKind::Soft),
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
    fn only_soft_denials_can_be_taken_for_approval() {
        let mut denials = RecentAutoReviewDenials::default();
        let mut hard = denied_event(1);
        hard.denial_kind = Some(GuardianDenialKind::Hard);
        denials.push(hard);

        assert!(denials.take_soft("review-1").is_none());
        assert_eq!(denials.entries().count(), 1);

        denials.push(denied_event(2));
        assert_eq!(
            denials.take_soft("review-2").map(|event| event.id),
            Some("review-2".to_string())
        );
    }
}
