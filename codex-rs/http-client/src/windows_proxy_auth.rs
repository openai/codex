//! Authenticates a connection to a Windows proxy after a CONNECT 407.
//! Codex owns the connection. Curl reaches it through a local socket used only for
//! authentication. The caller receives the authenticated connection and buffered data.
//! HTTPS proxies that require authentication tokens bound to their TLS connection
//! are not supported because curl does not own that connection.

mod pipe;

pub(crate) use pipe::ProxyPipe;
pub(crate) use pipe::pipe_for;

use std::io;
use std::io::Cursor;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::os::windows::io::IntoRawSocket;
use std::os::windows::io::RawSocket;
use std::sync::Arc;
use std::sync::Mutex;

use curl::easy::Auth;
use curl::easy::Easy2;
use curl::easy::Handler;
use curl::easy::List;
use reqwest::Url;
use rustls::ClientConfig;
use rustls::pki_types::ServerName;
use socket2::Socket;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::io::Interest;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio_rustls::TlsConnector;
use url::Host;

use crate::OutboundProxyRoute;
use crate::build_rustls_client_config_with_custom_ca;

/// Reads and writes a TCP or TLS connection to the proxy.
trait ProxyIo: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> ProxyIo for T {}

struct CurlSocket {
    // After CONNECT, this duplicate lets Tokio wait for curl's socket without polling.
    // All reads still go through curl so bytes in curl's own buffers are preserved.
    readiness: Option<std::net::TcpStream>,
    expected_peer: Arc<Mutex<Option<SocketAddr>>>,
    cancellation: watch::Sender<()>,
}

impl Handler for CurlSocket {
    fn open_socket(&mut self, family: i32, kind: i32, protocol: i32) -> Option<RawSocket> {
        let socket = Socket::new(family.into(), kind.into(), Some(protocol.into())).ok()?;
        socket
            .bind(&SocketAddr::from((Ipv4Addr::LOCALHOST, 0)).into())
            .ok()?;
        self.readiness = Some(socket.try_clone().ok()?.into());
        *self.expected_peer.lock().ok()? = Some(socket.local_addr().ok()?.as_socket()?);
        Some(socket.into_raw_socket())
    }

    fn progress(&mut self, _dltotal: f64, _dlnow: f64, _ultotal: f64, _ulnow: f64) -> bool {
        !self.cancellation.is_closed()
    }
}

/// TLS configuration for the connection to an HTTPS proxy.
#[derive(Clone)]
pub enum WindowsProxyTls {
    /// Use the shared default roots and custom CA configuration.
    Default,
    /// Reuse the caller's TLS configuration.
    Custom(Arc<ClientConfig>),
}

pub(crate) fn can_authenticate_route(target: &str, route: &OutboundProxyRoute) -> bool {
    matches!(route, OutboundProxyRoute::Proxy { url, .. }
        if Url::parse(target).is_ok_and(|target| target.scheme() == "https")
            && parse_proxy_url(url).is_ok())
}

fn parse_proxy_url(proxy_url: &str) -> io::Result<Url> {
    let proxy = Url::parse(proxy_url).map_err(|_| invalid_proxy())?;
    if !matches!(proxy.scheme(), "http" | "https")
        || !proxy.username().is_empty()
        || proxy.password().is_some()
    {
        return Err(invalid_proxy());
    }
    Ok(proxy)
}

