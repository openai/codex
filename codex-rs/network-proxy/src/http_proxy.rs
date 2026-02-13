use crate::attempt_metadata::attempt_id_from_proxy_authorization;
use crate::config::NetworkMode;
use crate::network_policy::NetworkDecision;
use crate::network_policy::NetworkDecisionSource;
use crate::network_policy::NetworkPolicyDecider;
use crate::network_policy::NetworkPolicyDecision;
use crate::network_policy::NetworkPolicyRequest;
use crate::network_policy::NetworkPolicyRequestArgs;
use crate::network_policy::NetworkProtocol;
use crate::network_policy::evaluate_host_policy;
use crate::policy::normalize_host;
use crate::reasons::REASON_METHOD_NOT_ALLOWED;
use crate::reasons::REASON_NOT_ALLOWED;
use crate::reasons::REASON_PROXY_DISABLED;
use crate::responses::PolicyDecisionDetails;
use crate::responses::blocked_header_value;
use crate::responses::blocked_message_with_policy;
use crate::responses::blocked_text_response_with_policy;
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
use rama_http::HeaderMap;
use rama_http::HeaderName;
use rama_http::HeaderValue;
use rama_http::Request;
use rama_http::Response;
use rama_http::StatusCode;
use rama_http::header;
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
use rama_tls_rustls::client::TlsConnectorDataBuilder;
use rama_tls_rustls::client::TlsConnectorLayer;
use serde::Serialize;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::net::TcpListener as StdTcpListener;
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

    run_http_proxy_with_listener(state, listener, policy_decider).await
}

pub async fn run_http_proxy_with_std_listener(
    state: Arc<NetworkProxyState>,
    listener: StdTcpListener,
    policy_decider: Option<Arc<dyn NetworkPolicyDecider>>,
) -> Result<()> {
    let listener =
        TcpListener::try_from(listener).context("convert std listener to HTTP proxy listener")?;
    run_http_proxy_with_listener(state, listener, policy_decider).await
}

