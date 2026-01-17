use crate::config::NetworkMode;
use crate::mitm;
use crate::network_policy::NetworkDecision;
use crate::network_policy::NetworkPolicyDecider;
use crate::network_policy::NetworkPolicyRequest;
use crate::network_policy::NetworkProtocol;
use crate::network_policy::evaluate_host_policy;
use crate::policy::normalize_host;
use crate::responses::blocked_header_value;
use crate::state::AppState;
use crate::state::BlockedRequest;
use anyhow::Context as _;
use anyhow::Result;
use rama::Layer;
use rama::Service;
use rama::extensions::ExtensionsMut;
use rama::extensions::ExtensionsRef;
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
use rama::layer::AddInputExtensionLayer;
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

pub async fn run_http_proxy(
    state: Arc<AppState>,
    addr: SocketAddr,
    policy_decider: Option<Arc<dyn NetworkPolicyDecider>>,
) -> Result<()> {
    let listener = TcpListener::build()
        .bind(addr)
        .await
        // Rama's `BoxError` is a `Box<dyn Error + Send + Sync>` without an explicit `'static`
        // lifetime bound, which means it doesn't satisfy `anyhow::Context`'s `StdError` constraint.
        // Wrap it in Rama's `OpaqueError` so we can preserve the original error as a source and
        // still use `anyhow` for chaining.
        .map_err(rama::error::OpaqueError::from)
        .map_err(anyhow::Error::from)
        .with_context(|| format!("bind HTTP proxy: {addr}"))?;

    let http_service = HttpServer::auto(rama::rt::Executor::new()).service(
        (
            UpgradeLayer::new(
                MethodMatcher::CONNECT,
                service_fn({
                    let policy_decider = policy_decider.clone();
                    move |req| http_connect_accept(policy_decider.clone(), req)
                }),
                service_fn(http_connect_proxy),
            ),
            RemoveResponseHeaderLayer::hop_by_hop(),
            RemoveRequestHeaderLayer::hop_by_hop(),
        )
            .into_layer(service_fn({
                let policy_decider = policy_decider.clone();
                move |req| http_plain_proxy(policy_decider.clone(), req)
            })),
    );

    info!("HTTP proxy listening on {addr}");

    listener
        .serve(AddInputExtensionLayer::new(state).into_layer(http_service))
        .await;
    Ok(())
}

async fn http_connect_accept(
    policy_decider: Option<Arc<dyn NetworkPolicyDecider>>,
    mut req: Request,
) -> Result<(Response, Request), Response> {
    let app_state = req
        .extensions()
        .get::<Arc<AppState>>()
        .cloned()
        .ok_or_else(|| text_response(StatusCode::INTERNAL_SERVER_ERROR, "missing state"))?;

    let authority = match RequestContext::try_from(&req).map(|ctx| ctx.host_with_port()) {
        Ok(authority) => authority,
        Err(err) => {
            warn!("CONNECT missing authority: {err}");
            return Err(text_response(StatusCode::BAD_REQUEST, "missing authority"));
        }
    };

    let host = normalize_host(&authority.host.to_string());
    if host.is_empty() {
        return Err(text_response(StatusCode::BAD_REQUEST, "invalid host"));
    }

    let client = client_addr(&req);

    let request = NetworkPolicyRequest::new(
        NetworkProtocol::HttpsConnect,
        host.clone(),
        authority.port,
        client.clone(),
        Some("CONNECT".to_string()),
        None,
        None,
    );

    match evaluate_host_policy(&app_state, policy_decider.as_ref(), &request).await {
        Ok(NetworkDecision::Deny { reason }) => {
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
            let client = client.as_deref().unwrap_or_default();
            warn!("CONNECT blocked (client={client}, host={host}, reason={reason})");
            return Err(blocked_text(&reason));
        }
        Ok(NetworkDecision::Allow) => {
            let client = client.as_deref().unwrap_or_default();
            info!("CONNECT allowed (client={client}, host={host})");
        }
        Err(err) => {
            error!("failed to evaluate host for CONNECT {host}: {err}");
            return Err(text_response(StatusCode::INTERNAL_SERVER_ERROR, "error"));
        }
    }

    let mode = match app_state.network_mode().await {
        Ok(mode) => mode,
        Err(err) => {
            error!("failed to read network mode: {err}");
            return Err(text_response(StatusCode::INTERNAL_SERVER_ERROR, "error"));
        }
    };

    let mitm_state = match app_state.mitm_state().await {
        Ok(state) => state,
        Err(err) => {
            error!("failed to load MITM state: {err}");
            return Err(text_response(StatusCode::INTERNAL_SERVER_ERROR, "error"));
        }
    };

    if mode == NetworkMode::Limited && mitm_state.is_none() {
        // Limited mode is designed to be read-only. Without MITM, a CONNECT tunnel would hide the
        // inner HTTP method/headers from the proxy, effectively bypassing method policy.
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
        let client = client.as_deref().unwrap_or_default();
        warn!(
            "CONNECT blocked; MITM required for read-only HTTPS in limited mode (client={client}, host={host}, mode=limited, allowed_methods=GET, HEAD, OPTIONS)"
        );
        return Err(blocked_text("mitm_required"));
    }

    req.extensions_mut().insert(ProxyTarget(authority));
    req.extensions_mut().insert(mode);
    if let Some(mitm_state) = mitm_state {
        req.extensions_mut().insert(mitm_state);
    }

    Ok((
        Response::builder()
            .status(StatusCode::OK)
            .body(Body::empty())
            .unwrap_or_else(|_| Response::new(Body::empty())),
        req,
    ))
}

