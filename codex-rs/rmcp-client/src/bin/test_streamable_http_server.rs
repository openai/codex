use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::body::to_bytes;
use axum::extract::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::HeaderValue;
use axum::http::Method;
use axum::http::Request;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::http::header::CONTENT_TYPE;
use axum::http::header::HOST;
use axum::http::header::WWW_AUTHENTICATE;
use axum::middleware;
use axum::middleware::Next;
use axum::response::Response;
use axum::routing::get;
use axum::routing::post;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::RmcpClient;
use codex_rmcp_client::StoredOAuthTokens;
use codex_rmcp_client::WrappedOAuthTokenResponse;
use codex_rmcp_client::save_oauth_tokens_async;
use futures::FutureExt as _;
use oauth2::AccessToken;
use oauth2::RefreshToken;
use oauth2::basic::BasicTokenType;
use rmcp::ErrorData as McpError;
use rmcp::handler::server::ServerHandler;
use rmcp::model::CallToolRequestParams;
use rmcp::model::CallToolResult;
use rmcp::model::ClientCapabilities;
use rmcp::model::ElicitationCapability;
use rmcp::model::FormElicitationCapability;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::JsonObject;
use rmcp::model::ListResourceTemplatesResult;
use rmcp::model::ListResourcesResult;
use rmcp::model::ListToolsResult;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ProtocolVersion;
use rmcp::model::RawResource;
use rmcp::model::RawResourceTemplate;
use rmcp::model::ReadResourceRequestParams;
use rmcp::model::ReadResourceResult;
use rmcp::model::Resource;
use rmcp::model::ResourceContents;
use rmcp::model::ResourceTemplate;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerInfo;
use rmcp::model::Tool;
use rmcp::model::ToolAnnotations;
use rmcp::transport::StreamableHttpServerConfig;
use rmcp::transport::StreamableHttpService;
use rmcp::transport::auth::OAuthTokenResponse;
use rmcp::transport::auth::VendorExtraTokenFields;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Mutex;
use tokio::task;
use tokio::time::sleep;
use urlencoding::decode;

static REFRESH_TOKEN_USES: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone)]
struct TestToolServer {
    tools: Arc<Vec<Tool>>,
    resources: Arc<Vec<Resource>>,
    resource_templates: Arc<Vec<ResourceTemplate>>,
}

const MEMO_URI: &str = "memo://codex/example-note";
const MEMO_CONTENT: &str = "This is a sample MCP resource served by the rmcp test server.";
const MCP_SESSION_ID_HEADER: &str = "mcp-session-id";
const SESSION_POST_FAILURE_CONTROL_PATH: &str = "/test/control/session-post-failure";

#[derive(Clone, Default)]
struct SessionFailureState {
    armed_failure: Arc<Mutex<Option<ArmedFailure>>>,
}

#[derive(Clone, Debug)]
struct ArmedFailure {
    status: StatusCode,
    remaining: usize,
    /// Raw `WWW-Authenticate` challenge header field values returned with the failure.
    www_authenticate_headers: Vec<HeaderValue>,
}

#[derive(Debug, Deserialize)]
struct ArmSessionPostFailureRequest {
    status: u16,
    remaining: usize,
    /// Raw `WWW-Authenticate` challenge header field values to add to the failure.
    #[serde(default)]
    www_authenticate_headers: Vec<String>,
}

