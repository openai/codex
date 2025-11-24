use std::{
    cmp,
    collections::VecDeque,
    convert::TryFrom,
    fmt, io, mem,
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use bytes::{Bytes, BytesMut};
use frame::StreamMetaVec;

use rand::{Rng, SeedableRng, rngs::StdRng};
use thiserror::Error;
use tracing::{debug, error, trace, trace_span, warn};

use crate::{
    Dir, Duration, EndpointConfig, Frame, INITIAL_MTU, Instant, MAX_CID_SIZE, MAX_STREAM_COUNT,
    MIN_INITIAL_SIZE, Side, StreamId, TIMER_GRANULARITY, TokenStore, Transmit, TransportError,
    TransportErrorCode, VarInt,
    cid_generator::ConnectionIdGenerator,
    cid_queue::CidQueue,
    coding::BufMutExt,
    config::{ServerConfig, TransportConfig},
    crypto::{self, KeyPair, Keys, PacketKey},
    frame::{self, Close, Datagram, FrameStruct, NewConnectionId, NewToken},
    packet::{
        FixedLengthConnectionIdParser, Header, InitialHeader, InitialPacket, LongType, Packet,
        PacketNumber, PartialDecode, SpaceId,
    },
    range_set::ArrayRangeSet,
    shared::{
        ConnectionEvent, ConnectionEventInner, ConnectionId, DatagramConnectionEvent, EcnCodepoint,
        EndpointEvent, EndpointEventInner,
    },
    token::{ResetToken, Token, TokenPayload},
    transport_parameters::TransportParameters,
};

mod ack_frequency;
use ack_frequency::AckFrequencyState;

mod assembler;
pub use assembler::Chunk;

mod cid_state;
use cid_state::CidState;

mod datagrams;
use datagrams::DatagramState;
pub use datagrams::{Datagrams, SendDatagramError};

mod mtud;
mod pacing;

mod packet_builder;
use packet_builder::PacketBuilder;

mod packet_crypto;
use packet_crypto::{PrevCrypto, ZeroRttCrypto};

mod paths;
pub use paths::RttEstimator;
use paths::{PathData, PathResponses};

pub(crate) mod qlog;

mod send_buffer;

mod spaces;
#[cfg(fuzzing)]
pub use spaces::Retransmits;
#[cfg(not(fuzzing))]
use spaces::Retransmits;
use spaces::{PacketNumberFilter, PacketSpace, SendableFrames, SentPacket, ThinRetransmits};

mod stats;
pub use stats::{ConnectionStats, FrameStats, PathStats, UdpStats};

mod streams;
#[cfg(fuzzing)]
pub use streams::StreamsState;
#[cfg(not(fuzzing))]
use streams::StreamsState;
pub use streams::{
    Chunks, ClosedStream, FinishError, ReadError, ReadableError, RecvStream, SendStream,
    ShouldTransmit, StreamEvent, Streams, WriteError, Written,
};

mod timer;
use crate::congestion::Controller;
use timer::{Timer, TimerTable};

/// Protocol state and logic for a single QUIC connection
///
/// Objects of this type receive [`ConnectionEvent`]s and emit [`EndpointEvent`]s and application
/// [`Event`]s to make progress. To handle timeouts, a `Connection` returns timer updates and
/// expects timeouts through various methods. A number of simple getter methods are exposed
/// to allow callers to inspect some of the connection state.
///
/// `Connection` has roughly 4 types of methods:
///
/// - A. Simple getters, taking `&self`
/// - B. Handlers for incoming events from the network or system, named `handle_*`.
/// - C. State machine mutators, for incoming commands from the application. For convenience we
///   refer to this as "performing I/O" below, however as per the design of this library none of the
///   functions actually perform system-level I/O. For example, [`read`](RecvStream::read) and
///   [`write`](SendStream::write), but also things like [`reset`](SendStream::reset).
/// - D. Polling functions for outgoing events or actions for the caller to
///   take, named `poll_*`.
///
/// The simplest way to use this API correctly is to call (B) and (C) whenever
/// appropriate, then after each of those calls, as soon as feasible call all
/// polling methods (D) and deal with their outputs appropriately, e.g. by
/// passing it to the application or by making a system-level I/O call. You
/// should call the polling functions in this order:
///
/// 1. [`poll_transmit`](Self::poll_transmit)
/// 2. [`poll_timeout`](Self::poll_timeout)
/// 3. [`poll_endpoint_events`](Self::poll_endpoint_events)
/// 4. [`poll`](Self::poll)
///
/// Currently the only actual dependency is from (2) to (1), however additional
/// dependencies may be added in future, so the above order is recommended.
///
/// (A) may be called whenever desired.
///
/// Care should be made to ensure that the input events represent monotonically
/// increasing time. Specifically, calling [`handle_timeout`](Self::handle_timeout)
/// with events of the same [`Instant`] may be interleaved in any order with a
/// call to [`handle_event`](Self::handle_event) at that same instant; however
/// events or timeouts with different instants must not be interleaved.
pub struct Connection {
    endpoint_config: Arc<EndpointConfig>,
    config: Arc<TransportConfig>,
    rng: StdRng,
    crypto: Box<dyn crypto::Session>,
    /// The CID we initially chose, for use during the handshake
    handshake_cid: ConnectionId,
    /// The CID the peer initially chose, for use during the handshake
    rem_handshake_cid: ConnectionId,
    /// The "real" local IP address which was was used to receive the initial packet.
    /// This is only populated for the server case, and if known
    local_ip: Option<IpAddr>,
    path: PathData,
    /// Incremented every time we see a new path
    ///
    /// Stored separately from `path.generation` to account for aborted migrations
    path_counter: u64,
    /// Whether MTU detection is supported in this environment
    allow_mtud: bool,
    prev_path: Option<(ConnectionId, PathData)>,
    state: State,
    side: ConnectionSide,
    /// Whether or not 0-RTT was enabled during the handshake. Does not imply acceptance.
    zero_rtt_enabled: bool,
    /// Set if 0-RTT is supported, then cleared when no longer needed.
    zero_rtt_crypto: Option<ZeroRttCrypto>,
    key_phase: bool,
    /// How many packets are in the current key phase. Used only for `Data` space.
    key_phase_size: u64,
    /// Transport parameters set by the peer
    peer_params: TransportParameters,
    /// Source ConnectionId of the first packet received from the peer
    orig_rem_cid: ConnectionId,
    /// Destination ConnectionId sent by the client on the first Initial
    initial_dst_cid: ConnectionId,
    /// The value that the server included in the Source Connection ID field of a Retry packet, if
    /// one was received
    retry_src_cid: Option<ConnectionId>,
    events: VecDeque<Event>,
    endpoint_events: VecDeque<EndpointEventInner>,
    /// Whether the spin bit is in use for this connection
    spin_enabled: bool,
    /// Outgoing spin bit state
    spin: bool,
    /// Packet number spaces: initial, handshake, 1-RTT
    spaces: [PacketSpace; 3],
    /// Highest usable packet number space
    highest_space: SpaceId,
    /// 1-RTT keys used prior to a key update
    prev_crypto: Option<PrevCrypto>,
    /// 1-RTT keys to be used for the next key update
    ///
    /// These are generated in advance to prevent timing attacks and/or DoS by third-party attackers
    /// spoofing key updates.
    next_crypto: Option<KeyPair<Box<dyn PacketKey>>>,
    accepted_0rtt: bool,
    /// Whether the idle timer should be reset the next time an ack-eliciting packet is transmitted.
    permit_idle_reset: bool,
    /// Negotiated idle timeout
    idle_timeout: Option<Duration>,
    timers: TimerTable,
    /// Number of packets received which could not be authenticated
    authentication_failures: u64,
    /// Why the connection was lost, if it has been
    error: Option<ConnectionError>,
    /// Identifies Data-space packet numbers to skip. Not used in earlier spaces.
    packet_number_filter: PacketNumberFilter,

    //
    // Queued non-retransmittable 1-RTT data
    //
    /// Responses to PATH_CHALLENGE frames
    path_responses: PathResponses,
    close: bool,

    //
    // ACK frequency
    //
    ack_frequency: AckFrequencyState,

    //
    // Loss Detection
    //
    /// The number of times a PTO has been sent without receiving an ack.
    pto_count: u32,

    //
    // Congestion Control
    //
    /// Whether the most recently received packet had an ECN codepoint set
    receiving_ecn: bool,
    /// Number of packets authenticated
    total_authed_packets: u64,
    /// Whether the last `poll_transmit` call yielded no data because there was
    /// no outgoing application data.
    app_limited: bool,

    streams: StreamsState,
    /// Surplus remote CIDs for future use on new paths
    rem_cids: CidQueue,
    // Attributes of CIDs generated by local peer
    local_cid_state: CidState,
    /// State of the unreliable datagram extension
    datagrams: DatagramState,
    /// Connection level statistics
    stats: ConnectionStats,
    /// QUIC version used for the connection.
    version: u32,
}

impl Connection {
    pub(crate) fn new(
        endpoint_config: Arc<EndpointConfig>,
        config: Arc<TransportConfig>,
        init_cid: ConnectionId,
        loc_cid: ConnectionId,
        rem_cid: ConnectionId,
        remote: SocketAddr,
        local_ip: Option<IpAddr>,
        crypto: Box<dyn crypto::Session>,
        cid_gen: &dyn ConnectionIdGenerator,
        now: Instant,
        version: u32,
        allow_mtud: bool,
        rng_seed: [u8; 32],
        side_args: SideArgs,
    ) -> Self {
        let pref_addr_cid = side_args.pref_addr_cid();
        let path_validated = side_args.path_validated();
        let connection_side = ConnectionSide::from(side_args);
        let side = connection_side.side();
        let initial_space = PacketSpace {
            crypto: Some(crypto.initial_keys(&init_cid, side)),
            ..PacketSpace::new(now)
        };
        let state = State::Handshake(state::Handshake {
            rem_cid_set: side.is_server(),
            expected_token: Bytes::new(),
            client_hello: None,
        });
        let mut rng = StdRng::from_seed(rng_seed);
        let mut this = Self {
            endpoint_config,
            crypto,
            handshake_cid: loc_cid,
            rem_handshake_cid: rem_cid,
            local_cid_state: CidState::new(
                cid_gen.cid_len(),
                cid_gen.cid_lifetime(),
                now,
                if pref_addr_cid.is_some() { 2 } else { 1 },
            ),
            path: PathData::new(remote, allow_mtud, None, 0, now, &config),
            path_counter: 0,
            allow_mtud,
            local_ip,
            prev_path: None,
            state,
            side: connection_side,
            zero_rtt_enabled: false,
            zero_rtt_crypto: None,
            key_phase: false,
            // A small initial key phase size ensures peers that don't handle key updates correctly
            // fail sooner rather than later. It's okay for both peers to do this, as the first one
            // to perform an update will reset the other's key phase size in `update_keys`, and a
            // simultaneous key update by both is just like a regular key update with a really fast
            // response. Inspired by quic-go's similar behavior of performing the first key update
            // at the 100th short-header packet.
            key_phase_size: rng.random_range(10..1000),
            peer_params: TransportParameters::default(),
            orig_rem_cid: rem_cid,
            initial_dst_cid: init_cid,
            retry_src_cid: None,
            events: VecDeque::new(),
            endpoint_events: VecDeque::new(),
            spin_enabled: config.allow_spin && rng.random_ratio(7, 8),
            spin: false,
            spaces: [initial_space, PacketSpace::new(now), PacketSpace::new(now)],
            highest_space: SpaceId::Initial,
            prev_crypto: None,
            next_crypto: None,
            accepted_0rtt: false,
            permit_idle_reset: true,
            idle_timeout: match config.max_idle_timeout {
                None | Some(VarInt(0)) => None,
                Some(dur) => Some(Duration::from_millis(dur.0)),
            },
            timers: TimerTable::default(),
            authentication_failures: 0,
            error: None,
            #[cfg(test)]
            packet_number_filter: match config.deterministic_packet_numbers {
                false => PacketNumberFilter::new(&mut rng),
                true => PacketNumberFilter::disabled(),
            },
            #[cfg(not(test))]
            packet_number_filter: PacketNumberFilter::new(&mut rng),

            path_responses: PathResponses::default(),
            close: false,

            ack_frequency: AckFrequencyState::new(get_max_ack_delay(
                &TransportParameters::default(),
            )),

            pto_count: 0,

            app_limited: false,
            receiving_ecn: false,
            total_authed_packets: 0,

            streams: StreamsState::new(
                side,
                config.max_concurrent_uni_streams,
                config.max_concurrent_bidi_streams,
                config.send_window,
                config.receive_window,
                config.stream_receive_window,
            ),
            datagrams: DatagramState::default(),
            config,
            rem_cids: CidQueue::new(rem_cid),
            rng,
            stats: ConnectionStats::default(),
            version,
        };
        if path_validated {
            this.on_path_validated();
        }
        if side.is_client() {
            // Kick off the connection
            this.write_crypto();
            this.init_0rtt();
        }
        this
    }

    /// Returns the next time at which `handle_timeout` should be called
    ///
    /// The value returned may change after:
    /// - the application performed some I/O on the connection
    /// - a call was made to `handle_event`
    /// - a call to `poll_transmit` returned `Some`
    /// - a call was made to `handle_timeout`
    #[must_use]
    pub fn poll_timeout(&mut self) -> Option<Instant> {
        self.timers.next_timeout()
    }

    /// Returns application-facing events
    ///
    /// Connections should be polled for events after:
    /// - a call was made to `handle_event`
    /// - a call was made to `handle_timeout`
    #[must_use]
    pub fn poll(&mut self) -> Option<Event> {
        if let Some(x) = self.events.pop_front() {
            return Some(x);
        }

        if let Some(event) = self.streams.poll() {
            return Some(Event::Stream(event));
        }

        if let Some(err) = self.error.take() {
            return Some(Event::ConnectionLost { reason: err });
        }

        None
    }

    /// Return endpoint-facing events
    #[must_use]
    pub fn poll_endpoint_events(&mut self) -> Option<EndpointEvent> {
        self.endpoint_events.pop_front().map(EndpointEvent)
    }

    /// Provide control over streams
    #[must_use]
    pub fn streams(&mut self) -> Streams<'_> {
        Streams {
            state: &mut self.streams,
            conn_state: &self.state,
        }
    }

    /// Provide control over streams
    #[must_use]
    pub fn recv_stream(&mut self, id: StreamId) -> RecvStream<'_> {
        assert!(id.dir() == Dir::Bi || id.initiator() != self.side.side());
        RecvStream {
            id,
            state: &mut self.streams,
            pending: &mut self.spaces[SpaceId::Data].pending,
        }
    }

    /// Provide control over streams
    #[must_use]
    pub fn send_stream(&mut self, id: StreamId) -> SendStream<'_> {
        assert!(id.dir() == Dir::Bi || id.initiator() == self.side.side());
        SendStream {
            id,
            state: &mut self.streams,
            pending: &mut self.spaces[SpaceId::Data].pending,
            conn_state: &self.state,
        }
    }

    /// Returns packets to transmit
    ///
    /// Connections should be polled for transmit after:
    /// - the application performed some I/O on the connection
    /// - a call was made to `handle_event`
    /// - a call was made to `handle_timeout`
    ///
    /// `max_datagrams` specifies how many datagrams can be returned inside a
    /// single Transmit using GSO. This must be at least 1.
    #[must_use]
    pub fn poll_transmit(
        &mut self,
        now: Instant,
        max_datagrams: usize,
        buf: &mut Vec<u8>,
    ) -> Option<Transmit> {
        assert!(max_datagrams != 0);
        let max_datagrams = match self.config.enable_segmentation_offload {
            false => 1,
            true => max_datagrams,
        };

        let mut num_datagrams = 0;
        // Position in `buf` of the first byte of the current UDP datagram. When coalescing QUIC
        // packets, this can be earlier than the start of the current QUIC packet.
        let mut datagram_start = 0;
        let mut segment_size = usize::from(self.path.current_mtu());

        if let Some(challenge) = self.send_path_challenge(now, buf) {
            return Some(challenge);
        }

        // If we need to send a probe, make sure we have something to send.
        for space in SpaceId::iter() {
            let request_immediate_ack =
                space == SpaceId::Data && self.peer_supports_ack_frequency();
            self.spaces[space].maybe_queue_probe(request_immediate_ack, &self.streams);
        }

        // Check whether we need to send a close message
        let close = match self.state {
            State::Drained => {
                self.app_limited = true;
                return None;
            }
            State::Draining | State::Closed(_) => {
                // self.close is only reset once the associated packet had been
                // encoded successfully
                if !self.close {
                    self.app_limited = true;
                    return None;
                }
                true
            }
            _ => false,
        };

        // Check whether we need to send an ACK_FREQUENCY frame
        if let Some(config) = &self.config.ack_frequency_config {
            self.spaces[SpaceId::Data].pending.ack_frequency = self
                .ack_frequency
                .should_send_ack_frequency(self.path.rtt.get(), config, &self.peer_params)
                && self.highest_space == SpaceId::Data
                && self.peer_supports_ack_frequency();
        }

        // Reserving capacity can provide more capacity than we asked for. However, we are not
        // allowed to write more than `segment_size`. Therefore the maximum capacity is tracked
        // separately.
        let mut buf_capacity = 0;

        let mut coalesce = true;
        let mut builder_storage: Option<PacketBuilder> = None;
        let mut sent_frames = None;
        let mut pad_datagram = false;
        let mut pad_datagram_to_mtu = false;
        let mut congestion_blocked = false;

        // Iterate over all spaces and find data to send
        let mut space_idx = 0;
        let spaces = [SpaceId::Initial, SpaceId::Handshake, SpaceId::Data];
        // This loop will potentially spend multiple iterations in the same `SpaceId`,
        // so we cannot trivially rewrite it to take advantage of `SpaceId::iter()`.
        while space_idx < spaces.len() {
            let space_id = spaces[space_idx];
            // Number of bytes available for frames if this is a 1-RTT packet. We're guaranteed to
            // be able to send an individual frame at least this large in the next 1-RTT
            // packet. This could be generalized to support every space, but it's only needed to
            // handle large fixed-size frames, which only exist in 1-RTT (application datagrams). We
            // don't account for coalesced packets potentially occupying space because frames can
            // always spill into the next datagram.
            let pn = self.packet_number_filter.peek(&self.spaces[SpaceId::Data]);
            let frame_space_1rtt =
                segment_size.saturating_sub(self.predict_1rtt_overhead(Some(pn)));

            // Is there data or a close message to send in this space?
            let can_send = self.space_can_send(space_id, frame_space_1rtt);
            if can_send.is_empty() && (!close || self.spaces[space_id].crypto.is_none()) {
                space_idx += 1;
                continue;
            }

            let mut ack_eliciting = !self.spaces[space_id].pending.is_empty(&self.streams)
                || self.spaces[space_id].ping_pending
                || self.spaces[space_id].immediate_ack_pending;
            if space_id == SpaceId::Data {
                ack_eliciting |= self.can_send_1rtt(frame_space_1rtt);
            }

            pad_datagram_to_mtu |= space_id == SpaceId::Data && self.config.pad_to_mtu;

            // Can we append more data into the current buffer?
            // It is not safe to assume that `buf.len()` is the end of the data,
            // since the last packet might not have been finished.
            let buf_end = if let Some(builder) = &builder_storage {
                buf.len().max(builder.min_size) + builder.tag_len
            } else {
                buf.len()
            };

            let tag_len = if let Some(ref crypto) = self.spaces[space_id].crypto {
                crypto.packet.local.tag_len()
            } else if space_id == SpaceId::Data {
                self.zero_rtt_crypto.as_ref().expect(
                    "sending packets in the application data space requires known 0-RTT or 1-RTT keys",
                ).packet.tag_len()
            } else {
                unreachable!("tried to send {:?} packet without keys", space_id)
            };
            if !coalesce || buf_capacity - buf_end < MIN_PACKET_SPACE + tag_len {
                // We need to send 1 more datagram and extend the buffer for that.

                // Is 1 more datagram allowed?
                if num_datagrams >= max_datagrams {
                    // No more datagrams allowed
                    break;
                }

                // Anti-amplification is only based on `total_sent`, which gets
                // updated at the end of this method. Therefore we pass the amount
                // of bytes for datagrams that are already created, as well as 1 byte
                // for starting another datagram. If there is any anti-amplification
                // budget left, we always allow a full MTU to be sent
                // (see https://github.com/quinn-rs/quinn/issues/1082)
                if self
                    .path
                    .anti_amplification_blocked(segment_size as u64 * (num_datagrams as u64) + 1)
                {
                    trace!("blocked by anti-amplification");
                    break;
                }

                // Congestion control and pacing checks
                // Tail loss probes must not be blocked by congestion, or a deadlock could arise
                if ack_eliciting && self.spaces[space_id].loss_probes == 0 {
                    // Assume the current packet will get padded to fill the segment
                    let untracked_bytes = if let Some(builder) = &builder_storage {
                        buf_capacity - builder.partial_encode.start
                    } else {
                        0
                    } as u64;
                    debug_assert!(untracked_bytes <= segment_size as u64);

                    let bytes_to_send = segment_size as u64 + untracked_bytes;
                    if self.path.in_flight.bytes + bytes_to_send >= self.path.congestion.window() {
                        space_idx += 1;
                        congestion_blocked = true;
                        // We continue instead of breaking here in order to avoid
                        // blocking loss probes queued for higher spaces.
                        trace!("blocked by congestion control");
                        continue;
                    }

                    // Check whether the next datagram is blocked by pacing
                    let smoothed_rtt = self.path.rtt.get();
                    if let Some(delay) = self.path.pacing.delay(
                        smoothed_rtt,
                        bytes_to_send,
                        self.path.current_mtu(),
                        self.path.congestion.window(),
                        now,
                    ) {
                        self.timers.set(Timer::Pacing, delay);
                        congestion_blocked = true;
                        // Loss probes should be subject to pacing, even though
                        // they are not congestion controlled.
                        trace!("blocked by pacing");
                        break;
                    }
                }

                // Finish current packet
                if let Some(mut builder) = builder_storage.take() {
                    if pad_datagram {
                        builder.pad_to(MIN_INITIAL_SIZE);
                    }

                    if num_datagrams > 1 || pad_datagram_to_mtu {
                        // If too many padding bytes would be required to continue the GSO batch
                        // after this packet, end the GSO batch here. Ensures that fixed-size frames
                        // with heterogeneous sizes (e.g. application datagrams) won't inadvertently
                        // waste large amounts of bandwidth. The exact threshold is a bit arbitrary
                        // and might benefit from further tuning, though there's no universally
                        // optimal value.
                        //
                        // Additionally, if this datagram is a loss probe and `segment_size` is
                        // larger than `INITIAL_MTU`, then padding it to `segment_size` to continue
                        // the GSO batch would risk failure to recover from a reduction in path
                        // MTU. Loss probes are the only packets for which we might grow
                        // `buf_capacity` by less than `segment_size`.
                        const MAX_PADDING: usize = 16;
                        let packet_len_unpadded = cmp::max(builder.min_size, buf.len())
                            - datagram_start
                            + builder.tag_len;
                        if (packet_len_unpadded + MAX_PADDING < segment_size
                            && !pad_datagram_to_mtu)
                            || datagram_start + segment_size > buf_capacity
                        {
                            trace!(
                                "GSO truncated by demand for {} padding bytes or loss probe",
                                segment_size - packet_len_unpadded
                            );
                            builder_storage = Some(builder);
                            break;
                        }

                        // Pad the current datagram to GSO segment size so it can be included in the
                        // GSO batch.
                        builder.pad_to(segment_size as u16);
                    }

                    builder.finish_and_track(now, self, sent_frames.take(), buf);

                    if num_datagrams == 1 {
                        // Set the segment size for this GSO batch to the size of the first UDP
                        // datagram in the batch. Larger data that cannot be fragmented
                        // (e.g. application datagrams) will be included in a future batch. When
                        // sending large enough volumes of data for GSO to be useful, we expect
                        // packet sizes to usually be consistent, e.g. populated by max-size STREAM
                        // frames or uniformly sized datagrams.
                        segment_size = buf.len();
                        // Clip the unused capacity out of the buffer so future packets don't
                        // overrun
                        buf_capacity = buf.len();

                        // Check whether the data we planned to send will fit in the reduced segment
                        // size. If not, bail out and leave it for the next GSO batch so we don't
                        // end up trying to send an empty packet. We can't easily compute the right
                        // segment size before the original call to `space_can_send`, because at
                        // that time we haven't determined whether we're going to coalesce with the
                        // first datagram or potentially pad it to `MIN_INITIAL_SIZE`.
                        if space_id == SpaceId::Data {
                            let frame_space_1rtt =
                                segment_size.saturating_sub(self.predict_1rtt_overhead(Some(pn)));
                            if self.space_can_send(space_id, frame_space_1rtt).is_empty() {
                                break;
                            }
                        }
                    }
                }

                // Allocate space for another datagram
                let next_datagram_size_limit = match self.spaces[space_id].loss_probes {
                    0 => segment_size,
                    _ => {
                        self.spaces[space_id].loss_probes -= 1;
                        // Clamp the datagram to at most the minimum MTU to ensure that loss probes
                        // can get through and enable recovery even if the path MTU has shrank
                        // unexpectedly.
                        std::cmp::min(segment_size, usize::from(INITIAL_MTU))
                    }
                };
                buf_capacity += next_datagram_size_limit;
                if buf.capacity() < buf_capacity {
                    // We reserve the maximum space for sending `max_datagrams` upfront
                    // to avoid any reallocations if more datagrams have to be appended later on.
                    // Benchmarks have shown shown a 5-10% throughput improvement
                    // compared to continuously resizing the datagram buffer.
                    // While this will lead to over-allocation for small transmits
                    // (e.g. purely containing ACKs), modern memory allocators
                    // (e.g. mimalloc and jemalloc) will pool certain allocation sizes
                    // and therefore this is still rather efficient.
                    buf.reserve(max_datagrams * segment_size);
                }
                num_datagrams += 1;
                coalesce = true;
                pad_datagram = false;
                datagram_start = buf.len();

                debug_assert_eq!(
                    datagram_start % segment_size,
                    0,
                    "datagrams in a GSO batch must be aligned to the segment size"
                );
            } else {
                // We can append/coalesce the next packet into the current
                // datagram.
                // Finish current packet without adding extra padding
                if let Some(builder) = builder_storage.take() {
                    builder.finish_and_track(now, self, sent_frames.take(), buf);
                }
            }

            debug_assert!(buf_capacity - buf.len() >= MIN_PACKET_SPACE);

            //
            // From here on, we've determined that a packet will definitely be sent.
            //

            if self.spaces[SpaceId::Initial].crypto.is_some()
                && space_id == SpaceId::Handshake
                && self.side.is_client()
            {
                // A client stops both sending and processing Initial packets when it
                // sends its first Handshake packet.
                self.discard_space(now, SpaceId::Initial);
            }
            if let Some(ref mut prev) = self.prev_crypto {
                prev.update_unacked = false;
            }

            debug_assert!(
                builder_storage.is_none() && sent_frames.is_none(),
                "Previous packet must have been finished"
            );

            let builder = builder_storage.insert(PacketBuilder::new(
                now,
                space_id,
                self.rem_cids.active(),
                buf,
                buf_capacity,
                datagram_start,
                ack_eliciting,
                self,
            )?);
            coalesce = coalesce && !builder.short_header;

            // https://tools.ietf.org/html/draft-ietf-quic-transport-34#section-14.1
            pad_datagram |=
                space_id == SpaceId::Initial && (self.side.is_client() || ack_eliciting);

            if close {
                trace!("sending CONNECTION_CLOSE");
                // Encode ACKs before the ConnectionClose message, to give the receiver
                // a better approximate on what data has been processed. This is
                // especially important with ack delay, since the peer might not
                // have gotten any other ACK for the data earlier on.
                if !self.spaces[space_id].pending_acks.ranges().is_empty() {
                    Self::populate_acks(
                        now,
                        self.receiving_ecn,
                        &mut SentFrames::default(),
                        &mut self.spaces[space_id],
                        buf,
                        &mut self.stats,
                    );
                }

                // Since there only 64 ACK frames there will always be enough space
                // to encode the ConnectionClose frame too. However we still have the
                // check here to prevent crashes if something changes.
                debug_assert!(
                    buf.len() + frame::ConnectionClose::SIZE_BOUND < builder.max_size,
                    "ACKs should leave space for ConnectionClose"
                );
                if buf.len() + frame::ConnectionClose::SIZE_BOUND < builder.max_size {
                    let max_frame_size = builder.max_size - buf.len();
                    match self.state {
                        State::Closed(state::Closed { ref reason }) => {
                            if space_id == SpaceId::Data || reason.is_transport_layer() {
                                reason.encode(buf, max_frame_size)
                            } else {
                                frame::ConnectionClose {
                                    error_code: TransportErrorCode::APPLICATION_ERROR,
                                    frame_type: None,
                                    reason: Bytes::new(),
                                }
                                .encode(buf, max_frame_size)
                            }
                        }
                        State::Draining => frame::ConnectionClose {
                            error_code: TransportErrorCode::NO_ERROR,
                            frame_type: None,
                            reason: Bytes::new(),
                        }
                        .encode(buf, max_frame_size),
                        _ => unreachable!(
                            "tried to make a close packet when the connection wasn't closed"
                        ),
                    }
                }
                if space_id == self.highest_space {
                    // Don't send another close packet
                    self.close = false;
                    // `CONNECTION_CLOSE` is the final packet
                    break;
                } else {
                    // Send a close frame in every possible space for robustness, per RFC9000
                    // "Immediate Close during the Handshake". Don't bother trying to send anything
                    // else.
                    space_idx += 1;
                    continue;
                }
            }

            // Send an off-path PATH_RESPONSE. Prioritized over on-path data to ensure that path
            // validation can occur while the link is saturated.
            if space_id == SpaceId::Data && num_datagrams == 1 {
                if let Some((token, remote)) = self.path_responses.pop_off_path(self.path.remote) {
                    // `unwrap` guaranteed to succeed because `builder_storage` was populated just
                    // above.
                    let mut builder = builder_storage.take().unwrap();
                    trace!("PATH_RESPONSE {:08x} (off-path)", token);
                    buf.write(frame::FrameType::PATH_RESPONSE);
                    buf.write(token);
                    self.stats.frame_tx.path_response += 1;
                    builder.pad_to(MIN_INITIAL_SIZE);
                    builder.finish_and_track(
                        now,
                        self,
                        Some(SentFrames {
                            non_retransmits: true,
                            ..SentFrames::default()
                        }),
                        buf,
                    );
                    self.stats.udp_tx.on_sent(1, buf.len());
                    return Some(Transmit {
                        destination: remote,
                        size: buf.len(),
                        ecn: None,
                        segment_size: None,
                        src_ip: self.local_ip,
                    });
                }
            }

            let sent =
                self.populate_packet(now, space_id, buf, builder.max_size, builder.exact_number);

            // ACK-only packets should only be sent when explicitly allowed. If we write them due to
            // any other reason, there is a bug which leads to one component announcing write
            // readiness while not writing any data. This degrades performance. The condition is
            // only checked if the full MTU is available and when potentially large fixed-size
            // frames aren't queued, so that lack of space in the datagram isn't the reason for just
            // writing ACKs.
            debug_assert!(
                !(sent.is_ack_only(&self.streams)
                    && !can_send.acks
                    && can_send.other
                    && (buf_capacity - builder.datagram_start) == self.path.current_mtu() as usize
                    && self.datagrams.outgoing.is_empty()),
                "SendableFrames was {can_send:?}, but only ACKs have been written"
            );
            pad_datagram |= sent.requires_padding;

            if sent.largest_acked.is_some() {
                self.spaces[space_id].pending_acks.acks_sent();
                self.timers.stop(Timer::MaxAckDelay);
            }

            // Keep information about the packet around until it gets finalized
            sent_frames = Some(sent);

            // Don't increment space_idx.
            // We stay in the current space and check if there is more data to send.
        }

        // Finish the last packet
        if let Some(mut builder) = builder_storage {
            if pad_datagram {
                builder.pad_to(MIN_INITIAL_SIZE);
            }

            // If this datagram is a loss probe and `segment_size` is larger than `INITIAL_MTU`,
            // then padding it to `segment_size` would risk failure to recover from a reduction in
            // path MTU.
            // Loss probes are the only packets for which we might grow `buf_capacity`
            // by less than `segment_size`.
            if pad_datagram_to_mtu && buf_capacity >= datagram_start + segment_size {
                builder.pad_to(segment_size as u16);
            }

            let last_packet_number = builder.exact_number;
            builder.finish_and_track(now, self, sent_frames, buf);
            self.path
                .congestion
                .on_sent(now, buf.len() as u64, last_packet_number);

            self.config.qlog_sink.emit_recovery_metrics(
                self.pto_count,
                &mut self.path,
                now,
                self.orig_rem_cid,
            );
        }

        self.app_limited = buf.is_empty() && !congestion_blocked;

        // Send MTU probe if necessary
        if buf.is_empty() && self.state.is_established() {
            let space_id = SpaceId::Data;
            let probe_size = self
                .path
                .mtud
                .poll_transmit(now, self.packet_number_filter.peek(&self.spaces[space_id]))?;

            let buf_capacity = probe_size as usize;
            buf.reserve(buf_capacity);

            let mut builder = PacketBuilder::new(
                now,
                space_id,
                self.rem_cids.active(),
                buf,
                buf_capacity,
                0,
                true,
                self,
            )?;

            // We implement MTU probes as ping packets padded up to the probe size
            buf.write(frame::FrameType::PING);
            self.stats.frame_tx.ping += 1;

            // If supported by the peer, we want no delays to the probe's ACK
            if self.peer_supports_ack_frequency() {
                buf.write(frame::FrameType::IMMEDIATE_ACK);
                self.stats.frame_tx.immediate_ack += 1;
            }

            builder.pad_to(probe_size);
            let sent_frames = SentFrames {
                non_retransmits: true,
                ..Default::default()
            };
            builder.finish_and_track(now, self, Some(sent_frames), buf);

            self.stats.path.sent_plpmtud_probes += 1;
            num_datagrams = 1;

            trace!(?probe_size, "writing MTUD probe");
        }

        if buf.is_empty() {
            return None;
        }

        trace!("sending {} bytes in {} datagrams", buf.len(), num_datagrams);
        self.path.total_sent = self.path.total_sent.saturating_add(buf.len() as u64);

        self.stats.udp_tx.on_sent(num_datagrams as u64, buf.len());

        Some(Transmit {
            destination: self.path.remote,
            size: buf.len(),
            ecn: if self.path.sending_ecn {
                Some(EcnCodepoint::Ect0)
            } else {
                None
            },
            segment_size: match num_datagrams {
                1 => None,
                _ => Some(segment_size),
            },
            src_ip: self.local_ip,
        })
    }

    /// Send PATH_CHALLENGE for a previous path if necessary
    fn send_path_challenge(&mut self, now: Instant, buf: &mut Vec<u8>) -> Option<Transmit> {
        let (prev_cid, prev_path) = self.prev_path.as_mut()?;
        if !prev_path.challenge_pending {
            return None;
        }
        prev_path.challenge_pending = false;
        let token = prev_path
            .challenge
            .expect("previous path challenge pending without token");
        let destination = prev_path.remote;
        debug_assert_eq!(
            self.highest_space,
            SpaceId::Data,
            "PATH_CHALLENGE queued without 1-RTT keys"
        );
        buf.reserve(MIN_INITIAL_SIZE as usize);

        let buf_capacity = buf.capacity();

        // Use the previous CID to avoid linking the new path with the previous path. We
        // don't bother accounting for possible retirement of that prev_cid because this is
        // sent once, immediately after migration, when the CID is known to be valid. Even
        // if a post-migration packet caused the CID to be retired, it's fair to pretend
        // this is sent first.
        let mut builder = PacketBuilder::new(
            now,
            SpaceId::Data,
            *prev_cid,
            buf,
            buf_capacity,
            0,
            false,
            self,
        )?;
        trace!("validating previous path with PATH_CHALLENGE {:08x}", token);
        buf.write(frame::FrameType::PATH_CHALLENGE);
        buf.write(token);
        self.stats.frame_tx.path_challenge += 1;

        // An endpoint MUST expand datagrams that contain a PATH_CHALLENGE frame
        // to at least the smallest allowed maximum datagram size of 1200 bytes,
        // unless the anti-amplification limit for the path does not permit
        // sending a datagram of this size
        builder.pad_to(MIN_INITIAL_SIZE);

        builder.finish(self, now, buf);
        self.stats.udp_tx.on_sent(1, buf.len());

        Some(Transmit {
            destination,
            size: buf.len(),
            ecn: None,
            segment_size: None,
            src_ip: self.local_ip,
        })
    }

    /// Indicate what types of frames are ready to send for the given space
    fn space_can_send(&self, space_id: SpaceId, frame_space_1rtt: usize) -> SendableFrames {
        if self.spaces[space_id].crypto.is_none()
            && (space_id != SpaceId::Data
                || self.zero_rtt_crypto.is_none()
                || self.side.is_server())
        {
            // No keys available for this space
            return SendableFrames::empty();
        }
        let mut can_send = self.spaces[space_id].can_send(&self.streams);
        if space_id == SpaceId::Data {
            can_send.other |= self.can_send_1rtt(frame_space_1rtt);
        }
        can_send
    }

    /// Process `ConnectionEvent`s generated by the associated `Endpoint`
    ///
    /// Will execute protocol logic upon receipt of a connection event, in turn preparing signals
    /// (including application `Event`s, `EndpointEvent`s and outgoing datagrams) that should be
    /// extracted through the relevant methods.
    pub fn handle_event(&mut self, event: ConnectionEvent) {
        use ConnectionEventInner::*;
        match event.0 {
            Datagram(DatagramConnectionEvent {
                now,
                remote,
                ecn,
                first_decode,
                remaining,
            }) => {
                // If this packet could initiate a migration and we're a client or a server that
                // forbids migration, drop the datagram. This could be relaxed to heuristically
                // permit NAT-rebinding-like migration.
                if remote != self.path.remote && !self.side.remote_may_migrate() {
                    trace!("discarding packet from unrecognized peer {}", remote);
                    return;
                }

                let was_anti_amplification_blocked = self.path.anti_amplification_blocked(1);

                self.stats.udp_rx.datagrams += 1;
                self.stats.udp_rx.bytes += first_decode.len() as u64;
                let data_len = first_decode.len();

                self.handle_decode(now, remote, ecn, first_decode);
                // The current `path` might have changed inside `handle_decode`,
                // since the packet could have triggered a migration. Make sure
                // the data received is accounted for the most recent path by accessing
                // `path` after `handle_decode`.
                self.path.total_recvd = self.path.total_recvd.saturating_add(data_len as u64);

                if let Some(data) = remaining {
                    self.stats.udp_rx.bytes += data.len() as u64;
                    self.handle_coalesced(now, remote, ecn, data);
                }

                self.config.qlog_sink.emit_recovery_metrics(
                    self.pto_count,
                    &mut self.path,
                    now,
                    self.orig_rem_cid,
                );

                if was_anti_amplification_blocked {
                    // A prior attempt to set the loss detection timer may have failed due to
                    // anti-amplification, so ensure it's set now. Prevents a handshake deadlock if
                    // the server's first flight is lost.
                    self.set_loss_detection_timer(now);
                }
            }
            NewIdentifiers(ids, now) => {
                self.local_cid_state.new_cids(&ids, now);
                ids.into_iter().rev().for_each(|frame| {
                    self.spaces[SpaceId::Data].pending.new_cids.push(frame);
                });
                // Update Timer::PushNewCid
                if self
                    .timers
                    .get(Timer::PushNewCid)
                    .map_or(true, |x| x <= now)
                {
                    self.reset_cid_retirement();
                }
            }
        }
    }

    /// Process timer expirations
    ///
    /// Executes protocol logic, potentially preparing signals (including application `Event`s,
    /// `EndpointEvent`s and outgoing datagrams) that should be extracted through the relevant
    /// methods.
    ///
    /// It is most efficient to call this immediately after the system clock reaches the latest
    /// `Instant` that was output by `poll_timeout`; however spurious extra calls will simply
    /// no-op and therefore are safe.
    pub fn handle_timeout(&mut self, now: Instant) {
        for &timer in &Timer::VALUES {
            if !self.timers.is_expired(timer, now) {
                continue;
            }
            self.timers.stop(timer);
            trace!(timer = ?timer, "timeout");
            match timer {
                Timer::Close => {
                    self.state = State::Drained;
                    self.endpoint_events.push_back(EndpointEventInner::Drained);
                }
                Timer::Idle => {
                    self.kill(ConnectionError::TimedOut);
                }
                Timer::KeepAlive => {
                    trace!("sending keep-alive");
                    self.ping();
                }
                Timer::LossDetection => {
                    self.on_loss_detection_timeout(now);

                    self.config.qlog_sink.emit_recovery_metrics(
                        self.pto_count,
                        &mut self.path,
                        now,
                        self.orig_rem_cid,
                    );
                }
                Timer::KeyDiscard => {
                    self.zero_rtt_crypto = None;
                    self.prev_crypto = None;
                }
                Timer::PathValidation => {
                    debug!("path validation failed");
                    if let Some((_, prev)) = self.prev_path.take() {
                        self.path = prev;
                    }
                    self.path.challenge = None;
                    self.path.challenge_pending = false;
                }
                Timer::Pacing => trace!("pacing timer expired"),
                Timer::PushNewCid => {
                    // Update `retire_prior_to` field in NEW_CONNECTION_ID frame
                    let num_new_cid = self.local_cid_state.on_cid_timeout().into();
                    if !self.state.is_closed() {
                        trace!(
                            "push a new cid to peer RETIRE_PRIOR_TO field {}",
                            self.local_cid_state.retire_prior_to()
                        );
                        self.endpoint_events
                            .push_back(EndpointEventInner::NeedIdentifiers(now, num_new_cid));
                    }
                }
                Timer::MaxAckDelay => {
                    trace!("max ack delay reached");
                    // This timer is only armed in the Data space
                    self.spaces[SpaceId::Data]
                        .pending_acks
                        .on_max_ack_delay_timeout()
                }
            }
        }
    }

    /// Close a connection immediately
    ///
    /// This does not ensure delivery of outstanding data. It is the application's responsibility to
    /// call this only when all important communications have been completed, e.g. by calling
    /// [`SendStream::finish`] on outstanding streams and waiting for the corresponding
    /// [`StreamEvent::Finished`] event.
    ///
    /// If [`Streams::send_streams`] returns 0, all outstanding stream data has been
    /// delivered. There may still be data from the peer that has not been received.
    ///
    /// [`StreamEvent::Finished`]: crate::StreamEvent::Finished
    pub fn close(&mut self, now: Instant, error_code: VarInt, reason: Bytes) {
        self.close_inner(
            now,
            Close::Application(frame::ApplicationClose { error_code, reason }),
        )
    }

    fn close_inner(&mut self, now: Instant, reason: Close) {
        let was_closed = self.state.is_closed();
        if !was_closed {
            self.close_common();
            self.set_close_timer(now);
            self.close = true;
            self.state = State::Closed(state::Closed { reason });
        }
    }

    /// Control datagrams
    pub fn datagrams(&mut self) -> Datagrams<'_> {
        Datagrams { conn: self }
    }

    /// Returns connection statistics
    pub fn stats(&self) -> ConnectionStats {
        let mut stats = self.stats;
        stats.path.rtt = self.path.rtt.get();
        stats.path.cwnd = self.path.congestion.window();
        stats.path.current_mtu = self.path.mtud.current_mtu();

        stats
    }

    /// Ping the remote endpoint
    ///
    /// Causes an ACK-eliciting packet to be transmitted.
    pub fn ping(&mut self) {
        self.spaces[self.highest_space].ping_pending = true;
    }

    /// Update traffic keys spontaneously
    ///
    /// This can be useful for testing key updates, as they otherwise only happen infrequently.
    pub fn force_key_update(&mut self) {
        if !self.state.is_established() {
            debug!("ignoring forced key update in illegal state");
            return;
        }
        if self.prev_crypto.is_some() {
            // We already just updated, or are currently updating, the keys. Concurrent key updates
            // are illegal.
            debug!("ignoring redundant forced key update");
            return;
        }
        self.update_keys(None, false);
    }

    // Compatibility wrapper for quinn < 0.11.7. Remove for 0.12.
    #[doc(hidden)]
    #[deprecated]
    pub fn initiate_key_update(&mut self) {
        self.force_key_update();
    }

    /// Get a session reference
    pub fn crypto_session(&self) -> &dyn crypto::Session {
        &*self.crypto
    }

    /// Whether the connection is in the process of being established
    ///
    /// If this returns `false`, the connection may be either established or closed, signaled by the
    /// emission of a `Connected` or `ConnectionLost` message respectively.
    pub fn is_handshaking(&self) -> bool {
        self.state.is_handshake()
    }

    /// Whether the connection is closed
    ///
    /// Closed connections cannot transport any further data. A connection becomes closed when
    /// either peer application intentionally closes it, or when either transport layer detects an
    /// error such as a time-out or certificate validation failure.
    ///
    /// A `ConnectionLost` event is emitted with details when the connection becomes closed.
    pub fn is_closed(&self) -> bool {
        self.state.is_closed()
    }

    /// Whether there is no longer any need to keep the connection around
    ///
    /// Closed connections become drained after a brief timeout to absorb any remaining in-flight
    /// packets from the peer. All drained connections have been closed.
    pub fn is_drained(&self) -> bool {
        self.state.is_drained()
    }

    /// For clients, if the peer accepted the 0-RTT data packets
    ///
    /// The value is meaningless until after the handshake completes.
    pub fn accepted_0rtt(&self) -> bool {
        self.accepted_0rtt
    }

    /// Whether 0-RTT is/was possible during the handshake
    pub fn has_0rtt(&self) -> bool {
        self.zero_rtt_enabled
    }

    /// Whether there are any pending retransmits
    pub fn has_pending_retransmits(&self) -> bool {
        !self.spaces[SpaceId::Data].pending.is_empty(&self.streams)
    }

    /// Look up whether we're the client or server of this Connection
    pub fn side(&self) -> Side {
        self.side.side()
    }

    /// The latest socket address for this connection's peer
    pub fn remote_address(&self) -> SocketAddr {
        self.path.remote
    }

    /// The local IP address which was used when the peer established
    /// the connection
    ///
    /// This can be different from the address the endpoint is bound to, in case
    /// the endpoint is bound to a wildcard address like `0.0.0.0` or `::`.
    ///
    /// This will return `None` for clients, or when no `local_ip` was passed to
    /// [`Endpoint::handle()`](crate::Endpoint::handle) for the datagrams establishing this
    /// connection.
    pub fn local_ip(&self) -> Option<IpAddr> {
        self.local_ip
    }

    /// Current best estimate of this connection's latency (round-trip-time)
    pub fn rtt(&self) -> Duration {
        self.path.rtt.get()
    }

    /// Current state of this connection's congestion controller, for debugging purposes
    pub fn congestion_state(&self) -> &dyn Controller {
        self.path.congestion.as_ref()
    }

    /// Resets path-specific settings.
    ///
    /// This will force-reset several subsystems related to a specific network path.
    /// Currently this is the congestion controller, round-trip estimator, and the MTU
    /// discovery.
    ///
    /// This is useful when it is known the underlying network path has changed and the old
    /// state of these subsystems is no longer valid or optimal. In this case it might be
    /// faster or reduce loss to settle on optimal values by restarting from the initial
    /// configuration in the [`TransportConfig`].
    pub fn path_changed(&mut self, now: Instant) {
        self.path.reset(now, &self.config);
    }

    /// Modify the number of remotely initiated streams that may be concurrently open
    ///
    /// No streams may be opened by the peer unless fewer than `count` are already open. Large
    /// `count`s increase both minimum and worst-case memory consumption.
    pub fn set_max_concurrent_streams(&mut self, dir: Dir, count: VarInt) {
        self.streams.set_max_concurrent(dir, count);
        // If the limit was reduced, then a flow control update previously deemed insignificant may
        // now be significant.
        let pending = &mut self.spaces[SpaceId::Data].pending;
        self.streams.queue_max_stream_id(pending);
    }

    /// Current number of remotely initiated streams that may be concurrently open
    ///
    /// If the target for this limit is reduced using [`set_max_concurrent_streams`](Self::set_max_concurrent_streams),
    /// it will not change immediately, even if fewer streams are open. Instead, it will
    /// decrement by one for each time a remotely initiated stream of matching directionality is closed.
    pub fn max_concurrent_streams(&self, dir: Dir) -> u64 {
        self.streams.max_concurrent(dir)
    }

    /// See [`TransportConfig::send_window()`]
    pub fn set_send_window(&mut self, send_window: u64) {
        self.streams.set_send_window(send_window);
    }

    /// See [`TransportConfig::receive_window()`]
    pub fn set_receive_window(&mut self, receive_window: VarInt) {
        if self.streams.set_receive_window(receive_window) {
            self.spaces[SpaceId::Data].pending.max_data = true;
        }
    }

    fn on_ack_received(
        &mut self,
        now: Instant,
        space: SpaceId,
        ack: frame::Ack,
    ) -> Result<(), TransportError> {
        if ack.largest >= self.spaces[space].next_packet_number {
            return Err(TransportError::PROTOCOL_VIOLATION("unsent packet acked"));
        }
        let new_largest = {
            let space = &mut self.spaces[space];
            if space
                .largest_acked_packet
                .map_or(true, |pn| ack.largest > pn)
            {
                space.largest_acked_packet = Some(ack.largest);
                if let Some(info) = space.sent_packets.get(&ack.largest) {
                    // This should always succeed, but a misbehaving peer might ACK a packet we
                    // haven't sent. At worst, that will result in us spuriously reducing the
                    // congestion window.
                    space.largest_acked_packet_sent = info.time_sent;
                }
                true
            } else {
                false
            }
        };

        // Avoid DoS from unreasonably huge ack ranges by filtering out just the new acks.
        let mut newly_acked = ArrayRangeSet::new();
        for range in ack.iter() {
            self.packet_number_filter.check_ack(space, range.clone())?;
            for (&pn, _) in self.spaces[space].sent_packets.range(range) {
                newly_acked.insert_one(pn);
            }
        }

        if newly_acked.is_empty() {
            return Ok(());
        }

        let mut ack_eliciting_acked = false;
        for packet in newly_acked.elts() {
            if let Some(info) = self.spaces[space].take(packet) {
                if let Some(acked) = info.largest_acked {
                    // Assume ACKs for all packets below the largest acknowledged in `packet` have
                    // been received. This can cause the peer to spuriously retransmit if some of
                    // our earlier ACKs were lost, but allows for simpler state tracking. See
                    // discussion at
                    // https://www.rfc-editor.org/rfc/rfc9000.html#name-limiting-ranges-by-tracking
                    self.spaces[space].pending_acks.subtract_below(acked);
                }
                ack_eliciting_acked |= info.ack_eliciting;

                // Notify MTU discovery that a packet was acked, because it might be an MTU probe
                let mtu_updated = self.path.mtud.on_acked(space, packet, info.size);
                if mtu_updated {
                    self.path
                        .congestion
                        .on_mtu_update(self.path.mtud.current_mtu());
                }

                // Notify ack frequency that a packet was acked, because it might contain an ACK_FREQUENCY frame
                self.ack_frequency.on_acked(packet);

                self.on_packet_acked(now, info);
            }
        }

        self.path.congestion.on_end_acks(
            now,
            self.path.in_flight.bytes,
            self.app_limited,
            self.spaces[space].largest_acked_packet,
        );

        if new_largest && ack_eliciting_acked {
            let ack_delay = if space != SpaceId::Data {
                Duration::from_micros(0)
            } else {
                cmp::min(
                    self.ack_frequency.peer_max_ack_delay,
                    Duration::from_micros(ack.delay << self.peer_params.ack_delay_exponent.0),
                )
            };
            let rtt = instant_saturating_sub(now, self.spaces[space].largest_acked_packet_sent);
            self.path.rtt.update(ack_delay, rtt);
            if self.path.first_packet_after_rtt_sample.is_none() {
                self.path.first_packet_after_rtt_sample =
                    Some((space, self.spaces[space].next_packet_number));
            }
        }

        // Must be called before crypto/pto_count are clobbered
        self.detect_lost_packets(now, space, true);

        if self.peer_completed_address_validation() {
            self.pto_count = 0;
        }

        // Explicit congestion notification
        if self.path.sending_ecn {
            if let Some(ecn) = ack.ecn {
                // We only examine ECN counters from ACKs that we are certain we received in transmit
                // order, allowing us to compute an increase in ECN counts to compare against the number
                // of newly acked packets that remains well-defined in the presence of arbitrary packet
                // reordering.
                if new_largest {
                    let sent = self.spaces[space].largest_acked_packet_sent;
                    self.process_ecn(now, space, newly_acked.len() as u64, ecn, sent);
                }
            } else {
                // We always start out sending ECN, so any ack that doesn't acknowledge it disables it.
                debug!("ECN not acknowledged by peer");
                self.path.sending_ecn = false;
            }
        }

        self.set_loss_detection_timer(now);
        Ok(())
    }

    /// Process a new ECN block from an in-order ACK
    fn process_ecn(
        &mut self,
        now: Instant,
        space: SpaceId,
        newly_acked: u64,
        ecn: frame::EcnCounts,
        largest_sent_time: Instant,
    ) {
        match self.spaces[space].detect_ecn(newly_acked, ecn) {
            Err(e) => {
                debug!("halting ECN due to verification failure: {}", e);
                self.path.sending_ecn = false;
                // Wipe out the existing value because it might be garbage and could interfere with
                // future attempts to use ECN on new paths.
                self.spaces[space].ecn_feedback = frame::EcnCounts::ZERO;
            }
            Ok(false) => {}
            Ok(true) => {
                self.stats.path.congestion_events += 1;
                self.path
                    .congestion
                    .on_congestion_event(now, largest_sent_time, false, 0);
            }
        }
    }

    // Not timing-aware, so it's safe to call this for inferred acks, such as arise from
    // high-latency handshakes
    fn on_packet_acked(&mut self, now: Instant, info: SentPacket) {
        self.remove_in_flight(&info);
        if info.ack_eliciting && self.path.challenge.is_none() {
            // Only pass ACKs to the congestion controller if we are not validating the current
            // path, so as to ignore any ACKs from older paths still coming in.
            self.path.congestion.on_ack(
                now,
                info.time_sent,
                info.size.into(),
                self.app_limited,
                &self.path.rtt,
            );
        }

        // Update state for confirmed delivery of frames
        if let Some(retransmits) = info.retransmits.get() {
            for (id, _) in retransmits.reset_stream.iter() {
                self.streams.reset_acked(*id);
            }
        }

        for frame in info.stream_frames {
            self.streams.received_ack_of(frame);
        }
    }

    fn set_key_discard_timer(&mut self, now: Instant, space: SpaceId) {
        let start = if self.zero_rtt_crypto.is_some() {
            now
        } else {
            self.prev_crypto
                .as_ref()
                .expect("no previous keys")
                .end_packet
                .as_ref()
                .expect("update not acknowledged yet")
                .1
        };
        self.timers
            .set(Timer::KeyDiscard, start + self.pto(space) * 3);
    }

    fn on_loss_detection_timeout(&mut self, now: Instant) {
        if let Some((_, pn_space)) = self.loss_time_and_space() {
            // Time threshold loss Detection
            self.detect_lost_packets(now, pn_space, false);
            self.set_loss_detection_timer(now);
            return;
        }

        let (_, space) = match self.pto_time_and_space(now) {
            Some(x) => x,
            None => {
                error!("PTO expired while unset");
                return;
            }
        };
        trace!(
            in_flight = self.path.in_flight.bytes,
            count = self.pto_count,
            ?space,
            "PTO fired"
        );

        let count = match self.path.in_flight.ack_eliciting {
            // A PTO when we're not expecting any ACKs must be due to handshake anti-amplification
            // deadlock preventions
            0 => {
                debug_assert!(!self.peer_completed_address_validation());
                1
            }
            // Conventional loss probe
            _ => 2,
        };
        self.spaces[space].loss_probes = self.spaces[space].loss_probes.saturating_add(count);
        self.pto_count = self.pto_count.saturating_add(1);
        self.set_loss_detection_timer(now);
    }

    fn detect_lost_packets(&mut self, now: Instant, pn_space: SpaceId, due_to_ack: bool) {
        let mut lost_packets = Vec::<u64>::new();
        let mut lost_mtu_probe = None;
        let in_flight_mtu_probe = self.path.mtud.in_flight_mtu_probe();
        let rtt = self.path.rtt.conservative();
        let loss_delay = cmp::max(rtt.mul_f32(self.config.time_threshold), TIMER_GRANULARITY);

        // Packets sent before this time are deemed lost.
        let lost_send_time = now.checked_sub(loss_delay).unwrap();
        let largest_acked_packet = self.spaces[pn_space].largest_acked_packet.unwrap();
        let packet_threshold = self.config.packet_threshold as u64;
        let mut size_of_lost_packets = 0u64;

        // InPersistentCongestion: Determine if all packets in the time period before the newest
        // lost packet, including the edges, are marked lost. PTO computation must always
        // include max ACK delay, i.e. operate as if in Data space (see RFC9001 §7.6.1).
        let congestion_period =
            self.pto(SpaceId::Data) * self.config.persistent_congestion_threshold;
        let mut persistent_congestion_start: Option<Instant> = None;
        let mut prev_packet = None;
        let mut in_persistent_congestion = false;

        let space = &mut self.spaces[pn_space];
        space.loss_time = None;

        for (&packet, info) in space.sent_packets.range(0..largest_acked_packet) {
            if prev_packet != Some(packet.wrapping_sub(1)) {
                // An intervening packet was acknowledged
                persistent_congestion_start = None;
            }

            if info.time_sent <= lost_send_time || largest_acked_packet >= packet + packet_threshold
            {
                if Some(packet) == in_flight_mtu_probe {
                    // Lost MTU probes are not included in `lost_packets`, because they should not
                    // trigger a congestion control response
                    lost_mtu_probe = in_flight_mtu_probe;
                } else {
                    lost_packets.push(packet);
                    size_of_lost_packets += info.size as u64;
                    if info.ack_eliciting && due_to_ack {
                        match persistent_congestion_start {
                            // Two ACK-eliciting packets lost more than congestion_period apart, with no
                            // ACKed packets in between
                            Some(start) if info.time_sent - start > congestion_period => {
                                in_persistent_congestion = true;
                            }
                            // Persistent congestion must start after the first RTT sample
                            None if self
                                .path
                                .first_packet_after_rtt_sample
                                .is_some_and(|x| x < (pn_space, packet)) =>
                            {
                                persistent_congestion_start = Some(info.time_sent);
                            }
                            _ => {}
                        }
                    }
                }
            } else {
                let next_loss_time = info.time_sent + loss_delay;
                space.loss_time = Some(
                    space
                        .loss_time
                        .map_or(next_loss_time, |x| cmp::min(x, next_loss_time)),
                );
                persistent_congestion_start = None;
            }

            prev_packet = Some(packet);
        }

        // OnPacketsLost
        if let Some(largest_lost) = lost_packets.last().cloned() {
            let old_bytes_in_flight = self.path.in_flight.bytes;
            let largest_lost_sent = self.spaces[pn_space].sent_packets[&largest_lost].time_sent;
            self.stats.path.lost_packets += lost_packets.len() as u64;
            self.stats.path.lost_bytes += size_of_lost_packets;
            trace!(
                "packets lost: {:?}, bytes lost: {}",
                lost_packets, size_of_lost_packets
            );

            for &packet in &lost_packets {
                let info = self.spaces[pn_space].take(packet).unwrap(); // safe: lost_packets is populated just above
                self.config.qlog_sink.emit_packet_lost(
                    packet,
                    &info,
                    lost_send_time,
                    pn_space,
                    now,
                    self.orig_rem_cid,
                );
                self.remove_in_flight(&info);
                for frame in info.stream_frames {
                    self.streams.retransmit(frame);
                }
                self.spaces[pn_space].pending |= info.retransmits;
                self.path.mtud.on_non_probe_lost(packet, info.size);
            }

            if self.path.mtud.black_hole_detected(now) {
                self.stats.path.black_holes_detected += 1;
                self.path
                    .congestion
                    .on_mtu_update(self.path.mtud.current_mtu());
                if let Some(max_datagram_size) = self.datagrams().max_size() {
                    self.datagrams.drop_oversized(max_datagram_size);
                }
            }

            // Don't apply congestion penalty for lost ack-only packets
            let lost_ack_eliciting = old_bytes_in_flight != self.path.in_flight.bytes;

            if lost_ack_eliciting {
                self.stats.path.congestion_events += 1;
                self.path.congestion.on_congestion_event(
                    now,
                    largest_lost_sent,
                    in_persistent_congestion,
                    size_of_lost_packets,
                );
            }
        }

        // Handle a lost MTU probe
        if let Some(packet) = lost_mtu_probe {
            let info = self.spaces[SpaceId::Data].take(packet).unwrap(); // safe: lost_mtu_probe is omitted from lost_packets, and therefore must not have been removed yet
            self.remove_in_flight(&info);
            self.path.mtud.on_probe_lost();
            self.stats.path.lost_plpmtud_probes += 1;
        }
    }

    fn loss_time_and_space(&self) -> Option<(Instant, SpaceId)> {
        SpaceId::iter()
            .filter_map(|id| Some((self.spaces[id].loss_time?, id)))
            .min_by_key(|&(time, _)| time)
    }

    fn pto_time_and_space(&self, now: Instant) -> Option<(Instant, SpaceId)> {
        let backoff = 2u32.pow(self.pto_count.min(MAX_BACKOFF_EXPONENT));
        let mut duration = self.path.rtt.pto_base() * backoff;

        if self.path.in_flight.ack_eliciting == 0 {
            debug_assert!(!self.peer_completed_address_validation());
            let space = match self.highest_space {
                SpaceId::Handshake => SpaceId::Handshake,
                _ => SpaceId::Initial,
            };
            return Some((now + duration, space));
        }

        let mut result = None;
        for space in SpaceId::iter() {
            if !self.spaces[space].has_in_flight() {
                continue;
            }
            if space == SpaceId::Data {
                // Skip ApplicationData until handshake completes.
                if self.is_handshaking() {
                    return result;
                }
                // Include max_ack_delay and backoff for ApplicationData.
                duration += self.ack_frequency.max_ack_delay_for_pto() * backoff;
            }
            let last_ack_eliciting = match self.spaces[space].time_of_last_ack_eliciting_packet {
                Some(time) => time,
                None => continue,
            };
            let pto = last_ack_eliciting + duration;
            if result.map_or(true, |(earliest_pto, _)| pto < earliest_pto) {
                result = Some((pto, space));
            }
        }
        result
    }

    fn peer_completed_address_validation(&self) -> bool {
        if self.side.is_server() || self.state.is_closed() {
            return true;
        }
        // The server is guaranteed to have validated our address if any of our handshake or 1-RTT
        // packets are acknowledged or we've seen HANDSHAKE_DONE and discarded handshake keys.
        self.spaces[SpaceId::Handshake]
            .largest_acked_packet
            .is_some()
            || self.spaces[SpaceId::Data].largest_acked_packet.is_some()
            || (self.spaces[SpaceId::Data].crypto.is_some()
                && self.spaces[SpaceId::Handshake].crypto.is_none())
    }

    fn set_loss_detection_timer(&mut self, now: Instant) {
        if self.state.is_closed() {
            // No loss detection takes place on closed connections, and `close_common` already
            // stopped time timer. Ensure we don't restart it inadvertently, e.g. in response to a
            // reordered packet being handled by state-insensitive code.
            return;
        }

        if let Some((loss_time, _)) = self.loss_time_and_space() {
            // Time threshold loss detection.
            self.timers.set(Timer::LossDetection, loss_time);
            return;
        }

        if self.path.anti_amplification_blocked(1) {
            // We wouldn't be able to send anything, so don't bother.
            self.timers.stop(Timer::LossDetection);
            return;
        }

        if self.path.in_flight.ack_eliciting == 0 && self.peer_completed_address_validation() {
            // There is nothing to detect lost, so no timer is set. However, the client needs to arm
            // the timer if the server might be blocked by the anti-amplification limit.
            self.timers.stop(Timer::LossDetection);
            return;
        }

        // Determine which PN space to arm PTO for.
        // Calculate PTO duration
        if let Some((timeout, _)) = self.pto_time_and_space(now) {
            self.timers.set(Timer::LossDetection, timeout);
        } else {
            self.timers.stop(Timer::LossDetection);
        }
    }

    /// Probe Timeout
    fn pto(&self, space: SpaceId) -> Duration {
        let max_ack_delay = match space {
            SpaceId::Initial | SpaceId::Handshake => Duration::ZERO,
            SpaceId::Data => self.ack_frequency.max_ack_delay_for_pto(),
        };
        self.path.rtt.pto_base() + max_ack_delay
    }

    fn on_packet_authenticated(
        &mut self,
        now: Instant,
        space_id: SpaceId,
        ecn: Option<EcnCodepoint>,
        packet: Option<u64>,
        spin: bool,
        is_1rtt: bool,
    ) {
        self.total_authed_packets += 1;
        self.reset_keep_alive(now);
        self.reset_idle_timeout(now, space_id);
        self.permit_idle_reset = true;
        self.receiving_ecn |= ecn.is_some();
        if let Some(x) = ecn {
            let space = &mut self.spaces[space_id];
            space.ecn_counters += x;

            if x.is_ce() {
                space.pending_acks.set_immediate_ack_required();
            }
        }

        let packet = match packet {
            Some(x) => x,
            None => return,
        };
        if self.side.is_server() {
            if self.spaces[SpaceId::Initial].crypto.is_some() && space_id == SpaceId::Handshake {
                // A server stops sending and processing Initial packets when it receives its first Handshake packet.
                self.discard_space(now, SpaceId::Initial);
            }
            if self.zero_rtt_crypto.is_some() && is_1rtt {
                // Discard 0-RTT keys soon after receiving a 1-RTT packet
                self.set_key_discard_timer(now, space_id)
            }
        }
        let space = &mut self.spaces[space_id];
        space.pending_acks.insert_one(packet, now);
        if packet >= space.rx_packet {
            space.rx_packet = packet;
            // Update outgoing spin bit, inverting iff we're the client
            self.spin = self.side.is_client() ^ spin;
        }

        self.config.qlog_sink.emit_packet_received(
            packet,
            space_id,
            !is_1rtt,
            now,
            self.orig_rem_cid,
        );
    }

    fn reset_idle_timeout(&mut self, now: Instant, space: SpaceId) {
        let timeout = match self.idle_timeout {
            None => return,
            Some(dur) => dur,
        };
        if self.state.is_closed() {
            self.timers.stop(Timer::Idle);
            return;
        }
        let dt = cmp::max(timeout, 3 * self.pto(space));
        self.timers.set(Timer::Idle, now + dt);
    }

    fn reset_keep_alive(&mut self, now: Instant) {
        let interval = match self.config.keep_alive_interval {
            Some(x) if self.state.is_established() => x,
            _ => return,
        };
        self.timers.set(Timer::KeepAlive, now + interval);
    }

    fn reset_cid_retirement(&mut self) {
        if let Some(t) = self.local_cid_state.next_timeout() {
            self.timers.set(Timer::PushNewCid, t);
        }
    }

    /// Handle the already-decrypted first packet from the client
    ///
    /// Decrypting the first packet in the `Endpoint` allows stateless packet handling to be more
    /// efficient.
    pub(crate) fn handle_first_packet(
        &mut self,
        now: Instant,
        remote: SocketAddr,
        ecn: Option<EcnCodepoint>,
        packet_number: u64,
        packet: InitialPacket,
        remaining: Option<BytesMut>,
    ) -> Result<(), ConnectionError> {
        let span = trace_span!("first recv");
        let _guard = span.enter();
        debug_assert!(self.side.is_server());
        let len = packet.header_data.len() + packet.payload.len();
        self.path.total_recvd = len as u64;

        match self.state {
            State::Handshake(ref mut state) => {
                state.expected_token = packet.header.token.clone();
            }
            _ => unreachable!("first packet must be delivered in Handshake state"),
        }

        self.on_packet_authenticated(
            now,
            SpaceId::Initial,
            ecn,
            Some(packet_number),
            false,
            false,
        );

        self.process_decrypted_packet(now, remote, Some(packet_number), packet.into())?;
        if let Some(data) = remaining {
            self.handle_coalesced(now, remote, ecn, data);
        }

        self.config.qlog_sink.emit_recovery_metrics(
            self.pto_count,
            &mut self.path,
            now,
            self.orig_rem_cid,
        );

        Ok(())
    }

    fn init_0rtt(&mut self) {
        let (header, packet) = match self.crypto.early_crypto() {
            Some(x) => x,
            None => return,
        };
        if self.side.is_client() {
            match self.crypto.transport_parameters() {
                Ok(params) => {
                    let params = params
                        .expect("crypto layer didn't supply transport parameters with ticket");
                    // Certain values must not be cached
                    let params = TransportParameters {
                        initial_src_cid: None,
                        original_dst_cid: None,
                        preferred_address: None,
                        retry_src_cid: None,
                        stateless_reset_token: None,
                        min_ack_delay: None,
                        ack_delay_exponent: TransportParameters::default().ack_delay_exponent,
                        max_ack_delay: TransportParameters::default().max_ack_delay,
                        ..params
                    };
                    self.set_peer_params(params);
                }
                Err(e) => {
                    error!("session ticket has malformed transport parameters: {}", e);
                    return;
                }
            }
        }
        trace!("0-RTT enabled");
        self.zero_rtt_enabled = true;
        self.zero_rtt_crypto = Some(ZeroRttCrypto { header, packet });
    }

    fn read_crypto(
        &mut self,
        space: SpaceId,
        crypto: &frame::Crypto,
        payload_len: usize,
    ) -> Result<(), TransportError> {
        let expected = if !self.state.is_handshake() {
            SpaceId::Data
        } else if self.highest_space == SpaceId::Initial {
            SpaceId::Initial
        } else {
            // On the server, self.highest_space can be Data after receiving the client's first
            // flight, but we expect Handshake CRYPTO until the handshake is complete.
            SpaceId::Handshake
        };
        // We can't decrypt Handshake packets when highest_space is Initial, CRYPTO frames in 0-RTT
        // packets are illegal, and we don't process 1-RTT packets until the handshake is
        // complete. Therefore, we will never see CRYPTO data from a later-than-expected space.
        debug_assert!(space <= expected, "received out-of-order CRYPTO data");

        let end = crypto.offset + crypto.data.len() as u64;
        if space < expected && end > self.spaces[space].crypto_stream.bytes_read() {
            warn!(
                "received new {:?} CRYPTO data when expecting {:?}",
                space, expected
            );
            return Err(TransportError::PROTOCOL_VIOLATION(
                "new data at unexpected encryption level",
            ));
        }

        let space = &mut self.spaces[space];
        let max = end.saturating_sub(space.crypto_stream.bytes_read());
        if max > self.config.crypto_buffer_size as u64 {
            return Err(TransportError::CRYPTO_BUFFER_EXCEEDED(""));
        }

        space
            .crypto_stream
            .insert(crypto.offset, crypto.data.clone(), payload_len);
        while let Some(chunk) = space.crypto_stream.read(usize::MAX, true) {
            trace!("consumed {} CRYPTO bytes", chunk.bytes.len());
            if self.crypto.read_handshake(&chunk.bytes)? {
                self.events.push_back(Event::HandshakeDataReady);
            }
        }

        Ok(())
    }

    fn write_crypto(&mut self) {
        loop {
            let space = self.highest_space;
            let mut outgoing = Vec::new();
            if let Some(crypto) = self.crypto.write_handshake(&mut outgoing) {
                match space {
                    SpaceId::Initial => {
                        self.upgrade_crypto(SpaceId::Handshake, crypto);
                    }
                    SpaceId::Handshake => {
                        self.upgrade_crypto(SpaceId::Data, crypto);
                    }
                    _ => unreachable!("got updated secrets during 1-RTT"),
                }
            }
            if outgoing.is_empty() {
                if space == self.highest_space {
                    break;
                } else {
                    // Keys updated, check for more data to send
                    continue;
                }
            }
            let offset = self.spaces[space].crypto_offset;
            let outgoing = Bytes::from(outgoing);
            if let State::Handshake(ref mut state) = self.state {
                if space == SpaceId::Initial && offset == 0 && self.side.is_client() {
                    state.client_hello = Some(outgoing.clone());
                }
            }
            self.spaces[space].crypto_offset += outgoing.len() as u64;
            trace!("wrote {} {:?} CRYPTO bytes", outgoing.len(), space);
            self.spaces[space].pending.crypto.push_back(frame::Crypto {
                offset,
                data: outgoing,
            });
        }
    }

    /// Switch to stronger cryptography during handshake
    fn upgrade_crypto(&mut self, space: SpaceId, crypto: Keys) {
        debug_assert!(
            self.spaces[space].crypto.is_none(),
            "already reached packet space {space:?}"
        );
        trace!("{:?} keys ready", space);
        if space == SpaceId::Data {
            // Precompute the first key update
            self.next_crypto = Some(
                self.crypto
                    .next_1rtt_keys()
                    .expect("handshake should be complete"),
            );
        }

        self.spaces[space].crypto = Some(crypto);
        debug_assert!(space as usize > self.highest_space as usize);
        self.highest_space = space;
        if space == SpaceId::Data && self.side.is_client() {
            // Discard 0-RTT keys because 1-RTT keys are available.
            self.zero_rtt_crypto = None;
        }
    }

    fn discard_space(&mut self, now: Instant, space_id: SpaceId) {
        debug_assert!(space_id != SpaceId::Data);
        trace!("discarding {:?} keys", space_id);
        if space_id == SpaceId::Initial {
            // No longer needed
            if let ConnectionSide::Client { token, .. } = &mut self.side {
                *token = Bytes::new();
            }
        }
        let space = &mut self.spaces[space_id];
        space.crypto = None;
        space.time_of_last_ack_eliciting_packet = None;
        space.loss_time = None;
        let sent_packets = mem::take(&mut space.sent_packets);
        for packet in sent_packets.into_values() {
            self.remove_in_flight(&packet);
        }
        self.set_loss_detection_timer(now)
    }

    fn handle_coalesced(
        &mut self,
        now: Instant,
        remote: SocketAddr,
        ecn: Option<EcnCodepoint>,
        data: BytesMut,
    ) {
        self.path.total_recvd = self.path.total_recvd.saturating_add(data.len() as u64);
        let mut remaining = Some(data);
        while let Some(data) = remaining {
            match PartialDecode::new(
                data,
                &FixedLengthConnectionIdParser::new(self.local_cid_state.cid_len()),
                &[self.version],
                self.endpoint_config.grease_quic_bit,
            ) {
                Ok((partial_decode, rest)) => {
                    remaining = rest;
                    self.handle_decode(now, remote, ecn, partial_decode);
                }
                Err(e) => {
                    trace!("malformed header: {}", e);
                    return;
                }
            }
        }
    }

    fn handle_decode(
        &mut self,
        now: Instant,
        remote: SocketAddr,
        ecn: Option<EcnCodepoint>,
        partial_decode: PartialDecode,
    ) {
        if let Some(decoded) = packet_crypto::unprotect_header(
            partial_decode,
            &self.spaces,
            self.zero_rtt_crypto.as_ref(),
            self.peer_params.stateless_reset_token,
        ) {
            self.handle_packet(now, remote, ecn, decoded.packet, decoded.stateless_reset);
        }
    }

    fn handle_packet(
        &mut self,
        now: Instant,
        remote: SocketAddr,
        ecn: Option<EcnCodepoint>,
        packet: Option<Packet>,
        stateless_reset: bool,
    ) {
        self.stats.udp_rx.ios += 1;
        if let Some(ref packet) = packet {
            trace!(
                "got {:?} packet ({} bytes) from {} using id {}",
                packet.header.space(),
                packet.payload.len() + packet.header_data.len(),
                remote,
                packet.header.dst_cid(),
            );
        }

        if self.is_handshaking() && remote != self.path.remote {
            debug!("discarding packet with unexpected remote during handshake");
            return;
        }

        let was_closed = self.state.is_closed();
        let was_drained = self.state.is_drained();

        let decrypted = match packet {
            None => Err(None),
            Some(mut packet) => self
                .decrypt_packet(now, &mut packet)
                .map(move |number| (packet, number)),
        };
        let result = match decrypted {
            _ if stateless_reset => {
                debug!("got stateless reset");
                Err(ConnectionError::Reset)
            }
            Err(Some(e)) => {
                warn!("illegal packet: {}", e);
                Err(e.into())
            }
            Err(None) => {
                debug!("failed to authenticate packet");
                self.authentication_failures += 1;
                let integrity_limit = self.spaces[self.highest_space]
                    .crypto
                    .as_ref()
                    .unwrap()
                    .packet
                    .local
                    .integrity_limit();
                if self.authentication_failures > integrity_limit {
                    Err(TransportError::AEAD_LIMIT_REACHED("integrity limit violated").into())
                } else {
                    return;
                }
            }
            Ok((packet, number)) => {
                let span = match number {
                    Some(pn) => trace_span!("recv", space = ?packet.header.space(), pn),
                    None => trace_span!("recv", space = ?packet.header.space()),
                };
                let _guard = span.enter();

                let is_duplicate = |n| self.spaces[packet.header.space()].dedup.insert(n);
                if number.is_some_and(is_duplicate) {
                    debug!("discarding possible duplicate packet");
                    return;
                } else if self.state.is_handshake() && packet.header.is_short() {
                    // TODO: SHOULD buffer these to improve reordering tolerance.
                    trace!("dropping short packet during handshake");
                    return;
                } else {
                    if let Header::Initial(InitialHeader { ref token, .. }) = packet.header {
                        if let State::Handshake(ref hs) = self.state {
                            if self.side.is_server() && token != &hs.expected_token {
                                // Clients must send the same retry token in every Initial. Initial
                                // packets can be spoofed, so we discard rather than killing the
                                // connection.
                                warn!("discarding Initial with invalid retry token");
                                return;
                            }
                        }
                    }

                    if !self.state.is_closed() {
                        let spin = match packet.header {
                            Header::Short { spin, .. } => spin,
                            _ => false,
                        };
                        self.on_packet_authenticated(
                            now,
                            packet.header.space(),
                            ecn,
                            number,
                            spin,
                            packet.header.is_1rtt(),
                        );
                    }

                    self.process_decrypted_packet(now, remote, number, packet)
                }
            }
        };

        // State transitions for error cases
        if let Err(conn_err) = result {
            self.error = Some(conn_err.clone());
            self.state = match conn_err {
                ConnectionError::ApplicationClosed(reason) => State::closed(reason),
                ConnectionError::ConnectionClosed(reason) => State::closed(reason),
                ConnectionError::Reset
                | ConnectionError::TransportError(TransportError {
                    code: TransportErrorCode::AEAD_LIMIT_REACHED,
                    ..
                }) => State::Drained,
                ConnectionError::TimedOut => {
                    unreachable!("timeouts aren't generated by packet processing");
                }
                ConnectionError::TransportError(err) => {
                    debug!("closing connection due to transport error: {}", err);
                    State::closed(err)
                }
                ConnectionError::VersionMismatch => State::Draining,
                ConnectionError::LocallyClosed => {
                    unreachable!("LocallyClosed isn't generated by packet processing");
                }
                ConnectionError::CidsExhausted => {
                    unreachable!("CidsExhausted isn't generated by packet processing");
                }
            };
        }

        if !was_closed && self.state.is_closed() {
            self.close_common();
            if !self.state.is_drained() {
                self.set_close_timer(now);
            }
        }
        if !was_drained && self.state.is_drained() {
            self.endpoint_events.push_back(EndpointEventInner::Drained);
            // Close timer may have been started previously, e.g. if we sent a close and got a
            // stateless reset in response
            self.timers.stop(Timer::Close);
        }

        // Transmit CONNECTION_CLOSE if necessary
        if let State::Closed(_) = self.state {
            self.close = remote == self.path.remote;
        }
    }

    fn process_decrypted_packet(
        &mut self,
        now: Instant,
        remote: SocketAddr,
        number: Option<u64>,
        packet: Packet,
    ) -> Result<(), ConnectionError> {
        let state = match self.state {
            State::Established => {
                match packet.header.space() {
                    SpaceId::Data => self.process_payload(now, remote, number.unwrap(), packet)?,
                    _ if packet.header.has_frames() => self.process_early_payload(now, packet)?,
                    _ => {
                        trace!("discarding unexpected pre-handshake packet");
                    }
                }
                return Ok(());
            }
            State::Closed(_) => {
                for result in frame::Iter::new(packet.payload.freeze())? {
                    let frame = match result {
                        Ok(frame) => frame,
                        Err(err) => {
                            debug!("frame decoding error: {err:?}");
                            continue;
                        }
                    };

                    if let Frame::Padding = frame {
                        continue;
                    };

                    self.stats.frame_rx.record(&frame);

                    if let Frame::Close(_) = frame {
                        trace!("draining");
                        self.state = State::Draining;
                        break;
                    }
                }
                return Ok(());
            }
            State::Draining | State::Drained => return Ok(()),
            State::Handshake(ref mut state) => state,
        };

        match packet.header {
            Header::Retry {
                src_cid: rem_cid, ..
            } => {
                if self.side.is_server() {
                    return Err(TransportError::PROTOCOL_VIOLATION("client sent Retry").into());
                }

                if self.total_authed_packets > 1
                            || packet.payload.len() <= 16 // token + 16 byte tag
                            || !self.crypto.is_valid_retry(
                                &self.rem_cids.active(),
                                &packet.header_data,
                                &packet.payload,
                            )
                {
                    trace!("discarding invalid Retry");
                    // - After the client has received and processed an Initial or Retry
                    //   packet from the server, it MUST discard any subsequent Retry
                    //   packets that it receives.
                    // - A client MUST discard a Retry packet with a zero-length Retry Token
                    //   field.
                    // - Clients MUST discard Retry packets that have a Retry Integrity Tag
                    //   that cannot be validated
                    return Ok(());
                }

                trace!("retrying with CID {}", rem_cid);
                let client_hello = state.client_hello.take().unwrap();
                self.retry_src_cid = Some(rem_cid);
                self.rem_cids.update_initial_cid(rem_cid);
                self.rem_handshake_cid = rem_cid;

                let space = &mut self.spaces[SpaceId::Initial];
                if let Some(info) = space.take(0) {
                    self.on_packet_acked(now, info);
                };

                self.discard_space(now, SpaceId::Initial); // Make sure we clean up after any retransmitted Initials
                self.spaces[SpaceId::Initial] = PacketSpace {
                    crypto: Some(self.crypto.initial_keys(&rem_cid, self.side.side())),
                    next_packet_number: self.spaces[SpaceId::Initial].next_packet_number,
                    crypto_offset: client_hello.len() as u64,
                    ..PacketSpace::new(now)
                };
                self.spaces[SpaceId::Initial]
                    .pending
                    .crypto
                    .push_back(frame::Crypto {
                        offset: 0,
                        data: client_hello,
                    });

                // Retransmit all 0-RTT data
                let zero_rtt = mem::take(&mut self.spaces[SpaceId::Data].sent_packets);
                for info in zero_rtt.into_values() {
                    self.remove_in_flight(&info);
                    self.spaces[SpaceId::Data].pending |= info.retransmits;
                }
                self.streams.retransmit_all_for_0rtt();

                let token_len = packet.payload.len() - 16;
                let ConnectionSide::Client { ref mut token, .. } = self.side else {
                    unreachable!("we already short-circuited if we're server");
                };
                *token = packet.payload.freeze().split_to(token_len);
                self.state = State::Handshake(state::Handshake {
                    expected_token: Bytes::new(),
                    rem_cid_set: false,
                    client_hello: None,
                });
                Ok(())
            }
            Header::Long {
                ty: LongType::Handshake,
                src_cid: rem_cid,
                ..
            } => {
                if rem_cid != self.rem_handshake_cid {
                    debug!(
                        "discarding packet with mismatched remote CID: {} != {}",
                        self.rem_handshake_cid, rem_cid
                    );
                    return Ok(());
                }
                self.on_path_validated();

                self.process_early_payload(now, packet)?;
                if self.state.is_closed() {
                    return Ok(());
                }

                if self.crypto.is_handshaking() {
                    trace!("handshake ongoing");
                    return Ok(());
                }

                if self.side.is_client() {
                    // Client-only because server params were set from the client's Initial
                    let params =
                        self.crypto
                            .transport_parameters()?
                            .ok_or_else(|| TransportError {
                                code: TransportErrorCode::crypto(0x6d),
                                frame: None,
                                reason: "transport parameters missing".into(),
                            })?;

                    if self.has_0rtt() {
                        if !self.crypto.early_data_accepted().unwrap() {
                            debug_assert!(self.side.is_client());
                            debug!("0-RTT rejected");
                            self.accepted_0rtt = false;
                            self.streams.zero_rtt_rejected();

                            // Discard already-queued frames
                            self.spaces[SpaceId::Data].pending = Retransmits::default();

                            // Discard 0-RTT packets
                            let sent_packets =
                                mem::take(&mut self.spaces[SpaceId::Data].sent_packets);
                            for packet in sent_packets.into_values() {
                                self.remove_in_flight(&packet);
                            }
                        } else {
                            self.accepted_0rtt = true;
                            params.validate_resumption_from(&self.peer_params)?;
                        }
                    }
                    if let Some(token) = params.stateless_reset_token {
                        self.endpoint_events
                            .push_back(EndpointEventInner::ResetToken(self.path.remote, token));
                    }
                    self.handle_peer_params(params)?;
                    self.issue_first_cids(now);
                } else {
                    // Server-only
                    self.spaces[SpaceId::Data].pending.handshake_done = true;
                    self.discard_space(now, SpaceId::Handshake);
                }

                self.events.push_back(Event::Connected);
                self.state = State::Established;
                trace!("established");
                Ok(())
            }
            Header::Initial(InitialHeader {
                src_cid: rem_cid, ..
            }) => {
                if !state.rem_cid_set {
                    trace!("switching remote CID to {}", rem_cid);
                    let mut state = state.clone();
                    self.rem_cids.update_initial_cid(rem_cid);
                    self.rem_handshake_cid = rem_cid;
                    self.orig_rem_cid = rem_cid;
                    state.rem_cid_set = true;
                    self.state = State::Handshake(state);
                } else if rem_cid != self.rem_handshake_cid {
                    debug!(
                        "discarding packet with mismatched remote CID: {} != {}",
                        self.rem_handshake_cid, rem_cid
                    );
                    return Ok(());
                }

                let starting_space = self.highest_space;
                self.process_early_payload(now, packet)?;

                if self.side.is_server()
                    && starting_space == SpaceId::Initial
                    && self.highest_space != SpaceId::Initial
                {
                    let params =
                        self.crypto
                            .transport_parameters()?
                            .ok_or_else(|| TransportError {
                                code: TransportErrorCode::crypto(0x6d),
                                frame: None,
                                reason: "transport parameters missing".into(),
                            })?;
                    self.handle_peer_params(params)?;
                    self.issue_first_cids(now);
                    self.init_0rtt();
                }
                Ok(())
            }
            Header::Long {
                ty: LongType::ZeroRtt,
                ..
            } => {
                self.process_payload(now, remote, number.unwrap(), packet)?;
                Ok(())
            }
            Header::VersionNegotiate { .. } => {
                if self.total_authed_packets > 1 {
                    return Ok(());
                }
                let supported = packet
                    .payload
                    .chunks(4)
                    .any(|x| match <[u8; 4]>::try_from(x) {
                        Ok(version) => self.version == u32::from_be_bytes(version),
                        Err(_) => false,
                    });
                if supported {
                    return Ok(());
                }
                debug!("remote doesn't support our version");
                Err(ConnectionError::VersionMismatch)
            }
            Header::Short { .. } => unreachable!(
                "short packets received during handshake are discarded in handle_packet"
            ),
        }
    }

    /// Process an Initial or Handshake packet payload
    fn process_early_payload(
        &mut self,
        now: Instant,
        packet: Packet,
    ) -> Result<(), TransportError> {
        debug_assert_ne!(packet.header.space(), SpaceId::Data);
        let payload_len = packet.payload.len();
        let mut ack_eliciting = false;
        for result in frame::Iter::new(packet.payload.freeze())? {
            let frame = result?;
            let span = match frame {
                Frame::Padding => continue,
                _ => Some(trace_span!("frame", ty = %frame.ty())),
            };

            self.stats.frame_rx.record(&frame);

            let _guard = span.as_ref().map(|x| x.enter());
            ack_eliciting |= frame.is_ack_eliciting();

            // Process frames
            match frame {
                Frame::Padding | Frame::Ping => {}
                Frame::Crypto(frame) => {
                    self.read_crypto(packet.header.space(), &frame, payload_len)?;
                }
                Frame::Ack(ack) => {
                    self.on_ack_received(now, packet.header.space(), ack)?;
                }
                Frame::Close(reason) => {
                    self.error = Some(reason.into());
                    self.state = State::Draining;
                    return Ok(());
                }
                _ => {
                    let mut err =
                        TransportError::PROTOCOL_VIOLATION("illegal frame type in handshake");
                    err.frame = Some(frame.ty());
                    return Err(err);
                }
            }
        }

        if ack_eliciting {
            // In the initial and handshake spaces, ACKs must be sent immediately
            self.spaces[packet.header.space()]
                .pending_acks
                .set_immediate_ack_required();
        }

        self.write_crypto();
        Ok(())
    }

    fn process_payload(
        &mut self,
        now: Instant,
        remote: SocketAddr,
        number: u64,
        packet: Packet,
    ) -> Result<(), TransportError> {
        let payload = packet.payload.freeze();
        let mut is_probing_packet = true;
        let mut close = None;
        let payload_len = payload.len();
        let mut ack_eliciting = false;
        for result in frame::Iter::new(payload)? {
            let frame = result?;
            let span = match frame {
                Frame::Padding => continue,
                _ => Some(trace_span!("frame", ty = %frame.ty())),
            };

            self.stats.frame_rx.record(&frame);
            // Crypto, Stream and Datagram frames are special cased in order no pollute
            // the log with payload data
            match &frame {
                Frame::Crypto(f) => {
                    trace!(offset = f.offset, len = f.data.len(), "got crypto frame");
                }
                Frame::Stream(f) => {
                    trace!(id = %f.id, offset = f.offset, len = f.data.len(), fin = f.fin, "got stream frame");
                }
                Frame::Datagram(f) => {
                    trace!(len = f.data.len(), "got datagram frame");
                }
                f => {
                    trace!("got frame {:?}", f);
                }
            }

            let _guard = span.as_ref().map(|x| x.enter());
            if packet.header.is_0rtt() {
                match frame {
                    Frame::Crypto(_) | Frame::Close(Close::Application(_)) => {
                        return Err(TransportError::PROTOCOL_VIOLATION(
                            "illegal frame type in 0-RTT",
                        ));
                    }
                    _ => {}
                }
            }
            ack_eliciting |= frame.is_ack_eliciting();

            // Check whether this could be a probing packet
            match frame {
                Frame::Padding
                | Frame::PathChallenge(_)
                | Frame::PathResponse(_)
                | Frame::NewConnectionId(_) => {}
                _ => {
                    is_probing_packet = false;
                }
            }
            match frame {
                Frame::Crypto(frame) => {
                    self.read_crypto(SpaceId::Data, &frame, payload_len)?;
                }
                Frame::Stream(frame) => {
                    if self.streams.received(frame, payload_len)?.should_transmit() {
                        self.spaces[SpaceId::Data].pending.max_data = true;
                    }
                }
                Frame::Ack(ack) => {
                    self.on_ack_received(now, SpaceId::Data, ack)?;
                }
                Frame::Padding | Frame::Ping => {}
                Frame::Close(reason) => {
                    close = Some(reason);
                }
                Frame::PathChallenge(token) => {
                    self.path_responses.push(number, token, remote);
                    if remote == self.path.remote {
                        // PATH_CHALLENGE on active path, possible off-path packet forwarding
                        // attack. Send a non-probing packet to recover the active path.
                        match self.peer_supports_ack_frequency() {
                            true => self.immediate_ack(),
                            false => self.ping(),
                        }
                    }
                }
                Frame::PathResponse(token) => {
                    if self.path.challenge == Some(token) && remote == self.path.remote {
                        trace!("new path validated");
                        self.timers.stop(Timer::PathValidation);
                        self.path.challenge = None;
                        self.path.validated = true;
                        if let Some((_, ref mut prev_path)) = self.prev_path {
                            prev_path.challenge = None;
                            prev_path.challenge_pending = false;
                        }
                    } else {
                        debug!(token, "ignoring invalid PATH_RESPONSE");
                    }
                }
                Frame::MaxData(bytes) => {
                    self.streams.received_max_data(bytes);
                }
                Frame::MaxStreamData { id, offset } => {
                    self.streams.received_max_stream_data(id, offset)?;
                }
                Frame::MaxStreams { dir, count } => {
                    self.streams.received_max_streams(dir, count)?;
                }
                Frame::ResetStream(frame) => {
                    if self.streams.received_reset(frame)?.should_transmit() {
                        self.spaces[SpaceId::Data].pending.max_data = true;
                    }
                }
                Frame::DataBlocked { offset } => {
                    debug!(offset, "peer claims to be blocked at connection level");
                }
                Frame::StreamDataBlocked { id, offset } => {
                    if id.initiator() == self.side.side() && id.dir() == Dir::Uni {
                        debug!("got STREAM_DATA_BLOCKED on send-only {}", id);
                        return Err(TransportError::STREAM_STATE_ERROR(
                            "STREAM_DATA_BLOCKED on send-only stream",
                        ));
                    }
                    debug!(
                        stream = %id,
                        offset, "peer claims to be blocked at stream level"
                    );
                }
                Frame::StreamsBlocked { dir, limit } => {
                    if limit > MAX_STREAM_COUNT {
                        return Err(TransportError::FRAME_ENCODING_ERROR(
                            "unrepresentable stream limit",
                        ));
                    }
                    debug!(
                        "peer claims to be blocked opening more than {} {} streams",
                        limit, dir
                    );
                }
                Frame::StopSending(frame::StopSending { id, error_code }) => {
                    if id.initiator() != self.side.side() {
                        if id.dir() == Dir::Uni {
                            debug!("got STOP_SENDING on recv-only {}", id);
                            return Err(TransportError::STREAM_STATE_ERROR(
                                "STOP_SENDING on recv-only stream",
                            ));
                        }
                    } else if self.streams.is_local_unopened(id) {
                        return Err(TransportError::STREAM_STATE_ERROR(
                            "STOP_SENDING on unopened stream",
                        ));
                    }
                    self.streams.received_stop_sending(id, error_code);
                }
                Frame::RetireConnectionId { sequence } => {
                    let allow_more_cids = self
                        .local_cid_state
                        .on_cid_retirement(sequence, self.peer_params.issue_cids_limit())?;
                    self.endpoint_events
                        .push_back(EndpointEventInner::RetireConnectionId(
                            now,
                            sequence,
                            allow_more_cids,
                        ));
                }
                Frame::NewConnectionId(frame) => {
                    trace!(
                        sequence = frame.sequence,
                        id = %frame.id,
                        retire_prior_to = frame.retire_prior_to,
                    );
                    if self.rem_cids.active().is_empty() {
                        return Err(TransportError::PROTOCOL_VIOLATION(
                            "NEW_CONNECTION_ID when CIDs aren't in use",
                        ));
                    }
                    if frame.retire_prior_to > frame.sequence {
                        return Err(TransportError::PROTOCOL_VIOLATION(
                            "NEW_CONNECTION_ID retiring unissued CIDs",
                        ));
                    }

                    use crate::cid_queue::InsertError;
                    match self.rem_cids.insert(frame) {
                        Ok(None) => {}
                        Ok(Some((retired, reset_token))) => {
                            let pending_retired =
                                &mut self.spaces[SpaceId::Data].pending.retire_cids;
                            /// Ensure `pending_retired` cannot grow without bound. Limit is
                            /// somewhat arbitrary but very permissive.
                            const MAX_PENDING_RETIRED_CIDS: u64 = CidQueue::LEN as u64 * 10;
                            // We don't bother counting in-flight frames because those are bounded
                            // by congestion control.
                            if (pending_retired.len() as u64)
                                .saturating_add(retired.end.saturating_sub(retired.start))
                                > MAX_PENDING_RETIRED_CIDS
                            {
                                return Err(TransportError::CONNECTION_ID_LIMIT_ERROR(
                                    "queued too many retired CIDs",
                                ));
                            }
                            pending_retired.extend(retired);
                            self.set_reset_token(reset_token);
                        }
                        Err(InsertError::ExceedsLimit) => {
                            return Err(TransportError::CONNECTION_ID_LIMIT_ERROR(""));
                        }
                        Err(InsertError::Retired) => {
                            trace!("discarding already-retired");
                            // RETIRE_CONNECTION_ID might not have been previously sent if e.g. a
                            // range of connection IDs larger than the active connection ID limit
                            // was retired all at once via retire_prior_to.
                            self.spaces[SpaceId::Data]
                                .pending
                                .retire_cids
                                .push(frame.sequence);
                            continue;
                        }
                    };

                    if self.side.is_server() && self.rem_cids.active_seq() == 0 {
                        // We're a server still using the initial remote CID for the client, so
                        // let's switch immediately to enable clientside stateless resets.
                        self.update_rem_cid();
                    }
                }
                Frame::NewToken(NewToken { token }) => {
                    let ConnectionSide::Client {
                        token_store,
                        server_name,
                        ..
                    } = &self.side
                    else {
                        return Err(TransportError::PROTOCOL_VIOLATION("client sent NEW_TOKEN"));
                    };
                    if token.is_empty() {
                        return Err(TransportError::FRAME_ENCODING_ERROR("empty token"));
                    }
                    trace!("got new token");
                    token_store.insert(server_name, token);
                }
                Frame::Datagram(datagram) => {
                    if self
                        .datagrams
                        .received(datagram, &self.config.datagram_receive_buffer_size)?
                    {
                        self.events.push_back(Event::DatagramReceived);
                    }
                }
                Frame::AckFrequency(ack_frequency) => {
                    // This frame can only be sent in the Data space
                    let space = &mut self.spaces[SpaceId::Data];

                    if !self
                        .ack_frequency
                        .ack_frequency_received(&ack_frequency, &mut space.pending_acks)?
                    {
                        // The AckFrequency frame is stale (we have already received a more recent one)
                        continue;
                    }

                    // Our `max_ack_delay` has been updated, so we may need to adjust its associated
                    // timeout
                    if let Some(timeout) = space
                        .pending_acks
                        .max_ack_delay_timeout(self.ack_frequency.max_ack_delay)
                    {
                        self.timers.set(Timer::MaxAckDelay, timeout);
                    }
                }
                Frame::ImmediateAck => {
                    // This frame can only be sent in the Data space
                    self.spaces[SpaceId::Data]
                        .pending_acks
                        .set_immediate_ack_required();
                }
                Frame::HandshakeDone => {
                    if self.side.is_server() {
                        return Err(TransportError::PROTOCOL_VIOLATION(
                            "client sent HANDSHAKE_DONE",
                        ));
                    }
                    if self.spaces[SpaceId::Handshake].crypto.is_some() {
                        self.discard_space(now, SpaceId::Handshake);
                    }
                }
            }
        }

        let space = &mut self.spaces[SpaceId::Data];
        if space
            .pending_acks
            .packet_received(now, number, ack_eliciting, &space.dedup)
        {
            self.timers
                .set(Timer::MaxAckDelay, now + self.ack_frequency.max_ack_delay);
        }

        // Issue stream ID credit due to ACKs of outgoing finish/resets and incoming finish/resets
        // on stopped streams. Incoming finishes/resets on open streams are not handled here as they
        // are only freed, and hence only issue credit, once the application has been notified
        // during a read on the stream.
        let pending = &mut self.spaces[SpaceId::Data].pending;
        self.streams.queue_max_stream_id(pending);

        if let Some(reason) = close {
            self.error = Some(reason.into());
            self.state = State::Draining;
            self.close = true;
        }

        if remote != self.path.remote
            && !is_probing_packet
            && number == self.spaces[SpaceId::Data].rx_packet
        {
            let ConnectionSide::Server { ref server_config } = self.side else {
                panic!("packets from unknown remote should be dropped by clients");
            };
            debug_assert!(
                server_config.migration,
                "migration-initiating packets should have been dropped immediately"
            );
            self.migrate(now, remote);
            // Break linkability, if possible
            self.update_rem_cid();
            self.spin = false;
        }

        Ok(())
    }

    fn migrate(&mut self, now: Instant, remote: SocketAddr) {
        trace!(%remote, "migration initiated");
        self.path_counter = self.path_counter.wrapping_add(1);
        // Reset rtt/congestion state for new path unless it looks like a NAT rebinding.
        // Note that the congestion window will not grow until validation terminates. Helps mitigate
        // amplification attacks performed by spoofing source addresses.
        let mut new_path = if remote.is_ipv4() && remote.ip() == self.path.remote.ip() {
            PathData::from_previous(remote, &self.path, self.path_counter, now)
        } else {
            let peer_max_udp_payload_size =
                u16::try_from(self.peer_params.max_udp_payload_size.into_inner())
                    .unwrap_or(u16::MAX);
            PathData::new(
                remote,
                self.allow_mtud,
                Some(peer_max_udp_payload_size),
                self.path_counter,
                now,
                &self.config,
            )
        };
        new_path.challenge = Some(self.rng.random());
        new_path.challenge_pending = true;
        let prev_pto = self.pto(SpaceId::Data);

        let mut prev = mem::replace(&mut self.path, new_path);
        // Don't clobber the original path if the previous one hasn't been validated yet
        if prev.challenge.is_none() {
            prev.challenge = Some(self.rng.random());
            prev.challenge_pending = true;
            // We haven't updated the remote CID yet, this captures the remote CID we were using on
            // the previous path.
            self.prev_path = Some((self.rem_cids.active(), prev));
        }

        self.timers.set(
            Timer::PathValidation,
            now + 3 * cmp::max(self.pto(SpaceId::Data), prev_pto),
        );
    }

    /// Handle a change in the local address, i.e. an active migration
    pub fn local_address_changed(&mut self) {
        self.update_rem_cid();
        self.ping();
    }

    /// Switch to a previously unused remote connection ID, if possible
    fn update_rem_cid(&mut self) {
        let (reset_token, retired) = match self.rem_cids.next() {
            Some(x) => x,
            None => return,
        };

        // Retire the current remote CID and any CIDs we had to skip.
        self.spaces[SpaceId::Data]
            .pending
            .retire_cids
            .extend(retired);
        self.set_reset_token(reset_token);
    }

    fn set_reset_token(&mut self, reset_token: ResetToken) {
        self.endpoint_events
            .push_back(EndpointEventInner::ResetToken(
                self.path.remote,
                reset_token,
            ));
        self.peer_params.stateless_reset_token = Some(reset_token);
    }

    /// Issue an initial set of connection IDs to the peer upon connection
    fn issue_first_cids(&mut self, now: Instant) {
        if self.local_cid_state.cid_len() == 0 {
            return;
        }

        // Subtract 1 to account for the CID we supplied while handshaking
        let mut n = self.peer_params.issue_cids_limit() - 1;
        if let ConnectionSide::Server { server_config } = &self.side {
            if server_config.has_preferred_address() {
                // We also sent a CID in the transport parameters
                n -= 1;
            }
        }
        self.endpoint_events
            .push_back(EndpointEventInner::NeedIdentifiers(now, n));
    }

    fn populate_packet(
        &mut self,
        now: Instant,
        space_id: SpaceId,
        buf: &mut Vec<u8>,
        max_size: usize,
        pn: u64,
    ) -> SentFrames {
        let mut sent = SentFrames::default();
        let space = &mut self.spaces[space_id];
        let is_0rtt = space_id == SpaceId::Data && space.crypto.is_none();
        space.pending_acks.maybe_ack_non_eliciting();

        // HANDSHAKE_DONE
        if !is_0rtt && mem::replace(&mut space.pending.handshake_done, false) {
            buf.write(frame::FrameType::HANDSHAKE_DONE);
            sent.retransmits.get_or_create().handshake_done = true;
            // This is just a u8 counter and the frame is typically just sent once
            self.stats.frame_tx.handshake_done =
                self.stats.frame_tx.handshake_done.saturating_add(1);
        }

        // PING
        if mem::replace(&mut space.ping_pending, false) {
            trace!("PING");
            buf.write(frame::FrameType::PING);
            sent.non_retransmits = true;
            self.stats.frame_tx.ping += 1;
        }

        // IMMEDIATE_ACK
        if mem::replace(&mut space.immediate_ack_pending, false) {
            trace!("IMMEDIATE_ACK");
            buf.write(frame::FrameType::IMMEDIATE_ACK);
            sent.non_retransmits = true;
            self.stats.frame_tx.immediate_ack += 1;
        }

        // ACK
        if space.pending_acks.can_send() {
            Self::populate_acks(
                now,
                self.receiving_ecn,
                &mut sent,
                space,
                buf,
                &mut self.stats,
            );
        }

        // ACK_FREQUENCY
        if mem::replace(&mut space.pending.ack_frequency, false) {
            let sequence_number = self.ack_frequency.next_sequence_number();

            // Safe to unwrap because this is always provided when ACK frequency is enabled
            let config = self.config.ack_frequency_config.as_ref().unwrap();

            // Ensure the delay is within bounds to avoid a PROTOCOL_VIOLATION error
            let max_ack_delay = self.ack_frequency.candidate_max_ack_delay(
                self.path.rtt.get(),
                config,
                &self.peer_params,
            );

            trace!(?max_ack_delay, "ACK_FREQUENCY");

            frame::AckFrequency {
                sequence: sequence_number,
                ack_eliciting_threshold: config.ack_eliciting_threshold,
                request_max_ack_delay: max_ack_delay.as_micros().try_into().unwrap_or(VarInt::MAX),
                reordering_threshold: config.reordering_threshold,
            }
            .encode(buf);

            sent.retransmits.get_or_create().ack_frequency = true;

            self.ack_frequency.ack_frequency_sent(pn, max_ack_delay);
            self.stats.frame_tx.ack_frequency += 1;
        }

        // PATH_CHALLENGE
        if buf.len() + 9 < max_size && space_id == SpaceId::Data {
            // Transmit challenges with every outgoing frame on an unvalidated path
            if let Some(token) = self.path.challenge {
                // But only send a packet solely for that purpose at most once
                self.path.challenge_pending = false;
                sent.non_retransmits = true;
                sent.requires_padding = true;
                trace!("PATH_CHALLENGE {:08x}", token);
                buf.write(frame::FrameType::PATH_CHALLENGE);
                buf.write(token);
                self.stats.frame_tx.path_challenge += 1;
            }
        }

        // PATH_RESPONSE
        if buf.len() + 9 < max_size && space_id == SpaceId::Data {
            if let Some(token) = self.path_responses.pop_on_path(self.path.remote) {
                sent.non_retransmits = true;
                sent.requires_padding = true;
                trace!("PATH_RESPONSE {:08x}", token);
                buf.write(frame::FrameType::PATH_RESPONSE);
                buf.write(token);
                self.stats.frame_tx.path_response += 1;
            }
        }

        // CRYPTO
        while buf.len() + frame::Crypto::SIZE_BOUND < max_size && !is_0rtt {
            let mut frame = match space.pending.crypto.pop_front() {
                Some(x) => x,
                None => break,
            };

            // Calculate the maximum amount of crypto data we can store in the buffer.
            // Since the offset is known, we can reserve the exact size required to encode it.
            // For length we reserve 2bytes which allows to encode up to 2^14,
            // which is more than what fits into normally sized QUIC frames.
            let max_crypto_data_size = max_size
                - buf.len()
                - 1 // Frame Type
                - VarInt::size(unsafe { VarInt::from_u64_unchecked(frame.offset) })
                - 2; // Maximum encoded length for frame size, given we send less than 2^14 bytes

            let len = frame
                .data
                .len()
                .min(2usize.pow(14) - 1)
                .min(max_crypto_data_size);

            let data = frame.data.split_to(len);
            let truncated = frame::Crypto {
                offset: frame.offset,
                data,
            };
            trace!(
                "CRYPTO: off {} len {}",
                truncated.offset,
                truncated.data.len()
            );
            truncated.encode(buf);
            self.stats.frame_tx.crypto += 1;
            sent.retransmits.get_or_create().crypto.push_back(truncated);
            if !frame.data.is_empty() {
                frame.offset += len as u64;
                space.pending.crypto.push_front(frame);
            }
        }

        if space_id == SpaceId::Data {
            self.streams.write_control_frames(
                buf,
                &mut space.pending,
                &mut sent.retransmits,
                &mut self.stats.frame_tx,
                max_size,
            );
        }

        // NEW_CONNECTION_ID
        while buf.len() + NewConnectionId::SIZE_BOUND < max_size {
            let issued = match space.pending.new_cids.pop() {
                Some(x) => x,
                None => break,
            };
            trace!(
                sequence = issued.sequence,
                id = %issued.id,
                "NEW_CONNECTION_ID"
            );
            frame::NewConnectionId {
                sequence: issued.sequence,
                retire_prior_to: self.local_cid_state.retire_prior_to(),
                id: issued.id,
                reset_token: issued.reset_token,
            }
            .encode(buf);
            sent.retransmits.get_or_create().new_cids.push(issued);
            self.stats.frame_tx.new_connection_id += 1;
        }

        // RETIRE_CONNECTION_ID
        while buf.len() + frame::RETIRE_CONNECTION_ID_SIZE_BOUND < max_size {
            let seq = match space.pending.retire_cids.pop() {
                Some(x) => x,
                None => break,
            };
            trace!(sequence = seq, "RETIRE_CONNECTION_ID");
            buf.write(frame::FrameType::RETIRE_CONNECTION_ID);
            buf.write_var(seq);
            sent.retransmits.get_or_create().retire_cids.push(seq);
            self.stats.frame_tx.retire_connection_id += 1;
        }

        // DATAGRAM
        let mut sent_datagrams = false;
        while buf.len() + Datagram::SIZE_BOUND < max_size && space_id == SpaceId::Data {
            match self.datagrams.write(buf, max_size) {
                true => {
                    sent_datagrams = true;
                    sent.non_retransmits = true;
                    self.stats.frame_tx.datagram += 1;
                }
                false => break,
            }
        }
        if self.datagrams.send_blocked && sent_datagrams {
            self.events.push_back(Event::DatagramsUnblocked);
            self.datagrams.send_blocked = false;
        }

        // NEW_TOKEN
        while let Some(remote_addr) = space.pending.new_tokens.pop() {
            debug_assert_eq!(space_id, SpaceId::Data);
            let ConnectionSide::Server { server_config } = &self.side else {
                panic!("NEW_TOKEN frames should not be enqueued by clients");
            };

            if remote_addr != self.path.remote {
                // NEW_TOKEN frames contain tokens bound to a client's IP address, and are only
                // useful if used from the same IP address.  Thus, we abandon enqueued NEW_TOKEN
                // frames upon an path change. Instead, when the new path becomes validated,
                // NEW_TOKEN frames may be enqueued for the new path instead.
                continue;
            }

            let token = Token::new(
                TokenPayload::Validation {
                    ip: remote_addr.ip(),
                    issued: server_config.time_source.now(),
                },
                &mut self.rng,
            );
            let new_token = NewToken {
                token: token.encode(&*server_config.token_key).into(),
            };

            if buf.len() + new_token.size() >= max_size {
                space.pending.new_tokens.push(remote_addr);
                break;
            }

            new_token.encode(buf);
            sent.retransmits
                .get_or_create()
                .new_tokens
                .push(remote_addr);
            self.stats.frame_tx.new_token += 1;
        }

        // STREAM
        if space_id == SpaceId::Data {
            sent.stream_frames =
                self.streams
                    .write_stream_frames(buf, max_size, self.config.send_fairness);
            self.stats.frame_tx.stream += sent.stream_frames.len() as u64;
        }

        sent
    }

    /// Write pending ACKs into a buffer
    ///
    /// This method assumes ACKs are pending, and should only be called if
    /// `!PendingAcks::ranges().is_empty()` returns `true`.
    fn populate_acks(
        now: Instant,
        receiving_ecn: bool,
        sent: &mut SentFrames,
        space: &mut PacketSpace,
        buf: &mut Vec<u8>,
        stats: &mut ConnectionStats,
    ) {
        debug_assert!(!space.pending_acks.ranges().is_empty());

        // 0-RTT packets must never carry acks (which would have to be of handshake packets)
        debug_assert!(space.crypto.is_some(), "tried to send ACK in 0-RTT");
        let ecn = if receiving_ecn {
            Some(&space.ecn_counters)
        } else {
            None
        };
        sent.largest_acked = space.pending_acks.ranges().max();

        let delay_micros = space.pending_acks.ack_delay(now).as_micros() as u64;

        // TODO: This should come from `TransportConfig` if that gets configurable.
        let ack_delay_exp = TransportParameters::default().ack_delay_exponent;
        let delay = delay_micros >> ack_delay_exp.into_inner();

        trace!(
            "ACK {:?}, Delay = {}us",
            space.pending_acks.ranges(),
            delay_micros
        );

        frame::Ack::encode(delay as _, space.pending_acks.ranges(), ecn, buf);
        stats.frame_tx.acks += 1;
    }

    fn close_common(&mut self) {
        trace!("connection closed");
        for &timer in &Timer::VALUES {
            self.timers.stop(timer);
        }
    }

    fn set_close_timer(&mut self, now: Instant) {
        self.timers
            .set(Timer::Close, now + 3 * self.pto(self.highest_space));
    }

    /// Handle transport parameters received from the peer
    fn handle_peer_params(&mut self, params: TransportParameters) -> Result<(), TransportError> {
        if Some(self.orig_rem_cid) != params.initial_src_cid
            || (self.side.is_client()
                && (Some(self.initial_dst_cid) != params.original_dst_cid
                    || self.retry_src_cid != params.retry_src_cid))
        {
            return Err(TransportError::TRANSPORT_PARAMETER_ERROR(
                "CID authentication failure",
            ));
        }

        self.set_peer_params(params);

        Ok(())
    }

    fn set_peer_params(&mut self, params: TransportParameters) {
        self.streams.set_params(&params);
        self.idle_timeout =
            negotiate_max_idle_timeout(self.config.max_idle_timeout, Some(params.max_idle_timeout));
        trace!("negotiated max idle timeout {:?}", self.idle_timeout);
        if let Some(ref info) = params.preferred_address {
            self.rem_cids.insert(frame::NewConnectionId {
                sequence: 1,
                id: info.connection_id,
                reset_token: info.stateless_reset_token,
                retire_prior_to: 0,
            }).expect("preferred address CID is the first received, and hence is guaranteed to be legal");
        }
        self.ack_frequency.peer_max_ack_delay = get_max_ack_delay(&params);
        self.peer_params = params;
        self.path.mtud.on_peer_max_udp_payload_size_received(
            u16::try_from(self.peer_params.max_udp_payload_size.into_inner()).unwrap_or(u16::MAX),
        );
    }

    fn decrypt_packet(
        &mut self,
        now: Instant,
        packet: &mut Packet,
    ) -> Result<Option<u64>, Option<TransportError>> {
        let result = packet_crypto::decrypt_packet_body(
            packet,
            &self.spaces,
            self.zero_rtt_crypto.as_ref(),
            self.key_phase,
            self.prev_crypto.as_ref(),
            self.next_crypto.as_ref(),
        )?;

        let result = match result {
            Some(r) => r,
            None => return Ok(None),
        };

        if result.outgoing_key_update_acked {
            if let Some(prev) = self.prev_crypto.as_mut() {
                prev.end_packet = Some((result.number, now));
                self.set_key_discard_timer(now, packet.header.space());
            }
        }

        if result.incoming_key_update {
            trace!("key update authenticated");
            self.update_keys(Some((result.number, now)), true);
            self.set_key_discard_timer(now, packet.header.space());
        }

        Ok(Some(result.number))
    }

    fn update_keys(&mut self, end_packet: Option<(u64, Instant)>, remote: bool) {
        trace!("executing key update");
        // Generate keys for the key phase after the one we're switching to, store them in
        // `next_crypto`, make the contents of `next_crypto` current, and move the current keys into
        // `prev_crypto`.
        let new = self
            .crypto
            .next_1rtt_keys()
            .expect("only called for `Data` packets");
        self.key_phase_size = new
            .local
            .confidentiality_limit()
            .saturating_sub(KEY_UPDATE_MARGIN);
        let old = mem::replace(
            &mut self.spaces[SpaceId::Data]
                .crypto
                .as_mut()
                .unwrap() // safe because update_keys() can only be triggered by short packets
                .packet,
            mem::replace(self.next_crypto.as_mut().unwrap(), new),
        );
        self.spaces[SpaceId::Data].sent_with_keys = 0;
        self.prev_crypto = Some(PrevCrypto {
            crypto: old,
            end_packet,
            update_unacked: remote,
        });
        self.key_phase = !self.key_phase;
    }

    fn peer_supports_ack_frequency(&self) -> bool {
        self.peer_params.min_ack_delay.is_some()
    }

    /// Send an IMMEDIATE_ACK frame to the remote endpoint
    ///
    /// According to the spec, this will result in an error if the remote endpoint does not support
    /// the Acknowledgement Frequency extension
    pub(crate) fn immediate_ack(&mut self) {
        self.spaces[self.highest_space].immediate_ack_pending = true;
    }

    /// Decodes a packet, returning its decrypted payload, so it can be inspected in tests
    #[cfg(test)]
    pub(crate) fn decode_packet(&self, event: &ConnectionEvent) -> Option<Vec<u8>> {
        let (first_decode, remaining) = match &event.0 {
            ConnectionEventInner::Datagram(DatagramConnectionEvent {
                first_decode,
                remaining,
                ..
            }) => (first_decode, remaining),
            _ => return None,
        };

        if remaining.is_some() {
            panic!("Packets should never be coalesced in tests");
        }

        let decrypted_header = packet_crypto::unprotect_header(
            first_decode.clone(),
            &self.spaces,
            self.zero_rtt_crypto.as_ref(),
            self.peer_params.stateless_reset_token,
        )?;

        let mut packet = decrypted_header.packet?;
        packet_crypto::decrypt_packet_body(
            &mut packet,
            &self.spaces,
            self.zero_rtt_crypto.as_ref(),
            self.key_phase,
            self.prev_crypto.as_ref(),
            self.next_crypto.as_ref(),
        )
        .ok()?;

        Some(packet.payload.to_vec())
    }

    /// The number of bytes of packets containing retransmittable frames that have not been
    /// acknowledged or declared lost.
    #[cfg(test)]
    pub(crate) fn bytes_in_flight(&self) -> u64 {
        self.path.in_flight.bytes
    }

    /// Number of bytes worth of non-ack-only packets that may be sent
    #[cfg(test)]
    pub(crate) fn congestion_window(&self) -> u64 {
        self.path
            .congestion
            .window()
            .saturating_sub(self.path.in_flight.bytes)
    }

    /// Whether no timers but keepalive, idle, rtt, pushnewcid, and key discard are running
    #[cfg(test)]
    pub(crate) fn is_idle(&self) -> bool {
        Timer::VALUES
            .iter()
            .filter(|&&t| !matches!(t, Timer::KeepAlive | Timer::PushNewCid | Timer::KeyDiscard))
            .filter_map(|&t| Some((t, self.timers.get(t)?)))
            .min_by_key(|&(_, time)| time)
            .map_or(true, |(timer, _)| timer == Timer::Idle)
    }

    /// Whether explicit congestion notification is in use on outgoing packets.
    #[cfg(test)]
    pub(crate) fn using_ecn(&self) -> bool {
        self.path.sending_ecn
    }

    /// The number of received bytes in the current path
    #[cfg(test)]
    pub(crate) fn total_recvd(&self) -> u64 {
        self.path.total_recvd
    }

    #[cfg(test)]
    pub(crate) fn active_local_cid_seq(&self) -> (u64, u64) {
        self.local_cid_state.active_seq()
    }

    /// Instruct the peer to replace previously issued CIDs by sending a NEW_CONNECTION_ID frame
    /// with updated `retire_prior_to` field set to `v`
    #[cfg(test)]
    pub(crate) fn rotate_local_cid(&mut self, v: u64, now: Instant) {
        let n = self.local_cid_state.assign_retire_seq(v);
        self.endpoint_events
            .push_back(EndpointEventInner::NeedIdentifiers(now, n));
    }

    /// Check the current active remote CID sequence
    #[cfg(test)]
    pub(crate) fn active_rem_cid_seq(&self) -> u64 {
        self.rem_cids.active_seq()
    }

    /// Returns the detected maximum udp payload size for the current path
    #[cfg(test)]
    pub(crate) fn path_mtu(&self) -> u16 {
        self.path.current_mtu()
    }

    /// Whether we have 1-RTT data to send
    ///
    /// See also `self.space(SpaceId::Data).can_send()`
    fn can_send_1rtt(&self, max_size: usize) -> bool {
        self.streams.can_send_stream_data()
            || self.path.challenge_pending
            || self
                .prev_path
                .as_ref()
                .is_some_and(|(_, x)| x.challenge_pending)
            || !self.path_responses.is_empty()
            || self
                .datagrams
                .outgoing
                .front()
                .is_some_and(|x| x.size(true) <= max_size)
    }

    /// Update counters to account for a packet becoming acknowledged, lost, or abandoned
    fn remove_in_flight(&mut self, packet: &SentPacket) {
        // Visit known paths from newest to oldest to find the one `packet` was sent on
        for path in [&mut self.path]
            .into_iter()
            .chain(self.prev_path.as_mut().map(|(_, data)| data))
        {
            if path.remove_in_flight(packet) {
                return;
            }
        }
    }

    /// Terminate the connection instantly, without sending a close packet
    fn kill(&mut self, reason: ConnectionError) {
        self.close_common();
        self.error = Some(reason);
        self.state = State::Drained;
        self.endpoint_events.push_back(EndpointEventInner::Drained);
    }

    /// Storage size required for the largest packet known to be supported by the current path
    ///
    /// Buffers passed to [`Connection::poll_transmit`] should be at least this large.
    pub fn current_mtu(&self) -> u16 {
        self.path.current_mtu()
    }

    /// Size of non-frame data for a 1-RTT packet
    ///
    /// Quantifies space consumed by the QUIC header and AEAD tag. All other bytes in a packet are
    /// frames. Changes if the length of the remote connection ID changes, which is expected to be
    /// rare. If `pn` is specified, may additionally change unpredictably due to variations in
    /// latency and packet loss.
    fn predict_1rtt_overhead(&self, pn: Option<u64>) -> usize {
        let pn_len = match pn {
            Some(pn) => PacketNumber::new(
                pn,
                self.spaces[SpaceId::Data].largest_acked_packet.unwrap_or(0),
            )
            .len(),
            // Upper bound
            None => 4,
        };

        // 1 byte for flags
        1 + self.rem_cids.active().len() + pn_len + self.tag_len_1rtt()
    }

    fn tag_len_1rtt(&self) -> usize {
        let key = match self.spaces[SpaceId::Data].crypto.as_ref() {
            Some(crypto) => Some(&*crypto.packet.local),
            None => self.zero_rtt_crypto.as_ref().map(|x| &*x.packet),
        };
        // If neither Data nor 0-RTT keys are available, make a reasonable tag length guess. As of
        // this writing, all QUIC cipher suites use 16-byte tags. We could return `None` instead,
        // but that would needlessly prevent sending datagrams during 0-RTT.
        key.map_or(16, |x| x.tag_len())
    }

    /// Mark the path as validated, and enqueue NEW_TOKEN frames to be sent as appropriate
    fn on_path_validated(&mut self) {
        self.path.validated = true;
        let ConnectionSide::Server { server_config } = &self.side else {
            return;
        };
        let new_tokens = &mut self.spaces[SpaceId::Data as usize].pending.new_tokens;
        new_tokens.clear();
        for _ in 0..server_config.validation_token.sent {
            new_tokens.push(self.path.remote);
        }
    }
}

