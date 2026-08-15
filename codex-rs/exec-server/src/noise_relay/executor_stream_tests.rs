use std::time::Duration;

use anyhow::Result;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::JSONRPCRequest;
use codex_exec_server_protocol::JSONRPCResponse;
use codex_exec_server_protocol::RequestId;
use codex_protocol::protocol::W3cTraceContext;
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
use crate::relay_proto::RelayMessageFrame;
use crate::server::ConnectionProcessor;

#[test]
fn executor_requests_attach_trace_context_only_to_the_first_noise_record() {
    let traceparent = "00-00000000000000000000000000000001-0000000000000002-01";
    let tracestate = "dd=s:1";
    let owned_traceparent = traceparent.to_string();
    let owned_tracestate = tracestate.to_string();
    let traceparent_ptr = owned_traceparent.as_ptr();
    let tracestate_ptr = owned_tracestate.as_ptr();
    let mut request = JSONRPCRequest {
        id: RequestId::Integer(1),
        method: "approval/request".to_string(),
        params: None,
        trace: Some(W3cTraceContext {
            traceparent: Some(owned_traceparent),
            tracestate: Some(owned_tracestate),
        }),
    };

    let first = RelayMessageFrame::data(
        "stream-1".to_string(),
        /*seq*/ 0,
        vec![1],
        request.trace.take(),
    );
    assert_eq!(first.traceparent.as_deref(), Some(traceparent));
    assert_eq!(first.tracestate.as_deref(), Some(tracestate));
    assert_eq!(
        first.traceparent.as_ref().unwrap().as_ptr(),
        traceparent_ptr
    );
    assert_eq!(first.tracestate.as_ref().unwrap().as_ptr(), tracestate_ptr);

    let second = RelayMessageFrame::data(
        "stream-1".to_string(),
        /*seq*/ 1,
        vec![2],
        request.trace.take(),
    );
    assert!(second.traceparent.is_none());
    assert!(second.tracestate.is_none());

    let response = RelayMessageFrame::data(
        "stream-1".to_string(),
        /*seq*/ 2,
        vec![3],
        /*trace*/ None,
    );
    assert!(response.traceparent.is_none());
    assert!(response.tracestate.is_none());
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
