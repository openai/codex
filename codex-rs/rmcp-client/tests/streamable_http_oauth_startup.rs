mod streamable_http_test_support;

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use codex_rmcp_client::OAUTH_REFRESH_REAUTHENTICATION_REQUIRED_ERROR;
use codex_rmcp_client::RmcpClient;
use codex_rmcp_client::StoredOAuthTokens;
use codex_rmcp_client::WrappedOAuthTokenResponse;
use codex_rmcp_client::save_oauth_tokens;
use oauth2::AccessToken;
use oauth2::RefreshToken;
use oauth2::basic::BasicTokenType;
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
use streamable_http_test_support::initialize_client_with_timeout;

const SERVER_NAME: &str = "test-streamable-http-oauth-startup";
const EXPIRED_ACCESS_TOKEN: &str = "expired-access-token";
const REFRESH_TOKEN: &str = "valid-refresh-token";
const REFRESHED_ACCESS_TOKEN: &str = "refreshed-access-token";
const REFRESHED_REFRESH_TOKEN: &str = "rotated-refresh-token";
const CHILD_SERVER_URL_ENV: &str = "MCP_TEST_OAUTH_STARTUP_SERVER_URL";
const CHILD_SCENARIO_ENV: &str = "MCP_TEST_OAUTH_STARTUP_SCENARIO";
const CREDENTIALS_FILENAME: &str = ".credentials.json";
const SCENARIO_SUCCESS: &str = "success";
const SCENARIO_INVALID_GRANT: &str = "invalid_grant";
const SCENARIO_HANDSHAKE_TIMEOUT_AFTER_REFRESH: &str = "handshake_timeout_after_refresh";
const SCENARIO_REFRESH_TIMEOUT: &str = "refresh_timeout";
const SCENARIO_TRANSIENT_FAILURE: &str = "transient_failure";
const SCENARIO_LOAD_FAILURE: &str = "load_failure";
const SCENARIO_PERSIST_FAILURE: &str = "persist_failure";

