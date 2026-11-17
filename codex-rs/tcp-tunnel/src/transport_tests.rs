//! Exercise admission, QUIC preference, and fallback against real H2/H3 peers.
#![cfg(test)]

use std::net::UdpSocket;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use bytes::Bytes;
use http::HeaderMap;
use http::HeaderValue;
use http::Request;
use http::Response;
use http::StatusCode;
use tokio::net::TcpListener;
use tokio::time::timeout;

use super::Config;
use super::Tunnel;
use crate::http2::tests::target;
use crate::http2::tests::tls_configs;

async fn sockets() -> Result<(TcpListener, UdpSocket)> {
    for _ in 0..32 {
        let udp = UdpSocket::bind("127.0.0.1:0")?;
        if let Ok(tcp) = TcpListener::bind(udp.local_addr()?).await {
            udp.set_nonblocking(true)?;
            return Ok((tcp, udp));
        }
    }
    anyhow::bail!("could not reserve TCP and UDP test port")
}
fn endpoint(udp: UdpSocket, tls: rustls::ServerConfig) -> Result<quinn::Endpoint> {
    let crypto = quinn::crypto::rustls::QuicServerConfig::try_from(tls)?;
    Ok(quinn::Endpoint::new(
        quinn::EndpointConfig::default(),
        Some(quinn::ServerConfig::with_crypto(Arc::new(crypto))),
        udp,
        Arc::new(quinn::TokioRuntime),
    )?)
}
fn request(address: std::net::SocketAddr) -> Result<Request<()>> {
    target(address).request(HeaderMap::new(), HeaderValue::from_static("Bearer fixture"))
}
async fn h2_peer(tcp: TcpListener, tls: rustls::ServerConfig) -> Result<()> {
    let tls = tokio_rustls::TlsAcceptor::from(Arc::new(tls))
        .accept(tcp.accept().await?.0)
        .await?;
    let mut conn: h2::server::Connection<_, Bytes> = h2::server::handshake(tls).await?;
    let (req, mut send) = conn.accept().await.unwrap()?;
    assert_eq!(req.headers()["authorization"], "Bearer fixture");
    let _stream = send.send_response(Response::new(()), false)?;
    let _ = conn.accept().await;
    Ok(())
}

#[tokio::test]
async fn prefers_admitted_quic_without_opening_tcp() -> Result<()> {
    timeout(Duration::from_secs(5), async {
        let (tls, client) = tls_configs()?;
        let (tcp, udp) = sockets().await?;
        let address = tcp.local_addr()?;
        let endpoint = endpoint(udp, tls)?;
        let peer = tokio::spawn(async move {
            let quic = endpoint.accept().await.unwrap().await?;
            let mut http: h3::server::Connection<_, Bytes> =
                h3::server::Connection::new(h3_quinn::Connection::new(quic.clone())).await?;
            let (req, mut stream) = http.accept().await?.unwrap().resolve_request().await?;
            assert_eq!(req.headers()["authorization"], "Bearer fixture");
            stream.send_response(Response::new(())).await?;
            quic.closed().await;
            Ok::<_, anyhow::Error>(())
        });
        let tunnel = Config::new(client)?
            .open(&target(address), async { request(address) })
            .await?;
        assert!(matches!(tunnel, Tunnel::Http3(_)));
        assert!(
            timeout(Duration::from_millis(20), tcp.accept())
                .await
                .is_err()
        );
        drop(tunnel);
        peer.abort();
        Ok::<_, anyhow::Error>(())
    })
    .await?
}

#[tokio::test]
async fn blocked_udp_uses_admitted_h2() -> Result<()> {
    let (tls, client) = tls_configs()?;
    let (tcp, _udp) = sockets().await?;
    let address = tcp.local_addr()?;
    let peer = tokio::spawn(h2_peer(tcp, tls));
    let tunnel = Config::new(client)?
        .open(&target(address), async { request(address) })
        .await?;
    assert!(matches!(tunnel, Tunnel::Http2 { .. }));
    drop(tunnel);
    peer.await??;
    Ok(())
}

