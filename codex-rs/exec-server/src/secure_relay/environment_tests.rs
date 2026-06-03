use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Result;
use futures::SinkExt;
use tokio::net::TcpListener;
use tokio::sync::Notify;
use tokio::time::timeout;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use super::HarnessKeyValidator;
use super::run_secure_multiplexed_environment;
use crate::ExecServerError;
use crate::ExecServerRuntimePaths;
use crate::relay::encode_relay_message_frame;
use crate::relay_proto::RelayMessageFrame;
use crate::secure_channel::InitiatorHandshake;
use crate::secure_channel::SecureChannelIdentity;
use crate::secure_channel::SecureChannelPublicKey;
use crate::secure_channel::secure_channel_prologue;
use crate::server::ConnectionProcessor;

const ENVIRONMENT_ID: &str = "environment-1";
const EXECUTOR_REGISTRATION_ID: &str = "registration-1";

#[derive(Clone)]
struct BlockingValidator {
    calls: Arc<AtomicUsize>,
    release: Arc<Notify>,
}

impl HarnessKeyValidator for BlockingValidator {
    fn validate_harness_key(
        &self,
        _harness_public_key: &SecureChannelPublicKey,
        _authorization: &str,
    ) -> impl std::future::Future<Output = Result<(), ExecServerError>> + Send {
        let calls = Arc::clone(&self.calls);
        let release = Arc::clone(&self.release);
        async move {
            calls.fetch_add(1, Ordering::SeqCst);
            release.notified().await;
            Ok(())
        }
    }
}

#[tokio::test]
async fn pending_harness_key_validation_does_not_block_new_handshakes() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let websocket_url = format!("ws://{}", listener.local_addr()?);
    let harness_connection = tokio::spawn(connect_async(websocket_url));
    let (socket, _peer_addr) = listener.accept().await?;
    let environment_websocket = accept_async(socket).await?;
    let (mut harness_websocket, _response) = harness_connection.await??;

    let environment_identity = SecureChannelIdentity::generate()?;
    let harness_identity = SecureChannelIdentity::generate()?;
    let calls = Arc::new(AtomicUsize::new(0));
    let environment_task = tokio::spawn(run_secure_multiplexed_environment(
        environment_websocket,
        ConnectionProcessor::new(ExecServerRuntimePaths::new(
            std::env::current_exe()?,
            /*codex_linux_sandbox_exe*/ None,
        )?),
        ENVIRONMENT_ID.to_string(),
        EXECUTOR_REGISTRATION_ID.to_string(),
        environment_identity.clone(),
        BlockingValidator {
            calls: Arc::clone(&calls),
            release: Arc::new(Notify::new()),
        },
    ));

    for stream_id in ["stream-1", "stream-2"] {
        let prologue =
            secure_channel_prologue(ENVIRONMENT_ID, EXECUTOR_REGISTRATION_ID, stream_id)?;
        let (_handshake, request) = InitiatorHandshake::start(
            &harness_identity,
            &environment_identity.public_key(),
            &prologue,
            b"authorization",
        )?;
        let frame = RelayMessageFrame::handshake(stream_id.to_string(), request);
        harness_websocket
            .send(Message::Binary(encode_relay_message_frame(&frame).into()))
            .await?;
    }

    timeout(Duration::from_secs(1), async {
        while calls.load(Ordering::SeqCst) != 2 {
            tokio::task::yield_now().await;
        }
    })
    .await?;

    harness_websocket.close(None).await?;
    timeout(Duration::from_secs(1), environment_task).await??;
    Ok(())
}
