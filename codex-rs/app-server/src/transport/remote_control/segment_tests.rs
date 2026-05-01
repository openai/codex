use super::protocol::ClientEnvelope;
use super::protocol::ClientEvent;
use super::protocol::ClientId;
use super::protocol::ServerEnvelope;
use super::protocol::ServerEvent;
use super::protocol::StreamId;
use super::segment::ClientSegmentObservation;
use super::segment::ClientSegmentReassembler;
use super::segment::REMOTE_CONTROL_SEGMENT_MAX_BYTES;
use super::segment::split_server_envelope_for_transport;
use crate::outgoing_message::OutgoingMessage;
use base64::Engine;
use codex_app_server_protocol::ConfigWarningNotification;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::ServerNotification;
use pretty_assertions::assert_eq;

#[test]
fn reassembles_client_message_chunks() {
    let message = JSONRPCMessage::Notification(JSONRPCNotification {
        method: "initialized".to_string(),
        params: None,
    });
    let raw = serde_json::to_vec(&message).expect("message should serialize");
    let split = raw.len() / 2;
    let client_id = ClientId("client-1".to_string());
    let stream_id = Some(StreamId("stream-1".to_string()));
    let mut reassembler = ClientSegmentReassembler::default();

    assert!(matches!(
        reassembler.observe(chunk_envelope(
            client_id.clone(),
            stream_id.clone(),
            /*seq_id*/ 7,
            /*segment_id*/ 0,
            /*segment_count*/ 2,
            raw.len(),
            &raw[..split],
        )),
        ClientSegmentObservation::Pending
    ));
    let reassembled = match reassembler.observe(chunk_envelope(
        client_id.clone(),
        stream_id,
        /*seq_id*/ 7,
        /*segment_id*/ 1,
        /*segment_count*/ 2,
        raw.len(),
        &raw[split..],
    )) {
        ClientSegmentObservation::Forward(reassembled) => *reassembled,
        ClientSegmentObservation::Pending | ClientSegmentObservation::Dropped => {
            panic!("message should reassemble")
        }
    };
    assert_eq!(reassembled.client_id, client_id);
    assert_eq!(
        reassembled.stream_id,
        Some(StreamId("stream-1".to_string()))
    );
    assert_eq!(reassembled.seq_id, Some(7));
    assert_eq!(reassembled.cursor, None);
    match reassembled.event {
        ClientEvent::ClientMessage {
            message: reassembled_message,
        } => assert_eq!(reassembled_message, message),
        other => panic!("expected client message, got {other:?}"),
    }
}

#[test]
fn splits_large_server_messages_into_wire_chunks() {
    let envelope = ServerEnvelope {
        event: ServerEvent::ServerMessage {
            message: Box::new(OutgoingMessage::AppServerNotification(
                ServerNotification::ConfigWarning(ConfigWarningNotification {
                    summary: "x".repeat(REMOTE_CONTROL_SEGMENT_MAX_BYTES),
                    details: None,
                    path: None,
                    range: None,
                }),
            )),
        },
        client_id: ClientId("client-1".to_string()),
        stream_id: StreamId("stream-1".to_string()),
        seq_id: 9,
    };

    let segments = split_server_envelope_for_transport(envelope).expect("split should succeed");

    assert!(segments.len() > 1);
    assert!(
        segments
            .iter()
            .all(|segment| matches!(segment.event, ServerEvent::ServerMessageChunk { .. }))
    );
    assert!(segments.iter().all(|segment| segment.seq_id == 9));
    assert!(segments.iter().all(|segment| {
        serde_json::to_vec(segment)
            .expect("segment should serialize")
            .len()
            <= REMOTE_CONTROL_SEGMENT_MAX_BYTES
    }));
}

