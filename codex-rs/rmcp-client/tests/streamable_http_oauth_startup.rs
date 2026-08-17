mod streamable_http_test_support;

use std::collections::HashMap;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use codex_rmcp_client::McpAuthState;
use codex_rmcp_client::McpLoginRequirement;
use codex_rmcp_client::OAuthDiscoveryTimeout;
use codex_rmcp_client::RmcpClient;
use codex_rmcp_client::StoredOAuthTokens;
use codex_rmcp_client::StreamableHttpRedirectMode;
use codex_rmcp_client::WrappedOAuthTokenResponse;
use codex_rmcp_client::determine_streamable_http_auth_status;
use codex_rmcp_client::is_authentication_required_error;
use codex_rmcp_client::save_oauth_tokens;
use codex_rmcp_client::with_http_headers_helper;
use codex_utils_cargo_bin::cargo_bin;
use oauth2::AccessToken;
use oauth2::RefreshToken;
use oauth2::basic::BasicTokenType;
use pretty_assertions::assert_eq;
use rmcp::transport::auth::OAuthTokenResponse;
use rmcp::transport::auth::VendorExtraTokenFields;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::process::Command;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_string_contains;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

use streamable_http_test_support::initialize_client;

const SERVER_NAME: &str = "test-streamable-http-oauth-startup";
const EXPIRED_ACCESS_TOKEN: &str = "expired-access-token";
const REFRESH_TOKEN: &str = "valid-refresh-token";
const REFRESHED_ACCESS_TOKEN: &str = "refreshed-access-token";
const RESOURCE_API_KEY: &str = "resource-api-key-secret";
const RESOURCE_USER_AGENT: &str = "resource-only-user-agent";
const MCP_USER_AGENT: &str = concat!("codex-mcp-client/", env!("CARGO_PKG_VERSION"));
const CHILD_SERVER_URL_ENV: &str = "MCP_TEST_OAUTH_STARTUP_SERVER_URL";
const CHILD_HELPER_COMMAND_ENV: &str = "MCP_TEST_OAUTH_STARTUP_HELPER_COMMAND";
const CHILD_RESOURCE_API_KEY_ENV: &str = "MCP_TEST_OAUTH_STARTUP_RESOURCE_API_KEY";
const UNREFRESHABLE_SERVER_URL: &str = "https://unrefreshable.example/mcp";
const UNEXPIRED_SERVER_URL: &str = "https://unexpired.example/mcp";
const REFRESHABLE_SERVER_URL: &str = "https://refreshable.example/mcp";

