use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Result;
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;

use super::ExecServerClient;
use crate::ExecServerError;
use crate::NoiseChannelIdentity;
use crate::NoiseChannelPublicKey;
use crate::NoiseRendezvousConnectBundle;
use crate::NoiseRendezvousConnectProvider;

struct SequenceNoiseConnectProvider {
    bundles: Mutex<VecDeque<NoiseRendezvousConnectBundle>>,
    returned_urls: Mutex<Vec<String>>,
}

impl SequenceNoiseConnectProvider {
    fn new(bundles: Vec<NoiseRendezvousConnectBundle>) -> Self {
        Self {
            bundles: Mutex::new(bundles.into()),
            returned_urls: Mutex::new(Vec::new()),
        }
    }

    fn returned_urls(&self) -> Vec<String> {
        self.returned_urls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl NoiseRendezvousConnectProvider for SequenceNoiseConnectProvider {
    fn connect_bundle(
        &self,
        _: NoiseChannelPublicKey,
    ) -> BoxFuture<'_, Result<NoiseRendezvousConnectBundle, ExecServerError>> {
        let result = self
            .bundles
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop_front()
            .ok_or_else(|| ExecServerError::Protocol("test Noise provider exhausted".to_string()));
        if let Ok(bundle) = &result {
            self.returned_urls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(bundle.websocket_url.clone());
        }
        Box::pin(async move { result })
    }
}

fn test_bundle(websocket_url: String) -> Result<NoiseRendezvousConnectBundle> {
    Ok(NoiseRendezvousConnectBundle {
        websocket_url,
        environment_id: "environment".to_string(),
        executor_registration_id: "registration".to_string(),
        executor_public_key: NoiseChannelIdentity::generate()?.public_key(),
        harness_key_authorization: "authorization".to_string(),
    })
}

#[tokio::test]
async fn initial_noise_connection_refreshes_bundle_after_unauthorized_handshake() -> Result<()> {
    let unauthorized_listener = TcpListener::bind("127.0.0.1:0").await?;
    let unauthorized_url = format!("ws://{}", unauthorized_listener.local_addr()?);
    let unauthorized_server = tokio::spawn(async move {
        let (mut socket, _) = unauthorized_listener.accept().await?;
        let mut request = [0_u8; 4096];
        let _ = socket.read(&mut request).await?;
        socket
            .write_all(
                b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .await?;
        socket.shutdown().await?;
        anyhow::Ok(())
    });
    let accepted_listener = TcpListener::bind("127.0.0.1:0").await?;
    let accepted_url = format!("ws://{}", accepted_listener.local_addr()?);
    let accepted_server = tokio::spawn(async move {
        let (socket, _) = accepted_listener.accept().await?;
        let _websocket = accept_async(socket).await?;
        anyhow::Ok(())
    });
    let sequence = Arc::new(SequenceNoiseConnectProvider::new(vec![
        test_bundle(unauthorized_url.clone())?,
        test_bundle(accepted_url.clone())?,
    ]));
    let provider: Arc<dyn NoiseRendezvousConnectProvider> = sequence.clone();
    let identity = NoiseChannelIdentity::generate()?;

    let _connection =
        ExecServerClient::open_initial_noise_rendezvous_connection(&provider, &identity).await?;

    assert_eq!(
        sequence.returned_urls(),
        vec![unauthorized_url, accepted_url]
    );
    unauthorized_server.await??;
    accepted_server.await??;
    Ok(())
}
