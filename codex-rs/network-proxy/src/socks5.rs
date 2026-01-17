use crate::config::NetworkMode;
use crate::network_policy::NetworkDecision;
use crate::network_policy::NetworkPolicyDecider;
use crate::network_policy::NetworkPolicyRequest;
use crate::network_policy::NetworkProtocol;
use crate::network_policy::evaluate_host_policy;
use crate::policy::normalize_host;
use crate::state::AppState;
use crate::state::BlockedRequest;
use anyhow::Context as _;
use anyhow::Result;
use rama::Layer;
use rama::Service;
use rama::extensions::ExtensionsRef;
use rama::layer::AddInputExtensionLayer;
use rama::net::stream::SocketInfo;
use rama::proxy::socks5::Socks5Acceptor;
use rama::proxy::socks5::server::DefaultConnector;
use rama::proxy::socks5::server::DefaultUdpRelay;
use rama::proxy::socks5::server::udp::RelayRequest;
use rama::proxy::socks5::server::udp::RelayResponse;
use rama::service::service_fn;
use rama::tcp::client::Request as TcpRequest;
use rama::tcp::client::service::TcpConnector;
use rama::tcp::server::TcpListener;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::error;
use tracing::info;
use tracing::warn;

pub async fn run_socks5(
    state: Arc<AppState>,
    addr: SocketAddr,
    policy_decider: Option<Arc<dyn NetworkPolicyDecider>>,
    enable_socks5_udp: bool,
) -> Result<()> {
    let listener = TcpListener::build()
        .bind(addr)
        .await
        // See `http_proxy.rs` for details on why we wrap `BoxError` before converting to anyhow.
        .map_err(rama::error::OpaqueError::from)
        .map_err(anyhow::Error::from)
        .with_context(|| format!("bind SOCKS5 proxy: {addr}"))?;

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
            async move {
                let app_state = req
                    .extensions()
                    .get::<Arc<AppState>>()
                    .cloned()
                    .ok_or_else(|| io::Error::other("missing state"))?;

                let host = normalize_host(&req.authority.host.to_string());
                let port = req.authority.port;
                let client = req
                    .extensions()
                    .get::<SocketInfo>()
                    .map(|info| info.peer_addr().to_string());
                match app_state.network_mode().await {
                    Ok(NetworkMode::Limited) => {
                        let _ = app_state
                            .record_blocked(BlockedRequest::new(
                                host.clone(),
                                "method_not_allowed".to_string(),
                                client.clone(),
                                None,
                                Some(NetworkMode::Limited),
                                "socks5".to_string(),
                            ))
                            .await;
                        let client = client.as_deref().unwrap_or_default();
                        warn!(
                            "SOCKS blocked by method policy (client={client}, host={host}, mode=limited, allowed_methods=GET, HEAD, OPTIONS)"
                        );
                        return Err(
                            io::Error::new(io::ErrorKind::PermissionDenied, "blocked").into()
                        );
                    }
                    Ok(NetworkMode::Full) => {}
                    Err(err) => {
                        error!("failed to evaluate method policy: {err}");
                        return Err(io::Error::other("proxy error").into());
                    }
                }

                let request = NetworkPolicyRequest::new(
                    NetworkProtocol::Socks5Tcp,
                    host.clone(),
                    port,
                    client.clone(),
                    None,
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
                                None,
                                None,
                                "socks5".to_string(),
                            ))
                            .await;
                        let client = client.as_deref().unwrap_or_default();
                        warn!("SOCKS blocked (client={client}, host={host}, reason={reason})");
                        return Err(
                            io::Error::new(io::ErrorKind::PermissionDenied, "blocked").into()
                        );
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
        }
    });

    let socks_connector = DefaultConnector::default().with_connector(policy_tcp_connector);
    let base = Socks5Acceptor::new().with_connector(socks_connector);

    if enable_socks5_udp {
        let udp_state = state.clone();
        let udp_decider = policy_decider.clone();
        let udp_relay = DefaultUdpRelay::default().with_async_inspector(service_fn(
            move |request: RelayRequest| {
                let udp_state = udp_state.clone();
                let udp_decider = udp_decider.clone();
                async move {
                    let RelayRequest {
                        server_address,
                        payload,
                        extensions,
                        ..
                    } = request;

                    let host = normalize_host(&server_address.ip_addr.to_string());
                    let port = server_address.port;
                    let client = extensions
                        .get::<SocketInfo>()
                        .map(|info| info.peer_addr().to_string());

                    match udp_state.network_mode().await {
                        Ok(NetworkMode::Limited) => {
                            let _ = udp_state
                                .record_blocked(BlockedRequest::new(
                                    host.clone(),
                                    "method_not_allowed".to_string(),
                                    client.clone(),
                                    None,
                                    Some(NetworkMode::Limited),
                                    "socks5-udp".to_string(),
                                ))
                                .await;
                            return Ok(RelayResponse {
                                maybe_payload: None,
                                extensions,
                            });
                        }
                        Ok(NetworkMode::Full) => {}
                        Err(err) => {
                            error!("failed to evaluate method policy: {err}");
                            return Err(io::Error::other("proxy error"));
                        }
                    }

                    let request = NetworkPolicyRequest::new(
                        NetworkProtocol::Socks5Udp,
                        host.clone(),
                        port,
                        client.clone(),
                        None,
                        None,
                        None,
                    );

                    match evaluate_host_policy(&udp_state, udp_decider.as_ref(), &request).await {
                        Ok(NetworkDecision::Deny { reason }) => {
                            let _ = udp_state
                                .record_blocked(BlockedRequest::new(
                                    host.clone(),
                                    reason.clone(),
                                    client.clone(),
                                    None,
                                    None,
                                    "socks5-udp".to_string(),
                                ))
                                .await;
                            let client = client.as_deref().unwrap_or_default();
                            warn!(
                                "SOCKS UDP blocked (client={client}, host={host}, reason={reason})"
                            );
                            Ok(RelayResponse {
                                maybe_payload: None,
                                extensions,
                            })
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
            },
        ));
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
