use crate::config::NetworkMode;
use crate::responses::json_response;
use crate::responses::text_response;
use crate::state::AppState;
use anyhow::Result;
use rama::Context as RamaContext;
use rama::http::Body;
use rama::http::Request;
use rama::http::Response;
use rama::http::StatusCode;
use rama::http::server::HttpServer;
use rama::service::service_fn;
use rama::tcp::server::TcpListener;
use serde::Deserialize;
use serde::Serialize;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::error;
use tracing::info;

type ContextState = Arc<AppState>;
type AdminContext = RamaContext<ContextState>;

pub async fn run_admin_api(state: Arc<AppState>, addr: SocketAddr) -> Result<()> {
    let listener = TcpListener::build_with_state(state)
        .bind(addr)
        .await
        .map_err(|err| anyhow::anyhow!("bind admin API: {err}"))?;

    let server =
        HttpServer::auto(rama::rt::Executor::new()).service(service_fn(handle_admin_request));
    info!("admin API listening on {addr}");
    listener.serve(server).await;
    Ok(())
}

async fn handle_admin_request(ctx: AdminContext, req: Request) -> Result<Response, Infallible> {
    const MODE_BODY_LIMIT: usize = 8 * 1024;

    let state = ctx.state().clone();
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