impl fmt::Debug for Connection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Connection")
            .field("handshake_cid", &self.handshake_cid)
            .finish()
    }
}

/// Fields of `Connection` specific to it being client-side or server-side
enum ConnectionSide {
    Client {
        /// Sent in every outgoing Initial packet. Always empty after Initial keys are discarded
        token: Bytes,
        token_store: Arc<dyn TokenStore>,
        server_name: String,
    },
    Server {
        server_config: Arc<ServerConfig>,
    },
}

impl ConnectionSide {
    fn remote_may_migrate(&self) -> bool {
        match self {
            Self::Server { server_config } => server_config.migration,
            Self::Client { .. } => false,
        }
    }

    fn is_client(&self) -> bool {
        self.side().is_client()
    }

    fn is_server(&self) -> bool {
        self.side().is_server()
    }

    fn side(&self) -> Side {
        match *self {
            Self::Client { .. } => Side::Client,
            Self::Server { .. } => Side::Server,
        }
    }
}

impl From<SideArgs> for ConnectionSide {
    fn from(side: SideArgs) -> Self {
        match side {
            SideArgs::Client {
                token_store,
                server_name,
            } => Self::Client {
                token: token_store.take(&server_name).unwrap_or_default(),
                token_store,
                server_name,
            },
            SideArgs::Server {
                server_config,
                pref_addr_cid: _,
                path_validated: _,
            } => Self::Server { server_config },
        }
    }
}

