use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::CodexErrorInfo;
use codex_app_server_protocol::ErrorNotification;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::ModelRerouteReason;
use codex_app_server_protocol::ModelReroutedNotification;
use codex_app_server_protocol::ModelVerification;
use codex_app_server_protocol::ModelVerificationNotification;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::ResponseTemplate;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const REQUESTED_MODEL: &str = "gpt-5.4";
const SERVER_MODEL: &str = "gpt-5.3-codex";
const OPENAI_MODEL_VERIFICATION_HEADER: &str = "OpenAI-Verification-Recommendation";
const TRUSTED_ACCESS_FOR_CYBER_VERIFICATION: &str = "trusted_access_for_cyber";
const CYBER_POLICY_MESSAGE: &str =
    "This request has been flagged for potentially high-risk cyber activity.";

#[tokio::test]
async fn openai_model_header_mismatch_emits_model_rerouted_notification_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "Done"),
        responses::ev_completed("resp-1"),
    ]);
    let response = responses::sse_response(body).insert_header("OpenAI-Model", SERVER_MODEL);
    let _response_mock = responses::mount_response_once(&server, response).await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some(REQUESTED_MODEL.to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "trigger safeguard".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let turn_start: TurnStartResponse = to_response(turn_resp)?;

    let rerouted = collect_turn_notifications_and_validate_no_warning_item(&mut mcp).await?;
    assert_eq!(
        rerouted,
        ModelReroutedNotification {
            thread_id: thread.id,
            turn_id: turn_start.turn.id,
            from_model: REQUESTED_MODEL.to_string(),
            to_model: SERVER_MODEL.to_string(),
            reason: ModelRerouteReason::HighRiskCyberActivity,
        }
    );

    Ok(())
}

#[tokio::test]
async fn cyber_policy_response_emits_typed_error_notification_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response = ResponseTemplate::new(400).set_body_json(serde_json::json!({
        "error": {
            "message": CYBER_POLICY_MESSAGE,
            "type": "invalid_request",
            "param": null,
            "code": "cyber_policy"
        }
    }));
    let _response_mock = responses::mount_response_once(&server, response).await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some(REQUESTED_MODEL.to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "trigger cyber policy error".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let turn_start: TurnStartResponse = to_response(turn_resp)?;

    let error = collect_cyber_policy_error_and_validate_no_reroute(&mut mcp).await?;
    assert_eq!(
        error,
        ErrorNotification {
            error: codex_app_server_protocol::TurnError {
                message: CYBER_POLICY_MESSAGE.to_string(),
                codex_error_info: Some(CodexErrorInfo::CyberPolicy),
                additional_details: None,
            },
            will_retry: false,
            thread_id: thread.id,
            turn_id: turn_start.turn.id,
        }
    );

    Ok(())
}

#[tokio::test]
async fn response_model_field_mismatch_emits_model_rerouted_notification_v2_when_header_matches_requested()
-> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        serde_json::json!({
            "type": "response.created",
            "response": {
                "id": "resp-1",
                "headers": {
                    "OpenAI-Model": SERVER_MODEL
                }
            }
        }),
        responses::ev_assistant_message("msg-1", "Done"),
        responses::ev_completed("resp-1"),
    ]);
    let response = responses::sse_response(body).insert_header("OpenAI-Model", REQUESTED_MODEL);
    let _response_mock = responses::mount_response_once(&server, response).await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some(REQUESTED_MODEL.to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "trigger response model check".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let turn_start: TurnStartResponse = to_response(turn_resp)?;

    let rerouted = collect_turn_notifications_and_validate_no_warning_item(&mut mcp).await?;
    assert_eq!(
        rerouted,
        ModelReroutedNotification {
            thread_id: thread.id,
            turn_id: turn_start.turn.id,
            from_model: REQUESTED_MODEL.to_string(),
            to_model: SERVER_MODEL.to_string(),
            reason: ModelRerouteReason::HighRiskCyberActivity,
        }
    );

    Ok(())
}

#[tokio::test]
async fn model_verification_emits_typed_notification_and_warning_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "Done"),
        responses::ev_completed("resp-1"),
    ]);
    let response = responses::sse_response(body).insert_header(
        OPENAI_MODEL_VERIFICATION_HEADER,
        TRUSTED_ACCESS_FOR_CYBER_VERIFICATION,
    );
    let _response_mock = responses::mount_response_once(&server, response).await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some(REQUESTED_MODEL.to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "trigger model verification".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let turn_start: TurnStartResponse = to_response(turn_resp)?;

    let verification =
        collect_model_verification_notifications_and_validate_no_warning_item(&mut mcp).await?;
    assert_eq!(
        verification,
        ModelVerificationNotification {
            thread_id: thread.id,
            turn_id: turn_start.turn.id,
            verifications: vec![ModelVerification::TrustedAccessForCyber],
        }
    );

    Ok(())
}

