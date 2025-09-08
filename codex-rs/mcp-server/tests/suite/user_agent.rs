use codex_core::default_client::DEFAULT_ORIGINATOR;
use codex_core::default_client::get_codex_user_agent;
use codex_protocol::mcp_protocol::GetUserAgentResponse;
use mcp_test_support::McpProcess;
use mcp_test_support::to_response;
use mcp_types::JSONRPCResponse;
use mcp_types::RequestId;
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

    let received: GetUserAgentResponse =
        to_response(response).expect("deserialize getUserAgent response");
    let expected = GetUserAgentResponse {
        user_agent: get_codex_user_agent(Some(DEFAULT_ORIGINATOR)),
    };

    assert_eq!(received, expected);
}
