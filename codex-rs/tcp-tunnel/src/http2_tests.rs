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

use super::connect;
use crate::ProxyTarget;
use crate::diagnostics::Code;
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
async fn forwards_bounded_data_in_both_half_close_orders_and_rejects_before_upload() -> Result<()> {
    timeout(Duration::from_secs(15), async {
        for (remote_first, status) in [(false, 200), (true, 200), (false, 403)] {
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
                    if remote_first { send.send_data(Bytes::from(response_bytes.clone()), true)?; }
                    let mut body = request.into_body();
                    let mut received = Vec::new();
                    while let Some(data) = body.data().await { let data = data?; received.extend_from_slice(&data); body.flow_control().release_capacity(data.len())?; }
                    assert_eq!(received, expected_upload);
                    if !remote_first { send.send_data(Bytes::from(response_bytes), true)?; }
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
                    .relay(input, output, /*exit_on_remote_eof*/ false).await
            });
            if status == 200 {
                let mut received = Vec::new();
                if remote_first { local.read_to_end(&mut received).await?; local.write_all(&upload).await?; local.shutdown().await?; }
                else { local.write_all(&upload).await?; local.shutdown().await?; local.read_to_end(&mut received).await?; }
                assert_eq!(received.len(), download.len());
                assert!(received == download);
                relay.await??;
            } else {
                local.write_all(b"must not be forwarded").await?;
                let error = relay.await?.unwrap_err();
                assert!(matches!(error.downcast_ref::<Code>(), Some(Code::Rejected(status)) if status.as_u16() == 403));
            }
            let _ = done.send(());
            peer.await??;
        }
        Ok::<_, anyhow::Error>(())
    }).await?
}

#[tokio::test]
async fn retries_next_address_after_non_certificate_tls_failure() -> Result<()> {
    timeout(Duration::from_secs(5), async {
        let (server, client) = tls_configs()?;
        let bad = TcpListener::bind("127.0.0.1:0").await?;
        let good = TcpListener::bind("127.0.0.1:0").await?;
        let addresses = [bad.local_addr()?, good.local_addr()?];
        let bad_task = tokio::spawn(async move {
            drop(bad.accept().await.unwrap());
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
        bad_task.await?;
        good_task.await??;
        Ok::<_, anyhow::Error>(())
    })
    .await?
}
