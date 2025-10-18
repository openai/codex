use app_test_support::ConfigBuilder;
use app_test_support::DEFAULT_READ_TIMEOUT;
use app_test_support::McpProcess;
use app_test_support::MockProviderConfig;
use app_test_support::login_with_api_key_via_mcp;
use app_test_support::to_response;
use codex_app_server_protocol::AuthMode;
use codex_app_server_protocol::GetAuthStatusParams;
use codex_app_server_protocol::GetAuthStatusResponse;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_auth_status_no_auth() {
    let codex_home = TempDir::new().unwrap_or_else(|e| panic!("create tempdir: {e}"));
    ConfigBuilder::default()
        .with_defaults()
        .write(codex_home.path())
        .unwrap_or_else(|err| panic!("write config.toml: {err}"));

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
    ConfigBuilder::default()
        .with_defaults()
        .write(codex_home.path())
        .unwrap_or_else(|err| panic!("write config.toml: {err}"));

    let mut mcp = McpProcess::new(codex_home.path())
        .await
        .expect("spawn mcp process");
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .expect("init timeout")
        .expect("init failed");

    login_with_api_key_via_mcp(&mut mcp, "sk-test-key")
        .await
        .expect("login with api key");

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
    ConfigBuilder::default()
        .with_defaults()
        .with_mock_provider(
            MockProviderConfig::new("http://127.0.0.1:0/v1").requires_openai_auth(false),
        )
        .write(codex_home.path())
        .unwrap_or_else(|err| panic!("write config.toml: {err}"));

    let mut mcp = McpProcess::new(codex_home.path())
        .await
        .expect("spawn mcp process");
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .expect("init timeout")
        .expect("init failed");

    login_with_api_key_via_mcp(&mut mcp, "sk-test-key")
        .await
        .expect("login with api key");

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
    ConfigBuilder::default()
        .with_defaults()
        .write(codex_home.path())
        .unwrap_or_else(|err| panic!("write config.toml: {err}"));

    let mut mcp = McpProcess::new(codex_home.path())
        .await
        .expect("spawn mcp process");
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .expect("init timeout")
        .expect("init failed");

    login_with_api_key_via_mcp(&mut mcp, "sk-test-key")
        .await
        .expect("login with api key");

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
