use std::path::Path;
use std::time::Duration;

use codex_protocol::mcp_protocol::CancelLoginChatGptParams;
use codex_protocol::mcp_protocol::CancelLoginChatGptResponse;
use codex_protocol::mcp_protocol::GetAuthStatusParams;
use codex_protocol::mcp_protocol::GetAuthStatusResponse;
use codex_protocol::mcp_protocol::LoginApiKeyParams;
use codex_protocol::mcp_protocol::LoginApiKeyResponse;
use codex_protocol::mcp_protocol::LoginChatGptResponse;
use codex_protocol::mcp_protocol::LogoutChatGptResponse;
use mcp_test_support::McpProcess;
use mcp_test_support::to_response;
use mcp_types::JSONRPCResponse;
use mcp_types::RequestId;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

fn unwrap_result<T, E: std::fmt::Display>(result: Result<T, E>, context: &str) -> T {
    result.unwrap_or_else(|err| panic!("{context}: {err}"))
}

// Helper to create a config.toml; mirrors create_conversation.rs
fn create_config_toml(codex_home: &Path) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "danger-full-access"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "http://127.0.0.1:0/v1"
wire_api = "chat"
request_max_retries = 0
stream_max_retries = 0
"#,
    )
}

async fn login_with_api_key_via_request(mcp: &mut McpProcess, api_key: &str) {
    let request_id = unwrap_result(
        mcp.send_login_api_key_request(LoginApiKeyParams {
            api_key: api_key.to_string(),
        })
        .await,
        "send loginApiKey",
    );

    let resp: JSONRPCResponse = unwrap_result(
        unwrap_result(
            timeout(
                DEFAULT_READ_TIMEOUT,
                mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
            )
            .await,
            "loginApiKey timeout",
        ),
        "loginApiKey response",
    );
    let _: LoginApiKeyResponse = unwrap_result(to_response(resp), "deserialize login response");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn logout_chatgpt_removes_auth() {
    let codex_home = TempDir::new().unwrap_or_else(|e| panic!("create tempdir: {e}"));
    create_config_toml(codex_home.path()).unwrap_or_else(|err| panic!("write config.toml: {err}"));

    let mut mcp = unwrap_result(
        McpProcess::new(codex_home.path()).await,
        "spawn mcp process",
    );
    unwrap_result(
        unwrap_result(
            timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await,
            "init timeout",
        ),
        "init failed",
    );

    login_with_api_key_via_request(&mut mcp, "sk-test-key").await;
    assert!(codex_home.path().join("auth.json").exists());

    let id = unwrap_result(
        mcp.send_logout_chat_gpt_request().await,
        "send logoutChatGpt",
    );
    let resp: JSONRPCResponse = unwrap_result(
        unwrap_result(
            timeout(
                DEFAULT_READ_TIMEOUT,
                mcp.read_stream_until_response_message(RequestId::Integer(id)),
            )
            .await,
            "logoutChatGpt timeout",
        ),
        "logoutChatGpt response",
    );
    let _ok: LogoutChatGptResponse =
        unwrap_result(to_response(resp), "deserialize logout response");

    assert!(
        !codex_home.path().join("auth.json").exists(),
        "auth.json should be deleted"
    );

    // Verify status reflects signed-out state.
    let status_id = unwrap_result(
        mcp.send_get_auth_status_request(GetAuthStatusParams {
            include_token: Some(true),
            refresh_token: Some(false),
        })
        .await,
        "send getAuthStatus",
    );
    let status_resp: JSONRPCResponse = unwrap_result(
        unwrap_result(
            timeout(
                DEFAULT_READ_TIMEOUT,
                mcp.read_stream_until_response_message(RequestId::Integer(status_id)),
            )
            .await,
            "getAuthStatus timeout",
        ),
        "getAuthStatus response",
    );
    let status: GetAuthStatusResponse =
        unwrap_result(to_response(status_resp), "deserialize status");
    assert_eq!(status.auth_method, None);
    assert_eq!(status.auth_token, None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn login_and_cancel_chatgpt() {
    let codex_home = TempDir::new().unwrap_or_else(|e| panic!("create tempdir: {e}"));
    create_config_toml(codex_home.path()).unwrap_or_else(|err| panic!("write config.toml: {err}"));

    let mut mcp = unwrap_result(
        McpProcess::new(codex_home.path()).await,
        "spawn mcp process",
    );
    unwrap_result(
        unwrap_result(
            timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await,
            "init timeout",
        ),
        "init failed",
    );

    let login_id = unwrap_result(mcp.send_login_chat_gpt_request().await, "send loginChatGpt");
    let login_resp: JSONRPCResponse = unwrap_result(
        unwrap_result(
            timeout(
                DEFAULT_READ_TIMEOUT,
                mcp.read_stream_until_response_message(RequestId::Integer(login_id)),
            )
            .await,
            "loginChatGpt timeout",
        ),
        "loginChatGpt response",
    );
    let login: LoginChatGptResponse =
        unwrap_result(to_response(login_resp), "deserialize login resp");

    let cancel_id = unwrap_result(
        mcp.send_cancel_login_chat_gpt_request(CancelLoginChatGptParams {
            login_id: login.login_id,
        })
        .await,
        "send cancelLoginChatGpt",
    );
    let cancel_resp: JSONRPCResponse = unwrap_result(
        unwrap_result(
            timeout(
                DEFAULT_READ_TIMEOUT,
                mcp.read_stream_until_response_message(RequestId::Integer(cancel_id)),
            )
            .await,
            "cancelLoginChatGpt timeout",
        ),
        "cancelLoginChatGpt response",
    );
    let _ok: CancelLoginChatGptResponse =
        unwrap_result(to_response(cancel_resp), "deserialize cancel response");

    // Optionally observe the completion notification; do not fail if it races.
    let maybe_note = timeout(
        Duration::from_secs(2),
        mcp.read_stream_until_notification_message("codex/event/login_chat_gpt_complete"),
    )
    .await;
    if maybe_note.is_err() {
        eprintln!("warning: did not observe login_chat_gpt_complete notification after cancel");
    }
}
