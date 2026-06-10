use super::*;
use codex_protocol::protocol::GuardianAssessmentDecisionSource;
use codex_protocol::protocol::GuardianAssessmentStatus;
use pretty_assertions::assert_eq;

#[test]
fn guardian_notification_preserves_organization_policy_decision_source() {
    let thread_id = ThreadId::new();
    let notification = guardian_auto_approval_review_notification(
        &thread_id,
        "event-turn",
        &GuardianAssessmentEvent {
            id: "guardian-1".to_string(),
            target_item_id: None,
            turn_id: "assessment-turn".to_string(),
            started_at_ms: 10,
            completed_at_ms: Some(20),
            status: GuardianAssessmentStatus::Approved,
            risk_level: None,
            user_authorization: None,
            rationale: Some("Organization policy allowed the request to proceed.".to_string()),
            decision_source: Some(GuardianAssessmentDecisionSource::OrganizationPolicy),
            action: GuardianAssessmentAction::McpToolCall {
                server: "github".to_string(),
                tool_name: "create_pull_request".to_string(),
                connector_id: None,
                connector_name: None,
                tool_title: None,
            },
        },
    );

    let ServerNotification::ItemGuardianApprovalReviewCompleted(notification) = notification else {
        panic!("expected completed Guardian review notification");
    };
    assert_eq!(
        notification.decision_source,
        AutoReviewDecisionSource::OrganizationPolicy
    );
}
