use anyhow::Result;
use anyhow::bail;
use app_test_support::McpProcess;
use app_test_support::to_response;

use app_test_support::ChatGptAuthFixture;
use app_test_support::ChatGptIdTokenClaims;
use app_test_support::encode_id_token;
use app_test_support::write_chatgpt_auth;
use app_test_support::write_models_cache;
use codex_app_server_protocol::Account;
use codex_app_server_protocol::AuthMode;
use codex_app_server_protocol::CancelLoginAccountParams;
use codex_app_server_protocol::CancelLoginAccountResponse;
use codex_app_server_protocol::CancelLoginAccountStatus;
use codex_app_server_protocol::ChatgptAuthTokensRefreshReason;
use codex_app_server_protocol::ChatgptAuthTokensRefreshResponse;
use codex_app_server_protocol::GetAccountParams;
use codex_app_server_protocol::GetAccountResponse;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::LoginAccountResponse;
use codex_app_server_protocol::LogoutAccountResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStatus;
use codex_core::auth::AuthCredentialsStoreMode;
use codex_login::login_with_api_key;
use codex_protocol::account::PlanType as AccountPlanType;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;
use serial_test::serial;
use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::MockServer;
use wiremock::ResponseTemplate;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

// Helper to create a minimal config.toml for the app server
#[derive(Default)]
struct CreateConfigTomlParams {
    forced_method: Option<String>,
    forced_workspace_id: Option<String>,
    requires_openai_auth: Option<bool>,
    base_url: Option<String>,
}

fn create_config_toml(codex_home: &Path, params: CreateConfigTomlParams) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    let base_url = params
        .base_url
        .unwrap_or_else(|| "http://127.0.0.1:0/v1".to_string());
    let forced_line = if let Some(method) = params.forced_method {
        format!("forced_login_method = \"{method}\"\n")
    } else {
        String::new()
    };
    let forced_workspace_line = if let Some(ws) = params.forced_workspace_id {
        format!("forced_chatgpt_workspace_id = \"{ws}\"\n")
    } else {
        String::new()
    };
    let requires_line = match params.requires_openai_auth {
        Some(true) => "requires_openai_auth = true\n".to_string(),
        Some(false) => String::new(),
        None => String::new(),
    };
    let contents = format!(
        r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "danger-full-access"
{forced_line}
{forced_workspace_line}

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{base_url}"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
{requires_line}
"#
    );
    std::fs::write(config_toml, contents)
}

#[tokio::test]
async fn logout_account_removes_auth_and_notifies() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), CreateConfigTomlParams::default())?;

    login_with_api_key(
        codex_home.path(),
        "sk-test-key",
        AuthCredentialsStoreMode::File,
    )?;
    assert!(codex_home.path().join("auth.json").exists());

    let mut mcp = McpProcess::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let id = mcp.send_logout_account_request().await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(id)),
    )
    .await??;
    let _ok: LogoutAccountResponse = to_response(resp)?;

    let note = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("account/updated"),
    )
    .await??;
    let parsed: ServerNotification = note.try_into()?;
    let ServerNotification::AccountUpdated(payload) = parsed else {
        bail!("unexpected notification: {parsed:?}");
    };
    assert!(
        payload.auth_mode.is_none(),
        "auth_method should be None after logout"
    );

    assert!(
        !codex_home.path().join("auth.json").exists(),
        "auth.json should be deleted"
    );

    let get_id = mcp
        .send_get_account_request(GetAccountParams {
            refresh_token: false,
        })
        .await?;
    let get_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(get_id)),
    )
    .await??;
    let account: GetAccountResponse = to_response(get_resp)?;
    assert_eq!(account.account, None);
    Ok(())
}

