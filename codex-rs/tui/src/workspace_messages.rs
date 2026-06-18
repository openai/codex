use codex_app_server_protocol::GetWorkspaceMessagesResponse;
use codex_app_server_protocol::WorkspaceMessageType;
use codex_protocol::account::PlanType;
use std::time::Duration;

pub(crate) const WORKSPACE_HEADLINE_REFRESH_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WorkspaceHeadlineFetchResult {
    Available(Option<String>),
    FeatureDisabled,
}

pub(crate) fn workspace_headline_from_response(
    response: GetWorkspaceMessagesResponse,
) -> WorkspaceHeadlineFetchResult {
    if !response.feature_enabled {
        return WorkspaceHeadlineFetchResult::FeatureDisabled;
    }

    WorkspaceHeadlineFetchResult::Available(response.messages.into_iter().find_map(|message| {
        (message.message_type == WorkspaceMessageType::Headline)
            .then(|| message.message_body.trim().to_string())
            .filter(|headline| !headline.is_empty())
    }))
}

pub(crate) fn plan_type_allows_workspace_headline(plan_type: Option<PlanType>) -> bool {
    matches!(
        plan_type,
        Some(PlanType::Enterprise | PlanType::EnterpriseCbpUsageBased)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::WorkspaceMessage;
    use pretty_assertions::assert_eq;

    #[test]
    fn workspace_headline_from_response_uses_first_non_empty_headline() {
        let response = GetWorkspaceMessagesResponse {
            feature_enabled: true,
            messages: vec![
                WorkspaceMessage {
                    message_id: "announcement-id".to_string(),
                    message_type: WorkspaceMessageType::Announcement,
                    message_body: "Announcement body".to_string(),
                    created_at: None,
                    archived_at: None,
                },
                WorkspaceMessage {
                    message_id: "empty-headline-id".to_string(),
                    message_type: WorkspaceMessageType::Headline,
                    message_body: "   ".to_string(),
                    created_at: None,
                    archived_at: None,
                },
                WorkspaceMessage {
                    message_id: "headline-id".to_string(),
                    message_type: WorkspaceMessageType::Headline,
                    message_body: " Workspace headline ".to_string(),
                    created_at: None,
                    archived_at: None,
                },
            ],
        };

        assert_eq!(
            workspace_headline_from_response(response),
            WorkspaceHeadlineFetchResult::Available(Some("Workspace headline".to_string()))
        );
    }

    #[test]
    fn workspace_headline_from_response_reports_feature_disabled() {
        let response = GetWorkspaceMessagesResponse {
            feature_enabled: false,
            messages: Vec::new(),
        };

        assert_eq!(
            workspace_headline_from_response(response),
            WorkspaceHeadlineFetchResult::FeatureDisabled
        );
    }

    #[test]
    fn workspace_headline_plan_gate_allows_enterprise_plans_only() {
        let cases = [
            (Some(PlanType::Enterprise), true),
            (Some(PlanType::EnterpriseCbpUsageBased), true),
            (Some(PlanType::Business), false),
            (Some(PlanType::Team), false),
            (Some(PlanType::Edu), false),
            (Some(PlanType::Pro), false),
            (None, false),
        ];

        for (plan_type, expected) in cases {
            assert_eq!(plan_type_allows_workspace_headline(plan_type), expected);
        }
    }
}
