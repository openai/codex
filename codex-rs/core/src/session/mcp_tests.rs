use super::*;

use codex_config::types::ApprovalsReviewer;
use serde_json::json;

fn empty_form_request() -> codex_protocol::approvals::ElicitationRequest {
    codex_protocol::approvals::ElicitationRequest::Form {
        meta: Some(json!({
            "connector_id": "browser-use",
            "connector_name": "Browser Use",
        })),
        message: "Allow Browser Use to access https://example.com?".to_string(),
        requested_schema: json!({
            "type": "object",
            "properties": {},
        }),
    }
}

#[tokio::test]
async fn mcp_elicitations_route_to_guardian_when_auto_review_is_configured() {
    let (_session, mut turn_context) = crate::session::tests::make_session_and_context().await;
    let mut config = (*turn_context.config).clone();
    config.approvals_reviewer = ApprovalsReviewer::AutoReview;
    turn_context.config = Arc::new(config);

    assert!(should_route_mcp_elicitation_to_guardian(
        &turn_context,
        &empty_form_request()
    ));
}

#[tokio::test]
async fn mcp_elicitations_do_not_route_to_guardian_for_manual_approvals() {
    let (_session, mut turn_context) = crate::session::tests::make_session_and_context().await;
    let mut config = (*turn_context.config).clone();
    config.approvals_reviewer = ApprovalsReviewer::User;
    turn_context.config = Arc::new(config);

    assert!(!should_route_mcp_elicitation_to_guardian(
        &turn_context,
        &empty_form_request()
    ));
}