#[tokio::test]
async fn set_auth_token_updates_account_and_notifies() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mock_server = MockServer::start().await;
    create_config_toml(
        codex_home.path(),
        CreateConfigTomlParams {
            requires_openai_auth: Some(true),
            base_url: Some(format!("{}/v1", mock_server.uri())),
            ..Default::default()
        },
    )?;
    write_models_cache(codex_home.path())?;

    let id_token = encode_id_token(
        &ChatGptIdTokenClaims::new()
            .email("embedded@example.com")
            .plan_type("pro")
            .chatgpt_account_id("org-embedded"),
    )?;
    let access_token = "access-embedded".to_string();

    let mut mcp = McpProcess::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let set_id = mcp
        .send_chatgpt_auth_tokens_login_request(id_token.clone(), access_token)
        .await?;
    let set_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(set_id)),
    )
    .await??;
    let response: LoginAccountResponse = to_response(set_resp)?;
    assert_eq!(response, LoginAccountResponse::ChatgptAuthTokens {});

    let note = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("account/updated"),
    )
    .await??;
    let parsed: ServerNotification = note.try_into()?;
    let ServerNotification::AccountUpdated(payload) = parsed else {
        bail!("unexpected notification: {parsed:?}");
    };
    assert_eq!(payload.auth_mode, Some(AuthMode::ChatgptAuthTokens));

    let get_id = mcp
        .send_get_account_request(GetAccountParams {
            refresh_token: false,
        })
        .await?;
    let get_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(get_id)),
    )
    .await??;
    let account: GetAccountResponse = to_response(get_resp)?;
    assert_eq!(
        account,
        GetAccountResponse {
            account: Some(Account::Chatgpt {
                email: "embedded@example.com".to_string(),
                plan_type: AccountPlanType::Pro,
            }),
            requires_openai_auth: true,
        }
    );

    Ok(())
}

#[tokio::test]
async fn account_read_refresh_token_is_noop_in_external_mode() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        CreateConfigTomlParams {
            requires_openai_auth: Some(true),
            ..Default::default()
        },
    )?;
    write_models_cache(codex_home.path())?;

    let id_token = encode_id_token(
        &ChatGptIdTokenClaims::new()
            .email("embedded@example.com")
            .plan_type("pro")
            .chatgpt_account_id("org-embedded"),
    )?;

    let mut mcp = McpProcess::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let set_id = mcp
        .send_chatgpt_auth_tokens_login_request(id_token, "access-embedded".to_string())
        .await?;
    let set_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(set_id)),
    )
    .await??;
    let response: LoginAccountResponse = to_response(set_resp)?;
    assert_eq!(response, LoginAccountResponse::ChatgptAuthTokens {});
    let _updated = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("account/updated"),
    )
    .await??;

    let get_id = mcp
        .send_get_account_request(GetAccountParams {
            refresh_token: true,
        })
        .await?;
    let get_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(get_id)),
    )
    .await??;
    let account: GetAccountResponse = to_response(get_resp)?;
    assert_eq!(
        account,
        GetAccountResponse {
            account: Some(Account::Chatgpt {
                email: "embedded@example.com".to_string(),
                plan_type: AccountPlanType::Pro,
            }),
            requires_openai_auth: true,
        }
    );

    let refresh_request = timeout(
        Duration::from_millis(250),
        mcp.read_stream_until_request_message(),
    )
    .await;
    assert!(
        refresh_request.is_err(),
        "external mode should not emit account/chatgptAuthTokens/refresh for refreshToken=true"
    );

    Ok(())
}

async fn respond_to_refresh_request(
    mcp: &mut McpProcess,
    access_token: &str,
    id_token: &str,
) -> Result<()> {
    let refresh_req: ServerRequest = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::ChatgptAuthTokensRefresh { request_id, params } = refresh_req else {
        bail!("expected account/chatgptAuthTokens/refresh request, got {refresh_req:?}");
    };
    assert_eq!(params.reason, ChatgptAuthTokensRefreshReason::Unauthorized);
    let response = ChatgptAuthTokensRefreshResponse {
        access_token: access_token.to_string(),
        id_token: id_token.to_string(),
    };
    mcp.send_response(request_id, serde_json::to_value(response)?)
        .await?;
    Ok(())
}

