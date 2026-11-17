//! Test TLS admission, stream forwarding, and CONNECT rejection.
#![cfg(test)]

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use bytes::Bytes;
use http::HeaderMap;
use http::HeaderValue;
use http::Method;
use http::Response;
use pretty_assertions::assert_eq;
use rcgen::CertifiedKey;
use rcgen::generate_simple_self_signed;
use rustls::pki_types::PrivatePkcs8KeyDer;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_rustls::TlsAcceptor;

use super::connect_addresses as connect;
use super::connect_through_proxy;
use super::gateway_uri;
use crate::ProxyTarget;
use crate::SetupError;
use crate::transport::Connection;

pub(crate) fn tls_configs() -> Result<(rustls::ServerConfig, rustls::ClientConfig)> {
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["127.0.0.1".to_string()])?;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut server = rustls::ServerConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(
            vec![cert.der().clone()],
            PrivatePkcs8KeyDer::from(signing_key.serialize_der()).into(),
        )?;
    server.alpn_protocols = vec![b"h3".to_vec(), b"h2".to_vec()];
    let mut roots = rustls::RootCertStore::empty();
    roots.add(cert.der().clone())?;
    let mut client = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .with_root_certificates(roots)
        .with_no_client_auth();
    client.alpn_protocols = vec![b"h2".to_vec()];
    Ok((server, client))
}

pub(crate) fn target(address: std::net::SocketAddr) -> ProxyTarget {
    ProxyTarget {
        host: address.ip().to_string(),
        port: address.port(),
        authority: "127.0.0.1:22".to_string(),
    }
}

#[tokio::test]
async fn trusted_h2_requires_settings_and_certificate() -> Result<()> {
    timeout(Duration::from_secs(5), async {
        let (server, client) = tls_configs()?;
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let task = tokio::spawn(async move {
            let tls = TlsAcceptor::from(Arc::new(server))
                .accept(listener.accept().await?.0)
                .await?;
            let mut connection: h2::server::Connection<_, Bytes> =
                h2::server::handshake(tls).await?;
            let _ = connection.accept().await;
            Ok::<_, anyhow::Error>(())
        });
        let (sender, connection) = connect(&target(address), Arc::new(client), &[address]).await?;
        drop((sender, connection));
        task.await??;

        let (bad_server, _) = tls_configs()?;
        let (_, client) = tls_configs()?;
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let task = tokio::spawn(async move {
            TlsAcceptor::from(Arc::new(bad_server))
                .accept(listener.accept().await.unwrap().0)
                .await
        });
        assert!(
            connect(&target(address), Arc::new(client), &[address])
                .await
                .is_err()
        );
        let _ = task.await?;
        Ok::<_, anyhow::Error>(())
    })
    .await?
}

#[tokio::test]
async fn forwards_upload_before_response_and_rejects_before_upload() -> Result<()> {
    timeout(Duration::from_secs(15), async {
        for status in [200, 403] {
            let (server, client) = tls_configs()?;
            let listener = TcpListener::bind("127.0.0.1:0").await?;
            let address = listener.local_addr()?;
            let upload = vec![42; 192 * 1024];
            let download = vec![81; 192 * 1024];
            let expected_upload = upload.clone();
            let response_bytes = download.clone();
            let (done, finished) = tokio::sync::oneshot::channel();
            let peer = tokio::spawn(async move {
                let tls = TlsAcceptor::from(Arc::new(server)).accept(listener.accept().await?.0).await?;
                let mut connection = h2::server::handshake(tls).await?;
                let (request, mut respond) = connection.accept().await.unwrap()?;
                assert_eq!(request.method(), Method::CONNECT);
                assert_eq!(request.uri().authority().unwrap().as_str(), "127.0.0.1:22");
                assert_eq!(request.headers()[http::header::AUTHORIZATION], "Bearer fixture");
                assert_eq!(request.headers()["x-portals-auth-mode"], "oauth");
                assert_eq!(request.headers()["x-portals-environment-id"], "env_fixture");
                assert_eq!(request.headers()["x-portals-port"], "22");
                let mut send = respond.send_response(Response::builder().status(status).body(())?, status != 200)?;
                let handler = async move {
                    if status != 200 { return Ok::<_, anyhow::Error>(()); }
                    let mut body = request.into_body();
                    let mut received = Vec::new();
                    while let Some(data) = body.data().await { let data = data?; received.extend_from_slice(&data); body.flow_control().release_capacity(data.len())?; }
                    assert_eq!(received, expected_upload);
                    send.send_data(Bytes::from(response_bytes), true)?;
                    Ok(())
                };
                let complete = async { handler.await?; finished.await?; Ok::<_, anyhow::Error>(()) };
                tokio::pin!(complete);
                tokio::select! { result = &mut complete => result, closed = std::future::poll_fn(|cx| connection.poll_closed(cx)) => { closed?; complete.await } }
            });
            let (sender, connection) = connect(&target(address), Arc::new(client), &[address]).await?;
            let mut headers = HeaderMap::new();
            headers.insert("x-portals-auth-mode", HeaderValue::from_static("oauth"));
            headers.insert("x-portals-environment-id", HeaderValue::from_static("env_fixture"));
            headers.insert("x-portals-port", HeaderValue::from_static("22"));
            let request = target(address).request(headers, HeaderValue::from_static("Bearer fixture"))?;
            let (mut local, socket) = tokio::io::duplex(256 * 1024);
            let (input, output) = tokio::io::split(socket);
            let relay = tokio::spawn(async move {
                Connection::Http2(sender, Box::new(connection)).admit(request).await?
                    .relay(input, output).await
            });
            if status == 200 {
                let mut received = Vec::new();
                local.write_all(&upload).await?; local.shutdown().await?; local.read_to_end(&mut received).await?;
                assert_eq!(received.len(), download.len());
                assert!(received == download);
                relay.await??;
            } else {
                local.write_all(b"must not be forwarded").await?;
                let error = relay.await?.unwrap_err();
                assert!(matches!(error.downcast_ref::<SetupError>(), Some(SetupError::Rejected(status)) if status.as_u16() == 403));
            }
            let _ = done.send(());
            peer.await??;
        }
        Ok::<_, anyhow::Error>(())
    }).await?
}

