use crate::config::NetworkMode;
use crate::mitm;
use crate::policy::normalize_host;
use crate::state::AppState;
use crate::state::BlockedRequest;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use rama::Context as RamaContext;
use rama::Layer;
use rama::Service;
use rama::http::Body;
use rama::http::Request;
use rama::http::Response;
use rama::http::StatusCode;
use rama::http::client::EasyHttpWebClient;
use rama::http::layer::remove_header::RemoveRequestHeaderLayer;
use rama::http::layer::remove_header::RemoveResponseHeaderLayer;
use rama::http::layer::upgrade::UpgradeLayer;
use rama::http::layer::upgrade::Upgraded;
use rama::http::matcher::MethodMatcher;
use rama::http::server::HttpServer;
use rama::net::http::RequestContext;
use rama::net::proxy::ProxyTarget;
use rama::net::stream::SocketInfo;
use rama::service::service_fn;
use rama::tcp::client::service::Forwarder;
use rama::tcp::server::TcpListener;
use serde_json::json;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::error;
use tracing::info;
use tracing::warn;

type ContextState = Arc<AppState>;
type ProxyContext = RamaContext<ContextState>;

pub async fn run_http_proxy(state: Arc<AppState>, addr: SocketAddr) -> Result<()> {
    let listener = TcpListener::build_with_state(state)
        .bind(addr)
        .await
        .map_err(|err| anyhow!("bind HTTP proxy: {err}"))?;

    let http_service = HttpServer::auto(rama::rt::Executor::new()).service(
        (
            UpgradeLayer::new(
                MethodMatcher::CONNECT,
                service_fn(http_connect_accept),
                service_fn(http_connect_proxy),
            ),
            RemoveResponseHeaderLayer::hop_by_hop(),
            RemoveRequestHeaderLayer::hop_by_hop(),
        )
            .into_layer(service_fn(http_plain_proxy)),
    );

    info!(addr = %addr, "HTTP proxy listening");

    listener.serve(http_service).await;
    Ok(())
}

async fn http_connect_accept(
    mut ctx: ProxyContext,
    req: Request,
) -> Result<(Response, ProxyContext, Request), Response> {
    let authority = match ctx
        .get_or_try_insert_with_ctx::<RequestContext, _>(|ctx| (ctx, &req).try_into())
        .map(|ctx| ctx.authority.clone())
    {
        Ok(authority) => authority,
        Err(err) => {
            warn!(error = %err, "CONNECT missing authority");
            return Err(text_response(StatusCode::BAD_REQUEST, "missing authority"));
        }
    };

    let host = normalize_host(&authority.host().to_string());
    if host.is_empty() {
        return Err(text_response(StatusCode::BAD_REQUEST, "invalid host"));
    }

    let app_state = ctx.state().clone();
    let client = client_addr(&ctx);

    match app_state.host_blocked(&host).await {
        Ok((true, reason)) => {
            let _ = app_state
                .record_blocked(BlockedRequest::new(
                    host.clone(),
                    reason.clone(),
                    client.clone(),
                    Some("CONNECT".to_string()),
                    None,
                    "http-connect".to_string(),
                ))
                .await;
            warn!(
                client = %client.as_deref().unwrap_or_default(),
                host = %host,
                reason = %reason,
                "CONNECT blocked"
            );
            return Err(blocked_text(&reason));
        }
        Ok((false, _)) => {
            info!(
                client = %client.as_deref().unwrap_or_default(),
                host = %host,
                "CONNECT allowed"
            );
        }
        Err(err) => {
            error!(error = %err, "failed to evaluate host");
            return Err(text_response(StatusCode::INTERNAL_SERVER_ERROR, "error"));
        }
    }

    let mode = match app_state.network_mode().await {
        Ok(mode) => mode,
        Err(err) => {
            error!(error = %err, "failed to read network mode");
            return Err(text_response(StatusCode::INTERNAL_SERVER_ERROR, "error"));
        }
    };

    let mitm_state = match app_state.mitm_state().await {
        Ok(state) => state,
        Err(err) => {
            error!(error = %err, "failed to load MITM state");
            return Err(text_response(StatusCode::INTERNAL_SERVER_ERROR, "error"));
        }
    };

    if mode == NetworkMode::Limited && mitm_state.is_none() {
        let _ = app_state
            .record_blocked(BlockedRequest::new(
                host.clone(),
                "mitm_required".to_string(),
                client.clone(),
                Some("CONNECT".to_string()),
                Some(NetworkMode::Limited),
                "http-connect".to_string(),
            ))
            .await;
        warn!(
            client = %client.as_deref().unwrap_or_default(),
            host = %host,
            mode = "limited",
            allowed_methods = "GET, HEAD, OPTIONS",
            "CONNECT blocked; MITM required for read-only HTTPS in limited mode"
        );
        return Err(blocked_text("mitm_required"));
    }

    ctx.insert(ProxyTarget(authority));
    ctx.insert(mode);
    if let Some(mitm_state) = mitm_state {
        ctx.insert(mitm_state);
    }

    Ok((
        Response::builder()
            .status(StatusCode::OK)
            .body(Body::empty())
            .unwrap_or_else(|_| Response::new(Body::empty())),
        ctx,
        req,
    ))
}