#[derive(Deserialize)]
struct EchoArgs {
    message: String,
    #[allow(dead_code)]
    env_var: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(server_url) = std::env::var("MCP_TEST_OAUTH_CLIENT_URL") {
        let server_name = std::env::var("MCP_TEST_OAUTH_SERVER_NAME")?;
        return run_oauth_test_client(&server_name, &server_url).await;
    }
    if let Ok(server_name) = std::env::var("MCP_TEST_OAUTH_WRITE_SERVER_NAME") {
        let server_url = std::env::var("MCP_TEST_OAUTH_WRITE_SERVER_URL")?;
        let access_token = std::env::var("MCP_TEST_OAUTH_WRITE_ACCESS_TOKEN")?;
        let refresh_token = std::env::var("MCP_TEST_OAUTH_WRITE_REFRESH_TOKEN")?;
        if let Ok(barrier) = std::env::var("MCP_TEST_OAUTH_WRITE_BARRIER") {
            while !std::path::Path::new(&barrier).exists() {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        write_oauth_test_credentials(&server_name, &server_url, &access_token, &refresh_token)
            .await?;
        return Ok(());
    }

    let bind_addr = parse_bind_addr()?;
    let session_failure_state = SessionFailureState::default();
    const MAX_BIND_RETRIES: u32 = 20;
    const BIND_RETRY_DELAY: Duration = Duration::from_millis(50);

    let mut bind_retries = 0;
    let listener = loop {
        match tokio::net::TcpListener::bind(&bind_addr).await {
            Ok(listener) => break listener,
            Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                eprintln!(
                    "failed to bind to {bind_addr}: {err}. make sure the process has network access"
                );
                return Ok(());
            }
            Err(err) if err.kind() == ErrorKind::AddrInUse && bind_retries < MAX_BIND_RETRIES => {
                bind_retries += 1;
                sleep(BIND_RETRY_DELAY).await;
            }
            Err(err) => return Err(err.into()),
        }
    };
    let actual_bind_addr = listener.local_addr()?;
    if let Ok(bound_addr_file) = std::env::var("MCP_STREAMABLE_HTTP_BOUND_ADDR_FILE") {
        fs::write(bound_addr_file, actual_bind_addr.to_string())?;
    }
    eprintln!("starting rmcp streamable http test server on http://{actual_bind_addr}/mcp");

    let router = Router::new()
        .route(
            SESSION_POST_FAILURE_CONTROL_PATH,
            post(arm_session_post_failure),
        )
        .route("/oauth/token", post(exchange_refresh_token))
        .route(
            "/.well-known/oauth-authorization-server/mcp",
            get({
                move |headers: HeaderMap| async move {
                    let metadata_base = headers
                        .get(HOST)
                        .and_then(|value| value.to_str().ok())
                        .map(|host| format!("http://{host}"))
                        .unwrap_or_else(|| format!("http://{actual_bind_addr}"));
                    #[expect(clippy::expect_used)]
                    Response::builder()
                        .status(StatusCode::OK)
                        .header(CONTENT_TYPE, "application/json")
                        .body(Body::from(
                            serde_json::to_vec(&json!({
                                "authorization_endpoint": format!("{metadata_base}/oauth/authorize"),
                                "token_endpoint": format!("{metadata_base}/oauth/token"),
                                "scopes_supported": [""],
                            })).expect("failed to serialize metadata"),
                        ))
                        .expect("valid metadata response")
                }
            }),
        )
        .nest_service(
            "/mcp",
            StreamableHttpService::new(
                || Ok(TestToolServer::new()),
                Arc::new(LocalSessionManager::default()),
                StreamableHttpServerConfig::default(),
            ),
        )
        .layer(middleware::from_fn_with_state(
            session_failure_state.clone(),
            fail_session_post_when_armed,
        ))
        .layer(middleware::from_fn(delay_mcp_initialize_when_configured))
        .with_state(session_failure_state);

    let router = if let Ok(token) = std::env::var("MCP_EXPECT_BEARER") {
        let expected = Arc::new(format!("Bearer {token}"));
        router.layer(middleware::from_fn_with_state(expected, require_bearer))
    } else {
        router
    };

    axum::serve(listener, router).await?;
    task::yield_now().await;
    Ok(())
}

async fn run_oauth_test_client(
    server_name: &str,
    server_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = RmcpClient::new_streamable_http_client(
        server_name,
        server_url,
        /*bearer_token*/ None,
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        OAuthCredentialsStoreMode::File,
        Environment::default_for_tests().get_http_client(),
        /*auth_provider*/ None,
    )
    .await?;
    let mut capabilities = ClientCapabilities::default();
    capabilities.elicitation = Some(ElicitationCapability {
        form: Some(FormElicitationCapability {
            schema_validation: None,
        }),
        url: None,
    });
    let params = InitializeRequestParams::new(
        capabilities,
        Implementation::new("codex-test", "0.0.0-test").with_title("Codex rmcp OAuth process test"),
    )
    .with_protocol_version(ProtocolVersion::V_2025_06_18);
    client
        .initialize(
            params,
            Some(Duration::from_secs(15)),
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

async fn write_oauth_test_credentials(
    server_name: &str,
    server_url: &str,
    access_token: &str,
    refresh_token: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut response = OAuthTokenResponse::new(
        AccessToken::new(access_token.to_string()),
        BasicTokenType::Bearer,
        VendorExtraTokenFields::default(),
    );
    response.set_refresh_token(Some(RefreshToken::new(refresh_token.to_string())));
    response.set_expires_in(Some(&Duration::from_secs(7200)));
    let stored = StoredOAuthTokens {
        server_name: server_name.to_string(),
        url: server_url.to_string(),
        client_id: "test-client-id".to_string(),
        token_response: WrappedOAuthTokenResponse(response),
        expires_at: None,
    };
    save_oauth_tokens_async(server_name, &stored, OAuthCredentialsStoreMode::File).await?;
    Ok(())
}

impl ServerHandler for TestToolServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_tool_list_changed()
                .enable_resources()
                .build(),
        )
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        let tools = self.tools.clone();
        async move {
            Ok(ListToolsResult {
                tools: (*tools).clone(),
                next_cursor: None,
                meta: None,
            })
        }
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListResourcesResult, McpError>> + Send + '_ {
        let resources = self.resources.clone();
        async move {
            Ok(ListResourcesResult {
                resources: (*resources).clone(),
                next_cursor: None,
                meta: None,
            })
        }
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        Ok(ListResourceTemplatesResult {
            resource_templates: (*self.resource_templates).clone(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        ReadResourceRequestParams { uri, .. }: ReadResourceRequestParams,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        if uri == MEMO_URI {
            Ok(ReadResourceResult::new(vec![
                ResourceContents::TextResourceContents {
                    uri,
                    mime_type: Some("text/plain".to_string()),
                    text: Self::memo_text().to_string(),
                    meta: None,
                },
            ]))
        } else {
            Err(McpError::resource_not_found(
                "resource_not_found",
                Some(json!({ "uri": uri })),
            ))
        }
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        match request.name.as_ref() {
            "echo" => {
                let args: EchoArgs = match request.arguments {
                    Some(arguments) => serde_json::from_value(serde_json::Value::Object(
                        arguments.into_iter().collect(),
                    ))
                    .map_err(|err| McpError::invalid_params(err.to_string(), None))?,
                    None => {
                        return Err(McpError::invalid_params(
                            "missing arguments for echo tool",
                            None,
                        ));
                    }
                };

                let env_snapshot: HashMap<String, String> = std::env::vars().collect();
                let structured_content = json!({
                    "echo": format!("ECHOING: {}", args.message),
                    "env": env_snapshot.get("MCP_TEST_VALUE"),
                });

                let mut result = CallToolResult::success(Vec::new());
                result.structured_content = Some(structured_content);
                Ok(result)
            }
            other => Err(McpError::invalid_params(
                format!("unknown tool: {other}"),
                None,
            )),
        }
    }
}

impl TestToolServer {
    fn new() -> Self {
        let tools = vec![Self::echo_tool()];
        let resources = vec![Self::memo_resource()];
        let resource_templates = vec![Self::memo_template()];
        Self {
            tools: Arc::new(tools),
            resources: Arc::new(resources),
            resource_templates: Arc::new(resource_templates),
        }
    }

    fn echo_tool() -> Tool {
        #[expect(clippy::expect_used)]
        let schema: JsonObject = serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "message": { "type": "string" },
                "env_var": { "type": "string" }
            },
            "required": ["message"],
            "additionalProperties": false
        }))
        .expect("echo tool schema should deserialize");

