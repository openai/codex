use std::time::Duration;

use anyhow::Result;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::JSONRPCResponse;
use codex_exec_server_protocol::RequestId;
use tokio::sync::mpsc;
use tokio::time::timeout;

use super::ClosedNoiseVirtualStream;
use super::spawn_noise_virtual_stream;
use crate::ExecServerRuntimePaths;
use crate::connection::CHANNEL_CAPACITY;
use crate::noise_channel::InitiatorHandshake;
use crate::noise_channel::NoiseChannelIdentity;
use crate::noise_channel::PendingResponderHandshake;
use crate::noise_relay::message_framing::frame_jsonrpc_message;
use crate::relay_proto::RelayData;
use crate::server::ConnectionProcessor;

#[tokio::test]
async fn processor_exit_reports_closed_virtual_stream() -> Result<()> {
    let executor_identity = NoiseChannelIdentity::generate()?;
    let harness_identity = NoiseChannelIdentity::generate()?;
    let prologue = b"test-prologue";
    let (initiator, request) = InitiatorHandshake::start(
        &harness_identity,
        &executor_identity.public_key(),
        prologue,
        b"authorization",
    )?;
    let pending = PendingResponderHandshake::read_request(&executor_identity, prologue, &request)?;
    let (executor_transport, response) = pending.complete()?;
    let mut harness_transport = initiator.finish(&response)?;

    let (physical_outgoing_tx, _physical_outgoing_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let (closed_stream_tx, mut closed_stream_rx) = mpsc::channel(1);
    let mut stream = spawn_noise_virtual_stream(
        "stream-1".to_string(),
        /*instance_id*/ 7,
        ConnectionProcessor::new(ExecServerRuntimePaths::new(
            std::env::current_exe()?,
            /*codex_linux_sandbox_exe*/ None,
        )?),
        physical_outgoing_tx,
        closed_stream_tx,
        executor_transport,
    );

    let message = JSONRPCMessage::Response(JSONRPCResponse {
        id: RequestId::Integer(1),
        result: serde_json::Value::Null,
    });
    let ciphertext = harness_transport.encrypt(&frame_jsonrpc_message(&message)?)?;
    stream.receive_data(RelayData {
        seq: 0,
        segment_index: 0,
        segment_count: 1,
        payload: ciphertext,
    })?;

    assert!(matches!(
        timeout(Duration::from_secs(1), closed_stream_rx.recv()).await?,
        Some(ClosedNoiseVirtualStream {
            stream_id,
            instance_id: 7,
        }) if stream_id == "stream-1"
    ));
    Ok(())
}
