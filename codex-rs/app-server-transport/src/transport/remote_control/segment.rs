use super::protocol::ClientEnvelope;
use super::protocol::ClientEvent;
use super::protocol::ClientId;
use super::protocol::ServerEnvelope;
use super::protocol::ServerEvent;
use super::protocol::StreamId;
use base64::DecodeSliceError;
use base64::Engine;
use codex_app_server_protocol::JSONRPCMessage;
use std::collections::HashMap;
use std::io;
use std::io::ErrorKind;
use std::io::Write;
use tokio::time::Instant;
use tracing::warn;

pub(super) const REMOTE_CONTROL_SEGMENT_TARGET_BYTES: usize = 100 * 1024;
pub(super) const REMOTE_CONTROL_SEGMENT_MAX_BYTES: usize = 150 * 1024;
pub(super) const REMOTE_CONTROL_REASSEMBLED_MAX_BYTES: usize = 100 * 1024 * 1024;
pub(super) const REMOTE_CONTROL_SEGMENT_COUNT_MAX: usize = 1024;
const REMOTE_CONTROL_SEGMENT_ASSEMBLY_MAX_COUNT: usize = 128;

#[derive(Debug)]
struct ClientSegmentAssembly {
    stream_id: StreamId,
    metadata: ClientSegmentMetadata,
    raw: Vec<u8>,
    next_segment_id: usize,
    last_chunk_seen_at: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ClientSegmentMetadata {
    seq_id: u64,
    segment_count: usize,
    message_size_bytes: usize,
}

#[derive(Default)]
pub(super) struct ClientSegmentReassembler {
    assemblies: HashMap<ClientId, ClientSegmentAssembly>,
}

pub(super) enum ClientSegmentObservation {
    Forward(Box<ClientEnvelope>),
    Pending,
    Dropped,
}

impl ClientSegmentReassembler {
    pub(super) fn observe(&mut self, envelope: ClientEnvelope) -> ClientSegmentObservation {
        let ClientEvent::ClientMessageChunk {
            segment_id,
            segment_count,
            message_size_bytes,
            message_chunk_base64,
        } = &envelope.event
        else {
            return ClientSegmentObservation::Forward(Box::new(envelope));
        };
        let segment_id = *segment_id;
        let segment_count = *segment_count;
        let message_size_bytes = *message_size_bytes;

        let Some(metadata) = ClientSegmentMetadata::from_envelope(&envelope) else {
            warn!(
                client_id = envelope.client_id.0.as_str(),
                "dropping segmented remote-control client envelope without seq_id"
            );
            return ClientSegmentObservation::Dropped;
        };
        let Some(stream_id) = envelope.stream_id.clone() else {
            warn!(
                client_id = envelope.client_id.0.as_str(),
                "dropping segmented remote-control client envelope without stream_id"
            );
            return ClientSegmentObservation::Dropped;
        };
        if self.should_ignore_chunk(&envelope.client_id, &stream_id, metadata.seq_id, segment_id) {
            return ClientSegmentObservation::Dropped;
        }
        if segment_count == 0
            || segment_count > REMOTE_CONTROL_SEGMENT_COUNT_MAX
            || segment_id >= segment_count
            || message_size_bytes == 0
            || message_size_bytes > REMOTE_CONTROL_REASSEMBLED_MAX_BYTES
            || message_chunk_base64.is_empty()
        {
            warn!(
                client_id = envelope.client_id.0.as_str(),
                "dropping invalid segmented remote-control client envelope"
            );
            self.remove_assembly(&envelope.client_id, &stream_id);
            return ClientSegmentObservation::Dropped;
        }

        let now = Instant::now();
        match self.assemblies.get(&envelope.client_id) {
            Some(assembly) if assembly.stream_id != stream_id => {
                warn!(
                    client_id = envelope.client_id.0.as_str(),
                    "resetting segmented remote-control client envelope after stream change"
                );
                self.assemblies.insert(
                    envelope.client_id.clone(),
                    ClientSegmentAssembly {
                        stream_id: stream_id.clone(),
                        metadata: metadata.clone(),
                        raw: Vec::new(),
                        next_segment_id: 0,
                        last_chunk_seen_at: now,
                    },
                );
            }
            Some(_) => {}
            None => {
                self.evict_assemblies_if_full();
                self.assemblies.insert(
                    envelope.client_id.clone(),
                    ClientSegmentAssembly {
                        stream_id: stream_id.clone(),
                        metadata: metadata.clone(),
                        raw: Vec::new(),
                        next_segment_id: 0,
                        last_chunk_seen_at: now,
                    },
                );
            }
        }
        let result = {
            let Some(assembly) = self.assemblies.get_mut(&envelope.client_id) else {
                warn!(
                    client_id = envelope.client_id.0.as_str(),
                    "dropping segmented remote-control client envelope without assembly"
                );
                return ClientSegmentObservation::Dropped;
            };
            if metadata.seq_id < assembly.metadata.seq_id {
                AssemblyUpdate::Ignore
            } else if assembly.metadata != metadata {
                warn!(
                    client_id = envelope.client_id.0.as_str(),
                    "resetting segmented remote-control client envelope after metadata mismatch"
                );
                AssemblyUpdate::Drop
            } else if segment_id < assembly.next_segment_id {
                AssemblyUpdate::Pending
            } else if segment_id != assembly.next_segment_id {
                warn!(
                    client_id = envelope.client_id.0.as_str(),
                    "dropping out-of-order segmented remote-control client envelope"
                );
                AssemblyUpdate::Drop
            } else {
                assembly.last_chunk_seen_at = now;
                let chunk_start = assembly.raw.len();
                let decoded_chunk_len = base64::decoded_len_estimate(message_chunk_base64.len());
                let chunk_end = usize::min(
                    message_size_bytes,
                    chunk_start.saturating_add(decoded_chunk_len),
                );
                assembly.raw.resize(chunk_end, 0);
                match base64::engine::general_purpose::STANDARD.decode_slice(
                    message_chunk_base64.as_bytes(),
                    &mut assembly.raw[chunk_start..],
                ) {
                    Ok(decoded_chunk_len) => {
                        assembly.raw.truncate(chunk_start + decoded_chunk_len);
                        assembly.next_segment_id += 1;
                        if assembly.next_segment_id < segment_count {
                            AssemblyUpdate::Pending
                        } else if assembly.raw.len() != message_size_bytes {
                            warn!(
                                client_id = envelope.client_id.0.as_str(),
                                "dropping reassembled remote-control client envelope with mismatched size"
                            );
                            AssemblyUpdate::Drop
                        } else {
                            match serde_json::from_slice::<JSONRPCMessage>(&assembly.raw) {
                                Ok(message) => AssemblyUpdate::Complete(message),
                                Err(err) => {
                                    warn!(
                                        client_id = envelope.client_id.0.as_str(),
                                        "dropping invalid reassembled remote-control client envelope: {err}"
                                    );
                                    AssemblyUpdate::Drop
                                }
                            }
                        }
                    }
                    Err(DecodeSliceError::OutputSliceTooSmall) => {
                        warn!(
                            client_id = envelope.client_id.0.as_str(),
                            "dropping segmented remote-control client envelope after size overflow"
                        );
                        AssemblyUpdate::Drop
                    }
                    Err(err) => {
                        warn!(
                            client_id = envelope.client_id.0.as_str(),
                            "dropping segmented remote-control client envelope with invalid base64: {err}"
                        );
                        AssemblyUpdate::Drop
                    }
                }
            }
        };

        match result {
            AssemblyUpdate::Pending => ClientSegmentObservation::Pending,
            AssemblyUpdate::Ignore => ClientSegmentObservation::Dropped,
            AssemblyUpdate::Drop => {
                self.remove_assembly(&envelope.client_id, &stream_id);
                ClientSegmentObservation::Dropped
            }
            AssemblyUpdate::Complete(message) => {
                self.remove_assembly(&envelope.client_id, &stream_id);
                ClientSegmentObservation::Forward(Box::new(ClientEnvelope {
                    event: ClientEvent::ClientMessage { message },
                    ..envelope
                }))
            }
        }
    }

