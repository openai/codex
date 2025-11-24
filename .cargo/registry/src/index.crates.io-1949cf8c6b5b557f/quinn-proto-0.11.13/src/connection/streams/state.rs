use std::{
    collections::{VecDeque, hash_map},
    convert::TryFrom,
    mem,
};

use bytes::BufMut;
use rustc_hash::FxHashMap;
use tracing::{debug, trace};

use super::{
    PendingStreamsQueue, Recv, Retransmits, Send, SendState, ShouldTransmit, StreamEvent,
    StreamHalf, ThinRetransmits,
};
use crate::{
    Dir, MAX_STREAM_COUNT, Side, StreamId, TransportError, VarInt,
    coding::BufMutExt,
    connection::stats::FrameStats,
    frame::{self, FrameStruct, StreamMetaVec},
    transport_parameters::TransportParameters,
};

/// Wrapper around `Recv` that facilitates reusing `Recv` instances
#[derive(Debug)]
pub(super) enum StreamRecv {
    /// A `Recv` that is ready to be opened
    Free(Box<Recv>),
    /// A `Recv` that has been opened
    Open(Box<Recv>),
}

impl StreamRecv {
    /// Returns a reference to the inner `Recv` if the stream is open
    pub(super) fn as_open_recv(&self) -> Option<&Recv> {
        match self {
            Self::Open(r) => Some(r),
            _ => None,
        }
    }

    // Returns a mutable reference to the inner `Recv` if the stream is open
    pub(super) fn as_open_recv_mut(&mut self) -> Option<&mut Recv> {
        match self {
            Self::Open(r) => Some(r),
            _ => None,
        }
    }

    // Returns the inner `Recv`
    pub(super) fn into_inner(self) -> Box<Recv> {
        match self {
            Self::Free(r) | Self::Open(r) => r,
        }
    }

    // Reinitialize the stream so the inner `Recv` can be reused
    pub(super) fn free(self, initial_max_data: u64) -> Self {
        match self {
            Self::Free(_) => unreachable!("Self::Free on reinit()"),
            Self::Open(mut recv) => {
                recv.reinit(initial_max_data);
                Self::Free(recv)
            }
        }
    }
}

#[allow(unreachable_pub)] // fuzzing only
pub struct StreamsState {
    pub(super) side: Side,
    // Set of streams that are currently open, or could be immediately opened by the peer
    pub(super) send: FxHashMap<StreamId, Option<Box<Send>>>,
    pub(super) recv: FxHashMap<StreamId, Option<StreamRecv>>,
    pub(super) free_recv: Vec<StreamRecv>,
    pub(super) next: [u64; 2],
    /// Maximum number of locally-initiated streams that may be opened over the lifetime of the
    /// connection so far, per direction
    pub(super) max: [u64; 2],
    /// Maximum number of remotely-initiated streams that may be opened over the lifetime of the
    /// connection so far, per direction
    pub(super) max_remote: [u64; 2],
    /// Value of `max_remote` most recently transmitted to the peer in a `MAX_STREAMS` frame
    sent_max_remote: [u64; 2],
    /// Number of streams that we've given the peer permission to open and which aren't fully closed
    pub(super) allocated_remote_count: [u64; 2],
    /// Size of the desired stream flow control window. May be smaller than `allocated_remote_count`
    /// due to `set_max_concurrent` calls.
    max_concurrent_remote_count: [u64; 2],
    /// Whether `max_concurrent_remote_count` has ever changed
    flow_control_adjusted: bool,
    /// Lowest remotely-initiated stream index that haven't actually been opened by the peer
    pub(super) next_remote: [u64; 2],
    /// Whether the remote endpoint has opened any streams the application doesn't know about yet,
    /// per directionality
    opened: [bool; 2],
    // Next to report to the application, once opened
    pub(super) next_reported_remote: [u64; 2],
    /// Number of outbound streams
    ///
    /// This differs from `self.send.len()` in that it does not include streams that the peer is
    /// permitted to open but which have not yet been opened.
    pub(super) send_streams: usize,
    /// Streams with outgoing data queued, sorted by priority
    pub(super) pending: PendingStreamsQueue,

    events: VecDeque<StreamEvent>,
    /// Streams blocked on connection-level flow control or stream window space
    ///
    /// Streams are only added to this list when a write fails.
    pub(super) connection_blocked: Vec<StreamId>,
    /// Connection-level flow control budget dictated by the peer
    pub(super) max_data: u64,
    /// The initial receive window
    receive_window: u64,
    /// Limit on incoming data, which is transmitted through `MAX_DATA` frames
    local_max_data: u64,
    /// The last value of `MAX_DATA` which had been queued for transmission in
    /// an outgoing `MAX_DATA` frame
    sent_max_data: VarInt,
    /// Sum of current offsets of all send streams.
    pub(super) data_sent: u64,
    /// Sum of end offsets of all receive streams. Includes gaps, so it's an upper bound.
    data_recvd: u64,
    /// Total quantity of unacknowledged outgoing data
    pub(super) unacked_data: u64,
    /// Configured upper bound for `unacked_data`.
    ///
    /// Note this may be less than `unacked_data` if the user has set a new value.
    pub(super) send_window: u64,
    /// Configured upper bound for how much unacked data the peer can send us per stream
    pub(super) stream_receive_window: u64,

    // Pertinent state from the TransportParameters supplied by the peer
    initial_max_stream_data_uni: VarInt,
    initial_max_stream_data_bidi_local: VarInt,
    initial_max_stream_data_bidi_remote: VarInt,

    /// The shrink to be applied to local_max_data when receive_window is shrunk
    receive_window_shrink_debt: u64,
}

impl StreamsState {
    #[allow(unreachable_pub)] // fuzzing only
    pub fn new(
        side: Side,
        max_remote_uni: VarInt,
        max_remote_bi: VarInt,
        send_window: u64,
        receive_window: VarInt,
        stream_receive_window: VarInt,
    ) -> Self {
        let mut this = Self {
            side,
            send: FxHashMap::default(),
            recv: FxHashMap::default(),
            free_recv: Vec::new(),
            next: [0, 0],
            max: [0, 0],
            max_remote: [max_remote_bi.into(), max_remote_uni.into()],
            sent_max_remote: [max_remote_bi.into(), max_remote_uni.into()],
            allocated_remote_count: [max_remote_bi.into(), max_remote_uni.into()],
            max_concurrent_remote_count: [max_remote_bi.into(), max_remote_uni.into()],
            flow_control_adjusted: false,
            next_remote: [0, 0],
            opened: [false, false],
            next_reported_remote: [0, 0],
            send_streams: 0,
            pending: PendingStreamsQueue::new(),
            events: VecDeque::new(),
            connection_blocked: Vec::new(),
            max_data: 0,
            receive_window: receive_window.into(),
            local_max_data: receive_window.into(),
            sent_max_data: receive_window,
            data_sent: 0,
            data_recvd: 0,
            unacked_data: 0,
            send_window,
            stream_receive_window: stream_receive_window.into(),
            initial_max_stream_data_uni: 0u32.into(),
            initial_max_stream_data_bidi_local: 0u32.into(),
            initial_max_stream_data_bidi_remote: 0u32.into(),
            receive_window_shrink_debt: 0,
        };

        for dir in Dir::iter() {
            for i in 0..this.max_remote[dir as usize] {
                this.insert(true, StreamId::new(!side, dir, i));
            }
        }

        this
    }

    pub(crate) fn set_params(&mut self, params: &TransportParameters) {
        self.initial_max_stream_data_uni = params.initial_max_stream_data_uni;
        self.initial_max_stream_data_bidi_local = params.initial_max_stream_data_bidi_local;
        self.initial_max_stream_data_bidi_remote = params.initial_max_stream_data_bidi_remote;
        self.max[Dir::Bi as usize] = params.initial_max_streams_bidi.into();
        self.max[Dir::Uni as usize] = params.initial_max_streams_uni.into();
        self.received_max_data(params.initial_max_data);
        for i in 0..self.max_remote[Dir::Bi as usize] {
            let id = StreamId::new(!self.side, Dir::Bi, i);
            if let Some(s) = self.send.get_mut(&id).and_then(|s| s.as_mut()) {
                s.max_data = params.initial_max_stream_data_bidi_local.into();
            }
        }
    }

