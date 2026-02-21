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
use crate::reasons::REASON_PROXY_DISABLED;
use crate::responses::PolicyDecisionDetails;
use crate::responses::blocked_message_with_policy;
use crate::state::BlockedRequest;
use crate::state::BlockedRequestArgs;
use crate::state::NetworkProxyState;
use anyhow::Context as _;
use anyhow::Result;
use rama_core::Layer;
use rama_core::Service;
use rama_core::error::BoxError;
use rama_core::extensions::ExtensionsRef;
use rama_core::layer::AddInputExtensionLayer;
use rama_core::service::service_fn;
use rama_net::client::EstablishedClientConnection;
use rama_net::stream::SocketInfo;
use rama_socks5::Socks5Acceptor;
use rama_socks5::server::DefaultConnector;
use rama_socks5::server::DefaultUdpRelay;
use rama_socks5::server::udp::RelayRequest;
use rama_socks5::server::udp::RelayResponse;
use rama_tcp::TcpStream;
use rama_tcp::client::Request as TcpRequest;
use rama_tcp::client::service::TcpConnector;
use rama_tcp::server::TcpListener;
use std::io;
use std::net::SocketAddr;
use std::net::TcpListener as StdTcpListener;
use std::sync::Arc;
use tracing::error;
use tracing::info;
use tracing::warn;

pub async fn run_socks5(
    state: Arc<NetworkProxyState>,
    addr: SocketAddr,
    policy_decider: Option<Arc<dyn NetworkPolicyDecider>>,
    enable_socks5_udp: bool,
) -> Result<()> {
    let listener = TcpListener::build()
        .bind(addr)
        .await
        // See `http_proxy.rs` for details on why we wrap `BoxError` before converting to anyhow.
        .map_err(rama_core::error::OpaqueError::from)
        .map_err(anyhow::Error::from)
        .with_context(|| format!("bind SOCKS5 proxy: {addr}"))?;

    run_socks5_with_listener(state, listener, policy_decider, enable_socks5_udp).await
}

pub async fn run_socks5_with_std_listener(
    state: Arc<NetworkProxyState>,
    listener: StdTcpListener,
    policy_decider: Option<Arc<dyn NetworkPolicyDecider>>,
    enable_socks5_udp: bool,
) -> Result<()> {
    let listener =
        TcpListener::try_from(listener).context("convert std listener to SOCKS5 proxy listener")?;
    run_socks5_with_listener(state, listener, policy_decider, enable_socks5_udp).await
}

async fn run_socks5_with_listener(
    state: Arc<NetworkProxyState>,
    listener: TcpListener,
    policy_decider: Option<Arc<dyn NetworkPolicyDecider>>,
    enable_socks5_udp: bool,
) -> Result<()> {
    let addr = listener
        .local_addr()
        .context("read SOCKS5 listener local addr")?;

    info!("SOCKS5 proxy listening on {addr}");

    match state.network_mode().await {
        Ok(NetworkMode::Limited) => {
            info!("SOCKS5 is blocked in limited mode; set mode=\"full\" to allow SOCKS5");
        }
        Ok(NetworkMode::Full) => {}
        Err(err) => {
            warn!("failed to read network mode: {err}");
        }
    }

    let tcp_connector = TcpConnector::default();
    let policy_tcp_connector = service_fn({
        let policy_decider = policy_decider.clone();
        move |req: TcpRequest| {
            let tcp_connector = tcp_connector.clone();
            let policy_decider = policy_decider.clone();
            async move { handle_socks5_tcp(req, tcp_connector, policy_decider).await }
        }
    });

    let socks_connector = DefaultConnector::default().with_connector(policy_tcp_connector);
    let base = Socks5Acceptor::new().with_connector(socks_connector);

    if enable_socks5_udp {
        let udp_state = state.clone();
        let udp_decider = policy_decider.clone();
        let udp_relay = DefaultUdpRelay::default().with_async_inspector(service_fn({
            move |request: RelayRequest| {
                let udp_state = udp_state.clone();
                let udp_decider = udp_decider.clone();
                async move { inspect_socks5_udp(request, udp_state, udp_decider).await }
            }
        }));
        let socks_acceptor = base.with_udp_associator(udp_relay);
        listener
            .serve(AddInputExtensionLayer::new(state).into_layer(socks_acceptor))
            .await;
    } else {
        listener
            .serve(AddInputExtensionLayer::new(state).into_layer(base))
            .await;
    }
    Ok(())
}

