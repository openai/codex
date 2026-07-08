use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use tokio::io::copy_bidirectional;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::timeout;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) struct DisconnectableWebSocketProxy {
    websocket_url: String,
    pause_tx: Option<oneshot::Sender<()>>,
    blocked_connection_rx: Option<oneshot::Receiver<()>>,
    resume_tx: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

impl DisconnectableWebSocketProxy {
    pub(super) async fn start(upstream_url: &str) -> Result<Self> {
        let upstream = upstream_url
            .strip_prefix("ws://")
            .context("exec-server websocket URL must use ws://")?
            .to_string();
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let websocket_url = format!("ws://{}", listener.local_addr()?);
        let (pause_tx, pause_rx) = oneshot::channel();
        let (blocked_connection_tx, blocked_connection_rx) = oneshot::channel();
        let (resume_tx, resume_rx) = oneshot::channel();
        let task = tokio::spawn(run_proxy(
            listener,
            upstream,
            pause_rx,
            blocked_connection_tx,
            resume_rx,
        ));
        Ok(Self {
            websocket_url,
            pause_tx: Some(pause_tx),
            blocked_connection_rx: Some(blocked_connection_rx),
            resume_tx: Some(resume_tx),
            task,
        })
    }

    pub(super) fn websocket_url(&self) -> &str {
        &self.websocket_url
    }

    pub(super) async fn pause_and_disconnect(&mut self) -> Result<()> {
        self.pause_tx
            .take()
            .context("disconnectable websocket proxy is already paused")?
            .send(())
            .map_err(|_| anyhow::anyhow!("disconnectable websocket proxy stopped"))?;
        let blocked_connection_rx = self
            .blocked_connection_rx
            .take()
            .context("disconnectable websocket proxy is already paused")?;
        timeout(CONNECT_TIMEOUT, blocked_connection_rx)
            .await
            .context("timed out waiting for client reconnect attempt")?
            .context("disconnectable websocket proxy stopped")?;
        Ok(())
    }

    pub(super) fn resume(&mut self) -> Result<()> {
        self.resume_tx
            .take()
            .context("disconnectable websocket proxy is already resumed")?
            .send(())
            .map_err(|_| anyhow::anyhow!("disconnectable websocket proxy stopped"))?;
        Ok(())
    }
}

impl Drop for DisconnectableWebSocketProxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn run_proxy(
    listener: TcpListener,
    upstream: String,
    pause_rx: oneshot::Receiver<()>,
    blocked_connection_tx: oneshot::Sender<()>,
    mut resume_rx: oneshot::Receiver<()>,
) {
    let Ok((mut downstream, _)) = listener.accept().await else {
        return;
    };
    let Ok(mut upstream_stream) = TcpStream::connect(&upstream).await else {
        return;
    };
    tokio::select! {
        _ = copy_bidirectional(&mut downstream, &mut upstream_stream) => return,
        _ = pause_rx => {}
    }
    drop(downstream);
    drop(upstream_stream);

    let mut blocked_connection_tx = Some(blocked_connection_tx);
    loop {
        tokio::select! {
            _ = &mut resume_rx => break,
            accepted = listener.accept() => {
                let Ok((blocked, _)) = accepted else {
                    return;
                };
                drop(blocked);
                if let Some(blocked_connection_tx) = blocked_connection_tx.take() {
                    let _ = blocked_connection_tx.send(());
                }
            }
        }
    }

    loop {
        let Ok((mut downstream, _)) = listener.accept().await else {
            return;
        };
        let Ok(mut upstream_stream) = TcpStream::connect(&upstream).await else {
            continue;
        };
        let _ = copy_bidirectional(&mut downstream, &mut upstream_stream).await;
    }
}