#[tokio::test]
// 401 response triggers account/chatgptAuthTokens/refresh and retries with new tokens.
async fn external_auth_refreshes_on_unauthorized() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mock_server = MockServer::start().await;
    create_config_toml(
        codex_home.path(),
        CreateConfigTomlParams {
            requires_openai_auth: Some(true),
            base_url: Some(format!("{}/v1", mock_server.uri())),
            ..Default::default()
        },
    )?;
    write_models_cache(codex_home.path())?;

    let success_sse = responses::sse(vec![
        responses::ev_response_created("resp-turn"),
        responses::ev_assistant_message("msg-turn", "turn ok"),
        responses::ev_completed("resp-turn"),
    ]);
    let unauthorized = ResponseTemplate::new(401).set_body_json(json!({
        "error": { "message": "unauthorized" }
    }));
    let responses_mock = responses::mount_response_sequence(
        &mock_server,
        vec![unauthorized, responses::sse_response(success_sse)],
    )
    .await;

    let initial_id_token = encode_id_token(
        &ChatGptIdTokenClaims::new()
            .email("initial@example.com")
            .plan_type("pro")
            .chatgpt_account_id("org-initial"),
    )?;
    let refreshed_id_token = encode_id_token(
        &ChatGptIdTokenClaims::new()
            .email("refreshed@example.com")
            .plan_type("pro")
            .chatgpt_account_id("org-refreshed"),
    )?;
    let initial_access_token = "access-initial".to_string();
    let refreshed_access_token = "access-refreshed".to_string();

    let mut mcp = McpProcess::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let set_id = mcp
        .send_chatgpt_auth_tokens_login_request(
            initial_id_token.clone(),
            initial_access_token.clone(),
        )
        .await?;
    let set_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(set_id)),
    )
    .await??;
    let response: LoginAccountResponse = to_response(set_resp)?;
    assert_eq!(response, LoginAccountResponse::ChatgptAuthTokens {});
    let _updated = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("account/updated"),
    )
    .await??;

    let thread_req = mcp
        .send_thread_start_request(codex_app_server_protocol::ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let thread = to_response::<codex_app_server_protocol::ThreadStartResponse>(thread_resp)?;

    let turn_req = mcp
        .send_turn_start_request(codex_app_server_protocol::TurnStartParams {
            thread_id: thread.thread.id,
            input: vec![codex_app_server_protocol::UserInput::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    respond_to_refresh_request(&mut mcp, &refreshed_access_token, &refreshed_id_token).await?;
    let _turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let _turn_completed = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let requests = responses_mock.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0].header("authorization"),
        Some(format!("Bearer {initial_access_token}"))
    );
    assert_eq!(
        requests[1].header("authorization"),
        Some(format!("Bearer {refreshed_access_token}"))
    );

    Ok(())
}

#[tokio::test]
// Client returns JSON-RPC error to refresh; turn fails.
async fn external_auth_refresh_error_fails_turn() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mock_server = MockServer::start().await;
    create_config_toml(
        codex_home.path(),
        CreateConfigTomlParams {
            requires_openai_auth: Some(true),
            base_url: Some(format!("{}/v1", mock_server.uri())),
            ..Default::default()
        },
    )?;
    write_models_cache(codex_home.path())?;

    let unauthorized = ResponseTemplate::new(401).set_body_json(json!({
        "error": { "message": "unauthorized" }
    }));
    let _responses_mock =
        responses::mount_response_sequence(&mock_server, vec![unauthorized]).await;

    let initial_id_token = encode_id_token(
        &ChatGptIdTokenClaims::new()
            .email("initial@example.com")
            .plan_type("pro")
            .chatgpt_account_id("org-initial"),
    )?;

    let mut mcp = McpProcess::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let set_id = mcp
        .send_chatgpt_auth_tokens_login_request(initial_id_token, "access-initial".to_string())
        .await?;
    let set_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(set_id)),
    )
    .await??;
    let response: LoginAccountResponse = to_response(set_resp)?;
    assert_eq!(response, LoginAccountResponse::ChatgptAuthTokens {});
    let _updated = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("account/updated"),
    )
    .await??;

    let thread_req = mcp
        .send_thread_start_request(codex_app_server_protocol::ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let thread = to_response::<codex_app_server_protocol::ThreadStartResponse>(thread_resp)?;

    let turn_req = mcp
        .send_turn_start_request(codex_app_server_protocol::TurnStartParams {
            thread_id: thread.thread.id.clone(),
            input: vec![codex_app_server_protocol::UserInput::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;

    let refresh_req: ServerRequest = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::ChatgptAuthTokensRefresh { request_id, .. } = refresh_req else {
        bail!("expected account/chatgptAuthTokens/refresh request, got {refresh_req:?}");
    };

    mcp.send_error(
        request_id,
        JSONRPCErrorError {
            code: -32_000,
            message: "refresh failed".to_string(),
            data: None,
        },
    )
    .await?;

    let _turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let completed_notif: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    let completed: TurnCompletedNotification = serde_json::from_value(
        completed_notif
            .params
            .expect("turn/completed params must be present"),
    )?;
    assert_eq!(completed.turn.status, TurnStatus::Failed);
    assert!(completed.turn.error.is_some());

    Ok(())
}

#[tokio::test]
// Refresh returns tokens for the wrong workspace; turn fails.
async fn external_auth_refresh_mismatched_workspace_fails_turn() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mock_server = MockServer::start().await;
    create_config_toml(
        codex_home.path(),
        CreateConfigTomlParams {
            forced_workspace_id: Some("org-expected".to_string()),
            requires_openai_auth: Some(true),
            base_url: Some(format!("{}/v1", mock_server.uri())),
            ..Default::default()
        },
    )?;
    write_models_cache(codex_home.path())?;

    let unauthorized = ResponseTemplate::new(401).set_body_json(json!({
        "error": { "message": "unauthorized" }
    }));
    let _responses_mock =
        responses::mount_response_sequence(&mock_server, vec![unauthorized]).await;

    let initial_id_token = encode_id_token(
        &ChatGptIdTokenClaims::new()
            .email("initial@example.com")
            .plan_type("pro")
            .chatgpt_account_id("org-expected"),
    )?;
    let refreshed_id_token = encode_id_token(
        &ChatGptIdTokenClaims::new()
            .email("refreshed@example.com")
            .plan_type("pro")
            .chatgpt_account_id("org-other"),
    )?;

    let mut mcp = McpProcess::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let set_id = mcp
        .send_chatgpt_auth_tokens_login_request(initial_id_token, "access-initial".to_string())
        .await?;
    let set_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(set_id)),
    )
    .await??;
    let response: LoginAccountResponse = to_response(set_resp)?;
    assert_eq!(response, LoginAccountResponse::ChatgptAuthTokens {});
    let _updated = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("account/updated"),
    )
    .await??;

    let thread_req = mcp
        .send_thread_start_request(codex_app_server_protocol::ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let thread = to_response::<codex_app_server_protocol::ThreadStartResponse>(thread_resp)?;

    let turn_req = mcp
        .send_turn_start_request(codex_app_server_protocol::TurnStartParams {
            thread_id: thread.thread.id.clone(),
            input: vec![codex_app_server_protocol::UserInput::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;

    let refresh_req: ServerRequest = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::ChatgptAuthTokensRefresh { request_id, .. } = refresh_req else {
        bail!("expected account/chatgptAuthTokens/refresh request, got {refresh_req:?}");
    };

    mcp.send_response(
        request_id,
        serde_json::to_value(ChatgptAuthTokensRefreshResponse {
            access_token: "access-refreshed".to_string(),
            id_token: refreshed_id_token,
        })?,
    )
    .await?;

    let _turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let completed_notif: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    let completed: TurnCompletedNotification = serde_json::from_value(
        completed_notif
            .params
            .expect("turn/completed params must be present"),
    )?;
    assert_eq!(completed.turn.status, TurnStatus::Failed);
    assert!(completed.turn.error.is_some());

    Ok(())
}

#[tokio::test]
// Refresh returns a malformed id_token; turn fails.
async fn external_auth_refresh_invalid_id_token_fails_turn() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mock_server = MockServer::start().await;
    create_config_toml(
        codex_home.path(),
        CreateConfigTomlParams {
            requires_openai_auth: Some(true),
            base_url: Some(format!("{}/v1", mock_server.uri())),
            ..Default::default()
        },
    )?;
    write_models_cache(codex_home.path())?;

    let unauthorized = ResponseTemplate::new(401).set_body_json(json!({
        "error": { "message": "unauthorized" }
    }));
    let _responses_mock =
        responses::mount_response_sequence(&mock_server, vec![unauthorized]).await;

    let initial_id_token = encode_id_token(
        &ChatGptIdTokenClaims::new()
            .email("initial@example.com")
            .plan_type("pro")
            .chatgpt_account_id("org-initial"),
    )?;

    let mut mcp = McpProcess::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let set_id = mcp
        .send_chatgpt_auth_tokens_login_request(initial_id_token, "access-initial".to_string())
        .await?;
    let set_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(set_id)),
    )
    .await??;
    let response: LoginAccountResponse = to_response(set_resp)?;
    assert_eq!(response, LoginAccountResponse::ChatgptAuthTokens {});
    let _updated = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("account/updated"),
    )
    .await??;

    let thread_req = mcp
        .send_thread_start_request(codex_app_server_protocol::ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let thread = to_response::<codex_app_server_protocol::ThreadStartResponse>(thread_resp)?;

    let turn_req = mcp
        .send_turn_start_request(codex_app_server_protocol::TurnStartParams {
            thread_id: thread.thread.id.clone(),
            input: vec![codex_app_server_protocol::UserInput::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;

    let refresh_req: ServerRequest = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::ChatgptAuthTokensRefresh { request_id, .. } = refresh_req else {
        bail!("expected account/chatgptAuthTokens/refresh request, got {refresh_req:?}");
    };

    mcp.send_response(
        request_id,
        serde_json::to_value(ChatgptAuthTokensRefreshResponse {
            access_token: "access-refreshed".to_string(),
            id_token: "not-a-jwt".to_string(),
        })?,
    )
    .await?;

    let _turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let completed_notif: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    let completed: TurnCompletedNotification = serde_json::from_value(
        completed_notif
            .params
            .expect("turn/completed params must be present"),
    )?;
    assert_eq!(completed.turn.status, TurnStatus::Failed);
    assert!(completed.turn.error.is_some());

    Ok(())
}

#[tokio::test]
async fn login_account_api_key_succeeds_and_notifies() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), CreateConfigTomlParams::default())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let req_id = mcp
        .send_login_account_api_key_request("sk-test-key")
        .await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(req_id)),
    )
    .await??;
    let login: LoginAccountResponse = to_response(resp)?;
    assert_eq!(login, LoginAccountResponse::ApiKey {});

    let note = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("account/login/completed"),
    )
    .await??;
    let parsed: ServerNotification = note.try_into()?;
    let ServerNotification::AccountLoginCompleted(payload) = parsed else {
        bail!("unexpected notification: {parsed:?}");
    };
    pretty_assertions::assert_eq!(payload.login_id, None);
    pretty_assertions::assert_eq!(payload.success, true);
    pretty_assertions::assert_eq!(payload.error, None);

    let note = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("account/updated"),
    )
    .await??;
    let parsed: ServerNotification = note.try_into()?;
    let ServerNotification::AccountUpdated(payload) = parsed else {
        bail!("unexpected notification: {parsed:?}");
    };
    pretty_assertions::assert_eq!(payload.auth_mode, Some(AuthMode::ApiKey));

    assert!(codex_home.path().join("auth.json").exists());
    Ok(())
}