async fn collect_turn_notifications_and_validate_no_warning_item(
    mcp: &mut McpProcess,
) -> Result<ModelReroutedNotification> {
    let mut rerouted = None;

    loop {
        let message = timeout(DEFAULT_READ_TIMEOUT, mcp.read_next_message()).await??;
        let JSONRPCMessage::Notification(notification) = message else {
            continue;
        };
        match notification.method.as_str() {
            "model/rerouted" => {
                let params = notification.params.ok_or_else(|| {
                    anyhow::anyhow!("model/rerouted notifications must include params")
                })?;
                let payload: ModelReroutedNotification = serde_json::from_value(params)?;
                rerouted = Some(payload);
            }
            "item/started" => {
                let params = notification.params.ok_or_else(|| {
                    anyhow::anyhow!("item/started notifications must include params")
                })?;
                let payload: ItemStartedNotification = serde_json::from_value(params)?;
                assert!(!is_warning_user_message_item(&payload.item));
            }
            "item/completed" => {
                let params = notification.params.ok_or_else(|| {
                    anyhow::anyhow!("item/completed notifications must include params")
                })?;
                let payload: ItemCompletedNotification = serde_json::from_value(params)?;
                assert!(!is_warning_user_message_item(&payload.item));
            }
            "turn/completed" => {
                return rerouted.ok_or_else(|| {
                    anyhow::anyhow!("expected model/rerouted notification before turn/completed")
                });
            }
            _ => {}
        }
    }
}

async fn collect_model_verification_notifications_and_validate_no_warning_item(
    mcp: &mut McpProcess,
) -> Result<ModelVerificationNotification> {
    let mut verification = None;

    loop {
        let message = timeout(DEFAULT_READ_TIMEOUT, mcp.read_next_message()).await??;
        let JSONRPCMessage::Notification(notification) = message else {
            continue;
        };
        match notification.method.as_str() {
            "model/verification" => {
                let params = notification.params.ok_or_else(|| {
                    anyhow::anyhow!("model/verification notifications must include params")
                })?;
                let payload: ModelVerificationNotification = serde_json::from_value(params)?;
                verification = Some(payload);
            }
            "warning" => {
                anyhow::bail!("verification-only response must not emit warning");
            }
            "model/rerouted" => {
                anyhow::bail!("verification-only response must not emit model/rerouted");
            }
            "item/started" => {
                let params = notification.params.ok_or_else(|| {
                    anyhow::anyhow!("item/started notifications must include params")
                })?;
                let payload: ItemStartedNotification = serde_json::from_value(params)?;
                assert!(!is_warning_user_message_item(&payload.item));
            }
            "item/completed" => {
                let params = notification.params.ok_or_else(|| {
                    anyhow::anyhow!("item/completed notifications must include params")
                })?;
                let payload: ItemCompletedNotification = serde_json::from_value(params)?;
                assert!(!is_warning_user_message_item(&payload.item));
            }
            "turn/completed" => {
                let verification = verification.ok_or_else(|| {
                    anyhow::anyhow!(
                        "expected model/verification notification before turn/completed"
                    )
                })?;
                return Ok(verification);
            }
            _ => {}
        }
    }
}

async fn collect_cyber_policy_error_and_validate_no_reroute(
    mcp: &mut McpProcess,
) -> Result<ErrorNotification> {
    let mut error = None;

    loop {
        let message = timeout(DEFAULT_READ_TIMEOUT, mcp.read_next_message()).await??;
        let JSONRPCMessage::Notification(notification) = message else {
            continue;
        };
        match notification.method.as_str() {
            "error" => {
                let params = notification
                    .params
                    .ok_or_else(|| anyhow::anyhow!("error notifications must include params"))?;
                let payload: ErrorNotification = serde_json::from_value(params)?;
                if payload.error.codex_error_info == Some(CodexErrorInfo::CyberPolicy) {
                    error = Some(payload);
                }
            }
            "model/rerouted" => {
                anyhow::bail!("cyber policy response must not emit model/rerouted");
            }
            "turn/completed" => {
                return error.ok_or_else(|| {
                    anyhow::anyhow!("expected cyber policy error before turn/completed")
                });
            }
            _ => {}
        }
    }
}

fn warning_text_from_item(item: &ThreadItem) -> Option<&str> {
    let ThreadItem::UserMessage { content, .. } = item else {
        return None;
    };

    content.iter().find_map(|input| match input {
        UserInput::Text { text, .. } if text.starts_with("Warning: ") => Some(text.as_str()),
        _ => None,
    })
}

fn is_warning_user_message_item(item: &ThreadItem) -> bool {
    warning_text_from_item(item).is_some()
}

fn create_config_toml(codex_home: &std::path::Path, server_uri: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "{REQUESTED_MODEL}"
approval_policy = "never"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[features]
remote_models = false
personality = true

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}
