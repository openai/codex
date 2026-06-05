use std::io;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

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
use crate::oauth::WrappedOAuthTokenResponse;

const SERVER_NAME: &str = "request-time-refresh-test";
const INITIAL_ACCESS_TOKEN: &str = "initial-access-token";
const INITIAL_REFRESH_TOKEN: &str = "initial-refresh-token";
const REFRESHED_ACCESS_TOKEN: &str = "refreshed-access-token";
const REFRESHED_REFRESH_TOKEN: &str = "refreshed-refresh-token";
const CHILD_SERVER_URL_ENV: &str = "MCP_TEST_REQUEST_TIME_REFRESH_SERVER_URL";
const CHILD_SCENARIO_ENV: &str = "MCP_TEST_REQUEST_TIME_REFRESH_SCENARIO";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scenario {
    RefreshSucceeds,
    ProviderRejectsRefresh,
}

impl Scenario {
    fn as_env(self) -> &'static str {
        match self {
            Self::RefreshSucceeds => "refresh-succeeds",
            Self::ProviderRejectsRefresh => "provider-rejects-refresh",
        }
    }

    fn from_env(value: &str) -> anyhow::Result<Self> {
        match value {
            "refresh-succeeds" => Ok(Self::RefreshSucceeds),
            "provider-rejects-refresh" => Ok(Self::ProviderRejectsRefresh),
            _ => anyhow::bail!("unknown request-time refresh scenario: {value}"),
        }
    }
}

#[derive(Clone, Copy)]
struct TokenResponder {
    scenario: Scenario,
}

impl Respond for TokenResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body = String::from_utf8_lossy(&request.body);
        assert!(
            body.contains(INITIAL_REFRESH_TOKEN),
            "unexpected refresh request body: {body}"
        );

        match self.scenario {
            Scenario::RefreshSucceeds => ResponseTemplate::new(200).set_body_json(json!({
                "access_token": REFRESHED_ACCESS_TOKEN,
                "token_type": "Bearer",
                "expires_in": 7200,
                "refresh_token": REFRESHED_REFRESH_TOKEN,
            })),
            Scenario::ProviderRejectsRefresh => ResponseTemplate::new(400).set_body_json(json!({
                "error": "invalid_grant",
                "error_description": "provider rejected refresh",
            })),
        }
    }
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
async fn proactive_refresh_succeeds_before_request() -> anyhow::Result<()> {
    run_parent_scenario(Scenario::RefreshSucceeds).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn provider_rejection_of_proactive_refresh_stops_before_request() -> anyhow::Result<()> {
    run_parent_scenario(Scenario::ProviderRejectsRefresh).await
}

async fn run_parent_scenario(scenario: Scenario) -> anyhow::Result<()> {
    let server = MockServer::start().await;
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
        .respond_with(TokenResponder { scenario })
        .expect(1)
        .mount(&server)
        .await;

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
        .env(CHILD_SCENARIO_ENV, scenario.as_env())
        .status()
        .await?;
    assert!(
        status.success(),
        "request-time refresh child failed: {status}"
    );

    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by request-time refresh parent tests"]
async fn oauth_request_time_child() -> anyhow::Result<()> {
    let server_url = std::env::var(CHILD_SERVER_URL_ENV)?;
    let scenario = Scenario::from_env(&std::env::var(CHILD_SCENARIO_ENV)?)?;
    let list_tools_calls = Arc::new(AtomicUsize::new(0));
    let client = RmcpClient::new_in_process_client(Arc::new(TestTransportFactory {
        server: CountingServer {
            list_tools_calls: Arc::clone(&list_tools_calls),
        },
    }))
    .await?;
    initialize_in_process_client(&client).await?;

    let mut response = OAuthTokenResponse::new(
        AccessToken::new(INITIAL_ACCESS_TOKEN.to_string()),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    response.set_refresh_token(Some(RefreshToken::new(INITIAL_REFRESH_TOKEN.to_string())));
    response.set_expires_in(Some(&Duration::from_secs(7200)));
    let mcp_url = format!("{server_url}/mcp");
    let initial_tokens = StoredOAuthTokens {
        server_name: SERVER_NAME.to_string(),
        url: mcp_url.clone(),
        client_id: "test-client-id".to_string(),
        token_response: WrappedOAuthTokenResponse(response),
        expires_at: Some(0),
    };
    let (_, runtime) = create_oauth_transport_and_runtime(
        SERVER_NAME,
        &mcp_url,
        initial_tokens,
        OAuthCredentialsStoreMode::File,
        HeaderMap::new(),
        Environment::default_for_tests().get_http_client(),
    )
    .await?;
    {
        let mut state = client.state.lock().await;
        match &mut *state {
            ClientState::Ready { oauth, .. } => *oauth = Some(runtime),
            ClientState::Connecting { .. } => panic!("client was not initialized"),
            ClientState::Closed => panic!("client was unexpectedly closed"),
        }
    }

    let result = client
        .list_tools(/*params*/ None, Some(Duration::from_secs(5)))
        .await;
    match scenario {
        Scenario::RefreshSucceeds => {
            assert_eq!(result?.tools, Vec::new());
            assert_eq!(list_tools_calls.load(Ordering::SeqCst), 1);
        }
        Scenario::ProviderRejectsRefresh => {
            let error = match result {
                Ok(_) => panic!("provider rejection should fail before tools/list"),
                Err(error) => error,
            };
            let error_chain = format!("{error:#}");
            assert!(
                error_chain.contains(
                    "failed to refresh OAuth tokens for server request-time-refresh-test"
                ),
                "unexpected provider rejection error: {error_chain}"
            );
            assert!(
                error_chain.contains("invalid_grant"),
                "provider error was not preserved: {error_chain}"
            );
            assert_eq!(list_tools_calls.load(Ordering::SeqCst), 0);
        }
    }

    client.shutdown().await;
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
