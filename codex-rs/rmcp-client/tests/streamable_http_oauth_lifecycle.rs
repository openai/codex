mod streamable_http_test_support;

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use codex_rmcp_client::RmcpClient;
use codex_rmcp_client::StoredOAuthTokens;
use codex_rmcp_client::WrappedOAuthTokenResponse;
use codex_rmcp_client::save_oauth_tokens;
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
use wiremock::Respond;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use streamable_http_test_support::initialize_client;

const SERVER_NAME: &str = "test-streamable-http-oauth-lifecycle";
const INITIAL_ACCESS_TOKEN: &str = "initial-expired-access-token";
const INITIAL_REFRESH_TOKEN: &str = "initial-refresh-token";
const SHORT_LIVED_ACCESS_TOKEN: &str = "short-lived-access-token";
const SHORT_LIVED_REFRESH_TOKEN: &str = "short-lived-refresh-token";
const LONG_LIVED_ACCESS_TOKEN: &str = "long-lived-access-token";
const LONG_LIVED_REFRESH_TOKEN: &str = "long-lived-refresh-token";
const CHILD_SERVER_URL_ENV: &str = "MCP_TEST_OAUTH_LIFECYCLE_SERVER_URL";
const CHILD_CHECKPOINT_URL_ENV: &str = "MCP_TEST_OAUTH_LIFECYCLE_CHECKPOINT_URL";
const CHILD_SCENARIO_ENV: &str = "MCP_TEST_OAUTH_LIFECYCLE_SCENARIO";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scenario {
    RefreshSucceeds,
    RefreshTokenExpired,
}

