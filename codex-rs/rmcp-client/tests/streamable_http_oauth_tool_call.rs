//! Exercises OAuth recovery through a real MCP client, with isolated file credentials.

mod streamable_http_test_support;

use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use codex_rmcp_client::RmcpClient;
use codex_rmcp_client::StoredOAuthTokens;
use codex_rmcp_client::WrappedOAuthTokenResponse;
use codex_rmcp_client::is_authentication_required_error;
use codex_rmcp_client::save_oauth_tokens;
use codex_rmcp_client::stored_oauth_credentials;
use oauth2::AccessToken;
use oauth2::RefreshToken;
use oauth2::basic::BasicTokenType;
use pretty_assertions::assert_eq;
use rmcp::model::CallToolResult;
use rmcp::model::ContentBlock;
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
use wiremock::matchers::method;
use wiremock::matchers::path;

use streamable_http_test_support::initialize_client;

const SERVER_NAME: &str = "runtime-oauth";
const SCENARIO_ENV: &str = "MCP_TEST_RUNTIME_OAUTH_SCENARIO";

#[tokio::test]
async fn runtime_oauth_recovery() -> anyhow::Result<()> {
    run_scenarios(&[
        "no-refresh",
        "rejected",
        "malformed",
        "transient",
        "early-malformed",
        "early-transient",
        "refreshable",
        "unexpired",
        "unknown-expiry",
        "revoked",
    ])
    .await
}

#[tokio::test]
async fn startup_oauth_recovery() -> anyhow::Result<()> {
    run_scenarios(&[
        "startup-malformed",
        "startup-transient",
        "startup-refreshable",
    ])
    .await
}