        let mut tool = Tool::new(
            Cow::Borrowed("echo"),
            Cow::Borrowed("Echo back the provided message and include environment data."),
            Arc::new(schema),
        );
        #[expect(clippy::expect_used)]
        let output_schema: JsonObject = serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "echo": { "type": "string" },
                "env": {
                    "anyOf": [
                        { "type": "string" },
                        { "type": "null" }
                    ]
                }
            },
            "required": ["echo", "env"],
            "additionalProperties": false
        }))
        .expect("echo tool output schema should deserialize");
        tool.output_schema = Some(Arc::new(output_schema));
        tool.annotations = Some(ToolAnnotations::new().read_only(true));
        tool
    }

    fn memo_resource() -> Resource {
        let raw = RawResource {
            uri: MEMO_URI.to_string(),
            name: "example-note".to_string(),
            title: Some("Example Note".to_string()),
            description: Some("A sample MCP resource exposed for integration tests.".to_string()),
            mime_type: Some("text/plain".to_string()),
            size: None,
            icons: None,
            meta: None,
        };
        Resource::new(raw, None)
    }

    fn memo_template() -> ResourceTemplate {
        let raw = RawResourceTemplate {
            uri_template: "memo://codex/{slug}".to_string(),
            name: "codex-memo".to_string(),
            title: Some("Codex Memo".to_string()),
            description: Some(
                "Template for memo://codex/{slug} resources used in tests.".to_string(),
            ),
            mime_type: Some("text/plain".to_string()),
            icons: None,
        };
        ResourceTemplate::new(raw, None)
    }

    fn memo_text() -> &'static str {
        MEMO_CONTENT
    }
}