#[tokio::test]
async fn login_account_api_key_rejected_when_forced_chatgpt() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        CreateConfigTomlParams {
            forced_method: Some("chatgpt".to_string()),
            ..Default::default()
        },
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_login_account_api_key_request("sk-test-key")
        .await?;
    let err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(
        err.error.message,
        "API key login is disabled. Use ChatGPT login instead."
    );
    Ok(())
}

#[tokio::test]
async fn login_account_chatgpt_rejected_when_forced_api() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        CreateConfigTomlParams {
            forced_method: Some("api".to_string()),
            ..Default::default()
        },
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp.send_login_account_chatgpt_request().await?;
    let err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(
        err.error.message,
        "ChatGPT login is disabled. Use API key login instead."
    );
    Ok(())
}

#[tokio::test]
// Serialize tests that launch the login server since it binds to a fixed port.
#[serial(login_port)]
async fn login_account_chatgpt_start_can_be_cancelled() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), CreateConfigTomlParams::default())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp.send_login_account_chatgpt_request().await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let login: LoginAccountResponse = to_response(resp)?;
    let LoginAccountResponse::Chatgpt { login_id, auth_url } = login else {
        bail!("unexpected login response: {login:?}");
    };
    assert!(
        auth_url.contains("redirect_uri=http%3A%2F%2Flocalhost"),
        "auth_url should contain a redirect_uri to localhost"
    );

    let cancel_id = mcp
        .send_cancel_login_account_request(CancelLoginAccountParams {
            login_id: login_id.clone(),
        })
        .await?;
    let cancel_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(cancel_id)),
    )
    .await??;
    let _ok: CancelLoginAccountResponse = to_response(cancel_resp)?;

    let note = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("account/login/completed"),
    )
    .await??;
    let parsed: ServerNotification = note.try_into()?;
    let ServerNotification::AccountLoginCompleted(payload) = parsed else {
        bail!("unexpected notification: {parsed:?}");
    };
    pretty_assertions::assert_eq!(payload.login_id, Some(login_id));
    pretty_assertions::assert_eq!(payload.success, false);
    assert!(
        payload.error.is_some(),
        "expected a non-empty error on cancel"
    );

    let maybe_updated = timeout(
        Duration::from_millis(500),
        mcp.read_stream_until_notification_message("account/updated"),
    )
    .await;
    assert!(
        maybe_updated.is_err(),
        "account/updated should not be emitted when login is cancelled"
    );
    Ok(())
}

