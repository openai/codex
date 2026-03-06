use std::borrow::Cow;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::HeaderValue;
use axum::http::Request;
use axum::http::StatusCode;
use axum::http::header::COOKIE;
use axum::http::header::SET_COOKIE;
use axum::middleware;
use axum::middleware::Next;
use axum::response::Response;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::OAuthCredentialsStoreMode;
use codex_rmcp_client::RmcpClient;
use futures::FutureExt as _;
use pretty_assertions::assert_eq;
use reqwest::cookie::Jar;
use rmcp::handler::server::ServerHandler;
use rmcp::model::ClientCapabilities;
use rmcp::model::ElicitationCapability;
use rmcp::model::FormElicitationCapability;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::JsonObject;
use rmcp::model::ListToolsResult;
use rmcp::model::ProtocolVersion;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerInfo;
use rmcp::model::Tool;
use rmcp::transport::StreamableHttpServerConfig;
use rmcp::transport::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use serde_json::json;
use tokio::net::TcpListener;

const AFFINITY_COOKIE: &str = "__cflb=sticky";

#[derive(Default)]
struct AffinityCookieState {
    request_count: AtomicUsize,
}

#[derive(Clone)]
struct CookieToolServer {
    tools: Arc<Vec<Tool>>,
}

impl CookieToolServer {
    fn new() -> Self {
        Self {
            tools: Arc::new(vec![echo_tool()]),
        }
    }
}

impl ServerHandler for CookieToolServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..ServerInfo::default()
        }
    }

    fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, rmcp::ErrorData>> + Send + '_
    {
        let tools = Arc::clone(&self.tools);
        async move {
            Ok(ListToolsResult {
                tools: (*tools).clone(),
                next_cursor: None,
                meta: None,
            })
        }
    }
}

fn echo_tool() -> Tool {
    let schema: JsonObject = serde_json::from_value(json!({
        "type": "object",
        "additionalProperties": false,
    }))
    .unwrap_or_else(|err| panic!("cookie test tool schema should deserialize: {err}"));

    Tool::new(
        Cow::Borrowed("echo"),
        Cow::Borrowed("Echo back the provided message."),
        Arc::new(schema),
    )
}

fn init_params() -> InitializeRequestParams {
    InitializeRequestParams {
        meta: None,
        capabilities: ClientCapabilities {
            experimental: None,
            extensions: None,
            roots: None,
            sampling: None,
            elicitation: Some(ElicitationCapability {
                form: Some(FormElicitationCapability {
                    schema_validation: None,
                }),
                url: None,
            }),
            tasks: None,
        },
        client_info: Implementation {
            name: "codex-test".into(),
            version: "0.0.0-test".into(),
            title: Some("Codex rmcp cookie test".into()),
            description: None,
            icons: None,
            website_url: None,
        },
        protocol_version: ProtocolVersion::V_2025_06_18,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn streamable_http_client_replays_affinity_cookie_from_initialize_response() -> Result<()> {
    let affinity_cookie_state = Arc::new(AffinityCookieState::default());
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let mcp_service = StreamableHttpService::new(
        || Ok(CookieToolServer::new()),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );

    let router =
        Router::new()
            .nest_service("/mcp", mcp_service)
            .layer(middleware::from_fn_with_state(
                Arc::clone(&affinity_cookie_state),
                require_affinity_cookie,
            ));

    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    let cookie_jar = Arc::new(Jar::default());
    let client = RmcpClient::new_streamable_http_client(
        "codex_apps",
        &format!("http://{addr}/mcp"),
        Some("bearer-token".to_string()),
        None,
        None,
        OAuthCredentialsStoreMode::File,
        Some(cookie_jar),
    )
    .await?;

    client
        .initialize(
            init_params(),
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

    let tools = client
        .list_tools(None, Some(Duration::from_secs(5)))
        .await?;
    assert_eq!(
        tools,
        ListToolsResult {
            tools: vec![echo_tool()],
            next_cursor: None,
            meta: None,
        }
    );
    assert!(
        affinity_cookie_state.request_count.load(Ordering::SeqCst) >= 2,
        "expected initialize plus at least one follow-up request",
    );

    handle.abort();
    let _ = handle.await;
    Ok(())
}

async fn require_affinity_cookie(
    State(affinity_cookie_state): State<Arc<AffinityCookieState>>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let request_number = affinity_cookie_state
        .request_count
        .fetch_add(1, Ordering::SeqCst)
        + 1;
    if request_number > 1
        && !request
            .headers()
            .get(COOKIE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.contains(AFFINITY_COOKIE))
    {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let mut response = next.run(request).await;
    if request_number == 1 {
        response.headers_mut().insert(
            SET_COOKIE,
            HeaderValue::from_static("__cflb=sticky; Path=/"),
        );
    }
    Ok(response)
}