    /// Ensure we have space for at least a full flow control window of remotely-initiated streams
    /// to be open, and notify the peer if the window has moved
    fn ensure_remote_streams(&mut self, dir: Dir) {
        let new_count = self.max_concurrent_remote_count[dir as usize]
            .saturating_sub(self.allocated_remote_count[dir as usize]);
        for i in 0..new_count {
            let id = StreamId::new(!self.side, dir, self.max_remote[dir as usize] + i);
            self.insert(true, id);
        }
        self.allocated_remote_count[dir as usize] += new_count;
        self.max_remote[dir as usize] += new_count;
    }

    pub(crate) fn zero_rtt_rejected(&mut self) {
        // Revert to initial state for outgoing streams
        for dir in Dir::iter() {
            for i in 0..self.next[dir as usize] {
                // We don't bother calling `stream_freed` here because we explicitly reset affected
                // counters below.
                let id = StreamId::new(self.side, dir, i);
                self.send.remove(&id).unwrap();
                if let Dir::Bi = dir {
                    self.recv.remove(&id).unwrap();
                }
            }
            self.next[dir as usize] = 0;

            // If 0-RTT was rejected, any flow control frames we sent were lost.
            if self.flow_control_adjusted {
                // Conservative approximation of whatever we sent in transport parameters
                self.sent_max_remote[dir as usize] = 0;
            }
        }

        self.pending.clear();
        self.send_streams = 0;
        self.data_sent = 0;
        self.connection_blocked.clear();
    }

    /// Process incoming stream frame
    ///
    /// If successful, returns whether a `MAX_DATA` frame needs to be transmitted
    pub(crate) fn received(
        &mut self,
        frame: frame::Stream,
        payload_len: usize,
    ) -> Result<ShouldTransmit, TransportError> {
        let id = frame.id;
        self.validate_receive_id(id).map_err(|e| {
            debug!("received illegal STREAM frame");
            e
        })?;

        let rs = match self
            .recv
            .get_mut(&id)
            .map(get_or_insert_recv(self.stream_receive_window))
        {
            Some(rs) => rs,
            None => {
                trace!("dropping frame for closed stream");
                return Ok(ShouldTransmit(false));
            }
        };

        if !rs.is_receiving() {
            trace!("dropping frame for finished stream");
            return Ok(ShouldTransmit(false));
        }

        let (new_bytes, closed) =
            rs.ingest(frame, payload_len, self.data_recvd, self.local_max_data)?;
        self.data_recvd = self.data_recvd.saturating_add(new_bytes);

        if !rs.stopped {
            self.on_stream_frame(true, id);
            return Ok(ShouldTransmit(false));
        }

        // Stopped streams become closed instantly on FIN, so check whether we need to clean up
        if closed {
            let rs = self.recv.remove(&id).flatten().unwrap();
            self.stream_recv_freed(id, rs);
        }

        // We don't buffer data on stopped streams, so issue flow control credit immediately
        Ok(self.add_read_credits(new_bytes))
    }

    /// Process incoming RESET_STREAM frame
    ///
    /// If successful, returns whether a `MAX_DATA` frame needs to be transmitted
    #[allow(unreachable_pub)] // fuzzing only
    pub fn received_reset(
        &mut self,
        frame: frame::ResetStream,
    ) -> Result<ShouldTransmit, TransportError> {
        let frame::ResetStream {
            id,
            error_code,
            final_offset,
        } = frame;
        self.validate_receive_id(id).map_err(|e| {
            debug!("received illegal RESET_STREAM frame");
            e
        })?;

        let rs = match self
            .recv
            .get_mut(&id)
            .map(get_or_insert_recv(self.stream_receive_window))
        {
            Some(stream) => stream,
            None => {
                trace!("received RESET_STREAM on closed stream");
                return Ok(ShouldTransmit(false));
            }
        };

        // State transition
        if !rs.reset(
            error_code,
            final_offset,
            self.data_recvd,
            self.local_max_data,
        )? {
            // Redundant reset
            return Ok(ShouldTransmit(false));
        }
        let bytes_read = rs.assembler.bytes_read();
        let stopped = rs.stopped;
        let end = rs.end;
        if stopped {
            // Stopped streams should be disposed immediately on reset
            let rs = self.recv.remove(&id).flatten().unwrap();
            self.stream_recv_freed(id, rs);
        }
        self.on_stream_frame(!stopped, id);

        // Update connection-level flow control
        Ok(if bytes_read != final_offset.into_inner() {
            // bytes_read is always <= end, so this won't underflow.
            self.data_recvd = self
                .data_recvd
                .saturating_add(u64::from(final_offset) - end);
            self.add_read_credits(u64::from(final_offset) - bytes_read)
        } else {
            ShouldTransmit(false)
        })
    }

    /// Process incoming `STOP_SENDING` frame
    #[allow(unreachable_pub)] // fuzzing only
    pub fn received_stop_sending(&mut self, id: StreamId, error_code: VarInt) {
        let max_send_data = self.max_send_data(id);
        let stream = match self
            .send
            .get_mut(&id)
            .map(get_or_insert_send(max_send_data))
        {
            Some(ss) => ss,
            None => return,
        };

        if stream.try_stop(error_code) {
            self.events
                .push_back(StreamEvent::Stopped { id, error_code });
            self.on_stream_frame(false, id);
        }
    }

    pub(crate) fn reset_acked(&mut self, id: StreamId) {
        match self.send.entry(id) {
            hash_map::Entry::Vacant(_) => {}
            hash_map::Entry::Occupied(e) => {
                if let Some(SendState::ResetSent) = e.get().as_ref().map(|s| s.state) {
                    e.remove_entry();
                    self.stream_freed(id, StreamHalf::Send);
                }
            }
        }
    }

    /// Whether any stream data is queued, regardless of control frames
    pub(crate) fn can_send_stream_data(&self) -> bool {
        // Reset streams may linger in the pending stream list, but will never produce stream frames
        self.pending.iter().any(|stream| {
            self.send
                .get(&stream.id)
                .and_then(|s| s.as_ref())
                .is_some_and(|s| !s.is_reset())
        })
    }

    /// Whether MAX_STREAM_DATA frames could be sent for stream `id`
    pub(crate) fn can_send_flow_control(&self, id: StreamId) -> bool {
        self.recv
            .get(&id)
            .and_then(|s| s.as_ref())
            .and_then(|s| s.as_open_recv())
            .is_some_and(|s| s.can_send_flow_control())
    }