async fn http_connect_proxy(ctx: ProxyContext, upgraded: Upgraded) -> Result<(), Infallible> {
    let mode = ctx
        .get::<NetworkMode>()
        .copied()
        .unwrap_or(NetworkMode::Full);
    let authority = match ctx.get::<ProxyTarget>().map(|target| target.0.clone()) {
        Some(authority) => authority,
        None => {
            warn!("CONNECT missing proxy target");
            return Ok(());
        }
    };
    let host = normalize_host(&authority.host().to_string());

    if let Some(mitm_state) = ctx.get::<Arc<mitm::MitmState>>().cloned() {
        info!(host = %host, port = authority.port(), mode = ?mode, "CONNECT MITM enabled");
        if let Err(err) = mitm::mitm_tunnel(
            ctx,
            upgraded,
            host.as_str(),
            authority.port(),
            mode,
            mitm_state,
        )
        .await
        {
            warn!(error = %err, "MITM tunnel error");
        }
        return Ok(());
    }

    let forwarder = Forwarder::ctx();
    if let Err(err) = forwarder.serve(ctx, upgraded).await {
        warn!(error = %err, "tunnel error");
    }
    Ok(())
}

async fn http_plain_proxy(mut ctx: ProxyContext, req: Request) -> Result<Response, Infallible> {
    let app_state = ctx.state().clone();
    let client = client_addr(&ctx);

    let method_allowed = match app_state.method_allowed(req.method().as_str()).await {
        Ok(allowed) => allowed,
        Err(err) => {
            error!(error = %err, "failed to evaluate method policy");
            return Ok(text_response(StatusCode::INTERNAL_SERVER_ERROR, "error"));
        }
    };

    if let Some(socket_path) = req
        .headers()
        .get("x-unix-socket")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_string())
    {
        if !method_allowed {
            warn!(
                client = %client.as_deref().unwrap_or_default(),
                method = %req.method(),
                mode = "limited",
                allowed_methods = "GET, HEAD, OPTIONS",
                "unix socket blocked by method policy"
            );
            return Ok(json_blocked("unix-socket", "method_not_allowed"));
        }

        if !cfg!(target_os = "macos") {
            warn!(path = %socket_path, "unix socket proxy unsupported on this platform");
            return Ok(text_response(
                StatusCode::NOT_IMPLEMENTED,
                "unix sockets unsupported",
            ));
        }

        match app_state.is_unix_socket_allowed(&socket_path).await {
            Ok(true) => {
                info!(
                    client = %client.as_deref().unwrap_or_default(),
                    path = %socket_path,
                    "unix socket allowed"
                );
                match proxy_via_unix_socket(ctx, req, &socket_path).await {
                    Ok(resp) => return Ok(resp),
                    Err(err) => {
                        warn!(error = %err, "unix socket proxy failed");
                        return Ok(text_response(
                            StatusCode::BAD_GATEWAY,
                            "unix socket proxy failed",
                        ));
                    }
                }
            }
            Ok(false) => {
                warn!(
                    client = %client.as_deref().unwrap_or_default(),
                    path = %socket_path,
                    "unix socket blocked"
                );
                return Ok(json_blocked("unix-socket", "not_allowed"));
            }
            Err(err) => {
                warn!(error = %err, "unix socket check failed");
                return Ok(text_response(StatusCode::INTERNAL_SERVER_ERROR, "error"));
            }
        }
    }

    let authority = match ctx
        .get_or_try_insert_with_ctx::<RequestContext, _>(|ctx| (ctx, &req).try_into())
        .map(|ctx| ctx.authority.clone())
    {
        Ok(authority) => authority,
        Err(err) => {
            warn!(error = %err, "missing host");
            return Ok(text_response(StatusCode::BAD_REQUEST, "missing host"));
        }
    };
    let host = normalize_host(&authority.host().to_string());

    match app_state.host_blocked(&host).await {
        Ok((true, reason)) => {
            let _ = app_state
                .record_blocked(BlockedRequest::new(
                    host.clone(),
                    reason.clone(),
                    client.clone(),
                    Some(req.method().as_str().to_string()),
                    None,
                    "http".to_string(),
                ))
                .await;
            warn!(
                client = %client.as_deref().unwrap_or_default(),
                host = %host,
                reason = %reason,
                "request blocked"
            );
            return Ok(json_blocked(&host, &reason));
        }
        Ok((false, _)) => {}
        Err(err) => {
            error!(error = %err, "failed to evaluate host");
            return Ok(text_response(StatusCode::INTERNAL_SERVER_ERROR, "error"));
        }
    }

    if !method_allowed {
        let _ = app_state
            .record_blocked(BlockedRequest::new(
                host.clone(),
                "method_not_allowed".to_string(),
                client.clone(),
                Some(req.method().as_str().to_string()),
                Some(NetworkMode::Limited),
                "http".to_string(),
            ))
            .await;
        warn!(
            client = %client.as_deref().unwrap_or_default(),
            host = %host,
            method = %req.method(),
            mode = "limited",
            allowed_methods = "GET, HEAD, OPTIONS",
            "request blocked by method policy"
        );
        return Ok(json_blocked(&host, "method_not_allowed"));
    }

    info!(
        client = %client.as_deref().unwrap_or_default(),
        host = %host,
        method = %req.method(),
        "request allowed"
    );

    let client = EasyHttpWebClient::default();
    match client.serve(ctx, req).await {
        Ok(resp) => Ok(resp),
        Err(err) => {
            warn!(error = %err, "upstream request failed");
            Ok(text_response(StatusCode::BAD_GATEWAY, "upstream failure"))
        }
    }
}