/// Parameters to `Connection::new` specific to it being client-side or server-side
pub(crate) enum SideArgs {
    Client {
        token_store: Arc<dyn TokenStore>,
        server_name: String,
    },
    Server {
        server_config: Arc<ServerConfig>,
        pref_addr_cid: Option<ConnectionId>,
        path_validated: bool,
    },
}

impl SideArgs {
    pub(crate) fn pref_addr_cid(&self) -> Option<ConnectionId> {
        match *self {
            Self::Client { .. } => None,
            Self::Server { pref_addr_cid, .. } => pref_addr_cid,
        }
    }

    pub(crate) fn path_validated(&self) -> bool {
        match *self {
            Self::Client { .. } => true,
            Self::Server { path_validated, .. } => path_validated,
        }
    }

    pub(crate) fn side(&self) -> Side {
        match *self {
            Self::Client { .. } => Side::Client,
            Self::Server { .. } => Side::Server,
        }
    }
}

/// Reasons why a connection might be lost
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ConnectionError {
    /// The peer doesn't implement any supported version
    #[error("peer doesn't implement any supported version")]
    VersionMismatch,
    /// The peer violated the QUIC specification as understood by this implementation
    #[error(transparent)]
    TransportError(#[from] TransportError),
    /// The peer's QUIC stack aborted the connection automatically
    #[error("aborted by peer: {0}")]
    ConnectionClosed(frame::ConnectionClose),
    /// The peer closed the connection
    #[error("closed by peer: {0}")]
    ApplicationClosed(frame::ApplicationClose),
    /// The peer is unable to continue processing this connection, usually due to having restarted
    #[error("reset by peer")]
    Reset,
    /// Communication with the peer has lapsed for longer than the negotiated idle timeout
    ///
    /// If neither side is sending keep-alives, a connection will time out after a long enough idle
    /// period even if the peer is still reachable. See also [`TransportConfig::max_idle_timeout()`]
    /// and [`TransportConfig::keep_alive_interval()`].
    #[error("timed out")]
    TimedOut,
    /// The local application closed the connection
    #[error("closed")]
    LocallyClosed,
    /// The connection could not be created because not enough of the CID space is available
    ///
    /// Try using longer connection IDs.
    #[error("CIDs exhausted")]
    CidsExhausted,
}

impl From<Close> for ConnectionError {
    fn from(x: Close) -> Self {
        match x {
            Close::Connection(reason) => Self::ConnectionClosed(reason),
            Close::Application(reason) => Self::ApplicationClosed(reason),
        }
    }
}

// For compatibility with API consumers
impl From<ConnectionError> for io::Error {
    fn from(x: ConnectionError) -> Self {
        use ConnectionError::*;
        let kind = match x {
            TimedOut => io::ErrorKind::TimedOut,
            Reset => io::ErrorKind::ConnectionReset,
            ApplicationClosed(_) | ConnectionClosed(_) => io::ErrorKind::ConnectionAborted,
            TransportError(_) | VersionMismatch | LocallyClosed | CidsExhausted => {
                io::ErrorKind::Other
            }
        };
        Self::new(kind, x)
    }
}

#[allow(unreachable_pub)] // fuzzing only
#[derive(Clone)]
pub enum State {
    Handshake(state::Handshake),
    Established,
    Closed(state::Closed),
    Draining,
    /// Waiting for application to call close so we can dispose of the resources
    Drained,
}

impl State {
    fn closed<R: Into<Close>>(reason: R) -> Self {
        Self::Closed(state::Closed {
            reason: reason.into(),
        })
    }

    fn is_handshake(&self) -> bool {
        matches!(*self, Self::Handshake(_))
    }

    fn is_established(&self) -> bool {
        matches!(*self, Self::Established)
    }

    fn is_closed(&self) -> bool {
        matches!(*self, Self::Closed(_) | Self::Draining | Self::Drained)
    }

    fn is_drained(&self) -> bool {
        matches!(*self, Self::Drained)
    }
}

mod state {
    use super::*;

    #[allow(unreachable_pub)] // fuzzing only
    #[derive(Clone)]
    pub struct Handshake {
        /// Whether the remote CID has been set by the peer yet
        ///
        /// Always set for servers
        pub(super) rem_cid_set: bool,
        /// Stateless retry token received in the first Initial by a server.
        ///
        /// Must be present in every Initial. Always empty for clients.
        pub(super) expected_token: Bytes,
        /// First cryptographic message
        ///
        /// Only set for clients
        pub(super) client_hello: Option<Bytes>,
    }

    #[allow(unreachable_pub)] // fuzzing only
    #[derive(Clone)]
    pub struct Closed {
        pub(super) reason: Close,
    }
}

/// Events of interest to the application
#[derive(Debug)]
pub enum Event {
    /// The connection's handshake data is ready
    HandshakeDataReady,
    /// The connection was successfully established
    Connected,
    /// The connection was lost
    ///
    /// Emitted if the peer closes the connection or an error is encountered.
    ConnectionLost {
        /// Reason that the connection was closed
        reason: ConnectionError,
    },
    /// Stream events
    Stream(StreamEvent),
    /// One or more application datagrams have been received
    DatagramReceived,
    /// One or more application datagrams have been sent after blocking
    DatagramsUnblocked,
}

fn instant_saturating_sub(x: Instant, y: Instant) -> Duration {
    if x > y { x - y } else { Duration::ZERO }
}

fn get_max_ack_delay(params: &TransportParameters) -> Duration {
    Duration::from_micros(params.max_ack_delay.0 * 1000)
}

// Prevents overflow and improves behavior in extreme circumstances
const MAX_BACKOFF_EXPONENT: u32 = 16;

/// Minimal remaining size to allow packet coalescing, excluding cryptographic tag
///
/// This must be at least as large as the header for a well-formed empty packet to be coalesced,
/// plus some space for frames. We only care about handshake headers because short header packets
/// necessarily have smaller headers, and initial packets are only ever the first packet in a
/// datagram (because we coalesce in ascending packet space order and the only reason to split a
/// packet is when packet space changes).
const MIN_PACKET_SPACE: usize = MAX_HANDSHAKE_OR_0RTT_HEADER_SIZE + 32;

/// Largest amount of space that could be occupied by a Handshake or 0-RTT packet's header
///
/// Excludes packet-type-specific fields such as packet number or Initial token
// https://www.rfc-editor.org/rfc/rfc9000.html#name-0-rtt: flags + version + dcid len + dcid +
// scid len + scid + length + pn
const MAX_HANDSHAKE_OR_0RTT_HEADER_SIZE: usize =
    1 + 4 + 1 + MAX_CID_SIZE + 1 + MAX_CID_SIZE + VarInt::from_u32(u16::MAX as u32).size() + 4;

/// Perform key updates this many packets before the AEAD confidentiality limit.
///
/// Chosen arbitrarily, intended to be large enough to prevent spurious connection loss.
const KEY_UPDATE_MARGIN: u64 = 10_000;

#[derive(Default)]
struct SentFrames {
    retransmits: ThinRetransmits,
    largest_acked: Option<u64>,
    stream_frames: StreamMetaVec,
    /// Whether the packet contains non-retransmittable frames (like datagrams)
    non_retransmits: bool,
    requires_padding: bool,
}

impl SentFrames {
    /// Returns whether the packet contains only ACKs
    fn is_ack_only(&self, streams: &StreamsState) -> bool {
        self.largest_acked.is_some()
            && !self.non_retransmits
            && self.stream_frames.is_empty()
            && self.retransmits.is_empty(streams)
    }
}

/// Compute the negotiated idle timeout based on local and remote max_idle_timeout transport parameters.
///
/// According to the definition of max_idle_timeout, a value of `0` means the timeout is disabled; see <https://www.rfc-editor.org/rfc/rfc9000#section-18.2-4.4.1.>
///
/// According to the negotiation procedure, either the minimum of the timeouts or one specified is used as the negotiated value; see <https://www.rfc-editor.org/rfc/rfc9000#section-10.1-2.>
///
/// Returns the negotiated idle timeout as a `Duration`, or `None` when both endpoints have opted out of idle timeout.
fn negotiate_max_idle_timeout(x: Option<VarInt>, y: Option<VarInt>) -> Option<Duration> {
    match (x, y) {
        (Some(VarInt(0)) | None, Some(VarInt(0)) | None) => None,
        (Some(VarInt(0)) | None, Some(y)) => Some(Duration::from_millis(y.0)),
        (Some(x), Some(VarInt(0)) | None) => Some(Duration::from_millis(x.0)),
        (Some(x), Some(y)) => Some(Duration::from_millis(cmp::min(x, y).0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negotiate_max_idle_timeout_commutative() {
        let test_params = [
            (None, None, None),
            (None, Some(VarInt(0)), None),
            (None, Some(VarInt(2)), Some(Duration::from_millis(2))),
            (Some(VarInt(0)), Some(VarInt(0)), None),
            (
                Some(VarInt(2)),
                Some(VarInt(0)),
                Some(Duration::from_millis(2)),
            ),
            (
                Some(VarInt(1)),
                Some(VarInt(4)),
                Some(Duration::from_millis(1)),
            ),
        ];

        for (left, right, result) in test_params {
            assert_eq!(negotiate_max_idle_timeout(left, right), result);
            assert_eq!(negotiate_max_idle_timeout(right, left), result);
        }
    }
}