    pub(in crate::connection) fn write_control_frames(
        &mut self,
        buf: &mut Vec<u8>,
        pending: &mut Retransmits,
        retransmits: &mut ThinRetransmits,
        stats: &mut FrameStats,
        max_size: usize,
    ) {
        // RESET_STREAM
        while buf.len() + frame::ResetStream::SIZE_BOUND < max_size {
            let (id, error_code) = match pending.reset_stream.pop() {
                Some(x) => x,
                None => break,
            };
            let stream = match self.send.get_mut(&id).and_then(|s| s.as_mut()) {
                Some(x) => x,
                None => continue,
            };
            trace!(stream = %id, "RESET_STREAM");
            retransmits
                .get_or_create()
                .reset_stream
                .push((id, error_code));
            frame::ResetStream {
                id,
                error_code,
                final_offset: VarInt::try_from(stream.offset()).expect("impossibly large offset"),
            }
            .encode(buf);
            stats.reset_stream += 1;
        }

        // STOP_SENDING
        while buf.len() + frame::StopSending::SIZE_BOUND < max_size {
            let frame = match pending.stop_sending.pop() {
                Some(x) => x,
                None => break,
            };
            // We may need to transmit STOP_SENDING even for streams whose state we have discarded,
            // because we are able to discard local state for stopped streams immediately upon
            // receiving FIN, even if the peer still has arbitrarily large amounts of data to
            // (re)transmit due to loss or unconventional sending strategy. We could fine-tune this
            // a little by dropping the frame if we specifically know the stream's been reset by the
            // peer, but we discard that information as soon as the application consumes it, so it
            // can't be relied upon regardless.
            trace!(stream = %frame.id, "STOP_SENDING");
            frame.encode(buf);
            retransmits.get_or_create().stop_sending.push(frame);
            stats.stop_sending += 1;
        }

        // MAX_DATA
        if pending.max_data && buf.len() + 9 < max_size {
            pending.max_data = false;

            // `local_max_data` can grow bigger than `VarInt`.
            // For transmission inside QUIC frames we need to clamp it to the
            // maximum allowed `VarInt` size.
            let max = VarInt::try_from(self.local_max_data).unwrap_or(VarInt::MAX);

            trace!(value = max.into_inner(), "MAX_DATA");
            if max > self.sent_max_data {
                // Record that a `MAX_DATA` announcing a certain window was sent. This will
                // suppress enqueuing further `MAX_DATA` frames unless either the previous
                // transmission was not acknowledged or the window further increased.
                self.sent_max_data = max;
            }

            retransmits.get_or_create().max_data = true;
            buf.write(frame::FrameType::MAX_DATA);
            buf.write(max);
            stats.max_data += 1;
        }

        // MAX_STREAM_DATA
        while buf.len() + 17 < max_size {
            let id = match pending.max_stream_data.iter().next() {
                Some(x) => *x,
                None => break,
            };
            pending.max_stream_data.remove(&id);
            let rs = match self
                .recv
                .get_mut(&id)
                .and_then(|s| s.as_mut())
                .and_then(|s| s.as_open_recv_mut())
            {
                Some(x) => x,
                None => continue,
            };
            if !rs.can_send_flow_control() {
                continue;
            }
            retransmits.get_or_create().max_stream_data.insert(id);

            let (max, _) = rs.max_stream_data(self.stream_receive_window);
            rs.record_sent_max_stream_data(max);

            trace!(stream = %id, max = max, "MAX_STREAM_DATA");
            buf.write(frame::FrameType::MAX_STREAM_DATA);
            buf.write(id);
            buf.write_var(max);
            stats.max_stream_data += 1;
        }

        // MAX_STREAMS
        for dir in Dir::iter() {
            if !pending.max_stream_id[dir as usize] || buf.len() + 9 >= max_size {
                continue;
            }

            pending.max_stream_id[dir as usize] = false;
            retransmits.get_or_create().max_stream_id[dir as usize] = true;
            self.sent_max_remote[dir as usize] = self.max_remote[dir as usize];
            trace!(
                value = self.max_remote[dir as usize],
                "MAX_STREAMS ({:?})", dir
            );
            buf.write(match dir {
                Dir::Uni => frame::FrameType::MAX_STREAMS_UNI,
                Dir::Bi => frame::FrameType::MAX_STREAMS_BIDI,
            });
            buf.write_var(self.max_remote[dir as usize]);
            match dir {
                Dir::Uni => stats.max_streams_uni += 1,
                Dir::Bi => stats.max_streams_bidi += 1,
            }
        }
    }

    pub(crate) fn write_stream_frames(
        &mut self,
        buf: &mut Vec<u8>,
        max_buf_size: usize,
        fair: bool,
    ) -> StreamMetaVec {
        let mut stream_frames = StreamMetaVec::new();
        while buf.len() + frame::Stream::SIZE_BOUND < max_buf_size {
            if max_buf_size
                .checked_sub(buf.len() + frame::Stream::SIZE_BOUND)
                .is_none()
            {
                break;
            }

            // Pop the stream of the highest priority that currently has pending data
            // If the stream still has some pending data left after writing, it will be reinserted, otherwise not
            let Some(stream) = self.pending.pop() else {
                break;
            };

            let id = stream.id;

            let stream = match self.send.get_mut(&id).and_then(|s| s.as_mut()) {
                Some(s) => s,
                // Stream was reset with pending data and the reset was acknowledged
                None => continue,
            };

            // Reset streams aren't removed from the pending list and still exist while the peer
            // hasn't acknowledged the reset, but should not generate STREAM frames, so we need to
            // check for them explicitly.
            if stream.is_reset() {
                continue;
            }

            // Now that we know the `StreamId`, we can better account for how many bytes
            // are required to encode it.
            let max_buf_size = max_buf_size - buf.len() - 1 - VarInt::size(id.into());
            let (offsets, encode_length) = stream.pending.poll_transmit(max_buf_size);
            let fin = offsets.end == stream.pending.offset()
                && matches!(stream.state, SendState::DataSent { .. });
            if fin {
                stream.fin_pending = false;
            }

            if stream.is_pending() {
                // If the stream still has pending data, reinsert it, possibly with an updated priority value
                // Fairness with other streams is achieved by implementing round-robin scheduling,
                // so that the other streams will have a chance to write data
                // before we touch this stream again.
                if fair {
                    self.pending.push_pending(id, stream.priority);
                } else {
                    self.pending.reinsert_pending(id, stream.priority);
                }
            }

            let meta = frame::StreamMeta { id, offsets, fin };
            trace!(id = %meta.id, off = meta.offsets.start, len = meta.offsets.end - meta.offsets.start, fin = meta.fin, "STREAM");
            meta.encode(encode_length, buf);

            // The range might not be retrievable in a single `get` if it is
            // stored in noncontiguous fashion. Therefore this loop iterates
            // until the range is fully copied into the frame.
            let mut offsets = meta.offsets.clone();
            while offsets.start != offsets.end {
                let data = stream.pending.get(offsets.clone());
                offsets.start += data.len() as u64;
                buf.put_slice(data);
            }
            stream_frames.push(meta);
        }

        stream_frames
    }

    /// Notify the application that new streams were opened or a stream became readable.
    fn on_stream_frame(&mut self, notify_readable: bool, stream: StreamId) {
        if stream.initiator() == self.side {
            // Notifying about the opening of locally-initiated streams would be redundant.
            if notify_readable {
                self.events.push_back(StreamEvent::Readable { id: stream });
            }
            return;
        }
        let next = &mut self.next_remote[stream.dir() as usize];
        if stream.index() >= *next {
            *next = stream.index() + 1;
            self.opened[stream.dir() as usize] = true;
        } else if notify_readable {
            self.events.push_back(StreamEvent::Readable { id: stream });
        }
    }

    pub(crate) fn received_ack_of(&mut self, frame: frame::StreamMeta) {
        let mut entry = match self.send.entry(frame.id) {
            hash_map::Entry::Vacant(_) => return,
            hash_map::Entry::Occupied(e) => e,
        };

        let stream = match entry.get_mut().as_mut() {
            Some(s) => s,
            None => {
                // Because we only call this after sending data on this stream,
                // this closure should be unreachable. If we did somehow screw that up,
                // then we might hit an underflow below with unpredictable effects down
                // the line. Best to short-circuit.
                return;
            }
        };

        if stream.is_reset() {
            // We account for outstanding data on reset streams at time of reset
            return;
        }
        let id = frame.id;
        self.unacked_data -= frame.offsets.end - frame.offsets.start;
        if !stream.ack(frame) {
            // The stream is unfinished or may still need retransmits
            return;
        }

        entry.remove_entry();
        self.stream_freed(id, StreamHalf::Send);
        self.events.push_back(StreamEvent::Finished { id });
    }

    pub(crate) fn retransmit(&mut self, frame: frame::StreamMeta) {
        let stream = match self.send.get_mut(&frame.id).and_then(|s| s.as_mut()) {
            // Loss of data on a closed stream is a noop
            None => return,
            Some(x) => x,
        };
        if !stream.is_pending() {
            self.pending.push_pending(frame.id, stream.priority);
        }
        stream.fin_pending |= frame.fin;
        stream.pending.retransmit(frame.offsets);
    }

    pub(crate) fn retransmit_all_for_0rtt(&mut self) {
        for dir in Dir::iter() {
            for index in 0..self.next[dir as usize] {
                let id = StreamId::new(Side::Client, dir, index);
                let stream = match self.send.get_mut(&id).and_then(|s| s.as_mut()) {
                    Some(stream) => stream,
                    None => continue,
                };
                if stream.pending.is_fully_acked() && !stream.fin_pending {
                    // Stream data can't be acked in 0-RTT, so we must not have sent anything on
                    // this stream
                    continue;
                }
                if !stream.is_pending() {
                    self.pending.push_pending(id, stream.priority);
                }
                stream.pending.retransmit_all_for_0rtt();
            }
        }
    }

    pub(crate) fn received_max_streams(
        &mut self,
        dir: Dir,
        count: u64,
    ) -> Result<(), TransportError> {
        if count > MAX_STREAM_COUNT {
            return Err(TransportError::FRAME_ENCODING_ERROR(
                "unrepresentable stream limit",
            ));
        }

        let current = &mut self.max[dir as usize];
        if count > *current {
            *current = count;
            self.events.push_back(StreamEvent::Available { dir });
        }

        Ok(())
    }

    /// Handle increase to connection-level flow control limit
    pub(crate) fn received_max_data(&mut self, n: VarInt) {
        self.max_data = self.max_data.max(n.into());
    }

