use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use bytes::Bytes;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

use super::ClosedNoiseVirtualStream;
use super::spawn_noise_virtual_stream;
use crate::ExecServerTelemetry;
use crate::forward::Forwarder;
use crate::noise_channel::InitiatorHandshake;
use crate::noise_channel::NoiseChannelIdentity;
use crate::noise_channel::PendingResponderHandshake;
use crate::noise_relay::message_framing::MessageDecoder;
use crate::noise_relay::message_framing::frame_message;
use crate::relay::decode_relay_message_frame;
use crate::relay_proto::RelayData;

const TEST_TIMEOUT: Duration = Duration::from_secs(5);

#[tokio::test]
async fn forwards_opaque_noise_payloads_and_drains_before_closing() -> Result<()> {
    let executor_identity = NoiseChannelIdentity::generate()?;
    let harness_identity = NoiseChannelIdentity::generate()?;
    let prologue = b"forwarding-test";
    let (initiator, request) = InitiatorHandshake::start(
        &harness_identity,
        &executor_identity.public_key(),
        prologue,
        b"authorization",
    )?;
    let pending = PendingResponderHandshake::read_request(&executor_identity, prologue, &request)?;
    let (executor_transport, response) = pending.complete()?;
    let mut harness_transport = initiator.finish(&response)?;

    let destination = TcpListener::bind("127.0.0.1:0").await?;
    let forwarder = Forwarder::new(
        format!("ws://{}", destination.local_addr()?),
        &HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ExecServerTelemetry::default(),
    )?;
    // A single queued record makes the final response exercise writer draining.
    let (physical_outgoing_tx, mut physical_outgoing_rx) = mpsc::channel(1);
    let (closed_stream_tx, mut closed_stream_rx) = mpsc::channel(2);
    let mut stream = spawn_noise_virtual_stream(
        "stream-1".to_string(),
        /*instance_id*/ 7,
        forwarder,
        physical_outgoing_tx,
        closed_stream_tx,
        executor_transport,
    );
    let (socket, _) = timeout(TEST_TIMEOUT, destination.accept()).await??;
    let mut socket = timeout(TEST_TIMEOUT, accept_async(socket)).await??;

    let request = Bytes::from_static(b"not JSON\x00\xff");
    stream.receive_data(RelayData {
        seq: 0,
        segment_index: 0,
        segment_count: 1,
        payload: harness_transport.encrypt(&frame_message(&request)?)?,
    })?;
    let forwarded = timeout(TEST_TIMEOUT, socket.next())
        .await?
        .context("forwarded request")??;
    assert_eq!(forwarded.into_data(), request);

    let response = Bytes::from(vec![0xff; 128 * 1024]);
    socket.send(Message::Binary(response.clone())).await?;
    socket.close(None).await?;

    let mut decoder = MessageDecoder::default();
    let mut messages = Vec::new();
    let mut next_seq = 0;
    let closed = timeout(TEST_TIMEOUT, async {
        loop {
            tokio::select! {
                biased;
                Some(encoded) = physical_outgoing_rx.recv() => {
                    let frame = decode_relay_message_frame(&encoded)?;
                    assert_eq!(frame.stream_id, "stream-1");
                    let data = frame.into_data()?;
                    assert_eq!(data.seq, next_seq);
                    next_seq += 1;
                    messages.extend(decoder.push(&harness_transport.decrypt(&data.payload)?)?);
                }
                closed = closed_stream_rx.recv() => {
                    break closed.context("stream close notification");
                }
            }
        }
    })
    .await??;
    assert!(next_seq > 1);
    assert_eq!(messages, vec![response]);
    assert!(matches!(
        closed,
        ClosedNoiseVirtualStream { stream_id, instance_id: 7 } if stream_id == "stream-1"
    ));
    Ok(())
}
