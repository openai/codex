use std::time::Duration;

use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::configure_expiring_workload_identity;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::McpServerOauthLoginParams;
use codex_app_server_protocol::McpServerOauthLoginResponse;
use codex_app_server_protocol::RequestId;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn oauth_login_for_configured_server_survives_unavailable_wif() -> Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "authorization_endpoint": format!("{}/oauth/authorize", server.uri()),
            "token_endpoint": format!("{}/oauth/token", server.uri()),
            "scopes_supported": [],
        })))
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"
chatgpt_base_url = "{}/backend-api"
mcp_oauth_credentials_store = "file"

[mcp_servers.local]
url = "{}/mcp"

[mcp_servers.local.oauth]
client_id = "test-client"
"#,
            server.uri(),
            server.uri(),
        ),
    )?;
    let workload_identity =
        configure_expiring_workload_identity(codex_home.path(), &server).await?;

    let mut app_server =
        TestAppServer::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)]).await?;
    timeout(DEFAULT_TIMEOUT, app_server.initialize()).await??;
    workload_identity.remove_and_wait_for_expiry().await?;

    let request_id = app_server
        .send_raw_request(
            "mcpServer/oauth/login",
            Some(serde_json::to_value(McpServerOauthLoginParams {
                name: "local".to_string(),
                scopes: Some(Vec::new()),
                timeout_secs: Some(1),
            })?),
        )
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: McpServerOauthLoginResponse = to_response(response)?;

    assert!(
        response
            .authorization_url
            .starts_with(&format!("{}/oauth/authorize", server.uri())),
        "unexpected authorization URL: {}",
        response.authorization_url
    );
    server.verify().await;
    Ok(())
}
