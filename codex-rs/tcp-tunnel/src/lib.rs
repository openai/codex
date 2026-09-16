//! Loopback TCP forwarding through an HTTP/3 CONNECT proxy.
//! Bearer credentials and optional proxy-specific metadata enter only via stdin.
//! The listener survives transport loss; individual TCP streams are never replayed.
use std::io::BufRead;
use std::io::Write;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::ensure;
use bytes::Buf;
use bytes::Bytes;
use clap::Args as ClapArgs;
use http::HeaderMap;
use http::HeaderValue;
use http::Method;
use http::Request;
use http::Uri;
use http::header;
use http::uri::Authority;
use quinn::crypto::rustls::QuicClientConfig;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::watch;
use url::Host;
use url::Url;

mod control;

use control::read_auth_token;
use control::read_connect_headers;

const PROXY_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, ClapArgs)]
pub struct Args {
    /// HTTPS origin of the HTTP/3 proxy.
    #[arg(long)]
    proxy_url: Url,
    /// File containing one exact approved HTTPS proxy origin per line.
    #[arg(long)]
    proxy_origins_file: PathBuf,
    /// CONNECT target authority, including a nonzero port.
    #[arg(long)]
    target: String,
    #[arg(long, default_value = "127.0.0.1:0")]
    listen_addr: SocketAddr,
    /// Read the initial bearer from stdin.
    #[arg(long, default_value_t = false)]
    auth_token_stdin: bool,
    /// Read replacement bearers from stdin and stop when the controlling pipe closes.
    #[arg(long, requires = "auth_token_stdin")]
    auth_token_updates_stdin: bool,
    /// Read a JSON list of extension-header name/value pairs before the first bearer.
    #[arg(long, requires = "auth_token_stdin")]
    connect_headers_stdin: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct ProxyTarget {
    host: String,
    port: u16,
    authority: String,
}

struct TunnelHeaders {
    auth: watch::Receiver<HeaderValue>,
    connect: HeaderMap,
}

struct ProxyConnection {
    endpoint: quinn::Endpoint,
    quic: quinn::Connection,
    connection: h3::client::Connection<h3_quinn::Connection, Bytes>,
    sender: h3::client::SendRequest<h3_quinn::OpenStreams, Bytes>,
}

enum ProxyClosure {
    Draining,
    Closed(anyhow::Error),
}

impl ProxyTarget {
    fn parse(proxy_url: &Url, trusted_origins: &[String], target: &str) -> Result<Self> {
        ensure!(proxy_url.scheme() == "https", "proxy URL must use HTTPS");
        ensure!(
            proxy_url.username().is_empty()
                && proxy_url.password().is_none()
                && proxy_url.query().is_none()
                && proxy_url.fragment().is_none()
                && proxy_url.path() == "/",
            "proxy URL must be an HTTPS origin"
        );
        ensure!(
            trusted_origins.contains(&proxy_url.origin().ascii_serialization()),
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

fn parse_trusted_origins(origins: &str) -> Result<Vec<String>> {
    let trusted_origins = origins
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let origin = Url::parse(line).context("invalid trusted proxy origin")?;
            ensure!(
                origin.scheme() == "https"
                    && origin.username().is_empty()
                    && origin.password().is_none()
                    && origin.query().is_none()
                    && origin.fragment().is_none()
                    && origin.path() == "/"
                    && line == origin.origin().ascii_serialization(),
                "trusted proxy origins must be exact HTTPS origins"
            );
            Ok(line.to_owned())
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(!trusted_origins.is_empty(), "no trusted proxy origins");
    Ok(trusted_origins)
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;

pub async fn run(args: Args) -> Result<()> {
    ensure!(args.auth_token_stdin, "--auth-token-stdin is required");
    ensure!(
        args.listen_addr.ip().is_loopback(),
        "listener must be loopback"
    );
    let origins = std::fs::read_to_string(&args.proxy_origins_file)
        .context("reading trusted proxy origins")?;
    let trusted_origins = parse_trusted_origins(&origins)?;
    let target = ProxyTarget::parse(&args.proxy_url, &trusted_origins, &args.target)?;
    let (metadata, mut tokens) = control_input(
        std::io::BufReader::new(std::io::stdin()),
        args.connect_headers_stdin,
    )?;
    let connect = metadata.await.context("CONNECT metadata input closed")??;
    let initial_auth = tokens
        .recv()
        .await
        .context("MASQUE credential input closed")??
        .context("empty MASQUE token")?;
    let (updates, auth) = watch::channel(initial_auth);
    let headers = TunnelHeaders { auth, connect };

    let listener = TcpListener::bind(args.listen_addr)
        .await
        .context("binding loopback listener")?;
    let native = rustls_native_certs::load_native_certs();
    let mut roots = rustls::RootCertStore::empty();
    roots.add_parsable_certificates(native.certs);
    ensure!(
        !roots.is_empty(),
        "no native TLS roots: {:?}",
        native.errors
    );
    let mut tls = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()?
    .with_root_certificates(roots)
    .with_no_client_auth();
    tls.alpn_protocols = vec![b"h3".to_vec()];
    let mut config = quinn::ClientConfig::new(Arc::new(QuicClientConfig::try_from(tls)?));
    let mut transport = quinn::TransportConfig::default();
    transport.max_idle_timeout(Some(Duration::from_secs(30).try_into()?));
    transport.keep_alive_interval(Some(Duration::from_secs(10)));
    config.transport_config(Arc::new(transport));
    let (ready, readiness) = oneshot::channel();
    let tunnel = async move {
        let connected = ProxyConnection::connect(&target, &config).await?;
        // The line is the caller readiness contract, including the assigned port.
        writeln!(std::io::stdout(), "LISTENING {}", listener.local_addr()?)?;
        std::io::stdout().flush()?;
        let _ = ready.send(());
        serve(listener, target, config, headers, connected).await
    };
    if args.auth_token_updates_stdin {
        tokio::select! {
            result = tunnel => result,
            result = update_auth_tokens(&mut tokens, updates, readiness, std::io::stdout()) => result,
        }
    } else {
        tunnel.await
    }
}

type TokenStream = mpsc::Receiver<Result<Option<HeaderValue>>>;

async fn update_auth_tokens(
    tokens: &mut TokenStream,
    updates: watch::Sender<HeaderValue>,
    readiness: oneshot::Receiver<()>,
    mut output: impl Write,
) -> Result<()> {
    let mut readiness = Some(readiness);
    while let Some(result) = tokens.recv().await {
        let Some(token) = result? else {
            break;
        };
        updates.send_replace(token);
        if let Some(readiness) = readiness.take() {
            readiness.await.context("MASQUE tunnel did not start")?;
        }
        writeln!(output, "AUTH_UPDATED")?;
        output.flush()?;
    }
    Ok(())
}

fn control_input(
    mut reader: impl BufRead + Send + 'static,
    connect_headers_stdin: bool,
) -> Result<(oneshot::Receiver<Result<HeaderMap>>, TokenStream)> {
    let (metadata, initial_metadata) = oneshot::channel();
    let (send, tokens) = mpsc::channel(1);
    // Tokio's blocking stdin can prevent runtime shutdown after a proxy startup failure.
    std::thread::Builder::new()
        .name("masque-credentials".to_owned())
        .spawn(move || {
            let headers = if connect_headers_stdin {
                read_connect_headers(&mut reader)
            } else {
                Ok(HeaderMap::new())
            };
            let valid = headers.is_ok();
            let _ = metadata.send(headers);
            if !valid {
                return;
            }
            loop {
                let token = read_auth_token(&mut reader);
                let finished = !matches!(&token, Ok(Some(_)));
                if send.blocking_send(token).is_err() || finished {
                    break;
                }
            }
        })
        .context("starting MASQUE credential input")?;
    Ok((initial_metadata, tokens))
}

impl ProxyConnection {
    async fn connect(target: &ProxyTarget, config: &quinn::ClientConfig) -> Result<Self> {
        tokio::time::timeout(PROXY_CONNECT_TIMEOUT, async {
            let mut last_error = "MASQUE proxy resolved to no addresses".to_owned();
            for peer in tokio::net::lookup_host((target.host.as_str(), target.port)).await? {
                let bind = match peer.ip() {
                    IpAddr::V4(_) => "0.0.0.0:0",
                    IpAddr::V6(_) => "[::]:0",
                };
                let mut endpoint = match quinn::Endpoint::client(bind.parse()?) {
                    Ok(endpoint) => endpoint,
                    Err(error) => {
                        last_error = error.to_string();
                        continue;
                    }
                };
                endpoint.set_default_client_config(config.clone());
                let connecting = match endpoint.connect(peer, &target.host) {
                    Ok(connecting) => connecting,
                    Err(error) => {
                        last_error = error.to_string();
                        continue;
                    }
                };
                let quic = match tokio::time::timeout(Duration::from_secs(5), connecting).await {
                    Ok(Ok(quic)) => quic,
                    Ok(Err(error)) => {
                        last_error = error.to_string();
                        continue;
                    }
                    Err(_) => {
                        last_error = "QUIC handshake timed out".to_owned();
                        continue;
                    }
                };
                let (connection, sender) = match h3::client::builder()
                    .enable_extended_connect(true)
                    .build(h3_quinn::Connection::new(quic.clone()))
                    .await
                {
                    Ok(connection) => connection,
                    Err(error) => {
                        last_error = error.to_string();
                        continue;
                    }
                };
                return Ok(Self {
                    endpoint,
                    quic,
                    connection,
                    sender,
                });
            }
            bail!("QUIC handshake failed: {last_error}");
        })
        .await
        .context("connecting to MASQUE proxy timed out")?
    }
}

async fn serve(
    listener: TcpListener,
    target: ProxyTarget,
    config: quinn::ClientConfig,
    headers: TunnelHeaders,
    mut connected: ProxyConnection,
) -> Result<()> {
    loop {
        let ProxyConnection {
            endpoint,
            quic,
            mut connection,
            sender,
        } = connected;
        let mut driver = tokio::spawn(async move { connection.wait_idle().await });
        let (draining, mut drain_started) = mpsc::channel(1);
        let failure = loop {
            tokio::select! {
                incoming = listener.accept() => {
                    let (socket, _) = incoming?;
                    let mut sender = sender.clone();
                    let draining = draining.clone();
                    let mut request = Request::builder().method(Method::CONNECT)
                        .uri(Uri::builder().authority(target.authority.as_str()).build()?)
                        .body(())?;
                    *request.headers_mut() = headers.connect.clone();
                    request.headers_mut().insert(header::AUTHORIZATION, headers.auth.borrow().clone());
                    tokio::spawn(async move {
                        if let Err(error) = bridge(socket, &mut sender, request, draining).await {
                            eprintln!("MASQUE TCP connection failed: {error:#}");
                        }
                    });
                }
                _ = drain_started.recv() => break ProxyClosure::Draining,
                closed = &mut driver => break ProxyClosure::Closed(match closed {
                    Ok(error) => anyhow!("MASQUE HTTP/3 connection closed: {error}"),
                    Err(error) => anyhow!("MASQUE HTTP/3 driver stopped: {error}"),
                }),
                closed = quic.closed() => break ProxyClosure::Closed(anyhow!(closed).context("MASQUE QUIC connection closed")),
            }
        };
        drop(sender);
        match failure {
            ProxyClosure::Draining => {
                eprintln!("MASQUE HTTP/3 proxy is draining; reconnecting");
                // A refused new CONNECT is not replayed. Accepted streams keep their old transport.
                tokio::spawn(async move {
                    tokio::select! {
                        _ = &mut driver => {},
                        _ = quic.closed() => driver.abort(),
                    }
                    drop(endpoint);
                });
            }
            ProxyClosure::Closed(error) => {
                driver.abort();
                quic.close(/*error_code*/ 0_u8.into(), b"reconnecting");
                drop(endpoint);
                eprintln!("MASQUE proxy connection lost; reconnecting: {error:#}");
            }
        }

        let mut retry_delay = Duration::from_secs(1);
        loop {
            let result = {
                let attempt = async {
                    tokio::time::sleep(rand::random_range(retry_delay / 2..=retry_delay)).await;
                    ProxyConnection::connect(&target, &config).await
                };
                tokio::pin!(attempt);
                loop {
                    tokio::select! {
                        incoming = listener.accept() => {
                            // Offline local connections fail promptly; their traffic is never queued or replayed.
                            let (socket, _) = incoming?;
                            drop(socket);
                        }
                        result = &mut attempt => break result,
                    }
                }
            };
            match result {
                Ok(reconnected) => {
                    connected = reconnected;
                    break;
                }
                Err(error) => {
                    eprintln!("MASQUE proxy reconnect failed: {error:#}");
                    retry_delay = (retry_delay * 2).min(Duration::from_secs(15));
                }
            }
        }
    }
}

async fn bridge(
    mut socket: TcpStream,
    sender: &mut h3::client::SendRequest<h3_quinn::OpenStreams, Bytes>,
    request: Request<()>,
    draining: mpsc::Sender<()>,
) -> Result<()> {
    let mut stream = match sender.send_request(request).await {
        Ok(stream) => stream,
        Err(error) => {
            if matches!(error, h3::error::StreamError::RemoteClosing { .. }) {
                let _ = draining.try_send(());
            }
            return Err(error).context("sending CONNECT");
        }
    };
    let status = stream
        .recv_response()
        .await
        .context("receiving CONNECT response")?
        .status();
    if !status.is_success() {
        bail!("MASQUE CONNECT rejected with status {}", status.as_u16());
    }
    let (mut send, mut recv) = stream.split();
    let (mut local_read, mut local_write) = socket.split();
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
        }
        local_write.shutdown().await?;
        Ok::<_, anyhow::Error>(())
    };
    tokio::try_join!(upload, download)?;
    Ok(())
}
