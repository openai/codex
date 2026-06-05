use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use futures::FutureExt;
use oauth2::AccessToken;
use oauth2::RefreshToken;
use oauth2::basic::BasicTokenType;
use pretty_assertions::assert_eq;
use rmcp::ErrorData as McpError;
use rmcp::handler::server::ServerHandler;
use rmcp::model::ClientCapabilities;
use rmcp::model::Implementation;
use rmcp::model::ListToolsResult;
use rmcp::model::ProtocolVersion;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerInfo;
use rmcp::service::RequestContext;
use rmcp::service::RoleServer;
use rmcp::transport::auth::OAuthTokenResponse;
use rmcp::transport::auth::VendorExtraTokenFields;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::io::DuplexStream;
use tokio::process::Command;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::Respond;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::*;
use crate::oauth::OAUTH_REQUEST_REFRESH_FAILED_ERROR;
use crate::oauth::WrappedOAuthTokenResponse;
use crate::oauth::save_oauth_tokens;

const SERVER_NAME: &str = "request-time-refresh-test";
const INITIAL_ACCESS_TOKEN: &str = "initial-access-token";
const INITIAL_REFRESH_TOKEN: &str = "initial-refresh-token";
const REFRESHED_ACCESS_TOKEN: &str = "refreshed-access-token";
const REFRESHED_REFRESH_TOKEN: &str = "refreshed-refresh-token";
const CHILD_SERVER_URL_ENV: &str = "MCP_TEST_REQUEST_TIME_REFRESH_SERVER_URL";
const CHILD_CHECKPOINT_URL_ENV: &str = "MCP_TEST_REQUEST_TIME_REFRESH_CHECKPOINT_URL";
const CHILD_SCENARIO_ENV: &str = "MCP_TEST_REQUEST_TIME_REFRESH_SCENARIO";
const CREDENTIALS_FILE: &str = ".credentials.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scenario {
    RefreshSucceeds,
    ProviderRejectsRefresh,
    PersistenceFailsThenRetries,
    ConcurrentRefreshes,
    IdleThenRefresh,
}

impl Scenario {
    fn as_env(self) -> &'static str {
        match self {
            Self::RefreshSucceeds => "refresh-succeeds",
            Self::ProviderRejectsRefresh => "provider-rejects-refresh",
            Self::PersistenceFailsThenRetries => "persistence-fails-then-retries",
            Self::ConcurrentRefreshes => "concurrent-refreshes",
            Self::IdleThenRefresh => "idle-then-refresh",
        }
    }

    fn from_env(value: &str) -> anyhow::Result<Self> {
        match value {
            "refresh-succeeds" => Ok(Self::RefreshSucceeds),
            "provider-rejects-refresh" => Ok(Self::ProviderRejectsRefresh),
            "persistence-fails-then-retries" => Ok(Self::PersistenceFailsThenRetries),
            "concurrent-refreshes" => Ok(Self::ConcurrentRefreshes),
            "idle-then-refresh" => Ok(Self::IdleThenRefresh),
            _ => anyhow::bail!("unknown request-time refresh scenario: {value}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TimelineEvent {
    IdleStarted,
    IdleFinished,
    Refresh,
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

#[derive(Clone)]
struct TokenResponder {
    scenario: Scenario,
    timeline: Timeline,
}

impl Respond for TokenResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body = String::from_utf8_lossy(&request.body);
        assert!(
            body.contains(INITIAL_REFRESH_TOKEN),
            "unexpected refresh request body: {body}"
        );
        self.timeline.push(TimelineEvent::Refresh);

        match self.scenario {
            Scenario::ProviderRejectsRefresh => ResponseTemplate::new(400).set_body_json(json!({
                "error": "invalid_grant",
                "error_description": "x".repeat(20_000),
            })),
            Scenario::ConcurrentRefreshes => {
                refreshed_token_response().set_delay(Duration::from_millis(100))
            }
            Scenario::RefreshSucceeds
            | Scenario::PersistenceFailsThenRetries
            | Scenario::IdleThenRefresh => refreshed_token_response(),
        }
    }
}

fn refreshed_token_response() -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "access_token": REFRESHED_ACCESS_TOKEN,
        "token_type": "Bearer",
        "expires_in": 7200,
        "refresh_token": REFRESHED_REFRESH_TOKEN,
    }))
}