fn parse_bind_addr() -> Result<SocketAddr, Box<dyn std::error::Error>> {
    let default_addr = "127.0.0.1:3920";
    let bind_addr = std::env::var("MCP_STREAMABLE_HTTP_BIND_ADDR")
        .or_else(|_| std::env::var("BIND_ADDR"))
        .unwrap_or_else(|_| default_addr.to_string());
    Ok(bind_addr.parse()?)
}

async fn require_bearer(
    State(expected): State<Arc<String>>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = request.uri().path();
    if path.contains("/.well-known/") || path.starts_with("/oauth/") {
        return Ok(next.run(request).await);
    }
    if request
        .headers()
        .get(AUTHORIZATION)
        .is_some_and(|value| value.as_bytes() == expected.as_bytes())
    {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

async fn exchange_refresh_token(request: Request<Body>) -> Result<Response, StatusCode> {
    let body = to_bytes(request.into_body(), 16 * 1024)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let params = parse_form_body(&body)?;

    let expected_refresh_token =
        std::env::var("MCP_EXPECT_REFRESH_TOKEN").map_err(|_| StatusCode::BAD_REQUEST)?;
    let access_token =
        std::env::var("MCP_REFRESH_ACCESS_TOKEN").map_err(|_| StatusCode::BAD_REQUEST)?;

    if params.get("grant_type").map(String::as_str) != Some("refresh_token")
        || params.get("refresh_token").map(String::as_str) != Some(expected_refresh_token.as_str())
    {
        return oauth_error_response(StatusCode::BAD_REQUEST, "invalid_grant");
    }

    if let Ok(max_uses) = std::env::var("MCP_REFRESH_TOKEN_MAX_USES")
        && let Ok(max_uses) = max_uses.parse::<usize>()
    {
        let use_count = REFRESH_TOKEN_USES.fetch_add(1, Ordering::SeqCst) + 1;
        if use_count > max_uses {
            return oauth_error_response(StatusCode::BAD_REQUEST, "invalid_grant");
        }
    }

    if let Ok(error) = std::env::var("MCP_REFRESH_ERROR") {
        let status = if error == "server_error" {
            StatusCode::INTERNAL_SERVER_ERROR
        } else {
            StatusCode::BAD_REQUEST
        };
        return oauth_error_response(status, &error);
    }

    if let Ok(delay_ms) = std::env::var("MCP_REFRESH_TOKEN_DELAY_MS")
        && let Ok(delay_ms) = delay_ms.parse::<u64>()
    {
        sleep(Duration::from_millis(delay_ms)).await;
    }

    let mut token_response = json!({
        "access_token": access_token,
        "token_type": "Bearer",
        "expires_in": 7200,
    });
    if std::env::var("MCP_OMIT_ROTATED_REFRESH_TOKEN").is_err() {
        let refresh_token =
            std::env::var("MCP_ROTATED_REFRESH_TOKEN").map_err(|_| StatusCode::BAD_REQUEST)?;
        token_response["refresh_token"] = json!(refresh_token);
    }

    #[expect(clippy::expect_used)]
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_vec(&token_response).expect("failed to serialize token response"),
        ))
        .expect("valid token response"))
}

