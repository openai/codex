use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::GetUserAgentResponse;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_user_agent_returns_current_codex_user_agent() {
    let codex_home = TempDir::new().unwrap_or_else(|err| panic!("create tempdir: {err}"));

    let mut mcp = McpProcess::new(codex_home.path())
        .await
        .expect("spawn mcp process");
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .expect("initialize timeout")
        .expect("initialize request");

    let request_id = mcp
        .send_get_user_agent_request()
        .await
        .expect("send getUserAgent");
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await
    .expect("getUserAgent timeout")
    .expect("getUserAgent response");

    let os_info = os_info::get();
    let user_agent = format!(
        "codex_cli_rs/0.0.0 ({} {}; {}) {} (codex-app-server-tests; 0.1.0)",
        os_info.os_type(),
        os_info.version(),
        os_info.architecture().unwrap_or("unknown"),
        codex_core::terminal::user_agent()
    );

    let received: GetUserAgentResponse =
        to_response(response).expect("deserialize getUserAgent response");
    let expected = GetUserAgentResponse { user_agent };

    assert_eq!(received, expected);
}
