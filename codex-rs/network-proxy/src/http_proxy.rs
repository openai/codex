use crate::config::NetworkMode;
use crate::network_policy::NetworkDecision;
use crate::network_policy::NetworkPolicyDecider;
use crate::network_policy::NetworkPolicyRequest;
use crate::network_policy::NetworkPolicyRequestArgs;
use crate::network_policy::NetworkProtocol;
use crate::network_policy::evaluate_host_policy;
use crate::policy::normalize_host;
use crate::reasons::REASON_METHOD_NOT_ALLOWED;
use crate::reasons::REASON_NOT_ALLOWED;
use crate::reasons::REASON_PROXY_DISABLED;
use crate::responses::blocked_header_value;
use crate::responses::json_response;
use crate::runtime::unix_socket_permissions_supported;
use crate::state::BlockedRequest;
use crate::state::BlockedRequestArgs;
use crate::state::NetworkProxyState;
use crate::upstream::UpstreamClient;
use crate::upstream::proxy_for_connect;
use anyhow::Context as _;
use anyhow::Result;
use rama_core::Layer;
use rama_core::Service;
use rama_core::error::BoxError;
use rama_core::error::ErrorExt as _;
use rama_core::error::OpaqueError;
use rama_core::extensions::ExtensionsMut;
use rama_core::extensions::ExtensionsRef;
use rama_core::layer::AddInputExtensionLayer;
use rama_core::rt::Executor;
use rama_core::service::service_fn;
use rama_http::Body;
use rama_http::HeaderValue;
use rama_http::Request;
use rama_http::Response;
use rama_http::StatusCode;
use rama_http::layer::remove_header::RemoveRequestHeaderLayer;
use rama_http::layer::remove_header::RemoveResponseHeaderLayer;
use rama_http::matcher::MethodMatcher;
use rama_http_backend::client::proxy::layer::HttpProxyConnector;
use rama_http_backend::server::HttpServer;
use rama_http_backend::server::layer::upgrade::UpgradeLayer;
use rama_http_backend::server::layer::upgrade::Upgraded;
use rama_net::Protocol;
use rama_net::address::ProxyAddress;
use rama_net::client::ConnectorService;
use rama_net::client::EstablishedClientConnection;
use rama_net::http::RequestContext;
use rama_net::proxy::ProxyRequest;
use rama_net::proxy::ProxyTarget;
use rama_net::proxy::StreamForwardService;
use rama_net::stream::SocketInfo;
use rama_tcp::client::Request as TcpRequest;
use rama_tcp::client::service::TcpConnector;
use rama_tcp::server::TcpListener;
use rama_tls_boring::client::TlsConnectorDataBuilder;
use rama_tls_boring::client::TlsConnectorLayer;
use serde::Serialize;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::error;
use tracing::info;
use tracing::warn;

