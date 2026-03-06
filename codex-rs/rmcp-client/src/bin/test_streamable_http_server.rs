use std::borrow::Cow;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::Form;
use axum::extract::State;
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
use rmcp::transport::StreamableHttpServerConfig;
use rmcp::transport::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio::task;

#[derive(Clone)]
struct TestToolServer {
    tools: Arc<Vec<Tool>>,
    resources: Arc<Vec<Resource>>,
    resource_templates: Arc<Vec<ResourceTemplate>>,
}

#[derive(Clone)]
struct AuthState {
    current_bearer: Arc<RwLock<Option<String>>>,
    refresh_state: Option<Arc<Mutex<RefreshTokenState>>>,
}

#[derive(Debug)]
struct RefreshTokenState {
    current_refresh_token: String,
    next_access_token: String,
    next_refresh_token: String,
    expires_in: u64,
    single_use: bool,
    used_once: bool,
}

const MEMO_URI: &str = "memo://codex/example-note";
const MEMO_CONTENT: &str = "This is a sample MCP resource served by the rmcp test server.";

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

        Tool::new(
            Cow::Borrowed("echo"),
            Cow::Borrowed("Echo back the provided message and include environment data."),
            Arc::new(schema),
        )
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
    let listener = match tokio::net::TcpListener::bind(&bind_addr).await {
        Ok(listener) => listener,
        Err(err) if err.kind() == ErrorKind::PermissionDenied => {
            eprintln!(
                "failed to bind to {bind_addr}: {err}. make sure the process has network access"
            );
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };
    eprintln!("starting rmcp streamable http test server on http://{bind_addr}/mcp");

    let auth_state = AuthState {
        current_bearer: Arc::new(RwLock::new(
            std::env::var("MCP_EXPECT_BEARER")
                .ok()
                .map(|token| format!("Bearer {token}")),
        )),
        refresh_state: refresh_state_from_env(),
    };

    let router = Router::new()
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
        .route("/oauth/token", post(oauth_refresh_token))
        .nest_service(
            "/mcp",
            StreamableHttpService::new(
                || Ok(TestToolServer::new()),
                Arc::new(LocalSessionManager::default()),
                StreamableHttpServerConfig::default(),
            ),
        )
        .with_state(auth_state.clone())
        .layer(middleware::from_fn_with_state(auth_state, require_bearer));

    axum::serve(listener, router).await?;
    task::yield_now().await;
    Ok(())
}

fn refresh_state_from_env() -> Option<Arc<Mutex<RefreshTokenState>>> {
    let current_refresh_token = std::env::var("MCP_EXPECT_REFRESH_TOKEN").ok()?;
    let next_access_token = std::env::var("MCP_REFRESH_NEXT_ACCESS_TOKEN").ok()?;
    let next_refresh_token = std::env::var("MCP_REFRESH_NEXT_REFRESH_TOKEN").ok()?;
    let expires_in = std::env::var("MCP_REFRESH_EXPIRES_IN")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(3600);
    let single_use = std::env::var("MCP_REFRESH_SINGLE_USE")
        .ok()
        .is_some_and(|value| value == "1");

    Some(Arc::new(Mutex::new(RefreshTokenState {
        current_refresh_token,
        next_access_token,
        next_refresh_token,
        expires_in,
        single_use,
        used_once: false,
    })))
}

async fn oauth_refresh_token(
    State(state): State<AuthState>,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    let Some(refresh_state) = state.refresh_state.clone() else {
        return json_response(StatusCode::NOT_FOUND, json!({ "error": "not_found" }));
    };

    if form.get("grant_type").map(String::as_str) != Some("refresh_token") {
        return json_response(
            StatusCode::BAD_REQUEST,
            json!({ "error": "unsupported_grant_type" }),
        );
    }

    let provided_refresh_token = form.get("refresh_token").map(String::as_str);
    let mut refresh_state = refresh_state.lock().await;
    if refresh_state.single_use && refresh_state.used_once {
        return json_response(
            StatusCode::UNAUTHORIZED,
            json!({
                "error": "invalid_grant",
                "error_description": "refresh token was already used",
                "code": "refresh_token_reused",
            }),
        );
    }
    if provided_refresh_token != Some(refresh_state.current_refresh_token.as_str()) {
        return json_response(
            StatusCode::UNAUTHORIZED,
            json!({
                "error": "invalid_grant",
                "error_description": "refresh token was already used",
                "code": "refresh_token_reused",
            }),
        );
    }

    let access_token = refresh_state.next_access_token.clone();
    let refresh_token = refresh_state.next_refresh_token.clone();
    let expires_in = refresh_state.expires_in;
    refresh_state.current_refresh_token = refresh_token.clone();
    refresh_state.used_once = true;
    *state.current_bearer.write().await = Some(format!("Bearer {access_token}"));

    json_response(
        StatusCode::OK,
        json!({
            "access_token": access_token,
            "token_type": "Bearer",
            "refresh_token": refresh_token,
            "expires_in": expires_in,
        }),
    )
}

async fn require_bearer(
    State(state): State<AuthState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let request_path = request.uri().path();
    if request_path.contains("/.well-known/") || request_path.contains("/oauth/token") {
        return Ok(next.run(request).await);
    }

    let expected = state.current_bearer.read().await.clone();
    let Some(expected) = expected else {
        return Ok(next.run(request).await);
    };

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

fn json_response(status: StatusCode, body: serde_json::Value) -> Response {
    #[expect(clippy::expect_used)]
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_vec(&body).expect("failed to serialize JSON response"),
        ))
        .expect("valid JSON response")
}