#[tokio::test]
// Serialize tests that launch the login server since it binds to a fixed port.
#[serial(login_port)]
async fn set_auth_token_cancels_active_chatgpt_login() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), CreateConfigTomlParams::default())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    // Initiate the ChatGPT login flow
    let request_id = mcp.send_login_account_chatgpt_request().await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let login: LoginAccountResponse = to_response(resp)?;
    let LoginAccountResponse::Chatgpt { login_id, .. } = login else {
        bail!("unexpected login response: {login:?}");
    };

    let id_token = encode_id_token(
        &ChatGptIdTokenClaims::new()
            .email("embedded@example.com")
            .plan_type("pro")
            .chatgpt_account_id("org-embedded"),
    )?;
    // Set an external auth token instead of completing the ChatGPT login flow.
    // This should cancel the active login attempt.
    let set_id = mcp
        .send_chatgpt_auth_tokens_login_request(id_token, "access-embedded".to_string())
        .await?;
    let set_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(set_id)),
    )
    .await??;
    let response: LoginAccountResponse = to_response(set_resp)?;
    assert_eq!(response, LoginAccountResponse::ChatgptAuthTokens {});
    let _updated = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("account/updated"),
    )
    .await??;

    // Verify that the active login attempt was cancelled.
    // We check this by trying to cancel it and expecting a not found error.
    let cancel_id = mcp
        .send_cancel_login_account_request(CancelLoginAccountParams {
            login_id: login_id.clone(),
        })
        .await?;
    let cancel_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(cancel_id)),
    )
    .await??;
    let cancel: CancelLoginAccountResponse = to_response(cancel_resp)?;
    assert_eq!(cancel.status, CancelLoginAccountStatus::NotFound);

    Ok(())
}

