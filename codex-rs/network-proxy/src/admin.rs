use crate::config::NetworkMode;
use crate::responses::json_response;
use crate::responses::text_response;
use crate::state::AppState;
use anyhow::Result;
use hyper::Body;
use hyper::Method;
use hyper::Request;
use hyper::Response;
use hyper::Server;
use hyper::StatusCode;
use hyper::body::to_bytes;
use hyper::service::make_service_fn;
use hyper::service::service_fn;
use serde::Deserialize;
use serde_json::json;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::error;
use tracing::info;

pub async fn run_admin_api(state: Arc<AppState>, addr: SocketAddr) -> Result<()> {
    let make_svc = make_service_fn(move |_conn: &hyper::server::conn::AddrStream| {
        let state = state.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                handle_admin_request(req, state.clone())
            }))
        }
    });
    let server = Server::bind(&addr).serve(make_svc);
    info!(addr = %addr, "admin API listening");
    server.await?;
    Ok(())
}

async fn handle_admin_request(
    req: Request<Body>,
    state: Arc<AppState>,
) -> Result<Response<Body>, Infallible> {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let response = match (method, path.as_str()) {
        (Method::GET, "/health") => Response::new(Body::from("ok")),
        (Method::GET, "/config") => match state.current_cfg().await {
            Ok(cfg) => json_response(&cfg),
            Err(err) => {
                error!(error = %err, "failed to load config");
                text_response(StatusCode::INTERNAL_SERVER_ERROR, "error")
            }
        },
        (Method::GET, "/patterns") => match state.current_patterns().await {
            Ok((allow, deny)) => json_response(&json!({"allowed": allow, "denied": deny})),
            Err(err) => {
                error!(error = %err, "failed to load patterns");
                text_response(StatusCode::INTERNAL_SERVER_ERROR, "error")
            }
        },
        (Method::GET, "/blocked") => match state.drain_blocked().await {
            Ok(blocked) => json_response(&json!({ "blocked": blocked })),
            Err(err) => {
                error!(error = %err, "failed to read blocked queue");
                text_response(StatusCode::INTERNAL_SERVER_ERROR, "error")
            }
        },
        (Method::POST, "/mode") => {
            let body = match to_bytes(req.into_body()).await {
                Ok(bytes) => bytes,
                Err(err) => {
                    error!(error = %err, "failed to read mode body");
                    return Ok(text_response(StatusCode::BAD_REQUEST, "invalid body"));
                }
            };
            if body.is_empty() {
                return Ok(text_response(StatusCode::BAD_REQUEST, "missing body"));
            }
            let update: ModeUpdate = match serde_json::from_slice(&body) {
                Ok(update) => update,
                Err(err) => {
                    error!(error = %err, "failed to parse mode update");
                    return Ok(text_response(StatusCode::BAD_REQUEST, "invalid json"));
                }
            };
            match state.set_network_mode(update.mode).await {
                Ok(()) => json_response(&json!({"status": "ok", "mode": update.mode})),
                Err(err) => {
                    error!(error = %err, "mode update failed");
                    text_response(StatusCode::INTERNAL_SERVER_ERROR, "mode update failed")
                }
            }
        }
        (Method::POST, "/reload") => match state.force_reload().await {
            Ok(()) => json_response(&json!({"status": "reloaded"})),
            Err(err) => {
                error!(error = %err, "reload failed");
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
