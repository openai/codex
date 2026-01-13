use std::io::ErrorKind;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::Request;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::http::header::CONTENT_TYPE;
use axum::middleware;
use axum::middleware::Next;
use axum::response::Response;
use axum::routing::get;
use rmcp::transport::StreamableHttpServerConfig;
use rmcp::transport::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use serde_json::json;
use tokio::task;

use crate::resource_server::ResourceTestToolServer;

pub async fn run_streamable_http_server() -> Result<()> {
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
                            }))
                            .expect("failed to serialize metadata"),
                        ))
                        .expect("valid metadata response")
                }
            }),
        )
        .nest_service(
            "/mcp",
            StreamableHttpService::new(
                || Ok(ResourceTestToolServer::new(false)),
                Arc::new(LocalSessionManager::default()),
                StreamableHttpServerConfig::default(),
            ),
        );

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

fn parse_bind_addr() -> Result<SocketAddr> {
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