#[tokio::test]
async fn quic_handshake_without_working_http3_does_not_defeat_h2() -> Result<()> {
    timeout(Duration::from_secs(3), async {
        let (tls, client) = tls_configs()?;
        let (tcp, udp) = sockets().await?;
        let address = tcp.local_addr()?;
        let endpoint = endpoint(udp, tls.clone())?;
        let quic = tokio::spawn(async move {
            let connection = endpoint.accept().await.unwrap().await?;
            connection.closed().await;
            Ok::<_, anyhow::Error>(())
        });
        let h2 = tokio::spawn(h2_peer(tcp, tls));
        let tunnel = Config::new(client)?
            .open(&target(address), async { request(address) })
            .await?;
        assert!(matches!(tunnel, Tunnel::Http2 { .. }));
        drop(tunnel);
        h2.await??;
        quic.abort();
        Ok::<_, anyhow::Error>(())
    })
    .await?
}

#[tokio::test]
async fn admission_rejection_is_terminal() -> Result<()> {
    let (tls, client) = tls_configs()?;
    let (tcp, udp) = sockets().await?;
    let address = tcp.local_addr()?;
    let endpoint = endpoint(udp, tls)?;
    let peer = tokio::spawn(async move {
        let quic = endpoint.accept().await.unwrap().await?;
        let mut http: h3::server::Connection<_, Bytes> =
            h3::server::Connection::new(h3_quinn::Connection::new(quic.clone())).await?;
        let (_, mut stream) = http.accept().await?.unwrap().resolve_request().await?;
        stream
            .send_response(Response::builder().status(StatusCode::FORBIDDEN).body(())?)
            .await?;
        quic.closed().await;
        Ok::<_, anyhow::Error>(())
    });
    let error = Config::new(client)?
        .open(&target(address), async { request(address) })
        .await
        .err()
        .unwrap();
    assert!(matches!(
        error.downcast_ref::<crate::diagnostics::Code>(),
        Some(crate::diagnostics::Code::Rejected(StatusCode::FORBIDDEN))
    ));
    assert!(
        timeout(Duration::from_millis(20), tcp.accept())
            .await
            .is_err()
    );
    peer.abort();
    Ok(())
}

#[tokio::test]
async fn setup_deadline_includes_connect_response() -> Result<()> {
    let (tls, client) = tls_configs()?;
    let (tcp, udp) = sockets().await?;
    let address = tcp.local_addr()?;
    let endpoint = endpoint(udp, tls.clone())?;
    let quic = tokio::spawn(async move {
        let quic = endpoint.accept().await.unwrap().await?;
        let mut http: h3::server::Connection<_, Bytes> =
            h3::server::Connection::new(h3_quinn::Connection::new(quic.clone())).await?;
        let _stream = http.accept().await?.unwrap().resolve_request().await?;
        quic.closed().await;
        Ok::<_, anyhow::Error>(())
    });
    let h2 = tokio::spawn(async move {
        let tls = tokio_rustls::TlsAcceptor::from(Arc::new(tls))
            .accept(tcp.accept().await?.0)
            .await?;
        let mut conn: h2::server::Connection<_, Bytes> = h2::server::handshake(tls).await?;
        let _stream = conn.accept().await.unwrap()?;
        let _ = conn.accept().await;
        Ok::<_, anyhow::Error>(())
    });
    let result = timeout(
        crate::PROXY_CONNECT_TIMEOUT + Duration::from_secs(2),
        Config::new(client)?.open(&target(address), async { request(address) }),
    )
    .await?;
    assert!(matches!(
        result
            .err()
            .unwrap()
            .downcast_ref::<crate::diagnostics::Code>(),
        Some(crate::diagnostics::Code::Timeout)
    ));
    quic.abort();
    h2.abort();
    Ok(())
}
