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
        let tcp = TcpListener::bind("127.0.0.1:0").await?;
        if let Ok(udp) = UdpSocket::bind(tcp.local_addr()?) {
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
        error.downcast_ref::<crate::SetupError>(),
        Some(crate::SetupError::Rejected(StatusCode::FORBIDDEN))
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
        result.err().unwrap().downcast_ref::<crate::SetupError>(),
        Some(crate::SetupError::Timeout)
    ));
    quic.abort();
    h2.abort();
    Ok(())
}

/// Run with a local, unprivileged sshd and a simple stdio-to-TCP ProxyCommand.
/// This tests the SSH guarantee: a successful remote command has consumed the
/// complete upload, even while QUIC delivery and remote EOF race.
#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a local sshd fixture; see PORTALS_TEST_SSHD_PORT, PORTALS_TEST_KEY, and PORTALS_TEST_PROXY"]
async fn real_ssh_delivery_over_http3() -> Result<()> {
    real_ssh_case(/*remote_exits_early*/ false).await?;
    real_ssh_case(/*remote_exits_early*/ true).await
}

#[cfg(target_os = "linux")]
async fn real_ssh_case(remote_exits_early: bool) -> Result<()> {
    use bytes::Buf;
    use std::io::Write;
    use std::process::Command;
    use std::process::Stdio;
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;

    let sshd_port: u16 = std::env::var("PORTALS_TEST_SSHD_PORT")?.parse()?;
    let key = std::env::var("PORTALS_TEST_KEY")?;
    let proxy_script = std::env::var("PORTALS_TEST_PROXY")?;
    let user = std::env::var("USER")?;
    let (tls, client) = tls_configs()?;
    let (tcp, udp) = sockets().await?;
    let address = tcp.local_addr()?;
    let endpoint = endpoint(udp, tls)?;
    let gateway = tokio::spawn(async move {
        let quic = endpoint.accept().await.unwrap().await?;
        let mut http: h3::server::Connection<_, Bytes> =
            h3::server::Connection::new(h3_quinn::Connection::new(quic.clone())).await?;
        let (_, mut stream) = http.accept().await?.unwrap().resolve_request().await?;
        stream.send_response(Response::new(())).await?;
        let socket = tokio::net::TcpStream::connect(("127.0.0.1", sshd_port)).await?;
        let (mut read, mut write) = socket.into_split();
        let (mut send, mut recv) = stream.split();
        let upload = async {
            while let Some(mut data) = recv.recv_data().await? {
                while data.has_remaining() {
                    let part = data.chunk();
                    write.write_all(part).await?;
                    data.advance(part.len());
                }
            }
            write.shutdown().await?;
            Ok::<_, anyhow::Error>(())
        };
        let download = async {
            let mut buf = vec![0; 65536];
            loop {
                let n = read.read(&mut buf).await?;
                if n == 0 {
                    send.finish().await?;
                    return Ok::<_, anyhow::Error>(());
                }
                send.send_data(Bytes::copy_from_slice(&buf[..n])).await?;
            }
        };
        tokio::pin!(upload, download);
        tokio::select! {
            result = &mut download => result?,
            result = &mut upload => { result?; download.await?; }
        }
        quic.closed().await;
        Ok::<_, anyhow::Error>(())
    });
    let front = TcpListener::bind("127.0.0.1:0").await?;
    let port = front.local_addr()?.port();
    let ssh = tokio::task::spawn_blocking(move || -> Result<_> {
        let mut command = Command::new("timeout");
        command.args([
            "15",
            "ssh",
            "-F",
            "/dev/null",
            "-T",
            "-i",
            &key,
            "-o",
            "BatchMode=yes",
            "-o",
            "StrictHostKeyChecking=no",
            "-o",
            "UserKnownHostsFile=/dev/null",
            "-o",
            "LogLevel=ERROR",
            "-o",
            &format!("ProxyCommand=python3 {proxy_script} {port}"),
            &format!("{user}@localhost"),
            if remote_exits_early {
                "exit 0"
            } else {
                "wc -c"
            },
        ]);
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut stdin = child.stdin.take().unwrap();
        if remote_exits_early {
            let output = child.wait_with_output()?;
            drop(stdin);
            Ok(output)
        } else {
            stdin.write_all(&vec![b'x'; 1024 * 1024 + 17])?;
            drop(stdin);
            Ok(child.wait_with_output()?)
        }
    });
    let (socket, _) = front.accept().await?;
    let tunnel = Config::new(client)?
        .open(&target(address), async { request(address) })
        .await?;
    assert!(matches!(tunnel, Tunnel::Http3(_)));
    let (input, output) = socket.into_split();
    let relay = tokio::spawn(tunnel.relay(input, output));
    let output = ssh.await??;
    assert!(
        output.status.success(),
        "ssh failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout)?.trim(),
        if remote_exits_early { "" } else { "1048593" }
    );
    relay
        .await?
        .map_err(|error| anyhow::anyhow!("client relay: {error:#}"))?;
    gateway
        .await?
        .map_err(|error| anyhow::anyhow!("gateway: {error:#}"))?;
    Ok(())
}