#[tokio::test]
// Serialize tests that launch the login server since it binds to a fixed port.
#[serial(login_port)]
async fn login_account_chatgpt_includes_forced_workspace_query_param() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        CreateConfigTomlParams {
            forced_workspace_id: Some("ws-forced".to_string()),
            ..Default::default()
        },
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp.send_login_account_chatgpt_request().await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let login: LoginAccountResponse = to_response(resp)?;
    let LoginAccountResponse::Chatgpt { auth_url, .. } = login else {
        bail!("unexpected login response: {login:?}");
    };
    assert!(
        auth_url.contains("allowed_workspace_id=ws-forced"),
        "auth URL should include forced workspace"
    );
    Ok(())
}

#[tokio::test]
async fn get_account_no_auth() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        CreateConfigTomlParams {
            requires_openai_auth: Some(true),
            ..Default::default()
        },
    )?;

    let mut mcp = McpProcess::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let params = GetAccountParams {
        refresh_token: false,
    };
    let request_id = mcp.send_get_account_request(params).await?;

    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let account: GetAccountResponse = to_response(resp)?;

    assert_eq!(account.account, None, "expected no account");
    assert_eq!(account.requires_openai_auth, true);
    Ok(())
}

#[tokio::test]
async fn get_account_with_api_key() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        CreateConfigTomlParams {
            requires_openai_auth: Some(true),
            ..Default::default()
        },
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let req_id = mcp
        .send_login_account_api_key_request("sk-test-key")
        .await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(req_id)),
    )
    .await??;
    let _login_ok = to_response::<LoginAccountResponse>(resp)?;

    let params = GetAccountParams {
        refresh_token: false,
    };
    let request_id = mcp.send_get_account_request(params).await?;

    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: GetAccountResponse = to_response(resp)?;

    let expected = GetAccountResponse {
        account: Some(Account::ApiKey {}),
        requires_openai_auth: true,
    };
    assert_eq!(received, expected);
    Ok(())
}

