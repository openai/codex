use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::CodexErrorInfo;
use codex_app_server_protocol::ErrorNotification;
use codex_app_server_protocol::MisalignmentErrorDetails;
use codex_app_server_protocol::MisalignmentSteer;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnError;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::ResponseTemplate;

const MESSAGE: &str = "This request violated the misalignment policy.";
const EXPLANATION: &str = "The agent attempted to transfer your files externally.";
const STEER: &str = "Continue without transferring the user's files externally.";
const REVIEW_TARGET: &str = "RB._~:-opaque";

#[tokio::test]
async fn streamed_policy_violation_completes_turn_with_typed_terminal_error() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let response = responses::sse_response(responses::sse(vec![
        responses::ev_response_created("resp-misalignment"),
        serde_json::json!({
            "type": "response.failed",
            "response": {
                "id": "resp-misalignment",
                "status": "failed",
                "error": {
                    "type": "invalid_request_error",
                    "code": "misalignment_policy_violation",
                    "message": MESSAGE
                }
            }
        }),
    ]));

    assert_policy_violation_completes_turn_with_typed_terminal_error(
        response, /*expected_misalignment*/ None,
    )
    .await
}

#[tokio::test]
async fn http_400_policy_violation_completes_turn_with_typed_terminal_error() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let response = ResponseTemplate::new(400).set_body_json(serde_json::json!({
        "error": {
            "type": "invalid_request_error",
            "code": "misalignment_policy_violation",
            "message": MESSAGE,
        }
    }));

    assert_policy_violation_completes_turn_with_typed_terminal_error(
        response, /*expected_misalignment*/ None,
    )
    .await
}

#[tokio::test]
async fn http_403_policy_violation_completes_turn_with_typed_terminal_error() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let response = ResponseTemplate::new(403).set_body_json(serde_json::json!({
        "error": {
            "type": "invalid_request_error",
            "code": "misalignment_policy_violation",
            "message": MESSAGE,
        }
    }));

    assert_policy_violation_completes_turn_with_typed_terminal_error(
        response, /*expected_misalignment*/ None,
    )
    .await
}

#[tokio::test]
async fn streamed_policy_violation_exposes_resumable_misalignment_details() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let response = responses::sse_response(responses::sse(vec![
        responses::ev_response_created("resp-misalignment"),
        serde_json::json!({
            "type": "response.failed",
            "response": {
                "id": "resp-misalignment",
                "status": "failed",
                "error": {
                    "code": "misalignment_policy_violation",
                    "message": MESSAGE,
                    "misalignment": {
                        "error_type": "unauthorized_data_transfer",
                        "detailed_explanation": EXPLANATION,
                        "review_target": REVIEW_TARGET,
                        "steer": { "message": STEER }
                    }
                }
            }
        }),
    ]));

    assert_policy_violation_completes_turn_with_typed_terminal_error(
        response,
        Some(resumable_misalignment()),
    )
    .await
}

#[tokio::test]
async fn http_403_policy_violation_exposes_resumable_misalignment_details() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let response = ResponseTemplate::new(403).set_body_json(serde_json::json!({
        "error": {
            "code": "misalignment_policy_violation",
            "message": MESSAGE,
            "misalignment": {
                "error_type": "unauthorized_data_transfer",
                "detailed_explanation": EXPLANATION,
                "review_target": REVIEW_TARGET,
                "steer": { "message": STEER }
            }
        }
    }));

    assert_policy_violation_completes_turn_with_typed_terminal_error(
        response,
        Some(resumable_misalignment()),
    )
    .await
}

#[tokio::test]
async fn classification_only_policy_violation_preserves_the_terminal_error() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let response = responses::sse_response(responses::sse(vec![
        responses::ev_response_created("resp-misalignment"),
        serde_json::json!({
            "type": "response.failed",
            "response": {
                "id": "resp-misalignment",
                "status": "failed",
                "error": {
                    "code": "misalignment_policy_violation",
                    "message": MESSAGE,
                    "misalignment": { "error_type": "unsafe_activity" }
                }
            }
        }),
    ]));

    assert_policy_violation_completes_turn_with_typed_terminal_error(
        response,
        Some(MisalignmentErrorDetails {
            error_type: Some("unsafe_activity".to_string()),
            review_target: None,
            detailed_explanation: None,
            steer: None,
        }),
    )
    .await
}

async fn assert_policy_violation_completes_turn_with_typed_terminal_error(
    response: ResponseTemplate,
    expected_misalignment: Option<MisalignmentErrorDetails>,
) -> Result<()> {
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_response_once(&server, response).await;

    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;

    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let ThreadStartResponse { thread, .. } = app_server
        .start_thread(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let TurnStartResponse { turn } = app_server
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread.id.clone(),
                client_user_message_id: None,
                input: vec![UserInput::Text {
                    text: "trigger policy violation".to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;

    let error: ErrorNotification = timeout(
        std::time::Duration::from_secs(10),
        app_server.read_notification("error"),
    )
    .await??;
    assert_eq!(
        error,
        ErrorNotification {
            error: TurnError {
                misalignment: expected_misalignment.clone(),
                message: MESSAGE.to_string(),
                codex_error_info: Some(CodexErrorInfo::MisalignmentPolicyViolation),
                additional_details: None,
            },
            will_retry: false,
            thread_id: thread.id.clone(),
            turn_id: turn.id.clone(),
        }
    );

    let completed: TurnCompletedNotification = timeout(
        std::time::Duration::from_secs(10),
        app_server.read_notification("turn/completed"),
    )
    .await??;
    assert_eq!(completed.thread_id, thread.id);
    assert_eq!(completed.turn.id, turn.id);
    assert_eq!(completed.turn.status, TurnStatus::Failed);
    assert_eq!(
        completed.turn.error,
        Some(TurnError {
            misalignment: expected_misalignment,
            message: MESSAGE.to_string(),
            codex_error_info: Some(CodexErrorInfo::MisalignmentPolicyViolation),
            additional_details: None,
        })
    );
    response_mock.single_request();

    let rollout = tokio::fs::read_to_string(
        thread
            .path
            .as_ref()
            .expect("non-ephemeral thread should have a rollout path"),
    )
    .await?;
    assert!(!rollout.contains(EXPLANATION));
    assert!(!rollout.contains(STEER));
    assert!(!rollout.contains(REVIEW_TARGET));

    Ok(())
}

fn resumable_misalignment() -> MisalignmentErrorDetails {
    MisalignmentErrorDetails {
        error_type: Some("unauthorized_data_transfer".to_string()),
        review_target: Some(REVIEW_TARGET.to_string()),
        detailed_explanation: Some(EXPLANATION.to_string()),
        steer: Some(MisalignmentSteer {
            message: STEER.to_string(),
        }),
    }
}
