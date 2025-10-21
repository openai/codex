use std::time::Duration;

use app_test_support::ChatGptAuthFixture;
use app_test_support::McpProcess;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::UserInfoResponse;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_info_returns_email_from_auth_json() {
    let codex_home = TempDir::new().expect("create tempdir");

    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("access")
            .refresh_token("refresh")
            .email("user@example.com"),
    )
    .expect("write chatgpt auth");

    let mut mcp = McpProcess::new(codex_home.path())
        .await
        .expect("spawn mcp process");
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .expect("initialize timeout")
        .expect("initialize request");

    let request_id = mcp.send_user_info_request().await.expect("send userInfo");
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await
    .expect("userInfo timeout")
    .expect("userInfo response");

    let received: UserInfoResponse = to_response(response).expect("deserialize userInfo response");
    let expected = UserInfoResponse {
        alleged_user_email: Some("user@example.com".to_string()),
    };

    assert_eq!(received, expected);
}