async fn proxy_via_unix_socket(
    ctx: ProxyContext,
    req: Request,
    socket_path: &str,
) -> Result<Response> {
    #[cfg(target_os = "macos")]
    {
        use rama::unix::client::UnixConnector;

        let client = EasyHttpWebClient::builder()
            .with_custom_transport_connector(UnixConnector::fixed(socket_path))
            .without_tls_proxy_support()
            .without_proxy_support()
            .without_tls_support()
            .build();

        let (mut parts, body) = req.into_parts();
        let path = parts
            .uri
            .path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/");
        parts.uri = path
            .parse()
            .with_context(|| format!("invalid unix socket request path: {path}"))?;
        parts.headers.remove("x-unix-socket");

        let req = Request::from_parts(parts, body);
        Ok(client.serve(ctx, req).await?)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = ctx;
        let _ = req;
        let _ = socket_path;
        Err(anyhow::anyhow!("unix sockets not supported"))
    }
}

fn client_addr(ctx: &ProxyContext) -> Option<String> {
    ctx.get::<SocketInfo>()
        .map(|info| info.peer_addr().to_string())
}

fn json_blocked(host: &str, reason: &str) -> Response {
    let body = Body::from(json!({"status":"blocked","host":host,"reason":reason}).to_string());
    Response::builder()
        .status(StatusCode::FORBIDDEN)
        .header("content-type", "application/json")
        .header("x-proxy-error", blocked_header_value(reason))
        .body(body)
        .unwrap_or_else(|_| Response::new(Body::from("blocked")))
}

fn blocked_text(reason: &str) -> Response {
    Response::builder()
        .status(StatusCode::FORBIDDEN)
        .header("content-type", "text/plain")
        .header("x-proxy-error", blocked_header_value(reason))
        .body(Body::from(blocked_message(reason)))
        .unwrap_or_else(|_| Response::new(Body::from("blocked")))
}

fn text_response(status: StatusCode, body: &str) -> Response {
    Response::builder()
        .status(status)
        .header("content-type", "text/plain")
        .body(Body::from(body.to_string()))
        .unwrap_or_else(|_| Response::new(Body::from(body.to_string())))
}

fn blocked_header_value(reason: &str) -> &'static str {
    match reason {
        "not_allowed" | "not_allowed_local" => "blocked-by-allowlist",
        "denied" => "blocked-by-denylist",
        "method_not_allowed" => "blocked-by-method-policy",
        "mitm_required" => "blocked-by-mitm-required",
        _ => "blocked-by-policy",
    }
}

fn blocked_message(reason: &str) -> &'static str {
    match reason {
        "not_allowed" => "Codex blocked this request: domain not in allowlist.",
        "not_allowed_local" => "Codex blocked this request: local addresses not allowed.",
        "denied" => "Codex blocked this request: domain denied by policy.",
        "method_not_allowed" => "Codex blocked this request: method not allowed in limited mode.",
        "mitm_required" => "Codex blocked this request: MITM required for limited HTTPS.",
        _ => "Codex blocked this request by network policy.",
    }
}