pub async fn run_http_proxy(
    state: Arc<NetworkProxyState>,
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
        .map_err(rama_core::error::OpaqueError::from)
        .map_err(anyhow::Error::from)
        .with_context(|| format!("bind HTTP proxy: {addr}"))?;

    let http_service = HttpServer::auto(Executor::new()).service(
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
        .get::<Arc<NetworkProxyState>>()
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

    let enabled = app_state
        .enabled()
        .await
        .map_err(|err| internal_error("failed to read enabled state", err))?;
    if !enabled {
        let client = client.as_deref().unwrap_or_default();
        warn!("CONNECT blocked; proxy disabled (client={client}, host={host})");
        return Err(proxy_disabled_response(
            &app_state,
            host,
            client_addr(&req),
            Some("CONNECT".to_string()),
            "http-connect",
        )
        .await);
    }

    let request = NetworkPolicyRequest::new(NetworkPolicyRequestArgs {
        protocol: NetworkProtocol::HttpsConnect,
        host: host.clone(),
        port: authority.port,
        client_addr: client.clone(),
        method: Some("CONNECT".to_string()),
        command: None,
        exec_policy_hint: None,
    });

    match evaluate_host_policy(&app_state, policy_decider.as_ref(), &request).await {
        Ok(NetworkDecision::Deny { reason }) => {
            let _ = app_state
                .record_blocked(BlockedRequest::new(BlockedRequestArgs {
                    host: host.clone(),
                    reason: reason.clone(),
                    client: client.clone(),
                    method: Some("CONNECT".to_string()),
                    mode: None,
                    protocol: "http-connect".to_string(),
                }))
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

    let mode = app_state
        .network_mode()
        .await
        .map_err(|err| internal_error("failed to read network mode", err))?;

    if mode == NetworkMode::Limited {
        let _ = app_state
            .record_blocked(BlockedRequest::new(BlockedRequestArgs {
                host: host.clone(),
                reason: REASON_METHOD_NOT_ALLOWED.to_string(),
                client: client.clone(),
                method: Some("CONNECT".to_string()),
                mode: Some(NetworkMode::Limited),
                protocol: "http-connect".to_string(),
            }))
            .await;
        let client = client.as_deref().unwrap_or_default();
        warn!("CONNECT blocked by method policy (client={client}, host={host}, mode=limited)");
        return Err(blocked_text(REASON_METHOD_NOT_ALLOWED));
    }

    req.extensions_mut().insert(ProxyTarget(authority));
    req.extensions_mut().insert(mode);

    Ok((
        Response::builder()
            .status(StatusCode::OK)
            .body(Body::empty())
            .unwrap_or_else(|_| Response::new(Body::empty())),
        req,
    ))
}

async fn http_connect_proxy(upgraded: Upgraded) -> Result<(), Infallible> {
    if upgraded.extensions().get::<ProxyTarget>().is_none() {
        warn!("CONNECT missing proxy target");
        return Ok(());
    }

    let allow_upstream_proxy = match upgraded
        .extensions()
        .get::<Arc<NetworkProxyState>>()
        .cloned()
    {
        Some(state) => match state.allow_upstream_proxy().await {
            Ok(allowed) => allowed,
            Err(err) => {
                error!("failed to read upstream proxy setting: {err}");
                false
            }
        },
        None => {
            error!("missing app state");
            false
        }
    };

    let proxy = if allow_upstream_proxy {
        proxy_for_connect()
    } else {
        None
    };

    if let Err(err) = forward_connect_tunnel(upgraded, proxy).await {
        warn!("tunnel error: {err}");
    }
    Ok(())
}

async fn forward_connect_tunnel(
    upgraded: Upgraded,
    proxy: Option<ProxyAddress>,
) -> Result<(), BoxError> {
    let authority = upgraded
        .extensions()
        .get::<ProxyTarget>()
        .map(|target| target.0.clone())
        .ok_or_else(|| OpaqueError::from_display("missing forward authority").into_boxed())?;

    let mut extensions = upgraded.extensions().clone();
    if let Some(proxy) = proxy {
        extensions.insert(proxy);
    }

    let req = TcpRequest::new_with_extensions(authority.clone(), extensions)
        .with_protocol(Protocol::HTTPS);
    let proxy_connector = HttpProxyConnector::optional(TcpConnector::new());
    let tls_config = TlsConnectorDataBuilder::new_http_auto().into_shared_builder();
    let connector = TlsConnectorLayer::tunnel(None)
        .with_connector_data(tls_config)
        .into_layer(proxy_connector);
    let EstablishedClientConnection { conn: target, .. } =
        connector.connect(req).await.map_err(|err| {
            OpaqueError::from_boxed(err)
                .with_context(|| format!("establish CONNECT tunnel to {authority}"))
                .into_boxed()
        })?;

    let proxy_req = ProxyRequest {
        source: upgraded,
        target,
    };
    StreamForwardService::default()
        .serve(proxy_req)
        .await
        .map_err(|err| {
            OpaqueError::from_boxed(err.into())
                .with_context(|| format!("forward CONNECT tunnel to {authority}"))
                .into_boxed()
        })
}

async fn http_plain_proxy(
    policy_decider: Option<Arc<dyn NetworkPolicyDecider>>,
    req: Request,
) -> Result<Response, Infallible> {
    let app_state = match req.extensions().get::<Arc<NetworkProxyState>>().cloned() {
        Some(state) => state,
        None => {
            error!("missing app state");
            return Ok(text_response(StatusCode::INTERNAL_SERVER_ERROR, "error"));
        }
    };
    let client = client_addr(&req);

    let method_allowed = match app_state
        .method_allowed(req.method().as_str())
        .await
        .map_err(|err| internal_error("failed to evaluate method policy", err))
    {
        Ok(allowed) => allowed,
        Err(resp) => return Ok(resp),
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
        let enabled = match app_state
            .enabled()
            .await
            .map_err(|err| internal_error("failed to read enabled state", err))
        {
            Ok(enabled) => enabled,
            Err(resp) => return Ok(resp),
        };
        if !enabled {
            let client = client.as_deref().unwrap_or_default();
            warn!("unix socket blocked; proxy disabled (client={client}, path={socket_path})");
            return Ok(proxy_disabled_response(
                &app_state,
                socket_path,
                client_addr(&req),
                Some(req.method().as_str().to_string()),
                "unix-socket",
            )
            .await);
        }
        if !method_allowed {
            let client = client.as_deref().unwrap_or_default();
            let method = req.method();
            warn!(
                "unix socket blocked by method policy (client={client}, method={method}, mode=limited, allowed_methods=GET, HEAD, OPTIONS)"
            );
            return Ok(json_blocked("unix-socket", REASON_METHOD_NOT_ALLOWED));
        }

        if !unix_socket_permissions_supported() {
            warn!("unix socket proxy unsupported on this platform (path={socket_path})");
            return Ok(text_response(
                StatusCode::NOT_IMPLEMENTED,
                "unix sockets unsupported",
            ));
        }

        return match app_state.is_unix_socket_allowed(&socket_path).await {
            Ok(true) => {
                let client = client.as_deref().unwrap_or_default();
                info!("unix socket allowed (client={client}, path={socket_path})");
                match proxy_via_unix_socket(req, &socket_path).await {
                    Ok(resp) => Ok(resp),
                    Err(err) => {
                        warn!("unix socket proxy failed: {err}");
                        Ok(text_response(
                            StatusCode::BAD_GATEWAY,
                            "unix socket proxy failed",
                        ))
                    }
                }
            }
            Ok(false) => {
                let client = client.as_deref().unwrap_or_default();
                warn!("unix socket blocked (client={client}, path={socket_path})");
                Ok(json_blocked("unix-socket", REASON_NOT_ALLOWED))
            }
            Err(err) => {
                warn!("unix socket check failed: {err}");
                Ok(text_response(StatusCode::INTERNAL_SERVER_ERROR, "error"))
            }
        };
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
    let enabled = match app_state
        .enabled()
        .await
        .map_err(|err| internal_error("failed to read enabled state", err))
    {
        Ok(enabled) => enabled,
        Err(resp) => return Ok(resp),
    };
    if !enabled {
        let client = client.as_deref().unwrap_or_default();
        let method = req.method();
        warn!("request blocked; proxy disabled (client={client}, host={host}, method={method})");
        return Ok(proxy_disabled_response(
            &app_state,
            host,
            client_addr(&req),
            Some(req.method().as_str().to_string()),
            "http",
        )
        .await);
    }

    let request = NetworkPolicyRequest::new(NetworkPolicyRequestArgs {
        protocol: NetworkProtocol::Http,
        host: host.clone(),
        port,
        client_addr: client.clone(),
        method: Some(req.method().as_str().to_string()),
        command: None,
        exec_policy_hint: None,
    });

    match evaluate_host_policy(&app_state, policy_decider.as_ref(), &request).await {
        Ok(NetworkDecision::Deny { reason }) => {
            let _ = app_state
                .record_blocked(BlockedRequest::new(BlockedRequestArgs {
                    host: host.clone(),
                    reason: reason.clone(),
                    client: client.clone(),
                    method: Some(req.method().as_str().to_string()),
                    mode: None,
                    protocol: "http".to_string(),
                }))
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
            .record_blocked(BlockedRequest::new(BlockedRequestArgs {
                host: host.clone(),
                reason: REASON_METHOD_NOT_ALLOWED.to_string(),
                client: client.clone(),
                method: Some(req.method().as_str().to_string()),
                mode: Some(NetworkMode::Limited),
                protocol: "http".to_string(),
            }))
            .await;
        let client = client.as_deref().unwrap_or_default();
        let method = req.method();
        warn!(
            "request blocked by method policy (client={client}, host={host}, method={method}, mode=limited, allowed_methods=GET, HEAD, OPTIONS)"
        );
        return Ok(json_blocked(&host, REASON_METHOD_NOT_ALLOWED));
    }

    let client = client.as_deref().unwrap_or_default();
    let method = req.method();
    info!("request allowed (client={client}, host={host}, method={method})");

    let allow_upstream_proxy = match app_state
        .allow_upstream_proxy()
        .await
        .map_err(|err| internal_error("failed to read upstream proxy config", err))
    {
        Ok(allow) => allow,
        Err(resp) => return Ok(resp),
    };
    let client = if allow_upstream_proxy {
        UpstreamClient::from_env_proxy()
    } else {
        UpstreamClient::direct()
    };

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
        let client = UpstreamClient::unix_socket(socket_path);

        let (mut parts, body) = req.into_parts();
        let path = parts
            .uri
            .path_and_query()
            .map(rama_http::uri::PathAndQuery::as_str)
            .unwrap_or("/");
        parts.uri = path
            .parse()
            .with_context(|| format!("invalid unix socket request path: {path}"))?;
        parts.headers.remove("x-unix-socket");

        let req = Request::from_parts(parts, body);
        client.serve(req).await.map_err(anyhow::Error::from)
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
    let response = BlockedResponse {
        status: "blocked",
        host,
        reason,
    };
    let mut resp = json_response(&response);
    *resp.status_mut() = StatusCode::FORBIDDEN;
    resp.headers_mut().insert(
        "x-proxy-error",
        HeaderValue::from_static(blocked_header_value(reason)),
    );
    resp
}

fn blocked_text(reason: &str) -> Response {
    crate::responses::blocked_text_response(reason)
}

async fn proxy_disabled_response(
    app_state: &NetworkProxyState,
    host: String,
    client: Option<String>,
    method: Option<String>,
    protocol: &str,
) -> Response {
    let _ = app_state
        .record_blocked(BlockedRequest::new(BlockedRequestArgs {
            host,
            reason: REASON_PROXY_DISABLED.to_string(),
            client,
            method,
            mode: None,
            protocol: protocol.to_string(),
        }))
        .await;
    text_response(StatusCode::SERVICE_UNAVAILABLE, "proxy disabled")
}

fn internal_error(context: &str, err: impl std::fmt::Display) -> Response {
    error!("{context}: {err}");
    text_response(StatusCode::INTERNAL_SERVER_ERROR, "error")
}

fn text_response(status: StatusCode, body: &str) -> Response {
    Response::builder()
        .status(status)
        .header("content-type", "text/plain")
        .body(Body::from(body.to_string()))
        .unwrap_or_else(|_| Response::new(Body::from(body.to_string())))
}

#[derive(Serialize)]
struct BlockedResponse<'a> {
    status: &'static str,
    host: &'a str,
    reason: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::NetworkMode;
    use crate::config::NetworkPolicy;
    use crate::runtime::network_proxy_state_for_policy;
    use pretty_assertions::assert_eq;
    use rama_http::Method;
    use rama_http::Request;
    use std::sync::Arc;

    #[tokio::test]
    async fn http_connect_accept_blocks_in_limited_mode() {
        let policy = NetworkPolicy {
            allowed_domains: vec!["example.com".to_string()],
            ..Default::default()
        };
        let state = Arc::new(network_proxy_state_for_policy(policy));
        state.set_network_mode(NetworkMode::Limited).await.unwrap();

        let mut req = Request::builder()
            .method(Method::CONNECT)
            .uri("https://example.com:443")
            .header("host", "example.com:443")
            .body(Body::empty())
            .unwrap();
        req.extensions_mut().insert(state);

        let response = http_connect_accept(None, req).await.unwrap_err();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response.headers().get("x-proxy-error").unwrap(),
            "blocked-by-method-policy"
        );
    }
}