async fn http_connect_proxy(upgraded: Upgraded) -> Result<(), Infallible> {
    let mode = upgraded
        .extensions()
        .get::<NetworkMode>()
        .copied()
        .unwrap_or(NetworkMode::Full);

    let Some(target) = upgraded
        .extensions()
        .get::<ProxyTarget>()
        .map(|t| t.0.clone())
    else {
        warn!("CONNECT missing proxy target");
        return Ok(());
    };
    let host = normalize_host(&target.host.to_string());

    if upgraded
        .extensions()
        .get::<Arc<mitm::MitmState>>()
        .is_some()
    {
        let port = target.port;
        info!("CONNECT MITM enabled (host={host}, port={port}, mode={mode:?})");
        if let Err(err) = mitm::mitm_tunnel(upgraded).await {
            warn!("MITM tunnel error: {err}");
        }
        return Ok(());
    }

    let forwarder = Forwarder::ctx();
    if let Err(err) = forwarder.serve(upgraded).await {
        warn!("tunnel error: {err}");
    }
    Ok(())
}

async fn http_plain_proxy(
    policy_decider: Option<Arc<dyn NetworkPolicyDecider>>,
    req: Request,
) -> Result<Response, Infallible> {
    let app_state = match req.extensions().get::<Arc<AppState>>().cloned() {
        Some(state) => state,
        None => {
            error!("missing app state");
            return Ok(text_response(StatusCode::INTERNAL_SERVER_ERROR, "error"));
        }
    };
    let client = client_addr(&req);

    let method_allowed = match app_state.method_allowed(req.method().as_str()).await {
        Ok(allowed) => allowed,
        Err(err) => {
            error!("failed to evaluate method policy: {err}");
            return Ok(text_response(StatusCode::INTERNAL_SERVER_ERROR, "error"));
        }
    };

    // `x-unix-socket` is an escape hatch for talking to local daemons. We keep it tightly scoped:
    // macOS-only + explicit allowlist, to avoid turning the proxy into a general local capability
    // escalation mechanism.
    if let Some(unix_socket_header) = req.headers().get("x-unix-socket") {
        let socket_path = match unix_socket_header.to_str() {
            Ok(value) => value.to_string(),
            Err(_) => {
                warn!("invalid x-unix-socket header value (non-UTF8)");
                return Ok(text_response(
                    StatusCode::BAD_REQUEST,
                    "invalid x-unix-socket header",
                ));
            }
        };
        if !method_allowed {
            let client = client.as_deref().unwrap_or_default();
            let method = req.method();
            warn!(
                "unix socket blocked by method policy (client={client}, method={method}, mode=limited, allowed_methods=GET, HEAD, OPTIONS)"
            );
            return Ok(json_blocked("unix-socket", "method_not_allowed"));
        }

        if !cfg!(target_os = "macos") {
            warn!("unix socket proxy unsupported on this platform (path={socket_path})");
            return Ok(text_response(
                StatusCode::NOT_IMPLEMENTED,
                "unix sockets unsupported",
            ));
        }

        match app_state.is_unix_socket_allowed(&socket_path).await {
            Ok(true) => {
                let client = client.as_deref().unwrap_or_default();
                info!("unix socket allowed (client={client}, path={socket_path})");
                match proxy_via_unix_socket(req, &socket_path).await {
                    Ok(resp) => return Ok(resp),
                    Err(err) => {
                        warn!("unix socket proxy failed: {err}");
                        return Ok(text_response(
                            StatusCode::BAD_GATEWAY,
                            "unix socket proxy failed",
                        ));
                    }
                }
            }
            Ok(false) => {
                let client = client.as_deref().unwrap_or_default();
                warn!("unix socket blocked (client={client}, path={socket_path})");
                return Ok(json_blocked("unix-socket", "not_allowed"));
            }
            Err(err) => {
                warn!("unix socket check failed: {err}");
                return Ok(text_response(StatusCode::INTERNAL_SERVER_ERROR, "error"));
            }
        }
    }

    let authority = match RequestContext::try_from(&req).map(|ctx| ctx.host_with_port()) {
        Ok(authority) => authority,
        Err(err) => {
            warn!("missing host: {err}");
            return Ok(text_response(StatusCode::BAD_REQUEST, "missing host"));
        }
    };
    let host = normalize_host(&authority.host.to_string());
    let port = authority.port;

    let request = NetworkPolicyRequest::new(
        NetworkProtocol::Http,
        host.clone(),
        port,
        client.clone(),
        Some(req.method().as_str().to_string()),
        None,
        None,
    );

    match evaluate_host_policy(&app_state, policy_decider.as_ref(), &request).await {
        Ok(NetworkDecision::Deny { reason }) => {
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
            let client = client.as_deref().unwrap_or_default();
            warn!("request blocked (client={client}, host={host}, reason={reason})");
            return Ok(json_blocked(&host, &reason));
        }
        Ok(NetworkDecision::Allow) => {}
        Err(err) => {
            error!("failed to evaluate host for {host}: {err}");
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
        let client = client.as_deref().unwrap_or_default();
        let method = req.method();
        warn!(
            "request blocked by method policy (client={client}, host={host}, method={method}, mode=limited, allowed_methods=GET, HEAD, OPTIONS)"
        );
        return Ok(json_blocked(&host, "method_not_allowed"));
    }

    let client = client.as_deref().unwrap_or_default();
    let method = req.method();
    info!("request allowed (client={client}, host={host}, method={method})");

    let client = EasyHttpWebClient::default();
    match client.serve(req).await {
        Ok(resp) => Ok(resp),
        Err(err) => {
            warn!("upstream request failed: {err}");
            Ok(text_response(StatusCode::BAD_GATEWAY, "upstream failure"))
        }
    }
}

async fn proxy_via_unix_socket(req: Request, socket_path: &str) -> Result<Response> {
    #[cfg(target_os = "macos")]
    {
        use rama::unix::client::UnixConnector;

        let client = EasyHttpWebClient::connector_builder()
            .with_custom_transport_connector(UnixConnector::fixed(socket_path))
            .without_tls_proxy_support()
            .without_proxy_support()
            .without_tls_support()
            .with_default_http_connector()
            .build_client();

        let (mut parts, body) = req.into_parts();
        let path = parts
            .uri
            .path_and_query()
            .map(rama::http::uri::PathAndQuery::as_str)
            .unwrap_or("/");
        parts.uri = path
            .parse()
            .with_context(|| format!("invalid unix socket request path: {path}"))?;
        parts.headers.remove("x-unix-socket");

        let req = Request::from_parts(parts, body);
        Ok(client.serve(req).await?)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = req;
        let _ = socket_path;
        Err(anyhow::anyhow!("unix sockets not supported"))
    }
}

fn client_addr<T: ExtensionsRef>(input: &T) -> Option<String> {
    input
        .extensions()
        .get::<SocketInfo>()
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
    crate::responses::blocked_text_response(reason)
}

fn text_response(status: StatusCode, body: &str) -> Response {
    Response::builder()
        .status(status)
        .header("content-type", "text/plain")
        .body(Body::from(body.to_string()))
        .unwrap_or_else(|_| Response::new(Body::from(body.to_string())))
}
