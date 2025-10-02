use std::time::Duration;

use anyhow::Context;
use app_test_support::McpProcess;
use app_test_support::to_response;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::UserInfoResponse;
use codex_core::auth::AuthDotJson;
use codex_core::auth::get_auth_file;
use codex_core::auth::write_auth_json;
use codex_core::token_data::IdTokenInfo;
use codex_core::token_data::TokenData;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_info_returns_email_from_auth_json() {
    let codex_home = TempDir::new().expect("create tempdir");

    let auth_path = get_auth_file(codex_home.path());
    let mut id_token = IdTokenInfo::default();
    id_token.email = Some("user@example.com".to_string());
    id_token.raw_jwt = encode_id_token_with_email("user@example.com").expect("encode id token");

    let auth = AuthDotJson {
        openai_api_key: None,
        tokens: Some(TokenData {
            id_token,
            access_token: "access".to_string(),
            refresh_token: "refresh".to_string(),
            account_id: None,
        }),
        last_refresh: None,
    };
    write_auth_json(&auth_path, &auth).expect("write auth.json");

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

fn encode_id_token_with_email(email: &str) -> anyhow::Result<String> {
    let header_b64 = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&json!({ "alg": "none", "typ": "JWT" }))
            .context("serialize jwt header")?,
    );
    let payload =
        serde_json::to_vec(&json!({ "email": email })).context("serialize jwt payload")?;
    let payload_b64 = URL_SAFE_NO_PAD.encode(payload);
    Ok(format!("{header_b64}.{payload_b64}.signature"))
}
