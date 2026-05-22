use super::*;
use crate::elicitation::elicitation_is_rejected_by_policy;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::GranularApprovalConfig;
use codex_rmcp_client::ElicitationResponse;
use pretty_assertions::assert_eq;
use rmcp::model::CreateElicitationRequestParams;
use rmcp::model::ElicitationAction;
use rmcp::model::NumberOrString;

#[test]
fn elicitation_granular_policy_defaults_to_prompting() {
    assert!(!elicitation_is_rejected_by_policy(
        AskForApproval::OnFailure
    ));
    assert!(!elicitation_is_rejected_by_policy(
        AskForApproval::OnRequest
    ));
    assert!(!elicitation_is_rejected_by_policy(
        AskForApproval::UnlessTrusted
    ));
    assert!(elicitation_is_rejected_by_policy(AskForApproval::Granular(
        GranularApprovalConfig {
            sandbox_approval: true,
            rules: true,
            skill_approval: true,
            request_permissions: true,
            mcp_elicitations: false,
        }
    )));
}

#[test]
fn elicitation_granular_policy_respects_never_and_config() {
    assert!(elicitation_is_rejected_by_policy(AskForApproval::Never));
    assert!(elicitation_is_rejected_by_policy(AskForApproval::Granular(
        GranularApprovalConfig {
            sandbox_approval: true,
            rules: true,
            skill_approval: true,
            request_permissions: true,
            mcp_elicitations: false,
        }
    )));
}

#[tokio::test]
async fn disabled_permissions_auto_accept_elicitation_with_empty_form_schema() {
    let manager = ElicitationRequestManager::new(
        AskForApproval::Never,
        PermissionProfile::Disabled,
        /*reviewer*/ None,
    );
    let (tx_event, _rx_event) = async_channel::bounded(1);
    let sender = manager.make_sender("server".to_string(), tx_event);

    let response = sender(
        NumberOrString::Number(1),
        CreateElicitationRequestParams::FormElicitationParams {
            meta: None,
            message: "Confirm?".to_string(),
            requested_schema: rmcp::model::ElicitationSchema::builder()
                .build()
                .expect("schema should build"),
        },
    )
    .await
    .expect("elicitation should auto accept");

    assert_eq!(
        response,
        ElicitationResponse {
            action: ElicitationAction::Accept,
            content: Some(serde_json::json!({})),
            meta: None,
        }
    );
}

#[tokio::test]
async fn disabled_permissions_do_not_auto_accept_elicitation_with_requested_fields() {
    let manager = ElicitationRequestManager::new(
        AskForApproval::Never,
        PermissionProfile::Disabled,
        /*reviewer*/ None,
    );
    let (tx_event, _rx_event) = async_channel::bounded(1);
    let sender = manager.make_sender("server".to_string(), tx_event);

    let response = sender(
        NumberOrString::Number(1),
        CreateElicitationRequestParams::FormElicitationParams {
            meta: None,
            message: "What should I say?".to_string(),
            requested_schema: rmcp::model::ElicitationSchema::builder()
                .required_property(
                    "message",
                    rmcp::model::PrimitiveSchema::String(rmcp::model::StringSchema::new()),
                )
                .build()
                .expect("schema should build"),
        },
    )
    .await
    .expect("elicitation should auto decline");

    assert_eq!(
        response,
        ElicitationResponse {
            action: ElicitationAction::Decline,
            content: None,
            meta: None,
        }
    );
}