#[test]
fn invalidates_incomplete_stream_assemblies() {
    let message = JSONRPCMessage::Notification(JSONRPCNotification {
        method: "initialized".to_string(),
        params: None,
    });
    let raw = serde_json::to_vec(&message).expect("message should serialize");
    let split = raw.len() / 2;
    let client_id = ClientId("client-1".to_string());
    let stream_id = StreamId("stream-1".to_string());
    let mut reassembler = ClientSegmentReassembler::default();

    assert!(matches!(
        reassembler.observe(chunk_envelope(
            client_id.clone(),
            Some(stream_id.clone()),
            /*seq_id*/ 7,
            /*segment_id*/ 0,
            /*segment_count*/ 2,
            raw.len(),
            &raw[..split],
        )),
        ClientSegmentObservation::Pending
    ));
    reassembler.invalidate_stream(&client_id, &stream_id);
    assert!(matches!(
        reassembler.observe(chunk_envelope(
            client_id,
            Some(stream_id),
            /*seq_id*/ 7,
            /*segment_id*/ 1,
            /*segment_count*/ 2,
            raw.len(),
            &raw[split..],
        )),
        ClientSegmentObservation::Dropped
    ));
}

#[test]
fn resets_incomplete_client_assembly_when_stream_changes() {
    let message = JSONRPCMessage::Notification(JSONRPCNotification {
        method: "initialized".to_string(),
        params: None,
    });
    let raw = serde_json::to_vec(&message).expect("message should serialize");
    let split = raw.len() / 2;
    let client_id = ClientId("client-1".to_string());
    let first_stream_id = StreamId("stream-1".to_string());
    let second_stream_id = StreamId("stream-2".to_string());
    let mut reassembler = ClientSegmentReassembler::default();

    assert!(matches!(
        reassembler.observe(chunk_envelope(
            client_id.clone(),
            Some(first_stream_id.clone()),
            /*seq_id*/ 7,
            /*segment_id*/ 0,
            /*segment_count*/ 2,
            raw.len(),
            &raw[..split],
        )),
        ClientSegmentObservation::Pending
    ));
    assert!(matches!(
        reassembler.observe(chunk_envelope(
            client_id.clone(),
            Some(second_stream_id.clone()),
            /*seq_id*/ 8,
            /*segment_id*/ 0,
            /*segment_count*/ 2,
            raw.len(),
            &raw[..split],
        )),
        ClientSegmentObservation::Pending
    ));
    let reassembled = match reassembler.observe(chunk_envelope(
        client_id.clone(),
        Some(second_stream_id),
        /*seq_id*/ 8,
        /*segment_id*/ 1,
        /*segment_count*/ 2,
        raw.len(),
        &raw[split..],
    )) {
        ClientSegmentObservation::Forward(reassembled) => *reassembled,
        ClientSegmentObservation::Pending | ClientSegmentObservation::Dropped => {
            panic!("replacement stream should reassemble")
        }
    };
    assert_eq!(
        reassembled.stream_id,
        Some(StreamId("stream-2".to_string()))
    );
    assert!(matches!(
        reassembler.observe(chunk_envelope(
            client_id,
            Some(first_stream_id),
            /*seq_id*/ 7,
            /*segment_id*/ 1,
            /*segment_count*/ 2,
            raw.len(),
            &raw[split..],
        )),
        ClientSegmentObservation::Dropped
    ));
}

#[test]
fn ignores_stale_chunks_without_dropping_newer_assembly() {
    let message = JSONRPCMessage::Notification(JSONRPCNotification {
        method: "initialized".to_string(),
        params: None,
    });
    let raw = serde_json::to_vec(&message).expect("message should serialize");
    let split = raw.len() / 2;
    let client_id = ClientId("client-1".to_string());
    let stream_id = Some(StreamId("stream-1".to_string()));
    let mut reassembler = ClientSegmentReassembler::default();

    assert!(matches!(
        reassembler.observe(chunk_envelope(
            client_id.clone(),
            stream_id.clone(),
            /*seq_id*/ 8,
            /*segment_id*/ 0,
            /*segment_count*/ 2,
            raw.len(),
            &raw[..split],
        )),
        ClientSegmentObservation::Pending
    ));
    assert!(matches!(
        reassembler.observe(chunk_envelope(
            client_id.clone(),
            stream_id.clone(),
            /*seq_id*/ 7,
            /*segment_id*/ 0,
            /*segment_count*/ 2,
            raw.len(),
            &raw[..split],
        )),
        ClientSegmentObservation::Dropped
    ));
    assert!(matches!(
        reassembler.observe(chunk_envelope(
            client_id,
            stream_id,
            /*seq_id*/ 8,
            /*segment_id*/ 1,
            /*segment_count*/ 2,
            raw.len(),
            &raw[split..],
        )),
        ClientSegmentObservation::Forward(_)
    ));
}