#[derive(Clone, Copy)]
enum PersistedCredentialsAtInitialize {
    Refreshed,
    Stale,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn oauth_startup_refresh_and_storage_scenarios() -> anyhow::Result<()> {
    for scenario in [
        SCENARIO_SUCCESS,
        SCENARIO_INVALID_GRANT,
        SCENARIO_TRANSIENT_FAILURE,
        SCENARIO_REFRESH_TIMEOUT,
        SCENARIO_HANDSHAKE_TIMEOUT_AFTER_REFRESH,
        SCENARIO_LOAD_FAILURE,
        SCENARIO_PERSIST_FAILURE,
    ] {
        run_parent_scenario(scenario).await?;
    }
    Ok(())
}

async fn run_parent_scenario(scenario: &str) -> anyhow::Result<()> {
    let server = MockServer::start().await;
    let codex_home = TempDir::new()?;
    if scenario != SCENARIO_LOAD_FAILURE {
        mount_oauth_metadata(&server).await;
    }

    match scenario {
        SCENARIO_SUCCESS | SCENARIO_PERSIST_FAILURE | SCENARIO_HANDSHAKE_TIMEOUT_AFTER_REFRESH => {
            mount_successful_refresh(&server, /*expected_calls*/ 1).await;
        }
        SCENARIO_INVALID_GRANT => {
            mount_token_response(
                &server,
                ResponseTemplate::new(400).set_body_json(json!({
                    "error": "invalid_grant",
                    "error_description": "refresh token revoked",
                })),
                /*expected_calls*/ 1,
            )
            .await;
        }
        SCENARIO_TRANSIENT_FAILURE => {
            mount_token_response(
                &server,
                ResponseTemplate::new(503).set_body_json(json!({
                    "error": "temporarily_unavailable",
                    "error_description": "HTTP 503 Service Unavailable from token endpoint",
                })),
                /*expected_calls*/ 1,
            )
            .await;
        }
        SCENARIO_REFRESH_TIMEOUT => {
            mount_token_response(
                &server,
                successful_refresh_response().set_delay(Duration::from_millis(250)),
                /*expected_calls*/ 1,
            )
            .await;
        }
        SCENARIO_LOAD_FAILURE => {}
        other => anyhow::bail!("unknown OAuth startup scenario: {other}"),
    }

    let credentials_path = codex_home.path().join(CREDENTIALS_FILENAME);
    match scenario {
        SCENARIO_SUCCESS => {
            mount_successful_mcp_requests(
                &server,
                /*expected_calls*/ 2,
                credentials_path.clone(),
                PersistedCredentialsAtInitialize::Refreshed,
            )
            .await;
        }
        SCENARIO_PERSIST_FAILURE => {
            mount_successful_mcp_requests(
                &server,
                /*expected_calls*/ 3,
                credentials_path.clone(),
                PersistedCredentialsAtInitialize::Stale,
            )
            .await;
        }
        SCENARIO_HANDSHAKE_TIMEOUT_AFTER_REFRESH => {
            Mock::given(method("POST"))
                .and(path("/mcp"))
                .and(header(
                    "authorization",
                    format!("Bearer {REFRESHED_ACCESS_TOKEN}"),
                ))
                .respond_with(ResponseTemplate::new(500).set_delay(Duration::from_millis(250)))
                .expect(1)
                .mount(&server)
                .await;
        }
        _ => {
            Mock::given(method("POST"))
                .and(path("/mcp"))
                .respond_with(ResponseTemplate::new(500))
                .expect(0)
                .mount(&server)
                .await;
        }
    }

    run_oauth_startup_child(&server, scenario, &codex_home).await?;
    if scenario == SCENARIO_HANDSHAKE_TIMEOUT_AFTER_REFRESH {
        let persisted = fs::read_to_string(credentials_path)?;
        assert!(
            persisted.contains(REFRESHED_ACCESS_TOKEN)
                && persisted.contains(REFRESHED_REFRESH_TOKEN),
            "rotated credentials were lost when the handshake timed out: {persisted}"
        );
    }
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by OAuth startup integration tests"]
async fn oauth_startup_child() -> anyhow::Result<()> {
    let server_url = std::env::var(CHILD_SERVER_URL_ENV)?;
    let scenario = std::env::var(CHILD_SCENARIO_ENV)?;
    let credentials_path = credentials_path()?;

    let expected_read_cause = if scenario == SCENARIO_LOAD_FAILURE {
        fs::create_dir(&credentials_path)?;
        Some(
            fs::read_to_string(&credentials_path)
                .expect_err("credentials directory should reject file reads")
                .to_string(),
        )
    } else {
        save_expired_tokens(&server_url)?;
        None
    };

    // This mirrors create_client's transport and initialization setup, except
    // it omits the direct bearer token. Supplying that token would bypass the
    // persisted OAuth credentials and the startup refresh under test.
    let client = RmcpClient::new_streamable_http_client(
        SERVER_NAME,
        &server_url,
        /*bearer_token*/ None,
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        OAuthCredentialsStoreMode::File,
        Environment::default_for_tests().get_http_client(),
        /*auth_provider*/ None,
    )
    .await;

    if scenario == SCENARIO_LOAD_FAILURE {
        let error = match client {
            Ok(_) => anyhow::bail!("credential read failure should stop client construction"),
            Err(error) => format!("{error:#}"),
        };
        assert!(
            error.contains("failed to read OAuth credentials for MCP server"),
            "unexpected credential load failure: {error}"
        );
        let expected_read_cause =
            expected_read_cause.expect("load failure should capture the operating-system cause");
        assert!(
            error.contains(&expected_read_cause),
            "credential storage cause should be preserved: {error}"
        );
        assert!(
            !error.contains("Auth required"),
            "credential storage failure should not be classified as unauthenticated: {error}"
        );
        return Ok(());
    }
    let client = client?;

    let original_permissions = fs::metadata(&credentials_path)?.permissions();
    if scenario == SCENARIO_PERSIST_FAILURE {
        let mut readonly_permissions = original_permissions.clone();
        readonly_permissions.set_readonly(true);
        fs::set_permissions(&credentials_path, readonly_permissions)?;
        let stale_credentials = fs::read_to_string(&credentials_path)?;
        let write_error = fs::write(&credentials_path, &stale_credentials)
            .expect_err("read-only credentials file should reject writes");
        assert_eq!(write_error.kind(), std::io::ErrorKind::PermissionDenied);
    }

    let result = if matches!(
        scenario.as_str(),
        SCENARIO_REFRESH_TIMEOUT | SCENARIO_HANDSHAKE_TIMEOUT_AFTER_REFRESH
    ) {
        initialize_client_with_timeout(&client, Duration::from_millis(50)).await
    } else {
        initialize_client(&client).await
    };
    match scenario.as_str() {
        SCENARIO_SUCCESS => result?,
        SCENARIO_INVALID_GRANT => {
            let error = format!(
                "{:#}",
                result.expect_err("permanent refresh failure should stop startup")
            );
            assert!(
                error.contains(OAUTH_REFRESH_REAUTHENTICATION_REQUIRED_ERROR),
                "unexpected reauthentication failure: {error}"
            );
            assert!(
                !error.contains("invalid_grant") && !error.contains("refresh token revoked"),
                "revoked-token provider details should be hidden: {error}"
            );
        }
        SCENARIO_TRANSIENT_FAILURE => {
            let error = format!(
                "{:#}",
                result.expect_err("transient refresh failure should stop startup")
            );
            assert!(
                error.contains("OAuth token endpoint refresh failed"),
                "unexpected transient refresh failure: {error}"
            );
            assert!(
                error.contains("temporarily_unavailable")
                    && error.contains("HTTP 503 Service Unavailable from token endpoint"),
                "transient token endpoint status and cause should be preserved: {error}"
            );
            assert!(
                !error.contains(OAUTH_REFRESH_REAUTHENTICATION_REQUIRED_ERROR),
                "transient refresh failure should not request reauthentication: {error}"
            );
        }
        SCENARIO_REFRESH_TIMEOUT | SCENARIO_HANDSHAKE_TIMEOUT_AFTER_REFRESH => {
            let error = format!(
                "{:#}",
                result.expect_err("slow startup refresh should consume the initialize timeout")
            );
            assert!(
                error.contains("timed out handshaking with MCP server after 50ms"),
                "startup refresh escaped the initialize timeout budget: {error}"
            );
        }
        SCENARIO_PERSIST_FAILURE => {
            result?;
            let stale_credentials = fs::read_to_string(&credentials_path)?;
            assert!(
                stale_credentials.contains(EXPIRED_ACCESS_TOKEN),
                "failed pre-handshake and post-handshake writes should remain stale"
            );
            fs::set_permissions(&credentials_path, original_permissions)?;
            client
                .list_tools(
                    /*params*/ None,
                    /*timeout*/ Some(Duration::from_secs(5)),
                )
                .await?;
            assert!(
                credentials_path.is_file(),
                "failed persistence should leave the credentials file in place"
            );
            let refreshed_credentials = fs::read_to_string(&credentials_path)?;
            assert!(
                refreshed_credentials.contains(REFRESHED_ACCESS_TOKEN),
                "dirty refreshed credentials should persist after writes recover"
            );
            assert!(
                refreshed_credentials.contains(REFRESHED_REFRESH_TOKEN),
                "the rotated refresh token should persist on retry"
            );
        }
        SCENARIO_LOAD_FAILURE => unreachable!(),
        other => anyhow::bail!("unknown OAuth startup scenario: {other}"),
    }
    Ok(())
}

async fn run_oauth_startup_child(
    server: &MockServer,
    scenario: &str,
    codex_home: &TempDir,
) -> anyhow::Result<()> {
    let server_url = format!("{}/mcp", server.uri());

    // Credential storage resolves CODEX_HOME from the process environment.
    // Run the client half of the test in an ignored helper test so it can use
    // an isolated home without mutating the parent test runner's environment.
    let status = Command::new(std::env::current_exe()?)
        .args(["oauth_startup_child", "--exact", "--ignored", "--nocapture"])
        .env("CODEX_HOME", codex_home.path())
        .env(CHILD_SERVER_URL_ENV, server_url)
        .env(CHILD_SCENARIO_ENV, scenario)
        .status()
        .await?;
    assert!(status.success(), "OAuth startup child failed: {status}");
    Ok(())
}

async fn mount_oauth_metadata(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "authorization_endpoint": format!("{}/oauth/authorize", server.uri()),
            "token_endpoint": format!("{}/oauth/token", server.uri()),
            "scopes_supported": [""],
        })))
        .expect(1)
        .mount(server)
        .await;
}