#[tokio::test]
async fn reaches_third_address_while_two_tls_peers_stall() -> Result<()> {
    timeout(Duration::from_secs(2), async {
        let (server, client) = tls_configs()?;
        let first = TcpListener::bind("127.0.0.1:0").await?;
        let second = TcpListener::bind("127.0.0.1:0").await?;
        let good = TcpListener::bind("127.0.0.1:0").await?;
        let addresses = [
            first.local_addr()?,
            second.local_addr()?,
            good.local_addr()?,
        ];
        let stalls = tokio::spawn(async move {
            let (_first, _second) = tokio::join!(first.accept(), second.accept());
            std::future::pending::<()>().await;
        });
        let good_task = tokio::spawn(async move {
            let tls = TlsAcceptor::from(Arc::new(server))
                .accept(good.accept().await?.0)
                .await?;
            let mut connection: h2::server::Connection<_, Bytes> =
                h2::server::handshake(tls).await?;
            let _ = connection.accept().await;
            Ok::<_, anyhow::Error>(())
        });
        let connection = connect(&target(addresses[0]), Arc::new(client), &addresses).await?;
        drop(connection);
        stalls.abort();
        good_task.await??;
        Ok::<_, anyhow::Error>(())
    })
    .await?
}

#[tokio::test]
async fn proxy_only_egress_keeps_gateway_credentials_inside_tls() -> Result<()> {
    timeout(Duration::from_secs(3), async {
        let (server, client) = tls_configs()?;
        let gateway = TcpListener::bind("127.0.0.1:0").await?;
        let gateway_address = gateway.local_addr()?;
        let proxy = TcpListener::bind("127.0.0.1:0").await?;
        let proxy_address = proxy.local_addr()?;
        let proxy_task = tokio::spawn(async move {
            let (mut socket, _) = proxy.accept().await?;
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                header.push(socket.read_u8().await?);
                anyhow::ensure!(header.len() < 4096);
            }
            let header = String::from_utf8(header)?;
            assert!(header.starts_with(&format!("CONNECT {gateway_address} HTTP/1.1")));
            assert!(!header.to_lowercase().contains("authorization: bearer"));
            socket
                .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
                .await?;
            let mut upstream = tokio::net::TcpStream::connect(gateway_address).await?;
            tokio::io::copy_bidirectional(&mut socket, &mut upstream).await?;
            Ok::<_, anyhow::Error>(())
        });
        let gateway_task = tokio::spawn(async move {
            let tls = TlsAcceptor::from(Arc::new(server))
                .accept(gateway.accept().await?.0)
                .await?;
            let mut conn: h2::server::Connection<_, Bytes> = h2::server::handshake(tls).await?;
            let (request, mut response) = conn.accept().await.unwrap()?;
            assert_eq!(request.headers()["authorization"], "Bearer fixture");
            response.send_response(Response::new(()), /*end_of_stream*/ true)?;
            let _ = conn.accept().await;
            Ok::<_, anyhow::Error>(())
        });
        let target = target(gateway_address);
        let uri = gateway_uri(&target)?;
        let intercept = hyper_util::client::proxy::matcher::Matcher::builder()
            .https(format!("http://{proxy_address}"))
            .build()
            .intercept(&uri)
            .unwrap();
        let (sender, mut connection) =
            connect_through_proxy(&target, Arc::new(client), uri, intercept).await?;
        let request =
            target.request(HeaderMap::new(), HeaderValue::from_static("Bearer fixture"))?;
        let result = tokio::select! {
            result = super::admit(sender, request) => result,
            _ = &mut connection => anyhow::bail!("gateway closed"),
        };
        drop(result?);
        drop(connection);
        gateway_task.await??;
        proxy_task.await??;
        Ok::<_, anyhow::Error>(())
    })
    .await?
}
