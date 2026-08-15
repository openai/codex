use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::CodexErrorInfo;
use codex_app_server_protocol::ErrorNotification;
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

    assert_policy_violation_completes_turn_with_typed_terminal_error(response).await
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

    assert_policy_violation_completes_turn_with_typed_terminal_error(response).await
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

    assert_policy_violation_completes_turn_with_typed_terminal_error(response).await
}

async fn assert_policy_violation_completes_turn_with_typed_terminal_error(
    response: ResponseTemplate,
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
            message: MESSAGE.to_string(),
            codex_error_info: Some(CodexErrorInfo::MisalignmentPolicyViolation),
            additional_details: None,
        })
    );
    response_mock.single_request();

    Ok(())
}