async fn run_http_proxy_with_listener(
    state: Arc<NetworkProxyState>,
    listener: TcpListener,
    policy_decider: Option<Arc<dyn NetworkPolicyDecider>>,
) -> Result<()> {
    let addr = listener
        .local_addr()
        .context("read HTTP proxy listener local addr")?;

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
    let network_attempt_id = request_network_attempt_id(&req);

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
            authority.port,
            client_addr(&req),
            Some("CONNECT".to_string()),
            NetworkProtocol::HttpsConnect,
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
        attempt_id: network_attempt_id.clone(),
    });

    match evaluate_host_policy(&app_state, policy_decider.as_ref(), &request).await {
        Ok(NetworkDecision::Deny {
            reason,
            source,
            decision,
        }) => {
            let details = PolicyDecisionDetails {
                decision,
                reason: &reason,
                source,
                protocol: NetworkProtocol::HttpsConnect,
                host: &host,
                port: authority.port,
            };
            let _ = app_state
                .record_blocked(BlockedRequest::new(BlockedRequestArgs {
                    host: host.clone(),
                    reason: reason.clone(),
                    client: client.clone(),
                    method: Some("CONNECT".to_string()),
                    mode: None,
                    protocol: "http-connect".to_string(),
                    attempt_id: network_attempt_id.clone(),
                    decision: Some(details.decision.as_str().to_string()),
                    source: Some(details.source.as_str().to_string()),
                    port: Some(authority.port),
                }))
                .await;
            let client = client.as_deref().unwrap_or_default();
            warn!("CONNECT blocked (client={client}, host={host}, reason={reason})");
            return Err(blocked_text_with_details(&reason, &details));
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
        let details = PolicyDecisionDetails {
            decision: NetworkPolicyDecision::Deny,
            reason: REASON_METHOD_NOT_ALLOWED,
            source: NetworkDecisionSource::ModeGuard,
            protocol: NetworkProtocol::HttpsConnect,
            host: &host,
            port: authority.port,
        };
        let _ = app_state
            .record_blocked(BlockedRequest::new(BlockedRequestArgs {
                host: host.clone(),
                reason: REASON_METHOD_NOT_ALLOWED.to_string(),
                client: client.clone(),
                method: Some("CONNECT".to_string()),
                mode: Some(NetworkMode::Limited),
                protocol: "http-connect".to_string(),
                attempt_id: network_attempt_id,
                decision: Some(details.decision.as_str().to_string()),
                source: Some(details.source.as_str().to_string()),
                port: Some(authority.port),
            }))
            .await;
        let client = client.as_deref().unwrap_or_default();
        warn!("CONNECT blocked by method policy (client={client}, host={host}, mode=limited)");
        return Err(blocked_text_with_details(
            REASON_METHOD_NOT_ALLOWED,
            &details,
        ));
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
    let tls_config = TlsConnectorDataBuilder::new()
        .with_alpn_protocols_http_auto()
        .build();
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
    mut req: Request,
) -> Result<Response, Infallible> {
    let app_state = match req.extensions().get::<Arc<NetworkProxyState>>().cloned() {
        Some(state) => state,
        None => {
            error!("missing app state");
            return Ok(text_response(StatusCode::INTERNAL_SERVER_ERROR, "error"));
        }
    };
    let client = client_addr(&req);
    let network_attempt_id = request_network_attempt_id(&req);

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
                0,
                client_addr(&req),
                Some(req.method().as_str().to_string()),
                NetworkProtocol::Http,
            )
            .await);
        }
        if !method_allowed {
            let client = client.as_deref().unwrap_or_default();
            let method = req.method();
            warn!(
                "unix socket blocked by method policy (client={client}, method={method}, mode=limited, allowed_methods=GET, HEAD, OPTIONS)"
            );
            return Ok(json_blocked("unix-socket", REASON_METHOD_NOT_ALLOWED, None));
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
                Ok(json_blocked("unix-socket", REASON_NOT_ALLOWED, None))
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
            port,
            client_addr(&req),
            Some(req.method().as_str().to_string()),
            NetworkProtocol::Http,
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
        attempt_id: network_attempt_id.clone(),
    });

    match evaluate_host_policy(&app_state, policy_decider.as_ref(), &request).await {
        Ok(NetworkDecision::Deny {
            reason,
            source,
            decision,
        }) => {
            let details = PolicyDecisionDetails {
                decision,
                reason: &reason,
                source,
                protocol: NetworkProtocol::Http,
                host: &host,
                port,
            };
            let _ = app_state
                .record_blocked(BlockedRequest::new(BlockedRequestArgs {
                    host: host.clone(),
                    reason: reason.clone(),
                    client: client.clone(),
                    method: Some(req.method().as_str().to_string()),
                    mode: None,
                    protocol: "http".to_string(),
                    attempt_id: network_attempt_id.clone(),
                    decision: Some(details.decision.as_str().to_string()),
                    source: Some(details.source.as_str().to_string()),
                    port: Some(port),
                }))
                .await;
            let client = client.as_deref().unwrap_or_default();
            warn!("request blocked (client={client}, host={host}, reason={reason})");
            return Ok(json_blocked(&host, &reason, Some(&details)));
        }
        Ok(NetworkDecision::Allow) => {}
        Err(err) => {
            error!("failed to evaluate host for {host}: {err}");
            return Ok(text_response(StatusCode::INTERNAL_SERVER_ERROR, "error"));
        }
    }

    if !method_allowed {
        let details = PolicyDecisionDetails {
            decision: NetworkPolicyDecision::Deny,
            reason: REASON_METHOD_NOT_ALLOWED,
            source: NetworkDecisionSource::ModeGuard,
            protocol: NetworkProtocol::Http,
            host: &host,
            port,
        };
        let _ = app_state
            .record_blocked(BlockedRequest::new(BlockedRequestArgs {
                host: host.clone(),
                reason: REASON_METHOD_NOT_ALLOWED.to_string(),
                client: client.clone(),
                method: Some(req.method().as_str().to_string()),
                mode: Some(NetworkMode::Limited),
                protocol: "http".to_string(),
                attempt_id: network_attempt_id,
                decision: Some(details.decision.as_str().to_string()),
                source: Some(details.source.as_str().to_string()),
                port: Some(port),
            }))
            .await;
        let client = client.as_deref().unwrap_or_default();
        let method = req.method();
        warn!(
            "request blocked by method policy (client={client}, host={host}, method={method}, mode=limited, allowed_methods=GET, HEAD, OPTIONS)"
        );
        return Ok(json_blocked(
            &host,
            REASON_METHOD_NOT_ALLOWED,
            Some(&details),
        ));
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

    // Strip hop-by-hop headers only after extracting metadata used for policy correlation.
    remove_hop_by_hop_request_headers(req.headers_mut());
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
        remove_hop_by_hop_request_headers(&mut parts.headers);

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

fn request_network_attempt_id(req: &Request) -> Option<String> {
    // Some HTTP stacks normalize proxy credentials into `authorization`; accept both.
    attempt_id_from_proxy_authorization(req.headers().get("proxy-authorization"))
        .or_else(|| attempt_id_from_proxy_authorization(req.headers().get("authorization")))
}

fn remove_hop_by_hop_request_headers(headers: &mut HeaderMap) {
    while let Some(raw_connection) = headers.get(header::CONNECTION).cloned() {
        headers.remove(header::CONNECTION);
        if let Ok(raw_connection) = raw_connection.to_str() {
            let connection_headers: Vec<String> = raw_connection
                .split(',')
                .map(str::trim)
                .filter(|token| !token.is_empty())
                .map(ToOwned::to_owned)
                .collect();
            for token in connection_headers {
                if let Ok(name) = HeaderName::from_bytes(token.as_bytes()) {
                    headers.remove(name);
                }
            }
        }
    }
    for name in [
        &header::KEEP_ALIVE,
        &header::PROXY_CONNECTION,
        &header::PROXY_AUTHORIZATION,
        &header::TE,
        &header::TRAILER,
        &header::TRANSFER_ENCODING,
        &header::UPGRADE,
    ] {
        headers.remove(name);
    }
}

fn json_blocked(host: &str, reason: &str, details: Option<&PolicyDecisionDetails<'_>>) -> Response {
    let (message, decision, source, protocol, port) = details
        .map(|details| {
            (
                Some(blocked_message_with_policy(reason, details)),
                Some(details.decision.as_str()),
                Some(details.source.as_str()),
                Some(details.protocol.as_policy_protocol()),
                Some(details.port),
            )
        })
        .unwrap_or((None, None, None, None, None));
    let response = BlockedResponse {
        status: "blocked",
        host,
        reason,
        decision,
        source,
        protocol,
        port,
        message,
    };
    let mut resp = json_response(&response);
    *resp.status_mut() = StatusCode::FORBIDDEN;
    resp.headers_mut().insert(
        "x-proxy-error",
        HeaderValue::from_static(blocked_header_value(reason)),
    );
    resp
}

fn blocked_text_with_details(reason: &str, details: &PolicyDecisionDetails<'_>) -> Response {
    blocked_text_response_with_policy(reason, details)
}

async fn proxy_disabled_response(
    app_state: &NetworkProxyState,
    host: String,
    port: u16,
    client: Option<String>,
    method: Option<String>,
    protocol: NetworkProtocol,
) -> Response {
    let blocked_host = host.clone();
    let _ = app_state
        .record_blocked(BlockedRequest::new(BlockedRequestArgs {
            host: blocked_host,
            reason: REASON_PROXY_DISABLED.to_string(),
            client,
            method,
            mode: None,
            protocol: protocol.as_policy_protocol().to_string(),
            attempt_id: None,
            decision: Some("deny".to_string()),
            source: Some("proxy_state".to_string()),
            port: Some(port),
        }))
        .await;

    let details = PolicyDecisionDetails {
        decision: NetworkPolicyDecision::Deny,
        reason: REASON_PROXY_DISABLED,
        source: NetworkDecisionSource::ProxyState,
        protocol,
        host: &host,
        port,
    };
    text_response(
        StatusCode::SERVICE_UNAVAILABLE,
        &blocked_message_with_policy(REASON_PROXY_DISABLED, &details),
    )
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
    #[serde(skip_serializing_if = "Option::is_none")]
    decision: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    protocol: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::NetworkMode;
    use crate::config::NetworkProxySettings;
    use crate::runtime::network_proxy_state_for_policy;
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use pretty_assertions::assert_eq;
    use rama_http::Method;
    use rama_http::Request;
    use std::sync::Arc;

    #[tokio::test]
    async fn http_connect_accept_blocks_in_limited_mode() {
        let policy = NetworkProxySettings {
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

    #[test]
    fn request_network_attempt_id_reads_proxy_authorization_header() {
        let encoded = STANDARD.encode("codex-net-attempt-attempt-1:");
        let req = Request::builder()
            .method(Method::GET)
            .uri("http://example.com")
            .header("proxy-authorization", format!("Basic {encoded}"))
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            request_network_attempt_id(&req),
            Some("attempt-1".to_string())
        );
    }

    #[test]
    fn request_network_attempt_id_reads_authorization_header_fallback() {
        let encoded = STANDARD.encode("codex-net-attempt-attempt-2:");
        let req = Request::builder()
            .method(Method::GET)
            .uri("http://example.com")
            .header("authorization", format!("Basic {encoded}"))
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            request_network_attempt_id(&req),
            Some("attempt-2".to_string())
        );
    }

    #[test]
    fn remove_hop_by_hop_request_headers_keeps_forwarding_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONNECTION,
            HeaderValue::from_static("x-hop, keep-alive"),
        );
        headers.insert("x-hop", HeaderValue::from_static("1"));
        headers.insert(
            header::PROXY_AUTHORIZATION,
            HeaderValue::from_static("Basic abc"),
        );
        headers.insert(
            &header::X_FORWARDED_FOR,
            HeaderValue::from_static("127.0.0.1"),
        );
        headers.insert(header::HOST, HeaderValue::from_static("example.com"));

        remove_hop_by_hop_request_headers(&mut headers);

        assert_eq!(headers.get(header::CONNECTION), None);
        assert_eq!(headers.get("x-hop"), None);
        assert_eq!(headers.get(header::PROXY_AUTHORIZATION), None);
        assert_eq!(
            headers.get(&header::X_FORWARDED_FOR),
            Some(&HeaderValue::from_static("127.0.0.1"))
        );
        assert_eq!(
            headers.get(header::HOST),
            Some(&HeaderValue::from_static("example.com"))
        );
    }
}
