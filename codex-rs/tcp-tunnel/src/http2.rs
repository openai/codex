//! TLS HTTP/2 CONNECT transport for the same trusted proxy origin as HTTP/3.
//! Each connection carries one CONNECT and preserves TCP half-close and bounded buffering.

use std::future::Future;
use std::future::poll_fn;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::ensure;
use bytes::Bytes;
use http::Request;
use hyper_util::client::legacy::connect::proxy::Tunnel;
use hyper_util::client::proxy::matcher::Intercept;
use hyper_util::client::proxy::matcher::Matcher;
use hyper_util::rt::TokioIo;
use rustls::pki_types::ServerName;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tower_service::Service;

use crate::MAX_FIELD_SECTION_SIZE;
use crate::ProxyTarget;
use crate::SetupError;

const COPY_BUFFER_SIZE: usize = 64 * 1024;

pub(super) type Sender = h2::client::SendRequest<Bytes>;
pub(super) trait GatewayIo: AsyncRead + AsyncWrite + Send + Unpin {}
impl<T: AsyncRead + AsyncWrite + Send + Unpin> GatewayIo for T {}
pub(super) type Connection =
    h2::client::Connection<tokio_rustls::client::TlsStream<Box<dyn GatewayIo>>, Bytes>;

pub(super) async fn connect(
    target: &ProxyTarget,
    tls: Arc<rustls::ClientConfig>,
) -> Result<(Sender, Connection)> {
    let addresses: Vec<_> = tokio::net::lookup_host((target.host.as_str(), target.port))
        .await?
        .collect();
    connect_addresses(target, tls, &addresses).await
}

pub(super) fn system_proxy(target: &ProxyTarget) -> Result<Option<(http::Uri, Intercept)>> {
    let uri = gateway_uri(target)?;
    Ok(Matcher::from_system()
        .intercept(&uri)
        .map(|proxy| (uri, proxy)))
}

async fn connect_addresses(
    target: &ProxyTarget,
    tls: Arc<rustls::ClientConfig>,
    addresses: &[SocketAddr],
) -> Result<(Sender, Connection)> {
    let name = ServerName::try_from(target.host.clone())
        .map_err(|_| anyhow!("invalid proxy TLS server name"))?;
    let connector = TlsConnector::from(tls);
    let mut attempts = tokio::task::JoinSet::new();
    for &address in addresses {
        let connector = connector.clone();
        let name = name.clone();
        attempts.spawn(async move {
            let socket = TcpStream::connect(address).await?;
            socket.set_nodelay(/*nodelay*/ true)?;
            handshake(Box::new(socket), connector, name).await
        });
    }
    let mut invalid_certificate = false;
    while let Some(result) = attempts.join_next().await {
        match result {
            Ok(Ok(connection)) => return Ok(connection),
            Ok(Err(error)) => {
                invalid_certificate |= error.chain().any(|cause| {
                    matches!(
                        cause.downcast_ref::<rustls::Error>(),
                        Some(rustls::Error::InvalidCertificate(_))
                    )
                });
            }
            _ => {}
        }
    }
    if invalid_certificate {
        return Err(anyhow!("HTTP/2 proxy certificate verification failed"));
    }
    Err(anyhow!("HTTP/2 proxy connection failed"))
}

fn gateway_uri(target: &ProxyTarget) -> Result<http::Uri> {
    let host = if target.host.contains(':') {
        format!("[{}]", target.host)
    } else {
        target.host.clone()
    };
    Ok(format!("https://{host}:{}/", target.port).parse()?)
}

pub(super) async fn connect_through_proxy(
    target: &ProxyTarget,
    tls: Arc<rustls::ClientConfig>,
    uri: http::Uri,
    proxy: Intercept,
) -> Result<(Sender, Connection)> {
    ensure!(
        matches!(proxy.uri().scheme_str(), Some("http" | "https")),
        "unsupported HTTPS proxy scheme"
    );
    let mut proxy_tls = (*tls).clone();
    proxy_tls.alpn_protocols.clear();
    let connector = hyper_rustls::HttpsConnectorBuilder::new()
        .with_tls_config(proxy_tls)
        .https_or_http()
        .enable_http1()
        .build();
    let mut tunnel = Tunnel::new(proxy.uri().clone(), connector);
    if let Some(auth) = proxy.basic_auth() {
        tunnel = tunnel.with_auth(auth.clone());
    }
    let socket = TokioIo::new(tunnel.call(uri).await?);
    let name = ServerName::try_from(target.host.clone())
        .map_err(|_| anyhow!("invalid proxy TLS server name"))?;
    handshake(Box::new(socket), TlsConnector::from(tls), name).await
}