#[derive(Clone, Copy)]
enum OAuthStartupScenario {
    DirectAuthorizationMetadata,
    GatewayHeadersHelper,
    ProtectedResourceMetadata,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn refreshes_expired_persisted_token_before_initialize() -> anyhow::Result<()> {
    assert_expired_token_refresh(OAuthStartupScenario::DirectAuthorizationMetadata).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn refreshes_oauth_with_gateway_headers_helper() -> anyhow::Result<()> {
    assert_expired_token_refresh(OAuthStartupScenario::GatewayHeadersHelper).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn refresh_uses_discovered_protected_resource_audience() -> anyhow::Result<()> {
    assert_expired_token_refresh(OAuthStartupScenario::ProtectedResourceMetadata).await
}

async fn assert_expired_token_refresh(scenario: OAuthStartupScenario) -> anyhow::Result<()> {
    let server = MockServer::start().await;
    let authorization_server = MockServer::start().await;
    let resource_url = format!("{}/mcp", server.uri());
    let (server_url, mcp_path, authorization_metadata_path) = match scenario {
        OAuthStartupScenario::DirectAuthorizationMetadata
        | OAuthStartupScenario::GatewayHeadersHelper => (
            resource_url.clone(),
            "/mcp",
            "/.well-known/oauth-authorization-server/mcp",
        ),
        OAuthStartupScenario::ProtectedResourceMetadata => (
            format!("{resource_url}/"),
            "/mcp/",
            "/.well-known/oauth-authorization-server",
        ),
    };

    if matches!(scenario, OAuthStartupScenario::ProtectedResourceMetadata) {
        let resource_metadata_url = format!("{}/resource-metadata", server.uri());
        Mock::given(method("GET"))
            .and(path(mcp_path))
            .respond_with(ResponseTemplate::new(401).insert_header(
                "www-authenticate",
                format!("Bearer resource_metadata=\"{resource_metadata_url}\""),
            ))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/resource-metadata"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "resource": resource_url,
                "authorization_servers": [server.uri()],
            })))
            .expect(1)
            .mount(&server)
            .await;
    }

    let mut authorization_metadata = json!({
        "authorization_endpoint": format!("{}/oauth/authorize", authorization_server.uri()),
        "token_endpoint": format!("{}/oauth/token", authorization_server.uri()),
        "scopes_supported": [""],
    });
    if matches!(scenario, OAuthStartupScenario::ProtectedResourceMetadata) {
        authorization_metadata["issuer"] = json!(server.uri());
    }

    Mock::given(method("GET"))
        .and(path(authorization_metadata_path))
        .and(header("user-agent", RESOURCE_USER_AGENT))
        .and(header("x-api-key", RESOURCE_API_KEY))
        .respond_with(ResponseTemplate::new(200).set_body_json(authorization_metadata))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(header("user-agent", MCP_USER_AGENT))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains(format!(
            "refresh_token={REFRESH_TOKEN}"
        )))
        .and({
            let expected_resource = resource_url.clone();
            move |request: &Request| {
                url::form_urlencoded::parse(&request.body)
                    .any(|(name, value)| name == "resource" && value == expected_resource)
            }
        })
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": REFRESHED_ACCESS_TOKEN,
            "token_type": "Bearer",
            "expires_in": 7200,
            "refresh_token": REFRESH_TOKEN,
        })))
        .expect(1)
        .mount(&authorization_server)
        .await;
    Mock::given(method("POST"))
        .and(path(mcp_path))
        .and(header("user-agent", RESOURCE_USER_AGENT))
        .and(header("x-api-key", RESOURCE_API_KEY))
        .and(header(
            "authorization",
            format!("Bearer {REFRESHED_ACCESS_TOKEN}"),
        ))
        .respond_with(|request: &Request| {
            let body: Value = match request.body_json() {
                Ok(body) => body,
                Err(_) => {
                    return ResponseTemplate::new(400).set_body_string("invalid JSON-RPC request");
                }
            };
            match body.get("method").and_then(Value::as_str) {
                Some("initialize") => ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0",
                    "id": body.get("id").cloned().unwrap_or(Value::Null),
                    "result": {
                        "protocolVersion": body
                            .pointer("/params/protocolVersion")
                            .cloned()
                            .unwrap_or_else(|| json!("2025-06-18")),
                        "capabilities": {},
                        "serverInfo": {
                            "name": "oauth-startup-test",
                            "version": "0.0.0-test",
                        },
                    },
                })),
                Some("notifications/initialized") => ResponseTemplate::new(202),
                method => ResponseTemplate::new(400)
                    .set_body_string(format!("unexpected JSON-RPC method: {method:?}")),
            }
        })
        .expect(2)
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    let with_headers_helper = matches!(scenario, OAuthStartupScenario::GatewayHeadersHelper);

    // Credential storage resolves CODEX_HOME from the process environment.
    // Run the client half of the test in an ignored helper test so it can use
    // an isolated home without mutating the parent test runner's environment.
    let mut command = Command::new(std::env::current_exe()?);
    command
        .args(["oauth_startup_child", "--exact", "--ignored", "--nocapture"])
        .env("CODEX_HOME", codex_home.path())
        .env(CHILD_SERVER_URL_ENV, server_url)
        .env(CHILD_RESOURCE_API_KEY_ENV, RESOURCE_API_KEY)
        .env("MCP_TEST_AMBIENT_SECRET", "must-not-reach-helper");
    if with_headers_helper {
        command.env(
            CHILD_HELPER_COMMAND_ENV,
            format!(
                "\"{}\" --http-headers-helper",
                cargo_bin("test_streamable_http_server")?.display(),
            ),
        );
    }
    let status = command.status().await?;
    assert!(status.success(), "OAuth startup child failed: {status}");
    if with_headers_helper {
        let requests = server.received_requests().await.unwrap_or_default();
        assert!(requests.iter().all(|request| {
            request
                .headers
                .get("proxy-authorization")
                .is_some_and(|value| value == "Bearer gateway-token")
        }));
    }
    let authorization_requests = authorization_server
        .received_requests()
        .await
        .ok_or_else(|| anyhow::anyhow!("authorization server should record requests"))?;
    assert_eq!(authorization_requests.len(), 1);
    assert_eq!(authorization_requests[0].headers.get("x-api-key"), None);
    assert_eq!(
        authorization_requests[0].headers.get("proxy-authorization"),
        None
    );
    assert_eq!(
        authorization_requests[0]
            .headers
            .get("user-agent")
            .map(http::HeaderValue::as_bytes),
        Some(MCP_USER_AGENT.as_bytes())
    );
    server.verify().await;
    authorization_server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn reports_auth_status_for_persisted_credentials() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;

    let status = Command::new(std::env::current_exe()?)
        .args([
            "persisted_credentials_auth_status_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .status()
        .await?;

    assert!(
        status.success(),
        "persisted credentials auth status child failed: {status}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn identifies_expired_unrefreshable_token_startup_error() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "authorization_endpoint": format!("{}/oauth/authorize", server.uri()),
            "token_endpoint": format!("{}/oauth/token", server.uri()),
        })))
        .expect(1)
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    let status = Command::new(std::env::current_exe()?)
        .args([
            "expired_unrefreshable_startup_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .env(CHILD_SERVER_URL_ENV, format!("{}/mcp", server.uri()))
        .status()
        .await?;

    assert!(
        status.success(),
        "expired OAuth startup child failed: {status}"
    );
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by reports_auth_status_for_persisted_credentials"]
async fn persisted_credentials_auth_status_child() -> anyhow::Result<()> {
    let first_login_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "authorization_endpoint": format!("{}/oauth/authorize", first_login_server.uri()),
            "token_endpoint": format!("{}/oauth/token", first_login_server.uri()),
        })))
        .expect(1)
        .mount(&first_login_server)
        .await;

    let status = auth_status(&format!("{}/mcp", first_login_server.uri())).await?;
    assert_eq!(status, McpAuthState::LoggedOut(McpLoginRequirement::Login));
    first_login_server.verify().await;

    let response = OAuthTokenResponse::new(
        AccessToken::new(EXPIRED_ACCESS_TOKEN.to_string()),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    let tokens = StoredOAuthTokens {
        server_name: SERVER_NAME.to_string(),
        url: UNREFRESHABLE_SERVER_URL.to_string(),
        client_id: "test-client-id".to_string(),
        token_response: WrappedOAuthTokenResponse(response),
        expires_at: Some(0),
    };
    save_oauth_tokens(
        SERVER_NAME,
        &tokens,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;

    let status = auth_status(UNREFRESHABLE_SERVER_URL).await?;
    assert_eq!(
        status,
        McpAuthState::LoggedOut(McpLoginRequirement::Reauthentication)
    );

    let response = OAuthTokenResponse::new(
        AccessToken::new("unexpired-access-token".to_string()),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis() as u64;
    let tokens = StoredOAuthTokens {
        server_name: SERVER_NAME.to_string(),
        url: UNEXPIRED_SERVER_URL.to_string(),
        client_id: "test-client-id".to_string(),
        token_response: WrappedOAuthTokenResponse(response),
        expires_at: Some(now.saturating_add(/*rhs*/ 60_000)),
    };
    save_oauth_tokens(
        SERVER_NAME,
        &tokens,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;

    let status = auth_status(UNEXPIRED_SERVER_URL).await?;
    assert_eq!(status, McpAuthState::OAuth);

    let mut response = OAuthTokenResponse::new(
        AccessToken::new(EXPIRED_ACCESS_TOKEN.to_string()),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    response.set_refresh_token(Some(RefreshToken::new(REFRESH_TOKEN.to_string())));
    let tokens = StoredOAuthTokens {
        server_name: SERVER_NAME.to_string(),
        url: REFRESHABLE_SERVER_URL.to_string(),
        client_id: "test-client-id".to_string(),
        token_response: WrappedOAuthTokenResponse(response),
        expires_at: Some(0),
    };
    save_oauth_tokens(
        SERVER_NAME,
        &tokens,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;

    let status = auth_status(REFRESHABLE_SERVER_URL).await?;
    assert_eq!(status, McpAuthState::OAuth);
    Ok(())
}

async fn auth_status(server_url: &str) -> anyhow::Result<McpAuthState> {
    determine_streamable_http_auth_status(
        SERVER_NAME,
        server_url,
        /*bearer_token_env_var*/ None,
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
        Environment::default_for_tests().get_http_client(),
        OAuthDiscoveryTimeout::LOCAL,
        StreamableHttpRedirectMode::Legacy,
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by refreshes_expired_persisted_token_before_initialize"]
async fn oauth_startup_child() -> anyhow::Result<()> {
    let server_url = std::env::var(CHILD_SERVER_URL_ENV)?;

    // Save an expired access token with a valid refresh token so startup must
    // refresh before sending the initialize request.
    let mut response = OAuthTokenResponse::new(
        AccessToken::new(EXPIRED_ACCESS_TOKEN.to_string()),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    response.set_refresh_token(Some(RefreshToken::new(REFRESH_TOKEN.to_string())));
    response.set_expires_in(Some(&Duration::from_secs(7200)));
    let tokens = StoredOAuthTokens {
        server_name: SERVER_NAME.to_string(),
        url: server_url.clone(),
        client_id: "test-client-id".to_string(),
        token_response: WrappedOAuthTokenResponse(response),
        expires_at: Some(0),
    };
    save_oauth_tokens(
        SERVER_NAME,
        &tokens,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;

    // This mirrors create_client's transport and initialization setup, except
    // it omits the direct bearer token. Supplying that token would bypass the
    // persisted OAuth credentials and the startup refresh under test.
    let mut http_client = Environment::default_for_tests().get_http_client();
    if let Ok(helper_command) = std::env::var(CHILD_HELPER_COMMAND_ENV) {
        http_client = with_http_headers_helper(
            http_client,
            &server_url,
            &helper_command,
            std::env::current_dir()?,
        )?;
    }
    let client = RmcpClient::new_streamable_http_client(
        SERVER_NAME,
        &server_url,
        /*bearer_token*/ None,
        Some(HashMap::from([(
            "User-Agent".to_string(),
            RESOURCE_USER_AGENT.to_string(),
        )])),
        Some(HashMap::from([(
            "X-Api-Key".to_string(),
            CHILD_RESOURCE_API_KEY_ENV.to_string(),
        )])),
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
        http_client,
        /*auth_provider*/ None,
    )
    .await?;

    initialize_client(&client).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by identifies_expired_unrefreshable_token_startup_error"]
async fn expired_unrefreshable_startup_child() -> anyhow::Result<()> {
    let server_url = std::env::var(CHILD_SERVER_URL_ENV)?;
    let response = OAuthTokenResponse::new(
        AccessToken::new(EXPIRED_ACCESS_TOKEN.to_string()),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    let tokens = StoredOAuthTokens {
        server_name: SERVER_NAME.to_string(),
        url: server_url.clone(),
        client_id: "test-client-id".to_string(),
        token_response: WrappedOAuthTokenResponse(response),
        expires_at: Some(0),
    };
    save_oauth_tokens(
        SERVER_NAME,
        &tokens,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;

    let client = RmcpClient::new_streamable_http_client(
        SERVER_NAME,
        &server_url,
        /*bearer_token*/ None,
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
        Environment::default_for_tests().get_http_client(),
        /*auth_provider*/ None,
    )
    .await?;

    let error = initialize_client(&client)
        .await
        .expect_err("expired token without a refresh token should fail startup");
    assert!(is_authentication_required_error(&error));
    Ok(())
}
