use crate::config::NetworkMode;
use crate::policy::normalize_host;
use crate::state::AppState;
use crate::state::BlockedRequest;
use anyhow::Context as _;
use anyhow::Result;
use rama::Context;
use rama::Layer;
use rama::Service;
use rama::layer::AddExtensionLayer;
use rama::net::stream::SocketInfo;
use rama::proxy::socks5::Socks5Acceptor;
use rama::proxy::socks5::server::DefaultConnector;
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

pub async fn run_socks5(state: Arc<AppState>, addr: SocketAddr) -> Result<()> {
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
    let policy_tcp_connector = service_fn(move |ctx: Context<()>, req: TcpRequest| {
        let tcp_connector = tcp_connector.clone();
        async move {
            let app_state = ctx
                .get::<Arc<AppState>>()
                .cloned()
                .ok_or_else(|| io::Error::other("missing state"))?;

            let host = normalize_host(&req.authority().host().to_string());
            let port = req.authority().port();
            let client = ctx
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
                    return Err(io::Error::new(io::ErrorKind::PermissionDenied, "blocked").into());
                }
                Ok(NetworkMode::Full) => {}
                Err(err) => {
                    error!("failed to evaluate method policy: {err}");
                    return Err(io::Error::other("proxy error").into());
                }
            }

            match app_state.host_blocked(&host, port).await {
                Ok((true, reason)) => {
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
                    return Err(io::Error::new(io::ErrorKind::PermissionDenied, "blocked").into());
                }
                Ok((false, _)) => {
                    let client = client.as_deref().unwrap_or_default();
                    info!("SOCKS allowed (client={client}, host={host}, port={port})");
                }
                Err(err) => {
                    error!("failed to evaluate host: {err}");
                    return Err(io::Error::other("proxy error").into());
                }
            }

            tcp_connector.serve(ctx, req).await
        }
    });

    let socks_connector = DefaultConnector::default().with_connector(policy_tcp_connector);
    let socks_acceptor = Socks5Acceptor::new().with_connector(socks_connector);

    listener
        .serve(AddExtensionLayer::new(state).into_layer(socks_acceptor))
        .await;
    Ok(())
}
