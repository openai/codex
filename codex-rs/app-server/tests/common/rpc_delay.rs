use std::io;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::task::JoinSet;
use tokio::time::Instant;
use tokio::time::sleep_until;
use tokio_util::task::AbortOnDropHandle;
use url::Host;
use url::Url;

const FORWARD_BUFFER_BYTES: usize = 64 * 1024;
const FORWARD_QUEUE_CHUNKS: usize = 16;

pub(crate) struct WebsocketDelayInterposer {
    websocket_url: String,
    accept_task: JoinHandle<()>,
}

impl WebsocketDelayInterposer {
    pub(crate) async fn start(upstream_url: &str, added_delay: Duration) -> Result<Self> {
        let upstream = websocket_authority(upstream_url)?;
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .context("bind RPC delay interposer")?;
        let websocket_url = format!("ws://{}", listener.local_addr()?);
        let accept_task = tokio::spawn(async move {
            let mut connections = JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let Ok((downstream, _peer)) = accepted else {
                            break;
                        };
                        let upstream = upstream.clone();
                        connections.spawn(async move {
                            let Ok(upstream) = TcpStream::connect(upstream).await else {
                                return;
                            };
                            let _ = proxy_connection(downstream, upstream, added_delay).await;
                        });
                    }
                    _ = connections.join_next(), if !connections.is_empty() => {}
                }
            }
        });
        Ok(Self {
            websocket_url,
            accept_task,
        })
    }

    pub(crate) fn websocket_url(&self) -> &str {
        &self.websocket_url
    }
}

impl Drop for WebsocketDelayInterposer {
    fn drop(&mut self) {
        self.accept_task.abort();
    }
}

fn websocket_authority(websocket_url: &str) -> Result<String> {
    let websocket_url = Url::parse(websocket_url).context("parse RPC delay upstream URL")?;
    if websocket_url.scheme() != "ws" {
        return Err(anyhow!("RPC delay requires a ws:// exec-server URL"));
    }
    let host = websocket_url
        .host()
        .ok_or_else(|| anyhow!("RPC delay exec-server URL has no host"))?;
    let port = websocket_url
        .port_or_known_default()
        .ok_or_else(|| anyhow!("RPC delay exec-server URL has no port"))?;
    let host = match host {
        Host::Domain(host) => host.to_string(),
        Host::Ipv4(host) => host.to_string(),
        Host::Ipv6(host) => format!("[{host}]"),
    };
    Ok(format!("{host}:{port}"))
}

async fn proxy_connection(
    downstream: TcpStream,
    upstream: TcpStream,
    added_delay: Duration,
) -> io::Result<()> {
    let (downstream_read, downstream_write) = downstream.into_split();
    let (upstream_read, upstream_write) = upstream.into_split();
    let client_to_server = forward_direction(downstream_read, upstream_write, added_delay);
    let server_to_client = forward_direction(upstream_read, downstream_write, added_delay);
    tokio::try_join!(client_to_server, server_to_client)?;
    Ok(())
}

async fn forward_direction<R, W>(
    mut reader: R,
    mut writer: W,
    added_delay: Duration,
) -> io::Result<()>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    // tokio::io::copy would wait before reading the next chunk, turning a
    // fixed propagation delay into a bandwidth limit. Timestamping reads into
    // a bounded queue lets close-together chunks emerge close together after
    // the same delay while still applying backpressure.
    let (tx, mut rx) = mpsc::channel::<DelayedChunk>(FORWARD_QUEUE_CHUNKS);
    let reader_task = AbortOnDropHandle::new(tokio::spawn(async move {
        loop {
            let mut bytes = vec![0; FORWARD_BUFFER_BYTES];
            let read = reader.read(&mut bytes).await?;
            if read == 0 {
                break;
            }
            bytes.truncate(read);
            let chunk = DelayedChunk {
                deliver_at: Instant::now() + added_delay,
                bytes,
            };
            if tx.send(chunk).await.is_err() {
                break;
            }
        }
        Ok::<(), io::Error>(())
    }));

    while let Some(chunk) = rx.recv().await {
        sleep_until(chunk.deliver_at).await;
        writer.write_all(&chunk.bytes).await?;
    }
    writer.shutdown().await?;
    reader_task
        .await
        .map_err(|err| io::Error::other(format!("RPC delay reader task failed: {err}")))??;
    Ok(())
}

struct DelayedChunk {
    deliver_at: Instant,
    bytes: Vec<u8>,
}

#[cfg(test)]
#[path = "rpc_delay_tests.rs"]
mod tests;
