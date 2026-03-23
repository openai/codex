use std::borrow::Cow;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::Json;
use axum::extract::State;
use axum::http::Method;
use axum::http::Request;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::http::header::CONTENT_TYPE;
use axum::middleware;
use axum::middleware::Next;
use axum::response::Response;
use axum::routing::get;
use axum::routing::post;
use rmcp::ErrorData as McpError;
use rmcp::handler::server::ServerHandler;
use rmcp::model::CallToolRequestParams;
use rmcp::model::CallToolResult;
use rmcp::model::JsonObject;
use rmcp::model::ListResourceTemplatesResult;
use rmcp::model::ListResourcesResult;
use rmcp::model::ListToolsResult;
use rmcp::model::PaginatedRequestParams;
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
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Mutex;
use tokio::task;
use tokio::time::sleep;

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

#[derive(Clone, Default)]
struct SessionFailureState {
    armed_failure: Arc<Mutex<Option<ArmedFailure>>>,
}

#[derive(Clone, Debug)]
struct ArmedFailure {
    status: StatusCode,
    remaining: usize,
}

#[derive(Debug, Deserialize)]
struct ArmSessionPostFailureRequest {
    status: u16,
    remaining: usize,
}

#[derive(Deserialize)]
struct EchoArgs {
    message: String,
    #[allow(dead_code)]
    env_var: Option<String>,
}

impl ServerHandler for TestToolServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_tool_list_changed()
                .enable_resources()
                .build(),
            ..ServerInfo::default()
        }
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
            Ok(ReadResourceResult {
                contents: vec![ResourceContents::TextResourceContents {
                    uri,
                    mime_type: Some("text/plain".to_string()),
                    text: Self::memo_text().to_string(),
                    meta: None,
                }],
            })
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

                Ok(CallToolResult {
                    content: Vec::new(),
                    structured_content: Some(structured_content),
                    is_error: Some(false),
                    meta: None,
                })
            }
            other => Err(McpError::invalid_params(
                format!("unknown tool: {other}"),
                None,
            )),
        }
    }
}

fn parse_bind_addr() -> Result<SocketAddr, Box<dyn std::error::Error>> {
    let default_addr = "127.0.0.1:3920";
    let bind_addr = std::env::var("MCP_STREAMABLE_HTTP_BIND_ADDR")
        .or_else(|_| std::env::var("BIND_ADDR"))
        .unwrap_or_else(|_| default_addr.to_string());
    Ok(bind_addr.parse()?)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    eprintln!("starting rmcp streamable http test server on http://{bind_addr}/mcp");

    let router = Router::new()
        .route(
            SESSION_POST_FAILURE_CONTROL_PATH,
            post(arm_session_post_failure),
        )
        .route(
            "/.well-known/oauth-authorization-server/mcp",
            get({
                move || async move {
                    let metadata_base = format!("http://{bind_addr}");
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

async fn require_bearer(
    State(expected): State<Arc<String>>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    if request.uri().path().contains("/.well-known/") {
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

async fn arm_session_post_failure(
    State(state): State<SessionFailureState>,
    Json(request): Json<ArmSessionPostFailureRequest>,
) -> Result<StatusCode, StatusCode> {
    let status = StatusCode::from_u16(request.status).map_err(|_| StatusCode::BAD_REQUEST)?;
    let armed_failure = if request.remaining == 0 {
        None
    } else {
        Some(ArmedFailure {
            status,
            remaining: request.remaining,
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

    let mut armed_failure = state.armed_failure.lock().await;
    if let Some(failure) = armed_failure.as_mut()
        && failure.remaining > 0
    {
        failure.remaining -= 1;
        let status = failure.status;
        if failure.remaining == 0 {
            *armed_failure = None;
        }
        let mut response = Response::new(Body::from(format!(
            "forced session failure with status {status}"
        )));
        *response.status_mut() = status;
        return response;
    }

    drop(armed_failure);
    next.run(request).await
}
