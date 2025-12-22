use crate::config::NetworkMode;
use crate::policy::normalize_host;
use crate::state::AppState;
use crate::state::BlockedRequest;
use anyhow::Result;
use anyhow::anyhow;
use rama::Context as RamaContext;
use rama::Service;
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
    let listener = TcpListener::build_with_state(state.clone())
        .bind(addr)
        .await
        .map_err(|err| anyhow!("bind SOCKS5 proxy: {err}"))?;

    info!(addr = %addr, "SOCKS5 proxy listening");

    match state.network_mode().await {
        Ok(NetworkMode::Limited) => {
            info!(
                mode = "limited",
                "SOCKS5 is blocked in limited mode; set mode=\"full\" to allow SOCKS5"
            );
        }
        Ok(NetworkMode::Full) => {}
        Err(err) => {
            warn!(error = %err, "failed to read network mode");
        }
    }

    let tcp_connector = TcpConnector::default();
    let policy_tcp_connector =
        service_fn(move |ctx: RamaContext<Arc<AppState>>, req: TcpRequest| {
            let tcp_connector = tcp_connector.clone();
            async move {
                let app_state = ctx.state().clone();
                let authority = req.authority().clone();
                let host = normalize_host(&authority.host().to_string());
                let port = authority.port();
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
                        warn!(
                            client = %client.as_deref().unwrap_or_default(),
                            host = %host,
                            mode = "limited",
                            allowed_methods = "GET, HEAD, OPTIONS",
                            "SOCKS blocked by method policy"
                        );
                        return Err(
                            io::Error::new(io::ErrorKind::PermissionDenied, "blocked").into()
                        );
                    }
                    Ok(NetworkMode::Full) => {}
                    Err(err) => {
                        error!(error = %err, "failed to evaluate method policy");
                        return Err(io::Error::new(io::ErrorKind::Other, "proxy error").into());
                    }
                }

                match app_state.host_blocked(&host).await {
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
                        warn!(
                            client = %client.as_deref().unwrap_or_default(),
                            host = %host,
                            reason = %reason,
                            "SOCKS blocked"
                        );
                        return Err(
                            io::Error::new(io::ErrorKind::PermissionDenied, "blocked").into()
                        );
                    }
                    Ok((false, _)) => {
                        info!(
                            client = %client.as_deref().unwrap_or_default(),
                            host = %host,
                            port = port,
                            "SOCKS allowed"
                        );
                    }
                    Err(err) => {
                        error!(error = %err, "failed to evaluate host");
                        return Err(io::Error::new(io::ErrorKind::Other, "proxy error").into());
                    }
                }

                tcp_connector.serve(ctx, req).await
            }
        });

    let socks_connector = DefaultConnector::default().with_connector(policy_tcp_connector);
    let socks_acceptor = Socks5Acceptor::new().with_connector(socks_connector);

    listener.serve(socks_acceptor).await;
    Ok(())
}