async fn run_scenarios(scenarios: &[&str]) -> anyhow::Result<()> {
    for scenario in scenarios {
        let home = TempDir::new()?;
        let output = Command::new(std::env::current_exe()?)
            .args(["runtime_oauth_child", "--exact", "--ignored", "--nocapture"])
            .env("CODEX_HOME", home.path())
            .env(SCENARIO_ENV, scenario)
            .output()
            .await?;
        assert!(
            output.status.success(),
            "{scenario}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

#[tokio::test]
#[ignore = "spawned by OAuth recovery tests with an isolated CODEX_HOME"]
async fn runtime_oauth_child() -> anyhow::Result<()> {
    let scenario = std::env::var(SCENARIO_ENV)?;
    let mcp = MockServer::start().await;
    let authorization = MockServer::start().await;
    let server_url = format!("{}/mcp", mcp.uri());
    let startup = scenario.starts_with("startup-");
    let revoked = scenario == "revoked";
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issuer": server_url,
            "authorization_endpoint": format!("{}/authorize", authorization.uri()),
            "token_endpoint": format!("{}/token", authorization.uri()),
        })))
        .mount(&mcp)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(move |request: &Request| {
            let body: Value = request.body_json().expect("JSON-RPC request");
            match body["method"].as_str() {
                Some("initialize") => ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0", "id": body["id"],
                    "result": {
                        "protocolVersion": body["params"]["protocolVersion"],
                        "capabilities": {"tools": {}},
                        "serverInfo": {"name": "runtime-oauth", "version": "1"},
                    },
                })),
                Some("notifications/initialized") => ResponseTemplate::new(202),
                Some("tools/list") => ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0", "id": body["id"], "result": {"tools": []},
                })),
                Some("tools/call") if revoked => ResponseTemplate::new(401)
                    .insert_header("www-authenticate", "Bearer error=\"invalid_token\""),
                Some("tools/call") => ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0", "id": body["id"],
                    "result": {"content": [{"type": "text", "text": "tool ran"}]},
                })),
                _ => ResponseTemplate::new(400),
            }
        })
        .mount(&mcp)
        .await;
    // A zero lifetime expires after the handshake; 30 seconds exercises the early-refresh
    // window while the token remains valid. Neither case needs sleeps or clock changes.
    let expires_in = match scenario.as_str() {
        "unexpired" | "startup-refreshable" => Some(3600),
        "early-malformed" | "early-transient" => Some(30),
        "unknown-expiry" => None,
        _ => Some(0),
    };
    let bootstrap_response = match scenario.as_str() {
        "startup-malformed" => ResponseTemplate::new(200).set_body_raw("{", "application/json"),
        "startup-transient" => {
            ResponseTemplate::new(503).set_body_json(json!({"error": "temporarily_unavailable"}))
        }
        _ => ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "initial-token", "token_type": "Bearer",
            "refresh_token": "refresh-token", "expires_in": expires_in,
        })),
    };
    let bootstrap = Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(bootstrap_response)
        .mount_as_scoped(&authorization)
        .await;
    let mut response = OAuthTokenResponse::new(
        AccessToken::new("old-token".to_string()),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    if !revoked {
        response.set_refresh_token(Some(RefreshToken::new("refresh-token".to_string())));
    }
    let tokens = StoredOAuthTokens {
        server_name: SERVER_NAME.to_string(),
        url: server_url.clone(),
        issuer: Some(server_url.clone()),
        client_id: "test-client".to_string(),
        token_response: WrappedOAuthTokenResponse(response),
        expires_at: Some(if revoked {
            u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())? + 3_600_000
        } else {
            0
        }),
    };
    save_oauth_tokens(
        SERVER_NAME,
        &tokens,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )
    .await?;
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
    let initialization = initialize_client(&client).await;
    if startup {
        if scenario == "startup-refreshable" {
            initialization?;
            let tools = client
                .list_tools(/*params*/ None, Some(Duration::from_secs(5)))
                .await?;
            assert!(tools.tools.is_empty());
        } else {
            let error = initialization.expect_err("expired credentials must require reconnect");
            assert!(is_authentication_required_error(&error), "{error:#}");
        }
        let requests = mcp.received_requests().await.expect("request recording");
        let rpc_requests = requests
            .iter()
            .filter(|request| request.method.as_str() == "POST" && request.url.path() == "/mcp")
            .collect::<Vec<_>>();
        let methods = rpc_requests
            .iter()
            .map(|request| {
                request.body_json::<Value>().expect("JSON-RPC request")["method"].clone()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            methods,
            if scenario == "startup-refreshable" {
                vec![
                    json!("initialize"),
                    json!("notifications/initialized"),
                    json!("tools/list"),
                ]
            } else {
                Vec::<Value>::new()
            }
        );
        for request in rpc_requests {
            assert_eq!(
                request
                    .headers
                    .get("authorization")
                    .and_then(|value| value.to_str().ok()),
                Some("Bearer initial-token")
            );
        }
        let after = stored_oauth_credentials(
            SERVER_NAME,
            &server_url,
            OAuthCredentialsStoreMode::File,
            AuthKeyringBackendKind::default(),
        )?
        .expect("credentials are not deleted");
        if scenario != "startup-refreshable" {
            assert_eq!(after, tokens);
        }
        assert_eq!(
            authorization
                .received_requests()
                .await
                .expect("request recording")
                .len(),
            1
        );
        return Ok(());
    }
    initialization?;
    drop(bootstrap);
    let mut before = stored_oauth_credentials(
        SERVER_NAME,
        &server_url,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?
    .expect("saved credentials");
    if scenario == "no-refresh" {
        before.token_response.0.set_refresh_token(None);
        save_oauth_tokens(
            SERVER_NAME,
            &before,
            OAuthCredentialsStoreMode::File,
            AuthKeyringBackendKind::default(),
        )
        .await?;
    }
    let refresh_response = match scenario.as_str() {
        "refreshable" => ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "renewed-token", "token_type": "Bearer", "expires_in": 3600,
        })),
        "malformed" | "early-malformed" => {
            ResponseTemplate::new(200).set_body_raw("{", "application/json")
        }
        "transient" | "early-transient" => {
            ResponseTemplate::new(503).set_body_json(json!({"error": "temporarily_unavailable"}))
        }
        _ => ResponseTemplate::new(400).set_body_json(json!({
            "error": "invalid_grant", "error_description": "private provider details",
        })),
    };
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .respond_with(refresh_response)
        .expect(
            if matches!(
                scenario.as_str(),
                "no-refresh" | "unexpired" | "unknown-expiry" | "revoked"
            ) {
                0
            } else {
                1
            },
        )
        .mount(&authorization)
        .await;

    let result = client
        .call_tool(
            "echo".to_string(),
            /*arguments*/ None,
            |_| Ok(None),
            Some(Duration::from_secs(5)),
        )
        .await;
    match scenario.as_str() {
        "no-refresh" | "rejected" | "malformed" | "transient" => {
            let mut expected = CallToolResult::error(vec![ContentBlock::text(
                "MCP authentication required. Reconnect to continue using this server.",
            )]);
            expected.meta = Some(
                serde_json::Map::from_iter([(
                    "mcp/www_authenticate".to_string(),
                    json!("Bearer error=\"invalid_token\""),
                )])
                .into(),
            );
            assert_eq!(result?, expected);
        }
        "revoked" => {
            let mut expected =
                CallToolResult::error(vec![ContentBlock::text("Authentication required")]);
            expected.meta = Some(
                serde_json::Map::from_iter([(
                    "mcp/www_authenticate".to_string(),
                    json!(["Bearer error=\"invalid_token\""]),
                )])
                .into(),
            );
            assert_eq!(result?, expected);
        }
        "early-malformed" | "early-transient" => {
            let error = result.expect_err("early refresh failures must remain ordinary errors");
            assert!(!is_authentication_required_error(&error));
        }
        "refreshable" | "unexpired" | "unknown-expiry" => {
            let result = result?;
            assert_eq!(result.content, vec![ContentBlock::text("tool ran")]);
            assert_eq!(result.meta, None);
        }
        _ => unreachable!("scenario selected by parent test"),
    }
    let tool_calls = mcp
        .received_requests()
        .await
        .expect("request recording")
        .iter()
        .filter(|request| {
            request
                .body_json::<Value>()
                .ok()
                .is_some_and(|body| body["method"] == "tools/call")
        })
        .count();
    assert_eq!(
        tool_calls,
        usize::from(matches!(
            scenario.as_str(),
            "refreshable" | "unexpired" | "unknown-expiry" | "revoked"
        ))
    );
    let after = stored_oauth_credentials(
        SERVER_NAME,
        &server_url,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?
    .expect("credentials are not deleted");
    if scenario != "refreshable" {
        assert_eq!(before, after);
    }
    authorization.verify().await;
    Ok(())
}