    pub(crate) fn received_max_stream_data(
        &mut self,
        id: StreamId,
        offset: u64,
    ) -> Result<(), TransportError> {
        if id.initiator() != self.side && id.dir() == Dir::Uni {
            debug!("got MAX_STREAM_DATA on recv-only {}", id);
            return Err(TransportError::STREAM_STATE_ERROR(
                "MAX_STREAM_DATA on recv-only stream",
            ));
        }

        let write_limit = self.write_limit();
        let max_send_data = self.max_send_data(id);
        if let Some(ss) = self
            .send
            .get_mut(&id)
            .map(get_or_insert_send(max_send_data))
        {
            if ss.increase_max_data(offset) {
                if write_limit > 0 {
                    self.events.push_back(StreamEvent::Writable { id });
                } else if !ss.connection_blocked {
                    // The stream is still blocked on the connection flow control
                    // window. In order to get unblocked when the window relaxes
                    // it needs to be in the connection blocked list.
                    ss.connection_blocked = true;
                    self.connection_blocked.push(id);
                }
            }
        } else if id.initiator() == self.side && self.is_local_unopened(id) {
            debug!("got MAX_STREAM_DATA on unopened {}", id);
            return Err(TransportError::STREAM_STATE_ERROR(
                "MAX_STREAM_DATA on unopened stream",
            ));
        }

        self.on_stream_frame(false, id);
        Ok(())
    }

    /// Returns the maximum amount of data this is allowed to be written on the connection
    pub(crate) fn write_limit(&self) -> u64 {
        (self.max_data - self.data_sent)
            // `send_window` can be set after construction to something *less* than `unacked_data`
            .min(self.send_window.saturating_sub(self.unacked_data))
    }

    /// Yield stream events
    pub(crate) fn poll(&mut self) -> Option<StreamEvent> {
        if let Some(dir) = Dir::iter().find(|&i| mem::replace(&mut self.opened[i as usize], false))
        {
            return Some(StreamEvent::Opened { dir });
        }

        if self.write_limit() > 0 {
            while let Some(id) = self.connection_blocked.pop() {
                let stream = match self.send.get_mut(&id).and_then(|s| s.as_mut()) {
                    None => continue,
                    Some(s) => s,
                };

                debug_assert!(stream.connection_blocked);
                stream.connection_blocked = false;

                // If it's no longer sensible to write to a stream (even to detect an error) then don't
                // report it.
                if stream.is_writable() && stream.max_data > stream.offset() {
                    return Some(StreamEvent::Writable { id });
                }
            }
        }

        self.events.pop_front()
    }

    /// Queues MAX_STREAM_ID frames in `pending` if needed
    ///
    /// Returns whether any frames were queued.
    pub(crate) fn queue_max_stream_id(&mut self, pending: &mut Retransmits) -> bool {
        let mut queued = false;
        for dir in Dir::iter() {
            let diff = self.max_remote[dir as usize] - self.sent_max_remote[dir as usize];
            // To reduce traffic, only announce updates if at least 1/8 of the flow control window
            // has been consumed.
            if diff > self.max_concurrent_remote_count[dir as usize] / 8 {
                pending.max_stream_id[dir as usize] = true;
                queued = true;
            }
        }
        queued
    }

    /// Check for errors entailed by the peer's use of `id` as a send stream
    fn validate_receive_id(&mut self, id: StreamId) -> Result<(), TransportError> {
        if self.side == id.initiator() {
            match id.dir() {
                Dir::Uni => {
                    return Err(TransportError::STREAM_STATE_ERROR(
                        "illegal operation on send-only stream",
                    ));
                }
                Dir::Bi if id.index() >= self.next[Dir::Bi as usize] => {
                    return Err(TransportError::STREAM_STATE_ERROR(
                        "operation on unopened stream",
                    ));
                }
                Dir::Bi => {}
            };
        } else {
            let limit = self.max_remote[id.dir() as usize];
            if id.index() >= limit {
                return Err(TransportError::STREAM_LIMIT_ERROR(""));
            }
        }
        Ok(())
    }

    /// Whether a locally initiated stream has never been open
    pub(crate) fn is_local_unopened(&self, id: StreamId) -> bool {
        id.index() >= self.next[id.dir() as usize]
    }

    pub(crate) fn set_max_concurrent(&mut self, dir: Dir, count: VarInt) {
        self.flow_control_adjusted = true;
        self.max_concurrent_remote_count[dir as usize] = count.into();
        self.ensure_remote_streams(dir);
    }

    pub(crate) fn max_concurrent(&self, dir: Dir) -> u64 {
        self.allocated_remote_count[dir as usize]
    }

    pub(crate) fn set_send_window(&mut self, send_window: u64) {
        self.send_window = send_window;
    }

    /// Set the receive_window and returns whether the receive_window has been
    /// expanded or shrunk: true if expanded, false if shrunk.
    pub(crate) fn set_receive_window(&mut self, receive_window: VarInt) -> bool {
        let receive_window = receive_window.into();
        let mut expanded = false;
        if receive_window > self.receive_window {
            self.local_max_data = self
                .local_max_data
                .saturating_add(receive_window - self.receive_window);
            expanded = true;
        } else {
            let diff = self.receive_window - receive_window;
            self.receive_window_shrink_debt = self.receive_window_shrink_debt.saturating_add(diff);
        }
        self.receive_window = receive_window;
        expanded
    }

    pub(super) fn insert(&mut self, remote: bool, id: StreamId) {
        let bi = id.dir() == Dir::Bi;
        // bidirectional OR (unidirectional AND NOT remote)
        if bi || !remote {
            assert!(self.send.insert(id, None).is_none());
        }
        // bidirectional OR (unidirectional AND remote)
        if bi || remote {
            let recv = self.free_recv.pop();
            assert!(self.recv.insert(id, recv).is_none());
        }
    }

    /// Adds credits to the connection flow control window
    ///
    /// Returns whether a `MAX_DATA` frame should be enqueued as soon as possible.
    /// This will only be the case if the window update would is significant
    /// enough. As soon as a window update with a `MAX_DATA` frame has been
    /// queued, the [`Recv::record_sent_max_stream_data`] function should be called to
    /// suppress sending further updates until the window increases significantly
    /// again.
    pub(super) fn add_read_credits(&mut self, credits: u64) -> ShouldTransmit {
        if credits > self.receive_window_shrink_debt {
            let net_credits = credits - self.receive_window_shrink_debt;
            self.local_max_data = self.local_max_data.saturating_add(net_credits);
            self.receive_window_shrink_debt = 0;
        } else {
            self.receive_window_shrink_debt -= credits;
        }

        if self.local_max_data > VarInt::MAX.into_inner() {
            return ShouldTransmit(false);
        }

        // Only announce a window update if it's significant enough
        // to make it worthwhile sending a MAX_DATA frame.
        // We use a fraction of the configured connection receive window to make
        // the decision, to accommodate for connection using bigger windows requiring
        // less updates.
        let diff = self.local_max_data - self.sent_max_data.into_inner();
        ShouldTransmit(diff >= (self.receive_window / 8))
    }

    /// Update counters for removal of a stream
    pub(super) fn stream_freed(&mut self, id: StreamId, half: StreamHalf) {
        if id.initiator() != self.side {
            let fully_free = id.dir() == Dir::Uni
                || match half {
                    StreamHalf::Send => !self.recv.contains_key(&id),
                    StreamHalf::Recv => !self.send.contains_key(&id),
                };
            if fully_free {
                self.allocated_remote_count[id.dir() as usize] -= 1;
                self.ensure_remote_streams(id.dir());
            }
        }
        if half == StreamHalf::Send {
            self.send_streams -= 1;
        }
    }

    pub(super) fn stream_recv_freed(&mut self, id: StreamId, recv: StreamRecv) {
        self.free_recv.push(recv.free(self.stream_receive_window));
        self.stream_freed(id, StreamHalf::Recv);
    }

    pub(super) fn max_send_data(&self, id: StreamId) -> VarInt {
        let remote = self.side != id.initiator();
        match id.dir() {
            Dir::Uni => self.initial_max_stream_data_uni,
            // Remote/local appear reversed here because the transport parameters are named from
            // the perspective of the peer.
            Dir::Bi if remote => self.initial_max_stream_data_bidi_local,
            Dir::Bi => self.initial_max_stream_data_bidi_remote,
        }
    }
}

