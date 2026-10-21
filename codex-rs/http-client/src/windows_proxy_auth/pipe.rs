//! Adapts an authenticated tunnel to reqwest's Windows named-pipe connector.
//! Each listener serves one proxy and destination; only an accepted connection can outlive it.

use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use reqwest::Url;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::net::windows::named_pipe::ServerOptions;
use tokio::task::JoinHandle;

use super::WindowsProxyTls;
use super::connect_windows_proxy;
use crate::HttpClientTlsConfig;

static NEXT_PIPE: AtomicU64 = AtomicU64::new(0);

pub(crate) struct ProxyPipe {
    path: PathBuf,
    listener: JoinHandle<()>,
}

impl ProxyPipe {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ProxyPipe {
    fn drop(&mut self) {
        self.listener.abort();
    }
}

/// Called only on a confirmed proxy CONNECT 407. A pipe cannot route an arbitrary URL:
/// reqwest sends TLS first, so the pipe must already know its proxy and destination.
pub(crate) async fn pipe_for(
    proxy_url: &str,
    target: &Url,
    tls: &HttpClientTlsConfig,
    connect_timeout: Option<Duration>,
) -> io::Result<ProxyPipe> {
    let proxy_tls = if Url::parse(proxy_url).map_err(io::Error::other)?.scheme() == "https" {
        tls.windows_proxy_tls()?
    } else {
        WindowsProxyTls::Default
    };
    // Surface auth failure before reqwest starts TLS on the pipe and obscures the error.
    let upstream =
        connect_with_timeout(proxy_url, target, proxy_tls.clone(), connect_timeout).await?;
    let id = NEXT_PIPE.fetch_add(1, Ordering::Relaxed);
    let path = PathBuf::from(format!(
        r"\\.\pipe\codex-proxy-auth-{}-{id}",
        std::process::id(),
    ));
    let first = ServerOptions::new()
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .create(&path)?;
    let proxy_url = proxy_url.to_owned();
    let target = target.clone();
    let listen_path = path.clone();
    let listener = tokio::spawn(async move {
        let mut next = first;
        let mut first_upstream = Some(upstream);
        while next.connect().await.is_ok() {
            // Always prepare the next instance before serving this connection.
            // Reqwest opens pipes without retrying ERROR_PIPE_BUSY.
            let mut accepted = next;
            let Ok(server) = ServerOptions::new()
                .reject_remote_clients(true)
                .create(&listen_path)
            else {
                break;
            };
            next = server;
            let proxy_url = proxy_url.clone();
            let target = target.clone();
            let proxy_tls = proxy_tls.clone();
            let connected = first_upstream.take();
            tokio::spawn(async move {
                let result = async {
                    let mut upstream = match connected {
                        Some(upstream) => upstream,
                        None => {
                            connect_with_timeout(&proxy_url, &target, proxy_tls, connect_timeout)
                                .await?
                        }
                    };
                    tokio::io::copy_bidirectional(&mut accepted, &mut upstream).await?;
                    Ok::<(), io::Error>(())
                }
                .await;
                if let Err(error) = result {
                    tracing::debug!(kind = ?error.kind(), "Windows proxy pipe ended");
                }
            });
        }
    });
    Ok(ProxyPipe { path, listener })
}

async fn connect_with_timeout(
    proxy_url: &str,
    target: &Url,
    proxy_tls: WindowsProxyTls,
    connect_timeout: Option<Duration>,
) -> io::Result<impl AsyncRead + AsyncWrite + Unpin + Send + use<>> {
    let connect = connect_windows_proxy(proxy_url, target, proxy_tls);
    match connect_timeout {
        Some(timeout) => tokio::time::timeout(timeout, connect).await.map_err(|_| {
            io::Error::new(io::ErrorKind::TimedOut, "proxy authentication timed out")
        })?,
        None => connect.await,
    }
}
