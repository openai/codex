use crate::config::NetworkMode;
use crate::responses::json_response;
use crate::responses::text_response;
use crate::state::NetworkProxyState;
use anyhow::Context;
use anyhow::Result;
use rama_core::rt::Executor;
use rama_core::service::service_fn;
use rama_http::Body;
use rama_http::Request;
use rama_http::Response;
use rama_http::StatusCode;
use rama_http_backend::server::HttpServer;
use rama_tcp::server::TcpListener;
use serde::Deserialize;
use serde::Serialize;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::net::TcpListener as StdTcpListener;
use std::sync::Arc;
use tracing::error;
use tracing::info;

pub async fn run_admin_api(state: Arc<NetworkProxyState>, addr: SocketAddr) -> Result<()> {
    // Debug-only admin API (health/config/patterns/blocked + mode/reload). Policy is config-driven
    // and constraint-enforced; this endpoint should not become a second policy/approval plane.
    let listener = TcpListener::build()
        .bind(addr)
        .await
        // See `http_proxy.rs` for details on why we wrap `BoxError` before converting to anyhow.
        .map_err(rama_core::error::OpaqueError::from)
        .map_err(anyhow::Error::from)
        .with_context(|| format!("bind admin API: {addr}"))?;

    run_admin_api_with_listener(state, listener).await
}

pub async fn run_admin_api_with_std_listener(
    state: Arc<NetworkProxyState>,
    listener: StdTcpListener,
) -> Result<()> {
    let listener =
        TcpListener::try_from(listener).context("convert std listener to admin API listener")?;
    run_admin_api_with_listener(state, listener).await
}

async fn run_admin_api_with_listener(
    state: Arc<NetworkProxyState>,
    listener: TcpListener,
) -> Result<()> {
    let addr = listener
        .local_addr()
        .context("read admin API listener local addr")?;

    let server_state = state.clone();
    let server = HttpServer::auto(Executor::new()).service(service_fn(move |req| {
        let state = server_state.clone();
        async move { handle_admin_request(state, req).await }
    }));
    info!("admin API listening on {addr}");
    listener.serve(server).await;
    Ok(())
}

async fn handle_admin_request(
    state: Arc<NetworkProxyState>,
    req: Request,
) -> Result<Response, Infallible> {
    const MODE_BODY_LIMIT: usize = 8 * 1024;

    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let response = match (method.as_str(), path.as_str()) {
        ("GET", "/health") => Response::new(Body::from("ok")),
        ("GET", "/config") => match state.current_cfg().await {
            Ok(cfg) => json_response(&cfg),
            Err(err) => {
                error!("failed to load config: {err}");
                text_response(StatusCode::INTERNAL_SERVER_ERROR, "error")
            }
        },
        ("GET", "/patterns") => match state.current_patterns().await {
            Ok((allow, deny)) => json_response(&PatternsResponse {
                allowed: allow,
                denied: deny,
            }),
            Err(err) => {
                error!("failed to load patterns: {err}");
                text_response(StatusCode::INTERNAL_SERVER_ERROR, "error")
            }
        },
        ("GET", "/blocked") => match state.drain_blocked().await {
            Ok(blocked) => json_response(&BlockedResponse { blocked }),
            Err(err) => {
                error!("failed to read blocked queue: {err}");
                text_response(StatusCode::INTERNAL_SERVER_ERROR, "error")
            }
        },
        ("POST", "/mode") => {
            let mut body = req.into_body();
            let mut buf: Vec<u8> = Vec::new();
            loop {
                let chunk = match body.chunk().await {
                    Ok(chunk) => chunk,
                    Err(err) => {
                        error!("failed to read mode body: {err}");
                        return Ok(text_response(StatusCode::BAD_REQUEST, "invalid body"));
                    }
                };
                let Some(chunk) = chunk else {
                    break;
                };

                if buf.len().saturating_add(chunk.len()) > MODE_BODY_LIMIT {
                    return Ok(text_response(
                        StatusCode::PAYLOAD_TOO_LARGE,
                        "body too large",
                    ));
                }
                buf.extend_from_slice(&chunk);
            }

            if buf.is_empty() {
                return Ok(text_response(StatusCode::BAD_REQUEST, "missing body"));
            }
            let update: ModeUpdate = match serde_json::from_slice(&buf) {
                Ok(update) => update,
                Err(err) => {
                    error!("failed to parse mode update: {err}");
                    return Ok(text_response(StatusCode::BAD_REQUEST, "invalid json"));
                }
            };
            match state.set_network_mode(update.mode).await {
                Ok(()) => json_response(&ModeUpdateResponse {
                    status: "ok",
                    mode: update.mode,
                }),
                Err(err) => {
                    error!("mode update failed: {err}");
                    text_response(StatusCode::INTERNAL_SERVER_ERROR, "mode update failed")
                }
            }
        }
        ("POST", "/reload") => match state.force_reload().await {
            Ok(()) => json_response(&ReloadResponse { status: "reloaded" }),
            Err(err) => {
                error!("reload failed: {err}");
                text_response(StatusCode::INTERNAL_SERVER_ERROR, "reload failed")
            }
        },
        _ => text_response(StatusCode::NOT_FOUND, "not found"),
    };
    Ok(response)
}

#[derive(Deserialize)]
struct ModeUpdate {
    mode: NetworkMode,
}

#[derive(Debug, Serialize)]
struct PatternsResponse {
    allowed: Vec<String>,
    denied: Vec<String>,
}

#[derive(Debug, Serialize)]
struct BlockedResponse<T> {
    blocked: T,
}

#[derive(Debug, Serialize)]
struct ModeUpdateResponse {
    status: &'static str,
    mode: NetworkMode,
}

#[derive(Debug, Serialize)]
struct ReloadResponse {
    status: &'static str,
}