async fn handle_socks5_tcp(
    req: TcpRequest,
    tcp_connector: TcpConnector,
    policy_decider: Option<Arc<dyn NetworkPolicyDecider>>,
) -> Result<EstablishedClientConnection<TcpStream, TcpRequest>, BoxError> {
    let app_state = req
        .extensions()
        .get::<Arc<NetworkProxyState>>()
        .cloned()
        .ok_or_else(|| io::Error::other("missing state"))?;

    let host = normalize_host(&req.authority.host.to_string());
    let port = req.authority.port;
    if host.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid host").into());
    }

    let client = req
        .extensions()
        .get::<SocketInfo>()
        .map(|info| info.peer_addr().to_string());

    match app_state.enabled().await {
        Ok(true) => {}
        Ok(false) => {
            let details = PolicyDecisionDetails {
                decision: NetworkPolicyDecision::Deny,
                reason: REASON_PROXY_DISABLED,
                source: NetworkDecisionSource::ProxyState,
                protocol: NetworkProtocol::Socks5Tcp,
                host: &host,
                port,
            };
            let _ = app_state
                .record_blocked(BlockedRequest::new(BlockedRequestArgs {
                    host: host.clone(),
                    reason: REASON_PROXY_DISABLED.to_string(),
                    client: client.clone(),
                    method: None,
                    mode: None,
                    protocol: "socks5".to_string(),
                    decision: Some(details.decision.as_str().to_string()),
                    source: Some(details.source.as_str().to_string()),
                    port: Some(port),
                }))
                .await;
            let client = client.as_deref().unwrap_or_default();
            warn!("SOCKS blocked; proxy disabled (client={client}, host={host})");
            return Err(policy_denied_error(REASON_PROXY_DISABLED, &details).into());
        }
        Err(err) => {
            error!("failed to read enabled state: {err}");
            return Err(io::Error::other("proxy error").into());
        }
    }

    match app_state.network_mode().await {
        Ok(NetworkMode::Limited) => {
            let details = PolicyDecisionDetails {
                decision: NetworkPolicyDecision::Deny,
                reason: REASON_METHOD_NOT_ALLOWED,
                source: NetworkDecisionSource::ModeGuard,
                protocol: NetworkProtocol::Socks5Tcp,
                host: &host,
                port,
            };
            let _ = app_state
                .record_blocked(BlockedRequest::new(BlockedRequestArgs {
                    host: host.clone(),
                    reason: REASON_METHOD_NOT_ALLOWED.to_string(),
                    client: client.clone(),
                    method: None,
                    mode: Some(NetworkMode::Limited),
                    protocol: "socks5".to_string(),
                    decision: Some(details.decision.as_str().to_string()),
                    source: Some(details.source.as_str().to_string()),
                    port: Some(port),
                }))
                .await;
            let client = client.as_deref().unwrap_or_default();
            warn!(
                "SOCKS blocked by method policy (client={client}, host={host}, mode=limited, allowed_methods=GET, HEAD, OPTIONS)"
            );
            return Err(policy_denied_error(REASON_METHOD_NOT_ALLOWED, &details).into());
        }
        Ok(NetworkMode::Full) => {}
        Err(err) => {
            error!("failed to evaluate method policy: {err}");
            return Err(io::Error::other("proxy error").into());
        }
    }

    let request = NetworkPolicyRequest::new(NetworkPolicyRequestArgs {
        protocol: NetworkProtocol::Socks5Tcp,
        host: host.clone(),
        port,
        client_addr: client.clone(),
        method: None,
        command: None,
        exec_policy_hint: None,
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
                protocol: NetworkProtocol::Socks5Tcp,
                host: &host,
                port,
            };
            let _ = app_state
                .record_blocked(BlockedRequest::new(BlockedRequestArgs {
                    host: host.clone(),
                    reason: reason.clone(),
                    client: client.clone(),
                    method: None,
                    mode: None,
                    protocol: "socks5".to_string(),
                    decision: Some(details.decision.as_str().to_string()),
                    source: Some(details.source.as_str().to_string()),
                    port: Some(port),
                }))
                .await;
            let client = client.as_deref().unwrap_or_default();
            warn!("SOCKS blocked (client={client}, host={host}, reason={reason})");
            return Err(policy_denied_error(&reason, &details).into());
        }
        Ok(NetworkDecision::Allow) => {
            let client = client.as_deref().unwrap_or_default();
            info!("SOCKS allowed (client={client}, host={host}, port={port})");
        }
        Err(err) => {
            error!("failed to evaluate host: {err}");
            return Err(io::Error::other("proxy error").into());
        }
    }

    tcp_connector.serve(req).await
}