/// Opens a new connection to the selected proxy and authenticates a CONNECT tunnel.
/// Call only after the proxy rejects a CONNECT request with 407.
pub async fn connect_windows_proxy(
    proxy_url: &str,
    target: &Url,
    proxy_tls: WindowsProxyTls,
) -> io::Result<impl AsyncRead + AsyncWrite + Unpin + Send + use<>> {
    let proxy = parse_proxy_url(proxy_url)?;
    let proxy_host = proxy.host_str().ok_or_else(invalid_proxy)?;
    let target_host = target.host_str().ok_or_else(invalid_proxy)?;
    let target_port = target.port_or_known_default().ok_or_else(invalid_proxy)?;
    let listener = Arc::new(TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?);
    let local_port = listener.local_addr()?.port();

    // Keep the real hostname for Windows' HTTP/<proxy host> identity. Resolve it to our
    // local socket for this handle only; Codex connects to the original proxy URL below.
    let mut dns = List::new();
    dns.append(&format!("{proxy_host}:{local_port}:127.0.0.1"))
        .map_err(io::Error::other)?;
    let mut auth = Auth::new();
    // Curl uses the signed-in user for Negotiate and separately advertised NTLM.
    // libcurl 8.22 removes NTLM from Windows Negotiate; check this behavior on upgrades.
    auth.gssnegotiate(true).ntlm(true);
    let expected_peer = Arc::new(Mutex::new(None));
    let (cancellation, _authentication_alive) = watch::channel(());
    let mut curl = Easy2::new(CurlSocket {
        readiness: None,
        expected_peer: Arc::clone(&expected_peer),
        cancellation,
    });
    curl.proxy(&format!("http://{proxy_host}:{local_port}"))
        .map_err(io::Error::other)?;
    curl.resolve(dns).map_err(io::Error::other)?;
    curl.noproxy("").map_err(io::Error::other)?;
    curl.url(&format!("http://{target_host}:{target_port}/"))
        .map_err(io::Error::other)?;
    curl.http_proxy_tunnel(true).map_err(io::Error::other)?;
    curl.connect_only(true).map_err(io::Error::other)?;
    curl.proxy_username("").map_err(io::Error::other)?;
    curl.proxy_password("").map_err(io::Error::other)?;
    curl.proxy_auth(&auth).map_err(io::Error::other)?;
    curl.progress(true).map_err(io::Error::other)?;

    let listener_guard = Arc::clone(&listener);
    let mut authentication = tokio::task::spawn_blocking(move || {
        // Keep the port reserved until curl stops, even if the caller cancels.
        let _listener_guard = listener_guard;
        curl.perform().map_err(io::Error::other)?;
        Ok::<_, io::Error>(curl)
    });
    // Curl can reconnect. Keep the proxy connection that completed authentication.
    let (authenticated, _) = watch::channel(None);
    // Owning the jobs here also closes the real and local sockets if the caller cancels.
    let mut connections = JoinSet::new();
    let mut curl = loop {
        tokio::select! {
            result = &mut authentication => break result.map_err(io::Error::other)??,
            accepted = listener.accept() => {
                let (local, peer) = accepted?;
                {
                    let mut expected = expected_peer
                        .lock()
                        .map_err(|_| io::Error::other("proxy socket state is poisoned"))?;
                    if *expected != Some(peer) {
                        continue;
                    }
                    *expected = None;
                }
                // Curl has moved to this socket, so earlier attempts cannot authenticate.
                connections.shutdown().await;
                let proxy = proxy.clone();
                let proxy_tls = proxy_tls.clone();
                let mut done = authenticated.subscribe();
                connections.spawn(async move {
                    let upstream = connect_to_proxy(&proxy, proxy_tls).await?;
                    let (mut from_curl, mut to_curl) = local.into_split();
                    let (from_proxy, mut to_proxy) = tokio::io::split(upstream);
                    let mut from_proxy = BufReader::new(from_proxy);
                    tokio::select! {
                        changed = done.changed() => {
                            changed.map_err(io::Error::other)?;
                            let selected = *done.borrow_and_update() == Some(peer.port());
                            if selected {
                                // Do not shut down the real writer. EOF on this side lets
                                // curl return all bytes already sent to its local socket.
                                to_curl.shutdown().await?;
                                return Ok(Some((from_proxy, to_proxy)));
                            }
                        }
                        result = async {
                            tokio::try_join!(
                                // copy flushes before waiting for more input, so Rustls
                                // sends CONNECT before curl waits for the proxy response.
                                tokio::io::copy(&mut from_curl, &mut to_proxy),
                                async {
                                    tokio::io::copy_buf(&mut from_proxy, &mut to_curl).await?;
                                    to_curl.shutdown().await
                                },
                            )
                        } => { result?; }
                    }
                    Ok::<_, io::Error>(None)
                });
            }
            _ = connections.join_next(), if !connections.is_empty() => {}
        }
    };
    authenticated.send_replace(Some(curl.local_port().map_err(io::Error::other)?));
    let (from_proxy, to_proxy) = loop {
        match connections.join_next().await {
            Some(Ok(Ok(Some(halves)))) => break halves,
            Some(_) => {}
            None => return Err(io::Error::other("authenticated proxy connection ended")),
        }
    };
    let readiness = curl
        .get_mut()
        .readiness
        .take()
        .ok_or_else(|| io::Error::other("curl did not open a proxy connection"))?;
    let readiness = TcpStream::from_std(readiness)?;
    let mut prefix = Vec::new();
    let mut buffer = [0; 8192];
    loop {
        // curl can hold bytes even while the socket isn't readable.
        let read = match curl.recv(&mut buffer) {
            Ok(read) => read,
            Err(error) if error.is_again() => {
                readiness
                    .async_io(Interest::READABLE, || {
                        curl.recv(&mut buffer).map_err(|error| {
                            if error.is_again() {
                                io::ErrorKind::WouldBlock.into()
                            } else {
                                io::Error::other(error)
                            }
                        })
                    })
                    .await?
            }
            Err(error) => return Err(io::Error::other(error)),
        };
        if read == 0 {
            break;
        }
        prefix.extend_from_slice(&buffer[..read]);
    }
    Ok(tokio::io::join(
        Cursor::new(prefix).chain(from_proxy),
        to_proxy,
    ))
}

async fn connect_to_proxy(proxy: &Url, proxy_tls: WindowsProxyTls) -> io::Result<Box<dyn ProxyIo>> {
    let host = match proxy.host().ok_or_else(invalid_proxy)? {
        Host::Domain(domain) => domain.to_owned(),
        Host::Ipv4(address) => address.to_string(),
        Host::Ipv6(address) => address.to_string(),
    };
    let port = proxy.port_or_known_default().ok_or_else(invalid_proxy)?;
    let tcp = TcpStream::connect((host.as_str(), port)).await?;
    tcp.set_nodelay(true)?;
    if proxy.scheme() == "https" {
        let config = match proxy_tls {
            WindowsProxyTls::Default => {
                build_rustls_client_config_with_custom_ca().map_err(io::Error::other)?
            }
            WindowsProxyTls::Custom(config) => config,
        };
        let server_name = ServerName::try_from(host).map_err(|_| invalid_proxy())?;
        Ok(Box::new(
            TlsConnector::from(config).connect(server_name, tcp).await?,
        ))
    } else {
        Ok(Box::new(tcp))
    }
}

fn invalid_proxy() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "invalid Windows authentication proxy route",
    )
}