#[derive(Clone)]
struct CountingServer {
    list_tools_calls: Arc<AtomicUsize>,
}

impl ServerHandler for CountingServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = std::result::Result<ListToolsResult, McpError>> + Send + '_ {
        self.list_tools_calls.fetch_add(1, Ordering::SeqCst);
        async {
            Ok(ListToolsResult {
                tools: Vec::new(),
                next_cursor: None,
                meta: None,
            })
        }
    }
}

#[derive(Clone)]
struct TestTransportFactory {
    server: CountingServer,
}

impl InProcessTransportFactory for TestTransportFactory {
    fn open(&self) -> BoxFuture<'static, io::Result<DuplexStream>> {
        let server = self.server.clone();
        async move {
            let (client_transport, server_transport) = tokio::io::duplex(64 * 1024);
            let _server_task = tokio::spawn(async move {
                if let Ok(service) = rmcp::serve_server(server, server_transport).await {
                    let _result = service.waiting().await;
                }
            });
            Ok(client_transport)
        }
        .boxed()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn proactive_refresh_succeeds_and_persists_before_request() -> anyhow::Result<()> {
    run_parent_scenario(Scenario::RefreshSucceeds).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn provider_rejection_is_bounded_and_stops_before_request() -> anyhow::Result<()> {
    run_parent_scenario(Scenario::ProviderRejectsRefresh).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn persistence_failure_does_not_abort_and_retries_without_refresh() -> anyhow::Result<()> {
    run_parent_scenario(Scenario::PersistenceFailsThenRetries).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn concurrent_near_expiry_requests_share_one_refresh() -> anyhow::Result<()> {
    run_parent_scenario(Scenario::ConcurrentRefreshes).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn near_expiry_credentials_do_not_refresh_while_idle_then_refresh_on_request()
-> anyhow::Result<()> {
    run_parent_scenario(Scenario::IdleThenRefresh).await
}

async fn run_parent_scenario(scenario: Scenario) -> anyhow::Result<()> {
    let server = MockServer::start().await;
    let timeline = Timeline::new();
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "authorization_endpoint": format!("{}/oauth/authorize", server.uri()),
            "token_endpoint": format!("{}/oauth/token", server.uri()),
            "scopes_supported": [""],
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(TokenResponder {
            scenario,
            timeline: timeline.clone(),
        })
        .expect(1)
        .mount(&server)
        .await;

    if scenario == Scenario::IdleThenRefresh {
        let idle_started_timeline = timeline.clone();
        Mock::given(method("POST"))
            .and(path("/checkpoint/idle-started"))
            .respond_with(move |_: &Request| {
                idle_started_timeline.push(TimelineEvent::IdleStarted);
                ResponseTemplate::new(204)
            })
            .expect(1)
            .mount(&server)
            .await;

        let idle_finished_timeline = timeline.clone();
        Mock::given(method("POST"))
            .and(path("/checkpoint/idle-finished"))
            .respond_with(move |_: &Request| {
                idle_finished_timeline.push(TimelineEvent::IdleFinished);
                assert_eq!(
                    idle_finished_timeline.snapshot(),
                    vec![TimelineEvent::IdleStarted, TimelineEvent::IdleFinished]
                );
                ResponseTemplate::new(204)
            })
            .expect(1)
            .mount(&server)
            .await;
    }

    let codex_home = TempDir::new()?;
    let status = Command::new(std::env::current_exe()?)
        .args([
            "rmcp_client::oauth_tests::oauth_request_time_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .env(CHILD_SERVER_URL_ENV, server.uri())
        .env(CHILD_CHECKPOINT_URL_ENV, server.uri())
        .env(CHILD_SCENARIO_ENV, scenario.as_env())
        .status()
        .await?;
    assert!(
        status.success(),
        "request-time refresh child failed: {status}"
    );

    if scenario == Scenario::IdleThenRefresh {
        assert_eq!(
            timeline.snapshot(),
            vec![
                TimelineEvent::IdleStarted,
                TimelineEvent::IdleFinished,
                TimelineEvent::Refresh,
            ]
        );
    }
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by request-time refresh parent tests"]
async fn oauth_request_time_child() -> anyhow::Result<()> {
    let server_url = std::env::var(CHILD_SERVER_URL_ENV)?;
    let checkpoint_url = std::env::var(CHILD_CHECKPOINT_URL_ENV)?;
    let scenario = Scenario::from_env(&std::env::var(CHILD_SCENARIO_ENV)?)?;
    let list_tools_calls = Arc::new(AtomicUsize::new(0));
    let client = Arc::new(
        RmcpClient::new_in_process_client(Arc::new(TestTransportFactory {
            server: CountingServer {
                list_tools_calls: Arc::clone(&list_tools_calls),
            },
        }))
        .await?,
    );
    initialize_in_process_client(&client).await?;

    let initial_tokens = near_expiry_initial_tokens(&server_url)?;
    save_oauth_tokens(
        SERVER_NAME,
        &initial_tokens,
        OAuthCredentialsStoreMode::File,
    )?;
    let initial_persisted = fs::read(credentials_path()?)?;
    let (_, runtime) = create_oauth_transport_and_runtime(
        SERVER_NAME,
        &initial_tokens.url,
        initial_tokens.clone(),
        OAuthCredentialsStoreMode::File,
        HeaderMap::new(),
        Environment::default_for_tests().get_http_client(),
    )
    .await?;
    let runtime_for_assertions = runtime.clone();
    {
        let mut state = client.state.lock().await;
        match &mut *state {
            ClientState::Ready { oauth, .. } => *oauth = Some(runtime),
            ClientState::Connecting { .. } => panic!("client was not initialized"),
            ClientState::Closed => panic!("client was unexpectedly closed"),
        }
    }

    match scenario {
        Scenario::RefreshSucceeds => {
            assert_eq!(list_tools(&client).await?.tools, Vec::new());
            assert_eq!(list_tools_calls.load(Ordering::SeqCst), 1);
            assert_refreshed_credentials_persisted()?;
        }
        Scenario::ProviderRejectsRefresh => {
            let error = match list_tools(&client).await {
                Ok(_) => panic!("provider rejection should fail before tools/list"),
                Err(error) => error,
            };
            assert_eq!(error.to_string(), OAUTH_REQUEST_REFRESH_FAILED_ERROR);
            assert_eq!(
                format!("{error:#}").chars().count(),
                OAUTH_REQUEST_REFRESH_FAILED_ERROR.chars().count()
            );
            assert_eq!(list_tools_calls.load(Ordering::SeqCst), 0);
            runtime_for_assertions.persist_if_needed().await?;
            assert_eq!(fs::read(credentials_path()?)?, initial_persisted);
        }
        Scenario::PersistenceFailsThenRetries => {
            let credentials_path = credentials_path()?;
            let original_permissions = fs::metadata(&credentials_path)?.permissions();
            let mut readonly_permissions = original_permissions.clone();
            readonly_permissions.set_readonly(true);
            fs::set_permissions(&credentials_path, readonly_permissions)?;

            assert_eq!(list_tools(&client).await?.tools, Vec::new());
            assert_eq!(list_tools_calls.load(Ordering::SeqCst), 1);
            assert_eq!(fs::read(&credentials_path)?, initial_persisted);

            fs::set_permissions(&credentials_path, original_permissions)?;
            assert_eq!(list_tools(&client).await?.tools, Vec::new());
            assert_eq!(list_tools_calls.load(Ordering::SeqCst), 2);
            assert_refreshed_credentials_persisted()?;
        }
        Scenario::ConcurrentRefreshes => {
            let (first, second) = tokio::join!(list_tools(&client), list_tools(&client));
            assert_eq!(first?.tools, Vec::new());
            assert_eq!(second?.tools, Vec::new());
            assert_eq!(list_tools_calls.load(Ordering::SeqCst), 2);
            assert_refreshed_credentials_persisted()?;
        }
        Scenario::IdleThenRefresh => {
            post_checkpoint(&checkpoint_url, "idle-started").await?;
            tokio::task::yield_now().await;
            post_checkpoint(&checkpoint_url, "idle-finished").await?;

            assert_eq!(list_tools(&client).await?.tools, Vec::new());
            assert_eq!(list_tools_calls.load(Ordering::SeqCst), 1);
            assert_refreshed_credentials_persisted()?;
        }
    }

    client.shutdown().await;
    Ok(())
}

fn near_expiry_initial_tokens(server_url: &str) -> anyhow::Result<StoredOAuthTokens> {
    let expires_in = Duration::from_secs(1);
    let mut response = OAuthTokenResponse::new(
        AccessToken::new(INITIAL_ACCESS_TOKEN.to_string()),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    response.set_refresh_token(Some(RefreshToken::new(INITIAL_REFRESH_TOKEN.to_string())));
    response.set_expires_in(Some(&expires_in));
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?;
    Ok(StoredOAuthTokens {
        server_name: SERVER_NAME.to_string(),
        url: format!("{server_url}/mcp"),
        client_id: "test-client-id".to_string(),
        token_response: WrappedOAuthTokenResponse(response),
        expires_at: Some(now.saturating_add(expires_in).as_millis() as u64),
    })
}

async fn list_tools(client: &RmcpClient) -> anyhow::Result<ListToolsResult> {
    client
        .list_tools(/*params*/ None, Some(Duration::from_secs(5)))
        .await
}

fn credentials_path() -> anyhow::Result<PathBuf> {
    Ok(PathBuf::from(std::env::var("CODEX_HOME")?).join(CREDENTIALS_FILE))
}

fn assert_refreshed_credentials_persisted() -> anyhow::Result<()> {
    let persisted: Value = serde_json::from_slice(&fs::read(credentials_path()?)?)?;
    let entries = match persisted.as_object() {
        Some(entries) => entries,
        None => anyhow::bail!("persisted credentials were not an object: {persisted}"),
    };
    assert_eq!(entries.len(), 1);
    let entry = match entries.values().next() {
        Some(entry) => entry,
        None => anyhow::bail!("persisted credentials object was empty"),
    };
    assert_eq!(
        entry.get("access_token").and_then(Value::as_str),
        Some(REFRESHED_ACCESS_TOKEN)
    );
    assert_eq!(
        entry.get("refresh_token").and_then(Value::as_str),
        Some(REFRESHED_REFRESH_TOKEN)
    );
    let expires_at = match entry.get("expires_at").and_then(Value::as_u64) {
        Some(expires_at) => expires_at,
        None => anyhow::bail!("persisted refreshed credentials had no expiry: {entry}"),
    };
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
    assert!(expires_at > now);
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

async fn initialize_in_process_client(client: &RmcpClient) -> anyhow::Result<()> {
    let params = InitializeRequestParams::new(
        ClientCapabilities::default(),
        Implementation::new("codex-test", "0.0.0-test"),
    )
    .with_protocol_version(ProtocolVersion::V_2025_06_18);
    client
        .initialize(
            params,
            Some(Duration::from_secs(5)),
            Box::new(|_, _| {
                async {
                    Ok(ElicitationResponse {
                        action: ElicitationAction::Accept,
                        content: Some(json!({})),
                        meta: None,
                    })
                }
                .boxed()
            }),
        )
        .await?;
    Ok(())
}
