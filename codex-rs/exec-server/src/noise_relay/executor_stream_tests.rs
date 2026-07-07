use std::sync::atomic::Ordering;
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
use crate::noise_channel::NoiseTransport;
use crate::noise_channel::PendingResponderHandshake;
use crate::noise_relay::message_framing::frame_jsonrpc_message;
use crate::relay::RelayFrameBodyKind;
use crate::relay::decode_relay_message_frame;
use crate::relay_proto::RelayData;
use crate::server::ConnectionProcessor;

#[tokio::test]
async fn processor_exit_reports_closed_virtual_stream() -> Result<()> {
    let (executor_transport, mut harness_transport) = completed_handshake()?;

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
        seq: 1,
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

#[tokio::test]
async fn full_physical_queue_defers_ack_without_resetting_stream() -> Result<()> {
    let (executor_transport, mut harness_transport) = completed_handshake()?;
    let (physical_outgoing_tx, mut physical_outgoing_rx) = mpsc::channel(1);
    physical_outgoing_tx.send(vec![0x5a]).await?;
    let (closed_stream_tx, _closed_stream_rx) = mpsc::channel(1);
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
    let framed = frame_jsonrpc_message(&message)?;
    let ciphertext = harness_transport.encrypt(&framed[..1])?;
    stream.receive_data(RelayData {
        seq: 1,
        segment_index: 0,
        segment_count: 1,
        payload: ciphertext,
    })?;

    assert_eq!(stream.inbound_ack.load(Ordering::Relaxed), 1);
    assert_eq!(physical_outgoing_rx.try_recv()?, vec![0x5a]);
    let ack = timeout(Duration::from_secs(1), physical_outgoing_rx.recv())
        .await?
        .expect("virtual stream writer should send the deferred ack");
    let ack = decode_relay_message_frame(&ack)?;
    assert_eq!(ack.validate()?, RelayFrameBodyKind::Ack);
    assert_eq!(ack.ack, 1);
    Ok(())
}

fn completed_handshake() -> Result<(NoiseTransport, NoiseTransport)> {
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
    let harness_transport = initiator.finish(&response)?;
    Ok((executor_transport, harness_transport))
}