async fn inspect_socks5_udp(
    request: RelayRequest,
    state: Arc<NetworkProxyState>,
    policy_decider: Option<Arc<dyn NetworkPolicyDecider>>,
) -> io::Result<RelayResponse> {
    let RelayRequest {
        server_address,
        payload,
        extensions,
        ..
    } = request;

    let host = normalize_host(&server_address.ip_addr.to_string());
    let port = server_address.port;
    if host.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid host"));
    }

    let client = extensions
        .get::<SocketInfo>()
        .map(|info| info.peer_addr().to_string());

    match state.enabled().await {
        Ok(true) => {}
        Ok(false) => {
            let details = PolicyDecisionDetails {
                decision: NetworkPolicyDecision::Deny,
                reason: REASON_PROXY_DISABLED,
                source: NetworkDecisionSource::ProxyState,
                protocol: NetworkProtocol::Socks5Udp,
                host: &host,
                port,
            };
            let _ = state
                .record_blocked(BlockedRequest::new(BlockedRequestArgs {
                    host: host.clone(),
                    reason: REASON_PROXY_DISABLED.to_string(),
                    client: client.clone(),
                    method: None,
                    mode: None,
                    protocol: "socks5-udp".to_string(),
                    decision: Some(details.decision.as_str().to_string()),
                    source: Some(details.source.as_str().to_string()),
                    port: Some(port),
                }))
                .await;
            let client = client.as_deref().unwrap_or_default();
            warn!("SOCKS UDP blocked; proxy disabled (client={client}, host={host})");
            return Err(policy_denied_error(REASON_PROXY_DISABLED, &details));
        }
        Err(err) => {
            error!("failed to read enabled state: {err}");
            return Err(io::Error::other("proxy error"));
        }
    }

    match state.network_mode().await {
        Ok(NetworkMode::Limited) => {
            let details = PolicyDecisionDetails {
                decision: NetworkPolicyDecision::Deny,
                reason: REASON_METHOD_NOT_ALLOWED,
                source: NetworkDecisionSource::ModeGuard,
                protocol: NetworkProtocol::Socks5Udp,
                host: &host,
                port,
            };
            let _ = state
                .record_blocked(BlockedRequest::new(BlockedRequestArgs {
                    host: host.clone(),
                    reason: REASON_METHOD_NOT_ALLOWED.to_string(),
                    client: client.clone(),
                    method: None,
                    mode: Some(NetworkMode::Limited),
                    protocol: "socks5-udp".to_string(),
                    decision: Some(details.decision.as_str().to_string()),
                    source: Some(details.source.as_str().to_string()),
                    port: Some(port),
                }))
                .await;
            return Err(policy_denied_error(REASON_METHOD_NOT_ALLOWED, &details));
        }
        Ok(NetworkMode::Full) => {}
        Err(err) => {
            error!("failed to evaluate method policy: {err}");
            return Err(io::Error::other("proxy error"));
        }
    }

    let request = NetworkPolicyRequest::new(NetworkPolicyRequestArgs {
        protocol: NetworkProtocol::Socks5Udp,
        host: host.clone(),
        port,
        client_addr: client.clone(),
        method: None,
        command: None,
        exec_policy_hint: None,
    });

    match evaluate_host_policy(&state, policy_decider.as_ref(), &request).await {
        Ok(NetworkDecision::Deny {
            reason,
            source,
            decision,
        }) => {
            let details = PolicyDecisionDetails {
                decision,
                reason: &reason,
                source,
                protocol: NetworkProtocol::Socks5Udp,
                host: &host,
                port,
            };
            let _ = state
                .record_blocked(BlockedRequest::new(BlockedRequestArgs {
                    host: host.clone(),
                    reason: reason.clone(),
                    client: client.clone(),
                    method: None,
                    mode: None,
                    protocol: "socks5-udp".to_string(),
                    decision: Some(details.decision.as_str().to_string()),
                    source: Some(details.source.as_str().to_string()),
                    port: Some(port),
                }))
                .await;
            let client = client.as_deref().unwrap_or_default();
            warn!("SOCKS UDP blocked (client={client}, host={host}, reason={reason})");
            Err(policy_denied_error(&reason, &details))
        }
        Ok(NetworkDecision::Allow) => Ok(RelayResponse {
            maybe_payload: Some(payload),
            extensions,
        }),
        Err(err) => {
            error!("failed to evaluate UDP host: {err}");
            Err(io::Error::other("proxy error"))
        }
    }
}

fn policy_denied_error(reason: &str, details: &PolicyDecisionDetails<'_>) -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        blocked_message_with_policy(reason, details),
    )
}