#[inline]
pub(super) fn get_or_insert_send(
    max_data: VarInt,
) -> impl Fn(&mut Option<Box<Send>>) -> &mut Box<Send> {
    move |opt| opt.get_or_insert_with(|| Send::new(max_data))
}

#[inline]
pub(super) fn get_or_insert_recv(
    initial_max_data: u64,
) -> impl FnMut(&mut Option<StreamRecv>) -> &mut Recv {
    move |opt| {
        *opt = opt.take().map(|s| match s {
            StreamRecv::Free(recv) => StreamRecv::Open(recv),
            s => s,
        });
        opt.get_or_insert_with(|| StreamRecv::Open(Recv::new(initial_max_data)))
            .as_open_recv_mut()
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ReadableError, RecvStream, SendStream, TransportErrorCode, WriteError,
        connection::State as ConnState, connection::Streams,
    };
    use bytes::Bytes;

    fn make(side: Side) -> StreamsState {
        StreamsState::new(
            side,
            128u32.into(),
            128u32.into(),
            1024 * 1024,
            (1024 * 1024u32).into(),
            (1024 * 1024u32).into(),
        )
    }

    #[test]
    fn trivial_flow_control() {
        let mut client = StreamsState::new(
            Side::Client,
            1u32.into(),
            1u32.into(),
            1024 * 1024,
            (1024 * 1024u32).into(),
            (1024 * 1024u32).into(),
        );
        let id = StreamId::new(Side::Server, Dir::Uni, 0);
        let initial_max = client.local_max_data;
        const MESSAGE_SIZE: usize = 2048;
        assert_eq!(
            client
                .received(
                    frame::Stream {
                        id,
                        offset: 0,
                        fin: true,
                        data: Bytes::from_static(&[0; MESSAGE_SIZE]),
                    },
                    2048
                )
                .unwrap(),
            ShouldTransmit(false)
        );
        assert_eq!(client.data_recvd, 2048);
        assert_eq!(client.local_max_data - initial_max, 0);

        let mut pending = Retransmits::default();
        let mut recv = RecvStream {
            id,
            state: &mut client,
            pending: &mut pending,
        };

        let mut chunks = recv.read(true).unwrap();
        assert_eq!(
            chunks.next(MESSAGE_SIZE).unwrap().unwrap().bytes.len(),
            MESSAGE_SIZE
        );
        assert!(chunks.next(0).unwrap().is_none());
        let should_transmit = chunks.finalize();
        assert!(should_transmit.0);
        assert!(pending.max_stream_id[Dir::Uni as usize]);
        assert_eq!(client.local_max_data - initial_max, MESSAGE_SIZE as u64);
    }

    #[test]
    fn reset_flow_control() {
        let mut client = make(Side::Client);
        let id = StreamId::new(Side::Server, Dir::Uni, 0);
        let initial_max = client.local_max_data;
        assert_eq!(
            client
                .received(
                    frame::Stream {
                        id,
                        offset: 0,
                        fin: false,
                        data: Bytes::from_static(&[0; 2048]),
                    },
                    2048
                )
                .unwrap(),
            ShouldTransmit(false)
        );
        assert_eq!(client.data_recvd, 2048);
        assert_eq!(client.local_max_data - initial_max, 0);

        let mut pending = Retransmits::default();
        let mut recv = RecvStream {
            id,
            state: &mut client,
            pending: &mut pending,
        };

        let mut chunks = recv.read(true).unwrap();
        chunks.next(1024).unwrap();
        let _ = chunks.finalize();
        assert_eq!(client.local_max_data - initial_max, 1024);
        assert_eq!(
            client
                .received_reset(frame::ResetStream {
                    id,
                    error_code: 0u32.into(),
                    final_offset: 4096u32.into(),
                })
                .unwrap(),
            ShouldTransmit(false)
        );

        assert_eq!(client.data_recvd, 4096);
        assert_eq!(client.local_max_data - initial_max, 4096);

        // Ensure reading after a reset doesn't issue redundant credit
        let mut recv = RecvStream {
            id,
            state: &mut client,
            pending: &mut pending,
        };
        let mut chunks = recv.read(true).unwrap();
        assert_eq!(
            chunks.next(1024).unwrap_err(),
            crate::ReadError::Reset(0u32.into())
        );
        let _ = chunks.finalize();
        assert_eq!(client.data_recvd, 4096);
        assert_eq!(client.local_max_data - initial_max, 4096);
    }

    #[test]
    fn reset_after_empty_frame_flow_control() {
        let mut client = make(Side::Client);
        let id = StreamId::new(Side::Server, Dir::Uni, 0);
        let initial_max = client.local_max_data;
        assert_eq!(
            client
                .received(
                    frame::Stream {
                        id,
                        offset: 4096,
                        fin: false,
                        data: Bytes::from_static(&[0; 0]),
                    },
                    0
                )
                .unwrap(),
            ShouldTransmit(false)
        );
        assert_eq!(client.data_recvd, 4096);
        assert_eq!(client.local_max_data - initial_max, 0);
        assert_eq!(
            client
                .received_reset(frame::ResetStream {
                    id,
                    error_code: 0u32.into(),
                    final_offset: 4096u32.into(),
                })
                .unwrap(),
            ShouldTransmit(false)
        );
        assert_eq!(client.data_recvd, 4096);
        assert_eq!(client.local_max_data - initial_max, 4096);
    }

    #[test]
    fn duplicate_reset_flow_control() {
        let mut client = make(Side::Client);
        let id = StreamId::new(Side::Server, Dir::Uni, 0);
        assert_eq!(
            client
                .received_reset(frame::ResetStream {
                    id,
                    error_code: 0u32.into(),
                    final_offset: 4096u32.into(),
                })
                .unwrap(),
            ShouldTransmit(false)
        );
        assert_eq!(client.data_recvd, 4096);
        assert_eq!(
            client
                .received_reset(frame::ResetStream {
                    id,
                    error_code: 0u32.into(),
                    final_offset: 4096u32.into(),
                })
                .unwrap(),
            ShouldTransmit(false)
        );
        assert_eq!(client.data_recvd, 4096);
    }

    #[test]
    fn recv_stopped() {
        let mut client = make(Side::Client);
        let id = StreamId::new(Side::Server, Dir::Uni, 0);
        let initial_max = client.local_max_data;
        assert_eq!(
            client
                .received(
                    frame::Stream {
                        id,
                        offset: 0,
                        fin: false,
                        data: Bytes::from_static(&[0; 32]),
                    },
                    32
                )
                .unwrap(),
            ShouldTransmit(false)
        );
        assert_eq!(client.local_max_data, initial_max);

        let mut pending = Retransmits::default();
        let mut recv = RecvStream {
            id,
            state: &mut client,
            pending: &mut pending,
        };

        recv.stop(0u32.into()).unwrap();
        assert_eq!(recv.pending.stop_sending.len(), 1);
        assert!(!recv.pending.max_data);

        assert!(recv.stop(0u32.into()).is_err());
        assert_eq!(recv.read(true).err(), Some(ReadableError::ClosedStream));
        assert_eq!(recv.read(false).err(), Some(ReadableError::ClosedStream));

        assert_eq!(client.local_max_data - initial_max, 32);
        assert_eq!(
            client
                .received(
                    frame::Stream {
                        id,
                        offset: 32,
                        fin: true,
                        data: Bytes::from_static(&[0; 16]),
                    },
                    16
                )
                .unwrap(),
            ShouldTransmit(false)
        );
        assert_eq!(client.local_max_data - initial_max, 48);
        assert!(!client.recv.contains_key(&id));
    }

    #[test]
    fn stopped_reset() {
        let mut client = make(Side::Client);
        let id = StreamId::new(Side::Server, Dir::Uni, 0);
        // Server opens stream
        assert_eq!(
            client
                .received(
                    frame::Stream {
                        id,
                        offset: 0,
                        fin: false,
                        data: Bytes::from_static(&[0; 32])
                    },
                    32
                )
                .unwrap(),
            ShouldTransmit(false)
        );

        let mut pending = Retransmits::default();
        let mut recv = RecvStream {
            id,
            state: &mut client,
            pending: &mut pending,
        };

        recv.stop(0u32.into()).unwrap();
        assert_eq!(pending.stop_sending.len(), 1);
        assert!(!pending.max_data);

        // Server complies
        let prev_max = client.max_remote[Dir::Uni as usize];
        assert_eq!(
            client
                .received_reset(frame::ResetStream {
                    id,
                    error_code: 0u32.into(),
                    final_offset: 32u32.into(),
                })
                .unwrap(),
            ShouldTransmit(false)
        );
        assert!(!client.recv.contains_key(&id), "stream state is freed");
        assert_eq!(client.max_remote[Dir::Uni as usize], prev_max + 1);
    }

    #[test]
    fn send_stopped() {
        let mut server = make(Side::Server);
        server.set_params(&TransportParameters {
            initial_max_streams_uni: 1u32.into(),
            initial_max_data: 42u32.into(),
            initial_max_stream_data_uni: 42u32.into(),
            ..TransportParameters::default()
        });

        let (mut pending, state) = (Retransmits::default(), ConnState::Established);
        let id = Streams {
            state: &mut server,
            conn_state: &state,
        }
        .open(Dir::Uni)
        .unwrap();

        let mut stream = SendStream {
            id,
            state: &mut server,
            pending: &mut pending,
            conn_state: &state,
        };

        let error_code = 0u32.into();
        stream.state.received_stop_sending(id, error_code);
        assert!(
            stream
                .state
                .events
                .contains(&StreamEvent::Stopped { id, error_code })
        );
        stream.state.events.clear();

        assert_eq!(stream.write(&[]), Err(WriteError::Stopped(error_code)));

        stream.reset(0u32.into()).unwrap();
        assert_eq!(stream.write(&[]), Err(WriteError::ClosedStream));

        // A duplicate frame is a no-op
        stream.state.received_stop_sending(id, error_code);
        assert!(stream.state.events.is_empty());
    }

    #[test]
    fn final_offset_flow_control() {
        let mut client = make(Side::Client);
        assert_eq!(
            client
                .received_reset(frame::ResetStream {
                    id: StreamId::new(Side::Server, Dir::Uni, 0),
                    error_code: 0u32.into(),
                    final_offset: VarInt::MAX,
                })
                .unwrap_err()
                .code,
            TransportErrorCode::FLOW_CONTROL_ERROR
        );
    }

    #[test]
    fn stream_priority() {
        let mut server = make(Side::Server);
        server.set_params(&TransportParameters {
            initial_max_streams_bidi: 3u32.into(),
            initial_max_data: 10u32.into(),
            initial_max_stream_data_bidi_remote: 10u32.into(),
            ..TransportParameters::default()
        });

        let (mut pending, state) = (Retransmits::default(), ConnState::Established);
        let mut streams = Streams {
            state: &mut server,
            conn_state: &state,
        };

        let id_high = streams.open(Dir::Bi).unwrap();
        let id_mid = streams.open(Dir::Bi).unwrap();
        let id_low = streams.open(Dir::Bi).unwrap();

        let mut mid = SendStream {
            id: id_mid,
            state: &mut server,
            pending: &mut pending,
            conn_state: &state,
        };
        mid.write(b"mid").unwrap();

        let mut low = SendStream {
            id: id_low,
            state: &mut server,
            pending: &mut pending,
            conn_state: &state,
        };
        low.set_priority(-1).unwrap();
        low.write(b"low").unwrap();

        let mut high = SendStream {
            id: id_high,
            state: &mut server,
            pending: &mut pending,
            conn_state: &state,
        };
        high.set_priority(1).unwrap();
        high.write(b"high").unwrap();

        let mut buf = Vec::with_capacity(40);
        let meta = server.write_stream_frames(&mut buf, 40, true);
        assert_eq!(meta[0].id, id_high);
        assert_eq!(meta[1].id, id_mid);
        assert_eq!(meta[2].id, id_low);

        assert!(!server.can_send_stream_data());
        assert_eq!(server.pending.len(), 0);
    }

    #[test]
    fn requeue_stream_priority() {
        let mut server = make(Side::Server);
        server.set_params(&TransportParameters {
            initial_max_streams_bidi: 3u32.into(),
            initial_max_data: 1000u32.into(),
            initial_max_stream_data_bidi_remote: 1000u32.into(),
            ..TransportParameters::default()
        });

        let (mut pending, state) = (Retransmits::default(), ConnState::Established);
        let mut streams = Streams {
            state: &mut server,
            conn_state: &state,
        };

        let id_high = streams.open(Dir::Bi).unwrap();
        let id_mid = streams.open(Dir::Bi).unwrap();

        let mut mid = SendStream {
            id: id_mid,
            state: &mut server,
            pending: &mut pending,
            conn_state: &state,
        };
        assert_eq!(mid.write(b"mid").unwrap(), 3);
        assert_eq!(server.pending.len(), 1);

        let mut high = SendStream {
            id: id_high,
            state: &mut server,
            pending: &mut pending,
            conn_state: &state,
        };
        high.set_priority(1).unwrap();
        assert_eq!(high.write(&[0; 200]).unwrap(), 200);
        assert_eq!(server.pending.len(), 2);

        // Requeue the high priority stream to lowest priority. The initial send
        // still uses high priority since it's queued that way. After that it will
        // switch to low priority
        let mut high = SendStream {
            id: id_high,
            state: &mut server,
            pending: &mut pending,
            conn_state: &state,
        };
        high.set_priority(-1).unwrap();

        let mut buf = Vec::with_capacity(1000);
        let meta = server.write_stream_frames(&mut buf, 40, true);
        assert_eq!(meta.len(), 1);
        assert_eq!(meta[0].id, id_high);

        // After requeuing we should end up with 2 priorities - not 3
        assert_eq!(server.pending.len(), 2);

        // Send the remaining data. The initial mid priority one should go first now
        let meta = server.write_stream_frames(&mut buf, 1000, true);
        assert_eq!(meta.len(), 2);
        assert_eq!(meta[0].id, id_mid);
        assert_eq!(meta[1].id, id_high);

        assert!(!server.can_send_stream_data());
        assert_eq!(server.pending.len(), 0);
    }

    #[test]
    fn same_stream_priority() {
        for fair in [true, false] {
            let mut server = make(Side::Server);
            server.set_params(&TransportParameters {
                initial_max_streams_bidi: 3u32.into(),
                initial_max_data: 300u32.into(),
                initial_max_stream_data_bidi_remote: 300u32.into(),
                ..TransportParameters::default()
            });

            let (mut pending, state) = (Retransmits::default(), ConnState::Established);
            let mut streams = Streams {
                state: &mut server,
                conn_state: &state,
            };

            // a, b and c all have the same priority
            let id_a = streams.open(Dir::Bi).unwrap();
            let id_b = streams.open(Dir::Bi).unwrap();
            let id_c = streams.open(Dir::Bi).unwrap();

            let mut stream_a = SendStream {
                id: id_a,
                state: &mut server,
                pending: &mut pending,
                conn_state: &state,
            };
            stream_a.write(&[b'a'; 100]).unwrap();

            let mut stream_b = SendStream {
                id: id_b,
                state: &mut server,
                pending: &mut pending,
                conn_state: &state,
            };
            stream_b.write(&[b'b'; 100]).unwrap();

            let mut stream_c = SendStream {
                id: id_c,
                state: &mut server,
                pending: &mut pending,
                conn_state: &state,
            };
            stream_c.write(&[b'c'; 100]).unwrap();

            let mut metas = vec![];
            let mut buf = Vec::with_capacity(1024);

            // loop until all the streams are written
            loop {
                let buf_len = buf.len();
                let meta = server.write_stream_frames(&mut buf, buf_len + 40, fair);
                if meta.is_empty() {
                    break;
                }
                metas.extend(meta);
            }

            assert!(!server.can_send_stream_data());
            assert_eq!(server.pending.len(), 0);

            let stream_ids = metas.iter().map(|m| m.id).collect::<Vec<_>>();
            if fair {
                // When fairness is enabled, if we run out of buffer space to write out a stream,
                // the stream is re-queued after all the streams with the same priority.
                assert_eq!(
                    stream_ids,
                    vec![id_a, id_b, id_c, id_a, id_b, id_c, id_a, id_b, id_c]
                );
            } else {
                // When fairness is disabled the stream is re-queued before all the other streams
                // with the same priority.
                assert_eq!(
                    stream_ids,
                    vec![id_a, id_a, id_a, id_b, id_b, id_b, id_c, id_c, id_c]
                );
            }
        }
    }

    #[test]
    fn unfair_priority_bump() {
        let mut server = make(Side::Server);
        server.set_params(&TransportParameters {
            initial_max_streams_bidi: 3u32.into(),
            initial_max_data: 300u32.into(),
            initial_max_stream_data_bidi_remote: 300u32.into(),
            ..TransportParameters::default()
        });

        let (mut pending, state) = (Retransmits::default(), ConnState::Established);
        let mut streams = Streams {
            state: &mut server,
            conn_state: &state,
        };

        // a, and b have the same priority, c has higher priority
        let id_a = streams.open(Dir::Bi).unwrap();
        let id_b = streams.open(Dir::Bi).unwrap();
        let id_c = streams.open(Dir::Bi).unwrap();

        let mut stream_a = SendStream {
            id: id_a,
            state: &mut server,
            pending: &mut pending,
            conn_state: &state,
        };
        stream_a.write(&[b'a'; 100]).unwrap();

        let mut stream_b = SendStream {
            id: id_b,
            state: &mut server,
            pending: &mut pending,
            conn_state: &state,
        };
        stream_b.write(&[b'b'; 100]).unwrap();

        let mut metas = vec![];
        let mut buf = Vec::with_capacity(1024);

        // Write the first chunk of stream_a
        let buf_len = buf.len();
        let meta = server.write_stream_frames(&mut buf, buf_len + 40, false);
        assert!(!meta.is_empty());
        metas.extend(meta);

        // Queue stream_c which has higher priority
        let mut stream_c = SendStream {
            id: id_c,
            state: &mut server,
            pending: &mut pending,
            conn_state: &state,
        };
        stream_c.set_priority(1).unwrap();
        stream_c.write(&[b'b'; 100]).unwrap();

        // loop until all the streams are written
        loop {
            let buf_len = buf.len();
            let meta = server.write_stream_frames(&mut buf, buf_len + 40, false);
            if meta.is_empty() {
                break;
            }
            metas.extend(meta);
        }

        assert!(!server.can_send_stream_data());
        assert_eq!(server.pending.len(), 0);

        let stream_ids = metas.iter().map(|m| m.id).collect::<Vec<_>>();
        assert_eq!(
            stream_ids,
            // stream_c bumps stream_b but doesn't bump stream_a which had already been partly
            // written out
            vec![id_a, id_a, id_a, id_c, id_c, id_c, id_b, id_b, id_b]
        );
    }

    #[test]
    fn stop_finished() {
        let mut client = make(Side::Client);
        let id = StreamId::new(Side::Server, Dir::Uni, 0);
        // Server finishes stream
        let _ = client
            .received(
                frame::Stream {
                    id,
                    offset: 0,
                    fin: true,
                    data: Bytes::from_static(&[0; 32]),
                },
                32,
            )
            .unwrap();
        let mut pending = Retransmits::default();
        let mut stream = RecvStream {
            id,
            state: &mut client,
            pending: &mut pending,
        };
        stream.stop(0u32.into()).unwrap();
        assert!(client.recv.get_mut(&id).is_none(), "stream is freed");
    }

    // Verify that a stream that's been reset doesn't cause the appearance of pending data
    #[test]
    fn reset_stream_cannot_send() {
        let mut server = make(Side::Server);
        server.set_params(&TransportParameters {
            initial_max_streams_uni: 1u32.into(),
            initial_max_data: 42u32.into(),
            initial_max_stream_data_uni: 42u32.into(),
            ..TransportParameters::default()
        });
        let (mut pending, state) = (Retransmits::default(), ConnState::Established);
        let mut streams = Streams {
            state: &mut server,
            conn_state: &state,
        };

        let id = streams.open(Dir::Uni).unwrap();
        let mut stream = SendStream {
            id,
            state: &mut server,
            pending: &mut pending,
            conn_state: &state,
        };
        stream.write(b"hello").unwrap();
        stream.reset(0u32.into()).unwrap();

        assert_eq!(pending.reset_stream, &[(id, 0u32.into())]);
        assert!(!server.can_send_stream_data());
    }

    #[test]
    fn stream_limit_fixed() {
        let mut client = make(Side::Client);
        // Open streams 0-127
        assert_eq!(
            client.received(
                frame::Stream {
                    id: StreamId::new(Side::Server, Dir::Uni, 127),
                    offset: 0,
                    fin: true,
                    data: Bytes::from_static(&[]),
                },
                0
            ),
            Ok(ShouldTransmit(false))
        );
        // Try to open stream 128, exceeding limit
        assert_eq!(
            client
                .received(
                    frame::Stream {
                        id: StreamId::new(Side::Server, Dir::Uni, 128),
                        offset: 0,
                        fin: true,
                        data: Bytes::from_static(&[]),
                    },
                    0
                )
                .unwrap_err()
                .code,
            TransportErrorCode::STREAM_LIMIT_ERROR
        );

        // Free stream 127
        let mut pending = Retransmits::default();
        let mut stream = RecvStream {
            id: StreamId::new(Side::Server, Dir::Uni, 127),
            state: &mut client,
            pending: &mut pending,
        };
        stream.stop(0u32.into()).unwrap();

        // Open stream 128
        assert_eq!(
            client.received(
                frame::Stream {
                    id: StreamId::new(Side::Server, Dir::Uni, 128),
                    offset: 0,
                    fin: true,
                    data: Bytes::from_static(&[]),
                },
                0
            ),
            Ok(ShouldTransmit(false))
        );
    }

    #[test]
    fn stream_limit_grows() {
        let mut client = make(Side::Client);
        // Open streams 0-127
        assert_eq!(
            client.received(
                frame::Stream {
                    id: StreamId::new(Side::Server, Dir::Uni, 127),
                    offset: 0,
                    fin: true,
                    data: Bytes::from_static(&[]),
                },
                0
            ),
            Ok(ShouldTransmit(false))
        );
        // Try to open stream 128, exceeding limit
        assert_eq!(
            client
                .received(
                    frame::Stream {
                        id: StreamId::new(Side::Server, Dir::Uni, 128),
                        offset: 0,
                        fin: true,
                        data: Bytes::from_static(&[]),
                    },
                    0
                )
                .unwrap_err()
                .code,
            TransportErrorCode::STREAM_LIMIT_ERROR
        );

        // Relax limit by one
        client.set_max_concurrent(Dir::Uni, 129u32.into());

        // Open stream 128
        assert_eq!(
            client.received(
                frame::Stream {
                    id: StreamId::new(Side::Server, Dir::Uni, 128),
                    offset: 0,
                    fin: true,
                    data: Bytes::from_static(&[]),
                },
                0
            ),
            Ok(ShouldTransmit(false))
        );
    }

    #[test]
    fn stream_limit_shrinks() {
        let mut client = make(Side::Client);
        // Open streams 0-127
        assert_eq!(
            client.received(
                frame::Stream {
                    id: StreamId::new(Side::Server, Dir::Uni, 127),
                    offset: 0,
                    fin: true,
                    data: Bytes::from_static(&[]),
                },
                0
            ),
            Ok(ShouldTransmit(false))
        );

        // Tighten limit by one
        client.set_max_concurrent(Dir::Uni, 127u32.into());

        // Free stream 127
        let mut pending = Retransmits::default();
        let mut stream = RecvStream {
            id: StreamId::new(Side::Server, Dir::Uni, 127),
            state: &mut client,
            pending: &mut pending,
        };
        stream.stop(0u32.into()).unwrap();

        // Try to open stream 128, still exceeding limit
        assert_eq!(
            client
                .received(
                    frame::Stream {
                        id: StreamId::new(Side::Server, Dir::Uni, 128),
                        offset: 0,
                        fin: true,
                        data: Bytes::from_static(&[]),
                    },
                    0
                )
                .unwrap_err()
                .code,
            TransportErrorCode::STREAM_LIMIT_ERROR
        );

        // Free stream 126
        assert_eq!(
            client.received_reset(frame::ResetStream {
                id: StreamId::new(Side::Server, Dir::Uni, 126),
                error_code: 0u32.into(),
                final_offset: 0u32.into(),
            }),
            Ok(ShouldTransmit(false))
        );
        let mut pending = Retransmits::default();
        let mut stream = RecvStream {
            id: StreamId::new(Side::Server, Dir::Uni, 126),
            state: &mut client,
            pending: &mut pending,
        };
        stream.stop(0u32.into()).unwrap();

        // Open stream 128
        assert_eq!(
            client.received(
                frame::Stream {
                    id: StreamId::new(Side::Server, Dir::Uni, 128),
                    offset: 0,
                    fin: true,
                    data: Bytes::from_static(&[]),
                },
                0
            ),
            Ok(ShouldTransmit(false))
        );
    }

    #[test]
    fn remote_stream_capacity() {
        let mut client = make(Side::Client);
        for _ in 0..2 {
            client.set_max_concurrent(Dir::Uni, 200u32.into());
            client.set_max_concurrent(Dir::Bi, 201u32.into());
            assert_eq!(client.recv.len(), 200 + 201);
            assert_eq!(client.max_remote[Dir::Uni as usize], 200);
            assert_eq!(client.max_remote[Dir::Bi as usize], 201);
        }
    }

    #[test]
    fn expand_receive_window() {
        let mut server = make(Side::Server);
        let new_receive_window = 2 * server.receive_window as u32;
        let expanded = server.set_receive_window(new_receive_window.into());
        assert!(expanded);
        assert_eq!(server.receive_window, new_receive_window as u64);
        assert_eq!(server.local_max_data, new_receive_window as u64);
        assert_eq!(server.receive_window_shrink_debt, 0);
        let prev_local_max_data = server.local_max_data;

        // credit, expecting all of them added to local_max_data
        let credits = 1024u64;
        let should_transmit = server.add_read_credits(credits);
        assert_eq!(server.receive_window_shrink_debt, 0);
        assert_eq!(server.local_max_data, prev_local_max_data + credits);
        assert!(should_transmit.should_transmit());
    }

    #[test]
    fn shrink_receive_window() {
        let mut server = make(Side::Server);
        let new_receive_window = server.receive_window as u32 / 2;
        let prev_local_max_data = server.local_max_data;

        // shrink the receive_winbow, local_max_data is not expected to be changed
        let shrink_diff = server.receive_window - new_receive_window as u64;
        let expanded = server.set_receive_window(new_receive_window.into());
        assert!(!expanded);
        assert_eq!(server.receive_window, new_receive_window as u64);
        assert_eq!(server.local_max_data, prev_local_max_data);
        assert_eq!(server.receive_window_shrink_debt, shrink_diff);
        let prev_local_max_data = server.local_max_data;

        // credit twice, local_max_data does not change as it is absorbed by receive_window_shrink_debt
        let credits = 1024u64;
        for _ in 0..2 {
            let expected_receive_window_shrink_debt = server.receive_window_shrink_debt - credits;
            let should_transmit = server.add_read_credits(credits);
            assert_eq!(
                server.receive_window_shrink_debt,
                expected_receive_window_shrink_debt
            );
            assert_eq!(server.local_max_data, prev_local_max_data);
            assert!(!should_transmit.should_transmit());
        }

        // credit again which exceeds all remaining expected_receive_window_shrink_debt
        let credits = 1024 * 512;
        let prev_local_max_data = server.local_max_data;
        let expected_local_max_data =
            server.local_max_data + (credits - server.receive_window_shrink_debt);
        let _should_transmit = server.add_read_credits(credits);
        assert_eq!(server.receive_window_shrink_debt, 0);
        assert_eq!(server.local_max_data, expected_local_max_data);
        assert!(server.local_max_data > prev_local_max_data);

        // credit again, all should be added to local_max_data
        let credits = 1024 * 512;
        let expected_local_max_data = server.local_max_data + credits;
        let should_transmit = server.add_read_credits(credits);
        assert_eq!(server.receive_window_shrink_debt, 0);
        assert_eq!(server.local_max_data, expected_local_max_data);
        assert!(should_transmit.should_transmit());
    }

    #[test]
    fn expand_send_window() {
        let mut server = make(Side::Server);

        let initial_send_window = server.send_window;
        let larger_send_window = initial_send_window * 2;

        // Set `initial_max_data` larger than `send_window` so we're limited by local flow control
        server.set_params(&TransportParameters {
            initial_max_data: VarInt::MAX,
            initial_max_stream_data_uni: VarInt::MAX,
            initial_max_streams_uni: VarInt::from_u32(100),
            ..TransportParameters::default()
        });

        assert_eq!(server.write_limit(), initial_send_window);
        assert_eq!(server.poll(), None);

        let mut retransmits = Retransmits::default();
        let conn_state = ConnState::Established;

        let stream_id = Streams {
            state: &mut server,
            conn_state: &conn_state,
        }
        .open(Dir::Uni)
        .expect("should be able to open a stream");

        let mut stream = SendStream {
            id: stream_id,
            state: &mut server,
            pending: &mut retransmits,
            conn_state: &conn_state,
        };

        // Check that the stream accepts `initial_send_window` bytes
        let initial_send_len = initial_send_window as usize;
        let data = vec![0xFFu8; initial_send_len];

        assert_eq!(stream.write(&data), Ok(initial_send_len));

        // Try to write the same data again, observe that it's blocked
        assert_eq!(stream.write(&data), Err(WriteError::Blocked));

        // Check that we get a `Writable` event after increasing the send window
        stream.state.set_send_window(larger_send_window);
        assert_eq!(
            stream.state.poll(),
            Some(StreamEvent::Writable { id: stream_id })
        );

        // Check that the stream accepts the exact same amount of data again
        assert_eq!(stream.write(&data), Ok(initial_send_len));
        assert_eq!(stream.write(&data), Err(WriteError::Blocked));

        assert_eq!(stream.state.poll(), None);

        // Ack the data
        stream.state.received_ack_of(frame::StreamMeta {
            id: stream_id,
            offsets: 0..larger_send_window,
            fin: false,
        });

        assert_eq!(
            stream.state.poll(),
            Some(StreamEvent::Writable { id: stream_id })
        );

        // Check that our full send window is available again
        assert_eq!(stream.write(&data), Ok(initial_send_len));
        assert_eq!(stream.write(&data), Ok(initial_send_len));
        assert_eq!(stream.write(&data), Err(WriteError::Blocked));
    }

    #[test]
    fn shrink_send_window() {
        let mut server = make(Side::Server);

        let initial_send_window = server.send_window;
        let smaller_send_window = server.send_window / 2;

        // Set `initial_max_data` larger than `send_window` so we're limited by local flow control
        server.set_params(&TransportParameters {
            initial_max_data: VarInt::MAX,
            initial_max_stream_data_uni: VarInt::MAX,
            initial_max_streams_uni: VarInt::from_u32(100),
            ..TransportParameters::default()
        });

        assert_eq!(server.write_limit(), initial_send_window);
        assert_eq!(server.poll(), None);

        let mut retransmits = Retransmits::default();
        let conn_state = ConnState::Established;

        let stream_id = Streams {
            state: &mut server,
            conn_state: &conn_state,
        }
        .open(Dir::Uni)
        .expect("should be able to open a stream");

        let mut stream = SendStream {
            id: stream_id,
            state: &mut server,
            pending: &mut retransmits,
            conn_state: &conn_state,
        };

        let initial_send_len = initial_send_window as usize;

        let data = vec![0xFFu8; initial_send_len];

        // Assert that the full send window is accepted
        assert_eq!(stream.write(&data), Ok(initial_send_len));
        assert_eq!(stream.write(&data), Err(WriteError::Blocked));

        assert_eq!(stream.state.write_limit(), 0);
        assert_eq!(stream.state.poll(), None);

        // Shrink our send window, assert that it's still not writable
        stream.state.set_send_window(smaller_send_window);
        assert_eq!(stream.state.write_limit(), 0);
        assert_eq!(stream.state.poll(), None);

        // Assert that data is still not accepted
        assert_eq!(stream.write(&data), Err(WriteError::Blocked));

        // Ack some data, assert that writes are still not accepted due to outstanding sends
        stream.state.received_ack_of(frame::StreamMeta {
            id: stream_id,
            offsets: 0..smaller_send_window,
            fin: false,
        });

        assert_eq!(stream.write(&data), Err(WriteError::Blocked));

        // Ack the rest of the data
        stream.state.received_ack_of(frame::StreamMeta {
            id: stream_id,
            offsets: smaller_send_window..initial_send_window,
            fin: false,
        });

        // This should generate a `Writable` event
        assert_eq!(
            stream.state.poll(),
            Some(StreamEvent::Writable { id: stream_id })
        );
        assert_eq!(stream.state.write_limit(), smaller_send_window);

        // Assert that only `smaller_send_window` bytes are accepted
        assert_eq!(stream.write(&data), Ok(smaller_send_window as usize));
        assert_eq!(stream.write(&data), Err(WriteError::Blocked));
    }
}
