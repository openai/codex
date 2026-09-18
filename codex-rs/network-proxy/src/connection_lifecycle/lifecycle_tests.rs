//! Exercises proxy teardown through live TCP connections, including half-closed tunnels.

use crate::NetworkProxy;
use crate::NetworkProxyConfig;
use crate::state::network_proxy_state_for_policy;
use anyhow::Context;
use anyhow::Result;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::future::Future;
use std::future::poll_fn;
use std::io::ErrorKind;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::time::timeout;

#[derive(Clone, Copy, Debug)]
enum ConnectionKind {
    HttpConnect(TunnelState),
    Socks5(TunnelState),
    HttpKeepAlive,
}

#[derive(Clone, Copy, Debug)]
enum TunnelState {
    Open,
    ClientWriteClosed,
    UpstreamWriteClosed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PeerSide {
    Client,
    Upstream,
}

struct Connection {
    client: TcpStream,
    upstream: TcpStream,
    eof_received_by: Option<PeerSide>,
}

#[derive(Clone, Copy, Debug)]
enum StopProxy {
    Shutdown,
    Drop,
    CancelWait,
}

#[tokio::test]
async fn shutdown_closes_proxy_connections() -> Result<()> {
    assert_stop_closes_connections(StopProxy::Shutdown).await
}

#[tokio::test]
async fn dropping_handle_closes_proxy_connections() -> Result<()> {
    assert_stop_closes_connections(StopProxy::Drop).await
}

#[tokio::test]
async fn canceling_wait_closes_proxy_connections() -> Result<()> {
    assert_stop_closes_connections(StopProxy::CancelWait).await
}

async fn assert_stop_closes_connections(stop: StopProxy) -> Result<()> {
    let mut config = NetworkProxyConfig {
        allow_local_binding: Some(true),
        allow_upstream_proxy: false,
        enable_socks5_udp: false,
        ..NetworkProxyConfig::default()
    };
    config.set_allowed_domains(vec![Ipv4Addr::LOCALHOST.to_string()]);
    let proxy = NetworkProxy::builder()
        .state(Arc::new(network_proxy_state_for_policy(config)))
        .managed_by_codex(!cfg!(target_os = "windows"))
        .http_addr(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .socks_addr(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .build()
        .await?;
    let handle = proxy.run().await?;
    // Environment listeners reserve ephemeral ports on every platform. Managed main
    // listeners do so off Windows, whose shared ingress has separate route tests.
    let prepared = proxy.prepare_for_remote_environment(HashMap::new(), "lifecycle-test")?;
    let scopes = [
        ("environment", prepared.env),
        #[cfg(not(target_os = "windows"))]
        (
            "main",
            proxy
                .prepare_for_optional_environment(HashMap::new(), /*environment_id*/ None)?
                .env,
        ),
    ];
    let mut connections = Vec::new();
    for (scope, env) in scopes {
        for kind in [
            ConnectionKind::HttpConnect(TunnelState::Open),
            ConnectionKind::HttpConnect(TunnelState::ClientWriteClosed),
            ConnectionKind::HttpConnect(TunnelState::UpstreamWriteClosed),
            ConnectionKind::Socks5(TunnelState::Open),
            ConnectionKind::Socks5(TunnelState::ClientWriteClosed),
            ConnectionKind::Socks5(TunnelState::UpstreamWriteClosed),
            ConnectionKind::HttpKeepAlive,
        ] {
            let (key, prefix) = match kind {
                ConnectionKind::HttpConnect(_) | ConnectionKind::HttpKeepAlive => {
                    ("HTTP_PROXY", "http://")
                }
                ConnectionKind::Socks5(_) => ("ALL_PROXY", "socks5h://"),
            };
            let addr = env[key]
                .strip_prefix(prefix)
                .context("proxy URL scheme")?
                .parse()?;
            let connection = timeout(Duration::from_secs(5), open_connection(kind, addr))
                .await
                .with_context(|| format!("{scope} {kind:?} connection did not become ready"))??;
            connections.push((scope, kind, connection));
        }
    }

    match stop {
        StopProxy::Shutdown => timeout(Duration::from_secs(2), handle.shutdown())
            .await
            .context("proxy shutdown did not finish with active connections")??,
        StopProxy::Drop => drop(handle),
        StopProxy::CancelWait => {
            let mut wait = Box::pin(handle.wait());
            let state = poll_fn(|cx| Poll::Ready(wait.as_mut().poll(cx))).await;
            assert!(
                state.is_pending(),
                "running proxy unexpectedly stopped: {state:?}"
            );
            drop(wait);
        }
    }

    // Retain every endpoint until all checks finish: dropping a test peer could
    // otherwise cause the EOF that a later assertion attributes to proxy teardown.
    for (scope, kind, connection) in &mut connections {
        for (side, stream) in [
            (PeerSide::Client, &mut connection.client),
            (PeerSide::Upstream, &mut connection.upstream),
        ] {
            if connection.eof_received_by == Some(side) {
                continue;
            }
            let mut byte = [0_u8; 1];
            let result = timeout(Duration::from_secs(2), stream.read(&mut byte))
                .await
                .with_context(|| {
                    format!("{scope} {kind:?} {side:?} connection remained open after {stop:?}")
                })?;
            match result {
                Ok(0) => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted
                    ) => {}
                result => anyhow::bail!(
                    "{scope} {kind:?} {side:?} did not close after {stop:?}: {result:?}"
                ),
            }
        }
    }
    Ok(())
}

async fn open_connection(kind: ConnectionKind, proxy_addr: SocketAddr) -> Result<Connection> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let target = listener.local_addr()?;
    let mut client = TcpStream::connect(proxy_addr).await?;
    let request = match kind {
        ConnectionKind::HttpConnect(_) => {
            format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n\r\n").into_bytes()
        }
        ConnectionKind::HttpKeepAlive => {
            format!("GET http://{target}/ HTTP/1.1\r\nHost: {target}\r\n\r\n").into_bytes()
        }
        ConnectionKind::Socks5(_) => {
            client.write_all(&[5, 1, 0]).await?;
            let mut greeting = [0_u8; 2];
            client.read_exact(&mut greeting).await?;
            assert_eq!(greeting, [5, 0]);
            let [port_high, port_low] = target.port().to_be_bytes();
            vec![5, 1, 0, 1, 127, 0, 0, 1, port_high, port_low]
        }
    };
    client.write_all(&request).await?;
    let (upstream, _) = listener.accept().await?;
    let mut connection = Connection {
        client,
        upstream,
        eof_received_by: None,
    };
    let tunnel_state = match kind {
        ConnectionKind::HttpConnect(state) => {
            let response = read_http_headers(&mut connection.client).await?;
            assert!(response.starts_with("HTTP/1.1 200 "), "{response:?}");
            state
        }
        ConnectionKind::Socks5(state) => {
            let mut response = [0_u8; 10];
            connection.client.read_exact(&mut response).await?;
            assert_eq!(&response[..4], &[5, 0, 0, 1]);
            state
        }
        ConnectionKind::HttpKeepAlive => {
            read_http_headers(&mut connection.upstream).await?;
            connection
                .upstream
                .write_all(b"HTTP/1.1 204 No Content\r\n\r\n")
                .await?;
            let response = read_http_headers(&mut connection.client).await?;
            assert!(response.starts_with("HTTP/1.1 204 "), "{response:?}");
            return Ok(connection);
        }
    };

    let (sender, receiver, receiver_side) = match tunnel_state {
        TunnelState::Open | TunnelState::ClientWriteClosed => (
            &mut connection.client,
            &mut connection.upstream,
            PeerSide::Upstream,
        ),
        TunnelState::UpstreamWriteClosed => (
            &mut connection.upstream,
            &mut connection.client,
            PeerSide::Client,
        ),
    };
    sender.write_all(b"request").await?;
    let mut request = [0_u8; 7];
    receiver.read_exact(&mut request).await?;
    assert_eq!(&request, b"request");
    match tunnel_state {
        TunnelState::Open => {}
        TunnelState::ClientWriteClosed | TunnelState::UpstreamWriteClosed => {
            sender.shutdown().await?;
            assert_eq!(receiver.read(&mut [0_u8; 1]).await?, 0);
            connection.eof_received_by = Some(receiver_side);
        }
    }
    // Traffic in the opposite direction remains valid after a write half closes.
    receiver.write_all(b"response").await?;
    let mut response = [0_u8; 8];
    sender.read_exact(&mut response).await?;
    assert_eq!(&response, b"response");
    Ok(connection)
}

async fn read_http_headers(stream: &mut TcpStream) -> Result<String> {
    let mut headers = Vec::new();
    while !headers.ends_with(b"\r\n\r\n") {
        headers.push(stream.read_u8().await?);
    }
    Ok(String::from_utf8(headers)?)
}