async fn mount_successful_refresh(server: &MockServer, expected_calls: u64) {
    mount_token_response(server, successful_refresh_response(), expected_calls).await;
}

fn successful_refresh_response() -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "access_token": REFRESHED_ACCESS_TOKEN,
        "token_type": "Bearer",
        "expires_in": 7200,
        "refresh_token": REFRESHED_REFRESH_TOKEN,
    }))
}

async fn mount_token_response(
    server: &MockServer,
    response: ResponseTemplate,
    expected_calls: u64,
) {
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains(format!(
            "refresh_token={REFRESH_TOKEN}"
        )))
        .respond_with(response)
        .expect(expected_calls)
        .mount(server)
        .await;
}

async fn mount_successful_mcp_requests(
    server: &MockServer,
    expected_calls: u64,
    credentials_path: PathBuf,
    persistence_expectation: PersistedCredentialsAtInitialize,
) {
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header(
            "authorization",
            format!("Bearer {REFRESHED_ACCESS_TOKEN}"),
        ))
        .respond_with(move |request: &Request| {
            let Ok(body): Result<Value, _> = request.body_json() else {
                return ResponseTemplate::new(400).set_body_string("invalid JSON-RPC request");
            };
            match body.get("method").and_then(Value::as_str) {
                Some("initialize") => {
                    let Ok(persisted) = fs::read_to_string(&credentials_path) else {
                        return ResponseTemplate::new(500)
                            .set_body_string("persisted credentials are unreadable");
                    };
                    match persistence_expectation {
                        PersistedCredentialsAtInitialize::Refreshed => {
                            assert!(
                                persisted.contains(REFRESHED_ACCESS_TOKEN)
                                    && persisted.contains(REFRESHED_REFRESH_TOKEN),
                                "rotated credentials were not persisted before initialize: {persisted}"
                            );
                        }
                        PersistedCredentialsAtInitialize::Stale => {
                            assert!(
                                persisted.contains(EXPIRED_ACCESS_TOKEN)
                                    && !persisted.contains(REFRESHED_ACCESS_TOKEN),
                                "failed credential write should leave stale storage before initialize: {persisted}"
                            );
                        }
                    }
                    ResponseTemplate::new(200).set_body_json(json!({
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
                    }))
                }
                Some("notifications/initialized") => ResponseTemplate::new(202),
                Some("tools/list") => ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0",
                    "id": body.get("id").cloned().unwrap_or(Value::Null),
                    "result": {
                        "tools": [],
                    },
                })),
                method => ResponseTemplate::new(400)
                    .set_body_string(format!("unexpected JSON-RPC method: {method:?}")),
            }
        })
        .expect(expected_calls)
        .mount(server)
        .await;
}

fn save_expired_tokens(server_url: &str) -> anyhow::Result<()> {
    let mut response = OAuthTokenResponse::new(
        AccessToken::new(EXPIRED_ACCESS_TOKEN.to_string()),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    response.set_refresh_token(Some(RefreshToken::new(REFRESH_TOKEN.to_string())));
    response.set_expires_in(Some(&Duration::from_secs(7200)));
    let tokens = StoredOAuthTokens {
        server_name: SERVER_NAME.to_string(),
        url: server_url.to_string(),
        client_id: "test-client-id".to_string(),
        token_response: WrappedOAuthTokenResponse(response),
        expires_at: Some(0),
    };
    save_oauth_tokens(SERVER_NAME, &tokens, OAuthCredentialsStoreMode::File)
}

fn credentials_path() -> anyhow::Result<PathBuf> {
    Ok(PathBuf::from(std::env::var("CODEX_HOME")?).join(CREDENTIALS_FILENAME))
}
