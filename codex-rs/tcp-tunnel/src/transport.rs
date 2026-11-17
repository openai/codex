//! Select the first authenticated CONNECT, preferring QUIC with a short head start.
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use bytes::Bytes;
use http::Request;
use quinn::crypto::rustls::QuicClientConfig;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;

use crate::PROXY_CONNECT_TIMEOUT;
use crate::ProxyConnection;
use crate::ProxyTarget;
use crate::diagnostics::Code;
use crate::http2;

const QUIC_HEAD_START: Duration = Duration::from_millis(250);

#[derive(Clone)]
pub(super) struct Config {
    quic: quinn::ClientConfig,
    tls: Arc<rustls::ClientConfig>,
}

impl Config {
    pub(super) fn new(mut tls: rustls::ClientConfig) -> Result<Self> {
        tls.enable_early_data = false;
        tls.alpn_protocols = vec![b"h3".to_vec()];
        let mut quic = quinn::ClientConfig::new(Arc::new(QuicClientConfig::try_from(tls.clone())?));
        let mut transport = quinn::TransportConfig::default();
        transport.enable_segmentation_offload(!cfg!(target_os = "linux"));
        transport.max_idle_timeout(Some(Duration::from_secs(30).try_into()?));
        transport.keep_alive_interval(Some(Duration::from_secs(10)));
        quic.transport_config(Arc::new(transport));
        tls.alpn_protocols = vec![b"h2".to_vec()];
        Ok(Self {
            quic,
            tls: Arc::new(tls),
        })
    }

    /// The request future is polled only after a gateway has passed TLS verification.
    pub(super) async fn open(
        &self,
        target: &ProxyTarget,
        request: impl Future<Output = Result<Request<()>>>,
    ) -> Result<Tunnel> {
        tokio::time::timeout(PROXY_CONNECT_TIMEOUT, async {
            let quic = async {
                ProxyConnection::connect(target, &self.quic)
                    .await
                    .map(|p| Connection::Http3(Box::new(p)))
            };
            let tcp = async {
                tokio::time::sleep(QUIC_HEAD_START).await;
                let addresses: Vec<_> =
                    tokio::net::lookup_host((target.host.as_str(), target.port))
                        .await?
                        .collect();
                http2::connect(target, self.tls.clone(), &addresses)
                    .await
                    .map(|(sender, connection)| Connection::Http2(sender, Box::new(connection)))
            };
            tokio::pin!(quic, tcp);
            tokio::select! {
                biased;
                first = &mut quic => finish_setup(first, tcp, request).await,
                first = &mut tcp => finish_setup(first, quic, request).await,
            }
        })
        .await
        .context(Code::Timeout)?
    }
}

async fn finish_setup(
    first: Result<Connection>,
    other: impl Future<Output = Result<Connection>>,
    request: impl Future<Output = Result<Request<()>>>,
) -> Result<Tunnel> {
    let first = match first {
        Ok(connection) => connection,
        Err(_) => return other.await?.admit(request.await?).await,
    };
    let request = request.await?;
    let preferred = first.admit(request.clone());
    let alternate = async { other.await?.admit(request).await };
    tokio::pin!(preferred, alternate);
    tokio::select! {
        biased;
        result = &mut preferred => complete_or_try_other(result, alternate).await,
        result = &mut alternate => complete_or_try_other(result, preferred).await,
    }
}

async fn complete_or_try_other(
    result: Result<Tunnel>,
    other: impl Future<Output = Result<Tunnel>>,
) -> Result<Tunnel> {
    match result {
        Ok(tunnel) => Ok(tunnel),
        Err(error)
            if error
                .downcast_ref::<Code>()
                .is_some_and(|c| matches!(c, Code::Rejected(_))) =>
        {
            Err(error)
        }
        Err(_) => other.await,
    }
}

pub(super) enum Connection {
    Http3(Box<ProxyConnection>),
    Http2(http2::Sender, Box<http2::Connection>),
}

pub(super) enum Tunnel {
    Http3(Box<(ProxyConnection, crate::H3Stream)>),
    Http2 {
        connection: Box<http2::Connection>,
        send: h2::SendStream<Bytes>,
        recv: h2::RecvStream,
    },
}

impl Connection {
    pub(super) async fn admit(self, request: Request<()>) -> Result<Tunnel> {
        match self {
            Self::Http3(mut proxy) => {
                let admit = async {
                    let mut stream = proxy
                        .sender
                        .send_request(request)
                        .await
                        .context("sending CONNECT")?;
                    crate::check_h3_response(&mut stream).await?;
                    Ok::<_, anyhow::Error>(stream)
                };
                let stream = tokio::select! {
                    result = admit => result?,
                    _ = proxy.connection.wait_idle() => return Err(anyhow!("HTTP/3 proxy transport closed")),
                };
                Ok(Tunnel::Http3(Box::new((*proxy, stream))))
            }
            Self::Http2(sender, mut connection) => {
                let (send, recv) = tokio::select! {
                    result = http2::admit(sender, request) => result?,
                    _ = &mut *connection => return Err(anyhow!("HTTP/2 proxy transport closed")),
                };
                Ok(Tunnel::Http2 {
                    connection,
                    send,
                    recv,
                })
            }
        }
    }
}

impl Tunnel {
    pub(super) async fn relay(
        self,
        input: impl AsyncRead + Unpin,
        output: impl AsyncWrite + Unpin,
        exit_on_remote_eof: bool,
    ) -> Result<()> {
        match self {
            Self::Http3(tunnel) => {
                let (mut proxy, stream) = *tunnel;
                let forward = crate::bridge_h3_stream(input, output, stream, exit_on_remote_eof);
                tokio::pin!(forward);
                tokio::select! {
                    result = &mut forward => result,
                    _ = proxy.connection.wait_idle() => forward.await,
                }
            }
            Self::Http2 {
                mut connection,
                send,
                recv,
            } => {
                let result = {
                    let forward = http2::bridge(input, output, send, recv, exit_on_remote_eof);
                    tokio::pin!(forward);
                    tokio::select! {
                        result = &mut forward => result,
                        result = &mut *connection => {
                            result.map_err(|_| anyhow!("HTTP/2 proxy transport closed"))?;
                            return forward.await;
                        },
                    }
                };
                result?;
                connection
                    .await
                    .map_err(|_| anyhow!("HTTP/2 proxy transport closed"))
            }
        }
    }
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
