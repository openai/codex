use std::path::Path;

use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::AuthMode;
use codex_app_server_protocol::GetAuthStatusParams;
use codex_app_server_protocol::GetAuthStatusResponse;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::LoginApiKeyParams;
use codex_app_server_protocol::LoginApiKeyResponse;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

fn create_config_toml_custom_provider(
    codex_home: &Path,
    requires_openai_auth: bool,
) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    let requires_line = if requires_openai_auth {
        "requires_openai_auth = true\n"
    } else {
        ""
    };
    let contents = format!(
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
{requires_line}
"#
    );
    std::fs::write(config_toml, contents)
}

fn create_config_toml(codex_home: &Path) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "danger-full-access"
"#,
    )
}

async fn login_with_api_key_via_request(mcp: &mut McpProcess, api_key: &str) {
    let request_id = mcp
        .send_login_api_key_request(LoginApiKeyParams {
            api_key: api_key.to_string(),
        })
        .await
        .unwrap_or_else(|e| panic!("send loginApiKey: {e}"));

    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await
    .unwrap_or_else(|e| panic!("loginApiKey timeout: {e}"))
    .unwrap_or_else(|e| panic!("loginApiKey response: {e}"));
    let _: LoginApiKeyResponse =
        to_response(resp).unwrap_or_else(|e| panic!("deserialize login response: {e}"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_auth_status_no_auth() {
    let codex_home = TempDir::new().unwrap_or_else(|e| panic!("create tempdir: {e}"));
    create_config_toml(codex_home.path()).unwrap_or_else(|err| panic!("write config.toml: {err}"));

    let mut mcp = McpProcess::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)])
        .await
        .expect("spawn mcp process");
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .expect("init timeout")
        .expect("init failed");

    let request_id = mcp
        .send_get_auth_status_request(GetAuthStatusParams {
            include_token: Some(true),
            refresh_token: Some(false),
        })
        .await
        .expect("send getAuthStatus");

    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await
    .expect("getAuthStatus timeout")
    .expect("getAuthStatus response");
    let status: GetAuthStatusResponse = to_response(resp).expect("deserialize status");
    assert_eq!(status.auth_method, None, "expected no auth method");
    assert_eq!(status.auth_token, None, "expected no token");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_auth_status_with_api_key() {
    let codex_home = TempDir::new().unwrap_or_else(|e| panic!("create tempdir: {e}"));
    create_config_toml(codex_home.path()).unwrap_or_else(|err| panic!("write config.toml: {err}"));

    let mut mcp = McpProcess::new(codex_home.path())
        .await
        .expect("spawn mcp process");
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .expect("init timeout")
        .expect("init failed");

    login_with_api_key_via_request(&mut mcp, "sk-test-key").await;

    let request_id = mcp
        .send_get_auth_status_request(GetAuthStatusParams {
            include_token: Some(true),
            refresh_token: Some(false),
        })
        .await
        .expect("send getAuthStatus");

    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await
    .expect("getAuthStatus timeout")
    .expect("getAuthStatus response");
    let status: GetAuthStatusResponse = to_response(resp).expect("deserialize status");
    assert_eq!(status.auth_method, Some(AuthMode::ApiKey));
    assert_eq!(status.auth_token, Some("sk-test-key".to_string()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_auth_status_with_api_key_when_auth_not_required() {
    let codex_home = TempDir::new().unwrap_or_else(|e| panic!("create tempdir: {e}"));
    create_config_toml_custom_provider(codex_home.path(), false)
        .unwrap_or_else(|err| panic!("write config.toml: {err}"));

    let mut mcp = McpProcess::new(codex_home.path())
        .await
        .expect("spawn mcp process");
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .expect("init timeout")
        .expect("init failed");

    login_with_api_key_via_request(&mut mcp, "sk-test-key").await;

    let request_id = mcp
        .send_get_auth_status_request(GetAuthStatusParams {
            include_token: Some(true),
            refresh_token: Some(false),
        })
        .await
        .expect("send getAuthStatus");

    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await
    .expect("getAuthStatus timeout")
    .expect("getAuthStatus response");
    let status: GetAuthStatusResponse = to_response(resp).expect("deserialize status");
    assert_eq!(status.auth_method, None, "expected no auth method");
    assert_eq!(status.auth_token, None, "expected no token");
    assert_eq!(
        status.requires_openai_auth,
        Some(false),
        "requires_openai_auth should be false",
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_auth_status_with_api_key_no_include_token() {
    let codex_home = TempDir::new().unwrap_or_else(|e| panic!("create tempdir: {e}"));
    create_config_toml(codex_home.path()).unwrap_or_else(|err| panic!("write config.toml: {err}"));

    let mut mcp = McpProcess::new(codex_home.path())
        .await
        .expect("spawn mcp process");
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .expect("init timeout")
        .expect("init failed");

    login_with_api_key_via_request(&mut mcp, "sk-test-key").await;

    // Build params via struct so None field is omitted in wire JSON.
    let params = GetAuthStatusParams {
        include_token: None,
        refresh_token: Some(false),
    };
    let request_id = mcp
        .send_get_auth_status_request(params)
        .await
        .expect("send getAuthStatus");

    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await
    .expect("getAuthStatus timeout")
    .expect("getAuthStatus response");
    let status: GetAuthStatusResponse = to_response(resp).expect("deserialize status");
    assert_eq!(status.auth_method, Some(AuthMode::ApiKey));
    assert!(status.auth_token.is_none(), "token must be omitted");
}