async fn handshake(
    socket: Box<dyn GatewayIo>,
    connector: TlsConnector,
    name: ServerName<'static>,
) -> Result<(Sender, Connection)> {
    let stream = connector.connect(name, socket).await?;
    ensure!(
        stream.get_ref().1.alpn_protocol() == Some(b"h2"),
        "proxy did not negotiate HTTP/2"
    );
    let (sender, mut connection) = h2::client::Builder::new()
        .max_header_list_size(MAX_FIELD_SECTION_SIZE as u32)
        .max_send_buffer_size(COPY_BUFFER_SIZE)
        .initial_max_send_streams(0)
        .enable_push(/*enabled*/ false)
        .handshake(stream)
        .await?;
    // A TLS peer advertising h2 is not usable until it sends SETTINGS
    // admitting a stream. Poll the connection without sending credentials.
    poll_fn(|cx| {
        if let Poll::Ready(result) = Pin::new(&mut connection).poll(cx) {
            return Poll::Ready(Err(result
                .err()
                .map(anyhow::Error::from)
                .unwrap_or_else(|| anyhow!("HTTP/2 proxy closed during setup"))));
        }
        if connection.max_concurrent_send_streams() > 0 {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    })
    .await?;
    Ok((sender, connection))
}

pub(super) async fn admit(
    sender: Sender,
    request: Request<()>,
) -> Result<(h2::SendStream<Bytes>, h2::RecvStream)> {
    let mut sender = sender.ready().await.map_err(|_| stream_error())?;
    let (response, mut send) = sender
        .send_request(request, /*end_of_stream*/ false)
        .map_err(|_| stream_error())?;
    let response = response.await.map_err(|_| stream_error())?;
    let status = response.status();
    if !status.is_success() {
        send.send_reset(h2::Reason::CANCEL);
        return Err(SetupError::Rejected(status).into());
    }
    Ok((send, response.into_body()))
}

pub(super) async fn bridge(
    mut read: impl AsyncRead + Unpin,
    mut write: impl AsyncWrite + Unpin,
    mut send: h2::SendStream<Bytes>,
    mut recv: h2::RecvStream,
) -> Result<()> {
    let upload = async {
        let mut buffer = vec![0; COPY_BUFFER_SIZE];
        loop {
            let count = read.read(&mut buffer).await.context("reading local TCP")?;
            if count == 0 {
                send.send_data(Bytes::new(), /*end_of_stream*/ true)
                    .map_err(|_| stream_error())?;
                return Ok::<(), anyhow::Error>(());
            }
            let mut data = Bytes::copy_from_slice(&buffer[..count]);
            send.reserve_capacity(data.len());
            while !data.is_empty() {
                let capacity = poll_fn(|cx| send.poll_capacity(cx))
                    .await
                    .context("HTTP/2 send stream closed")?
                    .map_err(|_| stream_error())?;
                let count = capacity.min(data.len());
                send.send_data(data.split_to(count), /*end_of_stream*/ false)
                    .map_err(|_| stream_error())?;
            }
        }
    };
    let download = async {
        while let Some(data) = recv.data().await {
            let data = data.map_err(|_| stream_error())?;
            write.write_all(&data).await.context("writing local TCP")?;
            write.flush().await.context("flushing local TCP")?;
            recv.flow_control()
                .release_capacity(data.len())
                .map_err(|_| stream_error())?;
        }
        write.shutdown().await.context("half-closing local TCP")?;
        Ok::<(), anyhow::Error>(())
    };
    tokio::pin!(upload, download);
    tokio::select! {
        result = &mut upload => { result?; download.await }
        result = &mut download => { result?; Ok(()) }
    }
}

fn stream_error() -> anyhow::Error {
    // Peer-supplied GOAWAY debug text must not reach diagnostics.
    anyhow!("HTTP/2 CONNECT stream failed")
}

#[cfg(test)]
#[path = "http2_tests.rs"]
pub(super) mod tests;
