//! One SSH-owned, authenticated tunnel to a verified Cloud Portals gateway.
use std::fmt;
use std::net::SocketAddr;
use std::num::NonZeroU16;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::ensure;
use bytes::Buf;
use bytes::Bytes;
use clap::Args as ClapArgs;
use http::HeaderMap;
use http::HeaderValue;
use http::Method;
use http::Request;
use http::StatusCode;
use http::Uri;
use http::header;
use http::uri::Authority;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use url::Host;
use url::Url;

mod http2;
mod ssh;
mod transport;

pub use ssh::run_ssh_proxy;
pub use ssh::run_ssh_proxy_from_file;

const PROXY_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_FIELD_SECTION_SIZE: u64 = 64 * 1024;
const MAX_TOKEN_BYTES: usize = 64 * 1024;

#[derive(Debug, ClapArgs)]
pub struct Args {
    /// Verified HTTPS gateway origin.
    #[arg(long, default_value = ssh::PUBLIC_GATEWAY)]
    pub proxy_url: Url,
    /// Canonical Cloud environment ID.
    #[arg(long, required_unless_present = "target", conflicts_with = "target")]
    environment_id: Option<String>,
    /// Published SSH port.
    #[arg(long, requires = "environment_id")]
    port: Option<NonZeroU16>,
    /// ERS-issued legacy CONNECT authority; requires a private token file.
    #[arg(long, requires = "token_file", conflicts_with = "environment_id")]
    target: Option<SocketAddr>,
    /// One-use bearer in a private file, supplied by Desktop. Never read from SSH stdin.
    #[arg(long)]
    pub token_file: Option<PathBuf>,
}

#[derive(Debug)]
enum SetupError {
    Timeout,
    Rejected(StatusCode),
}
impl fmt::Display for SetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timeout => f.write_str("SSH tunnel setup timed out"),
            Self::Rejected(status) => write!(f, "CONNECT rejected with status {}", status.as_u16()),
        }
    }
}
impl std::error::Error for SetupError {}

#[derive(Debug, PartialEq, Eq)]
struct ProxyTarget {
    host: String,
    port: u16,
    authority: String,
}
struct ProxyConnection {
    _endpoint: quinn::Endpoint,
    _quic: quinn::Connection,
    connection: h3::client::Connection<h3_quinn::Connection, Bytes>,
    sender: h3::client::SendRequest<h3_quinn::OpenStreams, Bytes>,
}

impl ProxyTarget {
    fn request(&self, mut headers: HeaderMap, bearer: HeaderValue) -> Result<Request<()>> {
        let mut request = Request::builder()
            .method(Method::CONNECT)
            .uri(Uri::builder().authority(self.authority.as_str()).build()?)
            .body(())?;
        headers.insert(header::AUTHORIZATION, bearer);
        *request.headers_mut() = headers;
        Ok(request)
    }

    fn parse(proxy_url: &Url, trusted_origins: &[impl AsRef<str>], target: &str) -> Result<Self> {
        ensure!(proxy_url.scheme() == "https", "proxy URL must use HTTPS");
        ensure!(
            proxy_url.username().is_empty()
                && proxy_url.password().is_none()
                && proxy_url.query().is_none()
                && proxy_url.fragment().is_none()
                && proxy_url.path() == "/",
            "proxy URL must be an HTTPS origin"
        );
        let origin = proxy_url.origin().ascii_serialization();
        ensure!(
            trusted_origins
                .iter()
                .any(|trusted| trusted.as_ref() == origin),
            "proxy URL origin is not trusted"
        );
        let host = match proxy_url.host().context("proxy URL has no host")? {
            Host::Domain(host) => host.to_owned(),
            Host::Ipv4(address) => address.to_string(),
            Host::Ipv6(address) => address.to_string(),
        };
        ensure!(
            !target.contains('@'),
            "CONNECT target must not contain credentials"
        );
        let authority: Authority = target.parse().context("invalid CONNECT target")?;
        ensure!(
            !authority.host().is_empty() && authority.port_u16().is_some_and(|port| port != 0),
            "CONNECT target must include a nonzero port"
        );
        Ok(Self {
            host,
            port: proxy_url.port_or_known_default().unwrap_or(443),
            authority: authority.to_string(),
        })
    }
}

fn tls_config() -> Result<rustls::ClientConfig> {
    let native = rustls_native_certs::load_native_certs();
    let mut roots = rustls::RootCertStore::empty();
    roots.add_parsable_certificates(native.certs);
    ensure!(
        !roots.is_empty(),
        "no native TLS roots: {:?}",
        native.errors
    );
    Ok(rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()?
    .with_root_certificates(roots)
    .with_no_client_auth())
}

impl ProxyConnection {
    async fn connect(target: &ProxyTarget, config: &quinn::ClientConfig) -> Result<Self> {
        for peer in tokio::net::lookup_host((target.host.as_str(), target.port)).await? {
            let attempt = async {
                let bind = if peer.is_ipv4() {
                    "0.0.0.0:0"
                } else {
                    "[::]:0"
                };
                let mut endpoint = quinn::Endpoint::client(bind.parse()?)?;
                endpoint.set_default_client_config(config.clone());
                let quic = endpoint.connect(peer, &target.host)?.await?;
                let (connection, sender) = h3::client::builder()
                    .max_field_section_size(MAX_FIELD_SECTION_SIZE)
                    .build(h3_quinn::Connection::new(quic.clone()))
                    .await?;
                Ok::<_, anyhow::Error>(Self {
                    _endpoint: endpoint,
                    _quic: quic,
                    connection,
                    sender,
                })
            };
            if let Ok(Ok(connection)) = tokio::time::timeout(Duration::from_secs(5), attempt).await
            {
                return Ok(connection);
            }
        }
        Err(anyhow!("HTTP/3 proxy connection failed"))
    }
}

type H3Stream = h3::client::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>;

async fn check_h3_response(stream: &mut H3Stream) -> Result<()> {
    let status = stream
        .recv_response()
        .await
        .context("receiving CONNECT response")?
        .status();
    if !status.is_success() {
        return Err(SetupError::Rejected(status).into());
    }
    Ok(())
}

async fn bridge_h3_stream(
    mut local_read: impl AsyncRead + Unpin,
    mut local_write: impl AsyncWrite + Unpin,
    stream: H3Stream,
) -> Result<()> {
    let (mut send, mut recv) = stream.split();
    let upload = async {
        let mut buffer = vec![0; 64 * 1024];
        loop {
            let count = local_read.read(&mut buffer).await?;
            if count == 0 {
                send.finish().await?;
                return Ok::<_, anyhow::Error>(());
            }
            send.send_data(Bytes::copy_from_slice(&buffer[..count]))
                .await?;
        }
    };
    let download = async {
        while let Some(mut data) = recv.recv_data().await? {
            while data.has_remaining() {
                let chunk = data.chunk();
                local_write.write_all(chunk).await?;
                data.advance(chunk.len());
            }
            local_write.flush().await?;
        }
        local_write.shutdown().await?;
        Ok::<_, anyhow::Error>(())
    };
    tokio::pin!(upload, download);
    tokio::select! {
        result = &mut upload => { result?; download.await }
        result = &mut download => {
            result?;
            Ok(())
        }
    }
}