fn oauth_error_response(status: StatusCode, error: &str) -> Result<Response, StatusCode> {
    #[expect(clippy::expect_used)]
    Ok(Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({ "error": error }))
                .expect("failed to serialize OAuth error response"),
        ))
        .expect("valid OAuth error response"))
}

async fn delay_mcp_initialize_when_configured(request: Request<Body>, next: Next) -> Response {
    if request.uri().path() == "/mcp"
        && request.method() == Method::POST
        && !request.headers().contains_key(MCP_SESSION_ID_HEADER)
        && let Ok(delay_ms) = std::env::var("MCP_INITIALIZE_DELAY_MS")
        && let Ok(delay_ms) = delay_ms.parse::<u64>()
    {
        sleep(Duration::from_millis(delay_ms)).await;
    }

    next.run(request).await
}

fn parse_form_body(body: &[u8]) -> Result<HashMap<String, String>, StatusCode> {
    let body = std::str::from_utf8(body).map_err(|_| StatusCode::BAD_REQUEST)?;
    body.split('&')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let (name, value) = part.split_once('=').ok_or(StatusCode::BAD_REQUEST)?;
            let name = decode(name)
                .map_err(|_| StatusCode::BAD_REQUEST)?
                .into_owned();
            let value = decode(value)
                .map_err(|_| StatusCode::BAD_REQUEST)?
                .into_owned();
            Ok((name, value))
        })
        .collect()
}

async fn arm_session_post_failure(
    State(state): State<SessionFailureState>,
    Json(request): Json<ArmSessionPostFailureRequest>,
) -> Result<StatusCode, StatusCode> {
    let status = StatusCode::from_u16(request.status).map_err(|_| StatusCode::BAD_REQUEST)?;
    let www_authenticate_headers = request
        .www_authenticate_headers
        .into_iter()
        .map(|value| HeaderValue::from_str(&value).map_err(|_| StatusCode::BAD_REQUEST))
        .collect::<Result<Vec<_>, _>>()?;
    let armed_failure = if request.remaining == 0 {
        None
    } else {
        Some(ArmedFailure {
            status,
            remaining: request.remaining,
            www_authenticate_headers,
        })
    };
    *state.armed_failure.lock().await = armed_failure;
    Ok(StatusCode::NO_CONTENT)
}

async fn fail_session_post_when_armed(
    State(state): State<SessionFailureState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if request.uri().path() != "/mcp"
        || request.method() != Method::POST
        || !request.headers().contains_key(MCP_SESSION_ID_HEADER)
    {
        return next.run(request).await;
    }

    {
        let mut armed_failure = state.armed_failure.lock().await;
        if let Some(failure) = armed_failure.as_mut()
            && failure.remaining > 0
        {
            failure.remaining -= 1;
            let status = failure.status;
            let www_authenticate_headers = failure.www_authenticate_headers.clone();
            if failure.remaining == 0 {
                *armed_failure = None;
            }
            let mut response = Response::new(Body::from(format!(
                "forced session failure with status {status}"
            )));
            *response.status_mut() = status;
            for www_authenticate_header in www_authenticate_headers {
                response
                    .headers_mut()
                    .append(WWW_AUTHENTICATE, www_authenticate_header);
            }
            return response;
        }
    }

    next.run(request).await
}