impl Scenario {
    fn as_env(self) -> &'static str {
        match self {
            Self::RefreshSucceeds => "refresh-succeeds",
            Self::RefreshTokenExpired => "refresh-token-expired",
        }
    }

    fn from_env(value: &str) -> anyhow::Result<Self> {
        match value {
            "refresh-succeeds" => Ok(Self::RefreshSucceeds),
            "refresh-token-expired" => Ok(Self::RefreshTokenExpired),
            _ => anyhow::bail!("unknown OAuth lifecycle scenario: {value}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TimelineEvent {
    Refresh(String),
    Mcp {
        method: String,
        authorization: String,
    },
    IdleStarted,
    IdleFinished,
}

#[derive(Clone)]
struct Timeline {
    events: Arc<Mutex<Vec<TimelineEvent>>>,
}

impl Timeline {
    fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn push(&self, event: TimelineEvent) {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(event);
    }

    fn snapshot(&self) -> Vec<TimelineEvent> {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

struct TokenResponder {
    scenario: Scenario,
    timeline: Timeline,
    calls: AtomicUsize,
}

impl Respond for TokenResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body = String::from_utf8_lossy(&request.body);
        let refresh_token = if body.contains(INITIAL_REFRESH_TOKEN) {
            INITIAL_REFRESH_TOKEN
        } else if body.contains(SHORT_LIVED_REFRESH_TOKEN) {
            SHORT_LIVED_REFRESH_TOKEN
        } else {
            panic!("unexpected refresh request body: {body}");
        };
        self.timeline
            .push(TimelineEvent::Refresh(refresh_token.to_string()));

        match self.calls.fetch_add(1, Ordering::SeqCst) {
            0 => ResponseTemplate::new(200).set_body_json(json!({
                "access_token": SHORT_LIVED_ACCESS_TOKEN,
                "token_type": "Bearer",
                "expires_in": 32,
                "refresh_token": SHORT_LIVED_REFRESH_TOKEN,
            })),
            1 if self.scenario == Scenario::RefreshSucceeds => ResponseTemplate::new(200)
                .set_body_json(json!({
                    "access_token": LONG_LIVED_ACCESS_TOKEN,
                    "token_type": "Bearer",
                    "expires_in": 7200,
                    "refresh_token": LONG_LIVED_REFRESH_TOKEN,
                })),
            1 | 2 if self.scenario == Scenario::RefreshTokenExpired => ResponseTemplate::new(400)
                .set_body_json(json!({
                    "error": "invalid_grant",
                    "error_description": "refresh token expired",
                })),
            call => panic!("unexpected OAuth token request #{call}"),
        }
    }
}

#[derive(Clone)]
struct McpResponder {
    timeline: Timeline,
}

impl Respond for McpResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: Value = match request.body_json() {
            Ok(body) => body,
            Err(error) => panic!("invalid JSON-RPC request: {error}"),
        };
        let method = match body.get("method").and_then(Value::as_str) {
            Some(method) => method,
            None => panic!("JSON-RPC request missing method: {body}"),
        };
        let authorization = match request
            .headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
        {
            Some(authorization) => authorization.to_string(),
            None => panic!("MCP request missing authorization header"),
        };
        self.timeline.push(TimelineEvent::Mcp {
            method: method.to_string(),
            authorization,
        });

        match method {
            "initialize" => ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0",
                "id": body.get("id").cloned().unwrap_or(Value::Null),
                "result": {
                    "protocolVersion": body
                        .pointer("/params/protocolVersion")
                        .cloned()
                        .unwrap_or_else(|| json!("2025-06-18")),
                    "capabilities": {},
                    "serverInfo": {
                        "name": "oauth-lifecycle-test",
                        "version": "0.0.0-test",
                    },
                },
            })),
            "notifications/initialized" => ResponseTemplate::new(202),
            "tools/list" => ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0",
                "id": body.get("id").cloned().unwrap_or(Value::Null),
                "result": {
                    "tools": [],
                },
            })),
            _ => ResponseTemplate::new(400)
                .set_body_string(format!("unexpected JSON-RPC method: {method}")),
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn refreshes_only_when_the_next_operation_starts_after_idle() -> anyhow::Result<()> {
    let timeline = run_scenario(Scenario::RefreshSucceeds).await?;

    assert_eq!(
        timeline,
        vec![
            TimelineEvent::Refresh(INITIAL_REFRESH_TOKEN.to_string()),
            TimelineEvent::Mcp {
                method: "initialize".to_string(),
                authorization: format!("Bearer {SHORT_LIVED_ACCESS_TOKEN}"),
            },
            TimelineEvent::Mcp {
                method: "notifications/initialized".to_string(),
                authorization: format!("Bearer {SHORT_LIVED_ACCESS_TOKEN}"),
            },
            TimelineEvent::IdleStarted,
            TimelineEvent::IdleFinished,
            TimelineEvent::Refresh(SHORT_LIVED_REFRESH_TOKEN.to_string()),
            TimelineEvent::Mcp {
                method: "tools/list".to_string(),
                authorization: format!("Bearer {LONG_LIVED_ACCESS_TOKEN}"),
            },
        ]
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn expired_refresh_token_before_next_operation_requires_reauthorization() -> anyhow::Result<()>
{
    let timeline = run_scenario(Scenario::RefreshTokenExpired).await?;

    assert_eq!(
        timeline,
        vec![
            TimelineEvent::Refresh(INITIAL_REFRESH_TOKEN.to_string()),
            TimelineEvent::Mcp {
                method: "initialize".to_string(),
                authorization: format!("Bearer {SHORT_LIVED_ACCESS_TOKEN}"),
            },
            TimelineEvent::Mcp {
                method: "notifications/initialized".to_string(),
                authorization: format!("Bearer {SHORT_LIVED_ACCESS_TOKEN}"),
            },
            TimelineEvent::IdleStarted,
            TimelineEvent::IdleFinished,
            TimelineEvent::Refresh(SHORT_LIVED_REFRESH_TOKEN.to_string()),
            TimelineEvent::Refresh(SHORT_LIVED_REFRESH_TOKEN.to_string()),
        ]
    );
    Ok(())
}

async fn run_scenario(scenario: Scenario) -> anyhow::Result<Vec<TimelineEvent>> {
    let server = MockServer::start().await;
    let timeline = Timeline::new();

    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "authorization_endpoint": format!("{}/oauth/authorize", server.uri()),
            "token_endpoint": format!("{}/oauth/token", server.uri()),
            "scopes_supported": [""],
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(TokenResponder {
            scenario,
            timeline: timeline.clone(),
            calls: AtomicUsize::new(0),
        })
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(McpResponder {
            timeline: timeline.clone(),
        })
        .mount(&server)
        .await;

    for (path, event) in [
        ("/checkpoint/idle-started", TimelineEvent::IdleStarted),
        ("/checkpoint/idle-finished", TimelineEvent::IdleFinished),
    ] {
        let timeline = timeline.clone();
        Mock::given(method("POST"))
            .and(wiremock::matchers::path(path))
            .respond_with(move |_: &Request| {
                timeline.push(event.clone());
                ResponseTemplate::new(204)
            })
            .mount(&server)
            .await;
    }

    let codex_home = TempDir::new()?;
    let status = Command::new(std::env::current_exe()?)
        .args([
            "oauth_lifecycle_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .env(CHILD_SERVER_URL_ENV, format!("{}/mcp", server.uri()))
        .env(CHILD_CHECKPOINT_URL_ENV, server.uri())
        .env(CHILD_SCENARIO_ENV, scenario.as_env())
        .status()
        .await?;
    assert!(status.success(), "OAuth lifecycle child failed: {status}");

    Ok(timeline.snapshot())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by OAuth lifecycle parent tests"]
async fn oauth_lifecycle_child() -> anyhow::Result<()> {
    let server_url = std::env::var(CHILD_SERVER_URL_ENV)?;
    let checkpoint_url = std::env::var(CHILD_CHECKPOINT_URL_ENV)?;
    let scenario = Scenario::from_env(&std::env::var(CHILD_SCENARIO_ENV)?)?;

    let mut response = OAuthTokenResponse::new(
        AccessToken::new(INITIAL_ACCESS_TOKEN.to_string()),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    response.set_refresh_token(Some(RefreshToken::new(INITIAL_REFRESH_TOKEN.to_string())));
    response.set_expires_in(Some(&Duration::from_secs(7200)));
    save_oauth_tokens(
        SERVER_NAME,
        &StoredOAuthTokens {
            server_name: SERVER_NAME.to_string(),
            url: server_url.clone(),
            client_id: "test-client-id".to_string(),
            token_response: WrappedOAuthTokenResponse(response),
            expires_at: Some(0),
        },
        OAuthCredentialsStoreMode::File,
    )?;

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
    .await?;
    initialize_client(&client).await?;

    post_checkpoint(&checkpoint_url, "idle-started").await?;
    tokio::time::sleep(Duration::from_secs(3)).await;
    post_checkpoint(&checkpoint_url, "idle-finished").await?;

    let result = client
        .list_tools(/*params*/ None, Some(Duration::from_secs(5)))
        .await;
    match scenario {
        Scenario::RefreshSucceeds => {
            assert_eq!(result?.tools, Vec::new());
        }
        Scenario::RefreshTokenExpired => {
            let error = result.expect_err("expired refresh token should fail the operation");
            assert!(
                error.to_string().contains("authorization required"),
                "unexpected expired refresh token error: {error:#}"
            );
        }
    }

    Ok(())
}

async fn post_checkpoint(base_url: &str, checkpoint: &str) -> anyhow::Result<()> {
    reqwest::Client::new()
        .post(format!("{base_url}/checkpoint/{checkpoint}"))
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}
