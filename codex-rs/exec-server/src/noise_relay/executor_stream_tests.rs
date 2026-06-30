use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::JSONRPCResponse;
use codex_exec_server_protocol::RequestId;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::Instrument;
use tracing::instrument::WithSubscriber;
use tracing_subscriber::prelude::*;

use super::ClosedNoiseVirtualStream;
use super::send_outbound_message;
use super::spawn_noise_virtual_stream;
use crate::ExecServerRuntimePaths;
use crate::connection::CHANNEL_CAPACITY;
use crate::noise_channel::InitiatorHandshake;
use crate::noise_channel::NoiseChannelIdentity;
use crate::noise_channel::PendingResponderHandshake;
use crate::noise_relay::message_framing::NOISE_RECORD_PLAINTEXT_LEN;
use crate::noise_relay::message_framing::frame_jsonrpc_message;
use crate::relay_proto::RelayData;
use crate::server::ConnectionProcessor;

#[tokio::test(flavor = "current_thread")]
async fn outbound_records_share_one_message_trace_context() -> Result<()> {
    let executor_identity = NoiseChannelIdentity::generate()?;
    let harness_identity = NoiseChannelIdentity::generate()?;
    let (initiator, request) = InitiatorHandshake::start(
        &harness_identity,
        &executor_identity.public_key(),
        b"test-prologue",
        b"authorization",
    )?;
    let pending =
        PendingResponderHandshake::read_request(&executor_identity, b"test-prologue", &request)?;
    let (executor_transport, response) = pending.complete()?;
    let _harness_transport = initiator.finish(&response)?;
    let executor_transport = std::sync::Mutex::new(executor_transport);
    let (physical_outgoing_tx, mut physical_outgoing_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let message = JSONRPCMessage::Response(JSONRPCResponse {
        id: RequestId::Integer(1),
        result: serde_json::Value::String("x".repeat(NOISE_RECORD_PLAINTEXT_LEN * 2)),
    });

    let provider = SdkTracerProvider::builder().build();
    let subscriber = tracing_subscriber::registry()
        .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("exec-server-test")));
    let mut next_seq = 0;
    async {
        tracing::callsite::rebuild_interest_cache();
        send_outbound_message(
            &physical_outgoing_tx,
            &executor_transport,
            "stream-1",
            &mut next_seq,
            &message,
        )
        .instrument(tracing::info_span!("outbound-message"))
        .await
    }
    .with_subscriber(subscriber)
    .await?;

    let mut frames = Vec::new();
    while let Ok(frame) = physical_outgoing_rx.try_recv() {
        frames.push(frame);
    }
    assert!(frames.len() > 1, "expected multiple physical records");
    let first_trace = frames[0].trace.as_ref().expect("first record trace");
    assert!(frames.iter().skip(1).all(|frame| {
        Arc::ptr_eq(
            first_trace,
            frame.trace.as_ref().expect("subsequent record trace"),
        )
    }));
    Ok(())
}

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
    stream.receive_data(
        RelayData {
            seq: 0,
            segment_index: 0,
            segment_count: 1,
            payload: ciphertext,
        },
        std::time::Instant::now(),
    )?;

    assert!(matches!(
        timeout(Duration::from_secs(1), closed_stream_rx.recv()).await?,
        Some(ClosedNoiseVirtualStream {
            stream_id,
            instance_id: 7,
        }) if stream_id == "stream-1"
    ));
    Ok(())
}