#[tokio::test]
async fn get_account_when_auth_not_required() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        CreateConfigTomlParams {
            requires_openai_auth: Some(false),
            ..Default::default()
        },
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let params = GetAccountParams {
        refresh_token: false,
    };
    let request_id = mcp.send_get_account_request(params).await?;

    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: GetAccountResponse = to_response(resp)?;

    let expected = GetAccountResponse {
        account: None,
        requires_openai_auth: false,
    };
    assert_eq!(received, expected);
    Ok(())
}

#[tokio::test]
async fn get_account_with_chatgpt() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        CreateConfigTomlParams {
            requires_openai_auth: Some(true),
            ..Default::default()
        },
    )?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("access-chatgpt")
            .email("user@example.com")
            .plan_type("pro"),
        AuthCredentialsStoreMode::File,
    )?;

    let mut mcp = McpProcess::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let params = GetAccountParams {
        refresh_token: false,
    };
    let request_id = mcp.send_get_account_request(params).await?;

    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: GetAccountResponse = to_response(resp)?;

    let expected = GetAccountResponse {
        account: Some(Account::Chatgpt {
            email: "user@example.com".to_string(),
            plan_type: AccountPlanType::Pro,
        }),
        requires_openai_auth: true,
    };
    assert_eq!(received, expected);
    Ok(())
}