    pub(super) fn invalidate_stream(&mut self, client_id: &ClientId, stream_id: &StreamId) {
        self.remove_assembly(client_id, stream_id);
    }

    pub(super) fn invalidate_client(&mut self, client_id: &ClientId) {
        self.assemblies.remove(client_id);
    }

    pub(super) fn should_ignore_chunk(
        &self,
        client_id: &ClientId,
        stream_id: &StreamId,
        seq_id: u64,
        segment_id: usize,
    ) -> bool {
        self.assemblies.get(client_id).is_some_and(|assembly| {
            assembly.stream_id == *stream_id
                && (seq_id < assembly.metadata.seq_id
                    || (seq_id == assembly.metadata.seq_id
                        && segment_id < assembly.next_segment_id))
        })
    }

    fn remove_assembly(&mut self, client_id: &ClientId, stream_id: &StreamId) {
        if self
            .assemblies
            .get(client_id)
            .is_some_and(|assembly| &assembly.stream_id == stream_id)
        {
            self.assemblies.remove(client_id);
        }
    }

    fn evict_assemblies_if_full(&mut self) {
        while self.assemblies.len() >= REMOTE_CONTROL_SEGMENT_ASSEMBLY_MAX_COUNT {
            let Some(client_id) = self
                .assemblies
                .iter()
                .min_by_key(|(_, assembly)| assembly.last_chunk_seen_at)
                .map(|(client_id, _)| client_id.clone())
            else {
                return;
            };
            self.assemblies.remove(&client_id);
        }
    }
}

enum AssemblyUpdate {
    Pending,
    Ignore,
    Drop,
    Complete(JSONRPCMessage),
}

impl ClientSegmentMetadata {
    fn from_envelope(envelope: &ClientEnvelope) -> Option<Self> {
        let ClientEvent::ClientMessageChunk {
            segment_count,
            message_size_bytes,
            ..
        } = &envelope.event
        else {
            return None;
        };
        Some(Self {
            seq_id: envelope.seq_id?,
            segment_count: *segment_count,
            message_size_bytes: *message_size_bytes,
        })
    }
}

pub(super) fn split_server_envelope_for_transport(
    envelope: ServerEnvelope,
) -> io::Result<Vec<ServerEnvelope>> {
    if !matches!(envelope.event, ServerEvent::ServerMessage { .. }) {
        return Ok(vec![envelope]);
    }

    let envelope_size_bytes = serialized_len(&envelope)?;
    if envelope_size_bytes <= REMOTE_CONTROL_SEGMENT_MAX_BYTES {
        return Ok(vec![envelope]);
    }

    let ServerEvent::ServerMessage { message } = envelope.event.clone() else {
        unreachable!("server message variant checked above");
    };
    let raw = serde_json::to_vec(message.as_ref()).map_err(io::Error::other)?;
    let message_size_bytes = raw.len();
    if message_size_bytes > REMOTE_CONTROL_REASSEMBLED_MAX_BYTES {
        warn!("dropping remote-control server envelope that exceeds reassembled size limit");
        return Ok(Vec::new());
    }

    let minimal_segment_count =
        usize::min(message_size_bytes.max(1), REMOTE_CONTROL_SEGMENT_COUNT_MAX);
    let minimal_chunk = &raw[..usize::min(raw.len(), 1)];
    if serialized_chunk_len(
        &envelope,
        /*segment_id*/ 0,
        minimal_segment_count,
        message_size_bytes,
        minimal_chunk,
    )? > REMOTE_CONTROL_SEGMENT_MAX_BYTES
    {
        warn!("dropping remote-control server envelope that cannot fit within segment size limit");
        return Ok(Vec::new());
    }

    let mut segment_count = usize::max(
        2,
        message_size_bytes.div_ceil(REMOTE_CONTROL_SEGMENT_TARGET_BYTES),
    );
    loop {
        let chunk_size = usize::max(1, message_size_bytes.div_ceil(segment_count));
        segment_count = message_size_bytes.div_ceil(chunk_size);
        let segments_fit = raw
            .chunks(chunk_size)
            .enumerate()
            .all(|(segment_id, chunk)| {
                serialized_chunk_len(
                    &envelope,
                    segment_id,
                    segment_count,
                    message_size_bytes,
                    chunk,
                )
                .is_ok_and(|size| size <= REMOTE_CONTROL_SEGMENT_MAX_BYTES)
            });
        if segments_fit {
            return raw
                .chunks(chunk_size)
                .enumerate()
                .map(|(segment_id, chunk)| {
                    build_chunk_envelope(
                        &envelope,
                        segment_id,
                        segment_count,
                        message_size_bytes,
                        chunk,
                    )
                })
                .collect();
        }
        if chunk_size == 1 {
            warn!(
                "dropping remote-control server envelope that cannot fit within segment size limit"
            );
            return Ok(Vec::new());
        }
        let next_segment_count = segment_count + 1;
        let next_chunk_size = usize::max(1, message_size_bytes.div_ceil(next_segment_count));
        segment_count = if next_chunk_size == chunk_size {
            message_size_bytes
        } else {
            next_segment_count
        };
    }
}

fn serialized_chunk_len(
    envelope: &ServerEnvelope,
    segment_id: usize,
    segment_count: usize,
    message_size_bytes: usize,
    chunk: &[u8],
) -> io::Result<usize> {
    serialized_len(&build_chunk_envelope(
        envelope,
        segment_id,
        segment_count,
        message_size_bytes,
        chunk,
    )?)
}

#[derive(Default)]
struct CountingWriter {
    len: usize,
}

impl Write for CountingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.len += buf.len();
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn serialized_len(value: &impl serde::Serialize) -> io::Result<usize> {
    let mut writer = CountingWriter::default();
    serde_json::to_writer(&mut writer, value).map_err(io::Error::other)?;
    Ok(writer.len)
}

fn build_chunk_envelope(
    envelope: &ServerEnvelope,
    segment_id: usize,
    segment_count: usize,
    message_size_bytes: usize,
    chunk: &[u8],
) -> io::Result<ServerEnvelope> {
    if segment_count > REMOTE_CONTROL_SEGMENT_COUNT_MAX {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "remote-control segment count exceeds maximum",
        ));
    }
    Ok(ServerEnvelope {
        event: ServerEvent::ServerMessageChunk {
            segment_id,
            segment_count,
            message_size_bytes,
            message_chunk_base64: base64::engine::general_purpose::STANDARD.encode(chunk),
        },
        client_id: envelope.client_id.clone(),
        stream_id: envelope.stream_id.clone(),
        seq_id: envelope.seq_id,
    })
}