#[test]
fn ignores_invalid_stale_chunks_without_dropping_newer_assembly() {
    let message = JSONRPCMessage::Notification(JSONRPCNotification {
        method: "initialized".to_string(),
        params: None,
    });
    let raw = serde_json::to_vec(&message).expect("message should serialize");
    let split = raw.len() / 2;
    let client_id = ClientId("client-1".to_string());
    let stream_id = Some(StreamId("stream-1".to_string()));
    let mut reassembler = ClientSegmentReassembler::default();

    assert!(matches!(
        reassembler.observe(chunk_envelope(
            client_id.clone(),
            stream_id.clone(),
            /*seq_id*/ 8,
            /*segment_id*/ 0,
            /*segment_count*/ 2,
            raw.len(),
            &raw[..split],
        )),
        ClientSegmentObservation::Pending
    ));
    assert!(matches!(
        reassembler.observe(chunk_envelope(
            client_id.clone(),
            stream_id.clone(),
            /*seq_id*/ 7,
            /*segment_id*/ 1,
            /*segment_count*/ 2,
            raw.len(),
            b"",
        )),
        ClientSegmentObservation::Dropped
    ));
    assert!(matches!(
        reassembler.observe(chunk_envelope(
            client_id,
            stream_id,
            /*seq_id*/ 8,
            /*segment_id*/ 1,
            /*segment_count*/ 2,
            raw.len(),
            &raw[split..],
        )),
        ClientSegmentObservation::Forward(_)
    ));
}

#[test]
fn ignores_invalid_duplicate_chunks_without_dropping_current_assembly() {
    let message = JSONRPCMessage::Notification(JSONRPCNotification {
        method: "initialized".to_string(),
        params: None,
    });
    let raw = serde_json::to_vec(&message).expect("message should serialize");
    let split = raw.len() / 2;
    let client_id = ClientId("client-1".to_string());
    let stream_id = Some(StreamId("stream-1".to_string()));
    let mut reassembler = ClientSegmentReassembler::default();

    assert!(matches!(
        reassembler.observe(chunk_envelope(
            client_id.clone(),
            stream_id.clone(),
            /*seq_id*/ 8,
            /*segment_id*/ 0,
            /*segment_count*/ 2,
            raw.len(),
            &raw[..split],
        )),
        ClientSegmentObservation::Pending
    ));
    assert!(matches!(
        reassembler.observe(chunk_envelope(
            client_id.clone(),
            stream_id.clone(),
            /*seq_id*/ 8,
            /*segment_id*/ 0,
            /*segment_count*/ 2,
            raw.len(),
            b"",
        )),
        ClientSegmentObservation::Dropped
    ));
    assert!(matches!(
        reassembler.observe(chunk_envelope(
            client_id,
            stream_id,
            /*seq_id*/ 8,
            /*segment_id*/ 1,
            /*segment_count*/ 2,
            raw.len(),
            &raw[split..],
        )),
        ClientSegmentObservation::Forward(_)
    ));
}

fn chunk_envelope(
    client_id: ClientId,
    stream_id: Option<StreamId>,
    seq_id: u64,
    segment_id: usize,
    segment_count: usize,
    message_size_bytes: usize,
    chunk: &[u8],
) -> ClientEnvelope {
    ClientEnvelope {
        event: ClientEvent::ClientMessageChunk {
            segment_id,
            segment_count,
            message_size_bytes,
            message_chunk_base64: base64::engine::general_purpose::STANDARD.encode(chunk),
        },
        client_id,
        stream_id,
        seq_id: Some(seq_id),
        cursor: None,
    }
}
