use std::{fmt, sync::Arc};
#[cfg(feature = "qlog")]
use std::{io, sync::Mutex, time::Instant};

#[cfg(feature = "qlog")]
use qlog::streamer::QlogStreamer;

#[cfg(feature = "qlog")]
use crate::QlogStream;
use crate::{
    Duration, INITIAL_MTU, MAX_UDP_PAYLOAD, VarInt, VarIntBoundsExceeded, congestion,
    connection::qlog::QlogSink,
};

/// Parameters governing the core QUIC state machine
///
/// Default values should be suitable for most internet applications. Applications protocols which
/// forbid remotely-initiated streams should set `max_concurrent_bidi_streams` and
/// `max_concurrent_uni_streams` to zero.
///
/// In some cases, performance or resource requirements can be improved by tuning these values to
/// suit a particular application and/or network connection. In particular, data window sizes can be
/// tuned for a particular expected round trip time, link capacity, and memory availability. Tuning
/// for higher bandwidths and latencies increases worst-case memory consumption, but does not impair
/// performance at lower bandwidths and latencies. The default configuration is tuned for a 100Mbps
/// link with a 100ms round trip time.
pub struct TransportConfig {
    pub(crate) max_concurrent_bidi_streams: VarInt,
    pub(crate) max_concurrent_uni_streams: VarInt,
    pub(crate) max_idle_timeout: Option<VarInt>,
    pub(crate) stream_receive_window: VarInt,
    pub(crate) receive_window: VarInt,
    pub(crate) send_window: u64,
    pub(crate) send_fairness: bool,

    pub(crate) packet_threshold: u32,
    pub(crate) time_threshold: f32,
    pub(crate) initial_rtt: Duration,
    pub(crate) initial_mtu: u16,
    pub(crate) min_mtu: u16,
    pub(crate) mtu_discovery_config: Option<MtuDiscoveryConfig>,
    pub(crate) pad_to_mtu: bool,
    pub(crate) ack_frequency_config: Option<AckFrequencyConfig>,

    pub(crate) persistent_congestion_threshold: u32,
    pub(crate) keep_alive_interval: Option<Duration>,
    pub(crate) crypto_buffer_size: usize,
    pub(crate) allow_spin: bool,
    pub(crate) datagram_receive_buffer_size: Option<usize>,
    pub(crate) datagram_send_buffer_size: usize,
    #[cfg(test)]
    pub(crate) deterministic_packet_numbers: bool,

    pub(crate) congestion_controller_factory: Arc<dyn congestion::ControllerFactory + Send + Sync>,

    pub(crate) enable_segmentation_offload: bool,

    pub(crate) qlog_sink: QlogSink,
}

impl TransportConfig {
    /// Maximum number of incoming bidirectional streams that may be open concurrently
    ///
    /// Must be nonzero for the peer to open any bidirectional streams.
    ///
    /// Worst-case memory use is directly proportional to `max_concurrent_bidi_streams *
    /// stream_receive_window`, with an upper bound proportional to `receive_window`.
    pub fn max_concurrent_bidi_streams(&mut self, value: VarInt) -> &mut Self {
        self.max_concurrent_bidi_streams = value;
        self
    }

    /// Variant of `max_concurrent_bidi_streams` affecting unidirectional streams
    pub fn max_concurrent_uni_streams(&mut self, value: VarInt) -> &mut Self {
        self.max_concurrent_uni_streams = value;
        self
    }

    /// Maximum duration of inactivity to accept before timing out the connection.
    ///
    /// The true idle timeout is the minimum of this and the peer's own max idle timeout. `None`
    /// represents an infinite timeout. Defaults to 30 seconds.
    ///
    /// **WARNING**: If a peer or its network path malfunctions or acts maliciously, an infinite
    /// idle timeout can result in permanently hung futures!
    ///
    /// ```
    /// # use std::{convert::TryInto, time::Duration};
    /// # use quinn_proto::{TransportConfig, VarInt, VarIntBoundsExceeded};
    /// # fn main() -> Result<(), VarIntBoundsExceeded> {
    /// let mut config = TransportConfig::default();
    ///
    /// // Set the idle timeout as `VarInt`-encoded milliseconds
    /// config.max_idle_timeout(Some(VarInt::from_u32(10_000).into()));
    ///
    /// // Set the idle timeout as a `Duration`
    /// config.max_idle_timeout(Some(Duration::from_secs(10).try_into()?));
    /// # Ok(())
    /// # }
    /// ```
    pub fn max_idle_timeout(&mut self, value: Option<IdleTimeout>) -> &mut Self {
        self.max_idle_timeout = value.map(|t| t.0);
        self
    }

    /// Maximum number of bytes the peer may transmit without acknowledgement on any one stream
    /// before becoming blocked.
    ///
    /// This should be set to at least the expected connection latency multiplied by the maximum
    /// desired throughput. Setting this smaller than `receive_window` helps ensure that a single
    /// stream doesn't monopolize receive buffers, which may otherwise occur if the application
    /// chooses not to read from a large stream for a time while still requiring data on other
    /// streams.
    pub fn stream_receive_window(&mut self, value: VarInt) -> &mut Self {
        self.stream_receive_window = value;
        self
    }

    /// Maximum number of bytes the peer may transmit across all streams of a connection before
    /// becoming blocked.
    ///
    /// This should be set to at least the expected connection latency multiplied by the maximum
    /// desired throughput. Larger values can be useful to allow maximum throughput within a
    /// stream while another is blocked.
    pub fn receive_window(&mut self, value: VarInt) -> &mut Self {
        self.receive_window = value;
        self
    }

    /// Maximum number of bytes to transmit to a peer without acknowledgment
    ///
    /// Provides an upper bound on memory when communicating with peers that issue large amounts of
    /// flow control credit. Endpoints that wish to handle large numbers of connections robustly
    /// should take care to set this low enough to guarantee memory exhaustion does not occur if
    /// every connection uses the entire window.
    pub fn send_window(&mut self, value: u64) -> &mut Self {
        self.send_window = value;
        self
    }

    /// Whether to implement fair queuing for send streams having the same priority.
    ///
    /// When enabled, connections schedule data from outgoing streams having the same priority in a
    /// round-robin fashion. When disabled, streams are scheduled in the order they are written to.
    ///
    /// Note that this only affects streams with the same priority. Higher priority streams always
    /// take precedence over lower priority streams.
    ///
    /// Disabling fairness can reduce fragmentation and protocol overhead for workloads that use
    /// many small streams.
    pub fn send_fairness(&mut self, value: bool) -> &mut Self {
        self.send_fairness = value;
        self
    }

    /// Maximum reordering in packet number space before FACK style loss detection considers a
    /// packet lost. Should not be less than 3, per RFC5681.
    pub fn packet_threshold(&mut self, value: u32) -> &mut Self {
        self.packet_threshold = value;
        self
    }

    /// Maximum reordering in time space before time based loss detection considers a packet lost,
    /// as a factor of RTT
    pub fn time_threshold(&mut self, value: f32) -> &mut Self {
        self.time_threshold = value;
        self
    }

    /// The RTT used before an RTT sample is taken
    pub fn initial_rtt(&mut self, value: Duration) -> &mut Self {
        self.initial_rtt = value;
        self
    }

    /// The initial value to be used as the maximum UDP payload size before running MTU discovery
    /// (see [`TransportConfig::mtu_discovery_config`]).
    ///
    /// Must be at least 1200, which is the default, and known to be safe for typical internet
    /// applications. Larger values are more efficient, but increase the risk of packet loss due to
    /// exceeding the network path's IP MTU. If the provided value is higher than what the network
    /// path actually supports, packet loss will eventually trigger black hole detection and bring
    /// it down to [`TransportConfig::min_mtu`].
    pub fn initial_mtu(&mut self, value: u16) -> &mut Self {
        self.initial_mtu = value.max(INITIAL_MTU);
        self
    }

    pub(crate) fn get_initial_mtu(&self) -> u16 {
        self.initial_mtu.max(self.min_mtu)
    }

    /// The maximum UDP payload size guaranteed to be supported by the network.
    ///
    /// Must be at least 1200, which is the default, and lower than or equal to
    /// [`TransportConfig::initial_mtu`].
    ///
    /// Real-world MTUs can vary according to ISP, VPN, and properties of intermediate network links
    /// outside of either endpoint's control. Extreme care should be used when raising this value
    /// outside of private networks where these factors are fully controlled. If the provided value
    /// is higher than what the network path actually supports, the result will be unpredictable and
    /// catastrophic packet loss, without a possibility of repair. Prefer
    /// [`TransportConfig::initial_mtu`] together with
    /// [`TransportConfig::mtu_discovery_config`] to set a maximum UDP payload size that robustly
    /// adapts to the network.
    pub fn min_mtu(&mut self, value: u16) -> &mut Self {
        self.min_mtu = value.max(INITIAL_MTU);
        self
    }

    /// Specifies the MTU discovery config (see [`MtuDiscoveryConfig`] for details).
    ///
    /// Enabled by default.
    pub fn mtu_discovery_config(&mut self, value: Option<MtuDiscoveryConfig>) -> &mut Self {
        self.mtu_discovery_config = value;
        self
    }

    /// Pad UDP datagrams carrying application data to current maximum UDP payload size
    ///
    /// Disabled by default. UDP datagrams containing loss probes are exempt from padding.
    ///
    /// Enabling this helps mitigate traffic analysis by network observers, but it increases
    /// bandwidth usage. Without this mitigation precise plain text size of application datagrams as
    /// well as the total size of stream write bursts can be inferred by observers under certain
    /// conditions. This analysis requires either an uncongested connection or application datagrams
    /// too large to be coalesced.
    pub fn pad_to_mtu(&mut self, value: bool) -> &mut Self {
        self.pad_to_mtu = value;
        self
    }

    /// Specifies the ACK frequency config (see [`AckFrequencyConfig`] for details)
    ///
    /// The provided configuration will be ignored if the peer does not support the acknowledgement
    /// frequency QUIC extension.
    ///
    /// Defaults to `None`, which disables controlling the peer's acknowledgement frequency. Even
    /// if set to `None`, the local side still supports the acknowledgement frequency QUIC
    /// extension and may use it in other ways.
    pub fn ack_frequency_config(&mut self, value: Option<AckFrequencyConfig>) -> &mut Self {
        self.ack_frequency_config = value;
        self
    }

    /// Number of consecutive PTOs after which network is considered to be experiencing persistent congestion.
    pub fn persistent_congestion_threshold(&mut self, value: u32) -> &mut Self {
        self.persistent_congestion_threshold = value;
        self
    }

    /// Period of inactivity before sending a keep-alive packet
    ///
    /// Keep-alive packets prevent an inactive but otherwise healthy connection from timing out.
    ///
    /// `None` to disable, which is the default. Only one side of any given connection needs keep-alive
    /// enabled for the connection to be preserved. Must be set lower than the idle_timeout of both
    /// peers to be effective.
    pub fn keep_alive_interval(&mut self, value: Option<Duration>) -> &mut Self {
        self.keep_alive_interval = value;
        self
    }

    /// Maximum quantity of out-of-order crypto layer data to buffer
    pub fn crypto_buffer_size(&mut self, value: usize) -> &mut Self {
        self.crypto_buffer_size = value;
        self
    }

    /// Whether the implementation is permitted to set the spin bit on this connection
    ///
    /// This allows passive observers to easily judge the round trip time of a connection, which can
    /// be useful for network administration but sacrifices a small amount of privacy.
    pub fn allow_spin(&mut self, value: bool) -> &mut Self {
        self.allow_spin = value;
        self
    }

    /// Maximum number of incoming application datagram bytes to buffer, or None to disable
    /// incoming datagrams
    ///
    /// The peer is forbidden to send single datagrams larger than this size. If the aggregate size
    /// of all datagrams that have been received from the peer but not consumed by the application
    /// exceeds this value, old datagrams are dropped until it is no longer exceeded.
    pub fn datagram_receive_buffer_size(&mut self, value: Option<usize>) -> &mut Self {
        self.datagram_receive_buffer_size = value;
        self
    }

    /// Maximum number of outgoing application datagram bytes to buffer
    ///
    /// While datagrams are sent ASAP, it is possible for an application to generate data faster
    /// than the link, or even the underlying hardware, can transmit them. This limits the amount of
    /// memory that may be consumed in that case. When the send buffer is full and a new datagram is
    /// sent, older datagrams are dropped until sufficient space is available.
    pub fn datagram_send_buffer_size(&mut self, value: usize) -> &mut Self {
        self.datagram_send_buffer_size = value;
        self
    }

    /// Whether to force every packet number to be used
    ///
    /// By default, packet numbers are occasionally skipped to ensure peers aren't ACKing packets
    /// before they see them.
    #[cfg(test)]
    pub(crate) fn deterministic_packet_numbers(&mut self, enabled: bool) -> &mut Self {
        self.deterministic_packet_numbers = enabled;
        self
    }

    /// How to construct new `congestion::Controller`s
    ///
    /// Typically the refcounted configuration of a `congestion::Controller`,
    /// e.g. a `congestion::NewRenoConfig`.
    ///
    /// # Example
    /// ```
    /// # use quinn_proto::*; use std::sync::Arc;
    /// let mut config = TransportConfig::default();
    /// config.congestion_controller_factory(Arc::new(congestion::NewRenoConfig::default()));
    /// ```
    pub fn congestion_controller_factory(
        &mut self,
        factory: Arc<dyn congestion::ControllerFactory + Send + Sync + 'static>,
    ) -> &mut Self {
        self.congestion_controller_factory = factory;
        self
    }

    /// Whether to use "Generic Segmentation Offload" to accelerate transmits, when supported by the
    /// environment
    ///
    /// Defaults to `true`.
    ///
    /// GSO dramatically reduces CPU consumption when sending large numbers of packets with the same
    /// headers, such as when transmitting bulk data on a connection. However, it is not supported
    /// by all network interface drivers or packet inspection tools. `quinn-udp` will attempt to
    /// disable GSO automatically when unavailable, but this can lead to spurious packet loss at
    /// startup, temporarily degrading performance.
    pub fn enable_segmentation_offload(&mut self, enabled: bool) -> &mut Self {
        self.enable_segmentation_offload = enabled;
        self
    }

    /// qlog capture configuration to use for a particular connection
    #[cfg(feature = "qlog")]
    pub fn qlog_stream(&mut self, stream: Option<QlogStream>) -> &mut Self {
        self.qlog_sink = stream.into();
        self
    }
}

impl Default for TransportConfig {
    fn default() -> Self {
        const EXPECTED_RTT: u32 = 100; // ms
        const MAX_STREAM_BANDWIDTH: u32 = 12500 * 1000; // bytes/s
        // Window size needed to avoid pipeline
        // stalls
        const STREAM_RWND: u32 = MAX_STREAM_BANDWIDTH / 1000 * EXPECTED_RTT;

        Self {
            max_concurrent_bidi_streams: 100u32.into(),
            max_concurrent_uni_streams: 100u32.into(),
            // 30 second default recommended by RFC 9308 § 3.2
            max_idle_timeout: Some(VarInt(30_000)),
            stream_receive_window: STREAM_RWND.into(),
            receive_window: VarInt::MAX,
            send_window: (8 * STREAM_RWND).into(),
            send_fairness: true,

            packet_threshold: 3,
            time_threshold: 9.0 / 8.0,
            initial_rtt: Duration::from_millis(333), // per spec, intentionally distinct from EXPECTED_RTT
            initial_mtu: INITIAL_MTU,
            min_mtu: INITIAL_MTU,
            mtu_discovery_config: Some(MtuDiscoveryConfig::default()),
            pad_to_mtu: false,
            ack_frequency_config: None,

            persistent_congestion_threshold: 3,
            keep_alive_interval: None,
            crypto_buffer_size: 16 * 1024,
            allow_spin: true,
            datagram_receive_buffer_size: Some(STREAM_RWND as usize),
            datagram_send_buffer_size: 1024 * 1024,
            #[cfg(test)]
            deterministic_packet_numbers: false,

            congestion_controller_factory: Arc::new(congestion::CubicConfig::default()),

            enable_segmentation_offload: true,

            qlog_sink: QlogSink::default(),
        }
    }
}

impl fmt::Debug for TransportConfig {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            max_concurrent_bidi_streams,
            max_concurrent_uni_streams,
            max_idle_timeout,
            stream_receive_window,
            receive_window,
            send_window,
            send_fairness,
            packet_threshold,
            time_threshold,
            initial_rtt,
            initial_mtu,
            min_mtu,
            mtu_discovery_config,
            pad_to_mtu,
            ack_frequency_config,
            persistent_congestion_threshold,
            keep_alive_interval,
            crypto_buffer_size,
            allow_spin,
            datagram_receive_buffer_size,
            datagram_send_buffer_size,
            #[cfg(test)]
                deterministic_packet_numbers: _,
            congestion_controller_factory: _,
            enable_segmentation_offload,
            qlog_sink,
        } = self;
        let mut s = fmt.debug_struct("TransportConfig");

        s.field("max_concurrent_bidi_streams", max_concurrent_bidi_streams)
            .field("max_concurrent_uni_streams", max_concurrent_uni_streams)
            .field("max_idle_timeout", max_idle_timeout)
            .field("stream_receive_window", stream_receive_window)
            .field("receive_window", receive_window)
            .field("send_window", send_window)
            .field("send_fairness", send_fairness)
            .field("packet_threshold", packet_threshold)
            .field("time_threshold", time_threshold)
            .field("initial_rtt", initial_rtt)
            .field("initial_mtu", initial_mtu)
            .field("min_mtu", min_mtu)
            .field("mtu_discovery_config", mtu_discovery_config)
            .field("pad_to_mtu", pad_to_mtu)
            .field("ack_frequency_config", ack_frequency_config)
            .field(
                "persistent_congestion_threshold",
                persistent_congestion_threshold,
            )
            .field("keep_alive_interval", keep_alive_interval)
            .field("crypto_buffer_size", crypto_buffer_size)
            .field("allow_spin", allow_spin)
            .field("datagram_receive_buffer_size", datagram_receive_buffer_size)
            .field("datagram_send_buffer_size", datagram_send_buffer_size)
            // congestion_controller_factory not debug
            .field("enable_segmentation_offload", enable_segmentation_offload);
        if cfg!(feature = "qlog") {
            s.field("qlog_stream", &qlog_sink.is_enabled());
        }

        s.finish_non_exhaustive()
    }
}

/// Parameters for controlling the peer's acknowledgement frequency
///
/// The parameters provided in this config will be sent to the peer at the beginning of the
/// connection, so it can take them into account when sending acknowledgements (see each parameter's
/// description for details on how it influences acknowledgement frequency).
///
/// Quinn's implementation follows the fourth draft of the
/// [QUIC Acknowledgement Frequency extension](https://datatracker.ietf.org/doc/html/draft-ietf-quic-ack-frequency-04).
/// The defaults produce behavior slightly different than the behavior without this extension,
/// because they change the way reordered packets are handled (see
/// [`AckFrequencyConfig::reordering_threshold`] for details).
#[derive(Clone, Debug)]
pub struct AckFrequencyConfig {
    pub(crate) ack_eliciting_threshold: VarInt,
    pub(crate) max_ack_delay: Option<Duration>,
    pub(crate) reordering_threshold: VarInt,
}

impl AckFrequencyConfig {
    /// The ack-eliciting threshold we will request the peer to use
    ///
    /// This threshold represents the number of ack-eliciting packets an endpoint may receive
    /// without immediately sending an ACK.
    ///
    /// The remote peer should send at least one ACK frame when more than this number of
    /// ack-eliciting packets have been received. A value of 0 results in a receiver immediately
    /// acknowledging every ack-eliciting packet.
    ///
    /// Defaults to 1, which sends ACK frames for every other ack-eliciting packet.
    pub fn ack_eliciting_threshold(&mut self, value: VarInt) -> &mut Self {
        self.ack_eliciting_threshold = value;
        self
    }

    /// The `max_ack_delay` we will request the peer to use
    ///
    /// This parameter represents the maximum amount of time that an endpoint waits before sending
    /// an ACK when the ack-eliciting threshold hasn't been reached.
    ///
    /// The effective `max_ack_delay` will be clamped to be at least the peer's `min_ack_delay`
    /// transport parameter, and at most the greater of the current path RTT or 25ms.
    ///
    /// Defaults to `None`, in which case the peer's original `max_ack_delay` will be used, as
    /// obtained from its transport parameters.
    pub fn max_ack_delay(&mut self, value: Option<Duration>) -> &mut Self {
        self.max_ack_delay = value;
        self
    }

    /// The reordering threshold we will request the peer to use
    ///
    /// This threshold represents the amount of out-of-order packets that will trigger an endpoint
    /// to send an ACK, without waiting for `ack_eliciting_threshold` to be exceeded or for
    /// `max_ack_delay` to be elapsed.
    ///
    /// A value of 0 indicates out-of-order packets do not elicit an immediate ACK. A value of 1
    /// immediately acknowledges any packets that are received out of order (this is also the
    /// behavior when the extension is disabled).
    ///
    /// It is recommended to set this value to [`TransportConfig::packet_threshold`] minus one.
    /// Since the default value for [`TransportConfig::packet_threshold`] is 3, this value defaults
    /// to 2.
    pub fn reordering_threshold(&mut self, value: VarInt) -> &mut Self {
        self.reordering_threshold = value;
        self
    }
}

impl Default for AckFrequencyConfig {
    fn default() -> Self {
        Self {
            ack_eliciting_threshold: VarInt(1),
            max_ack_delay: None,
            reordering_threshold: VarInt(2),
        }
    }
}

/// Configuration for qlog trace logging
#[cfg(feature = "qlog")]
pub struct QlogConfig {
    writer: Option<Box<dyn io::Write + Send + Sync>>,
    title: Option<String>,
    description: Option<String>,
    start_time: Instant,
}

#[cfg(feature = "qlog")]
impl QlogConfig {
    /// Where to write a qlog `TraceSeq`
    pub fn writer(&mut self, writer: Box<dyn io::Write + Send + Sync>) -> &mut Self {
        self.writer = Some(writer);
        self
    }

    /// Title to record in the qlog capture
    pub fn title(&mut self, title: Option<String>) -> &mut Self {
        self.title = title;
        self
    }

    /// Description to record in the qlog capture
    pub fn description(&mut self, description: Option<String>) -> &mut Self {
        self.description = description;
        self
    }

    /// Epoch qlog event times are recorded relative to
    pub fn start_time(&mut self, start_time: Instant) -> &mut Self {
        self.start_time = start_time;
        self
    }

    /// Construct the [`QlogStream`] described by this configuration
    pub fn into_stream(self) -> Option<QlogStream> {
        use tracing::warn;

        let writer = self.writer?;
        let trace = qlog::TraceSeq::new(
            qlog::VantagePoint {
                name: None,
                ty: qlog::VantagePointType::Unknown,
                flow: None,
            },
            self.title.clone(),
            self.description.clone(),
            Some(qlog::Configuration {
                time_offset: Some(0.0),
                original_uris: None,
            }),
            None,
        );

        let mut streamer = QlogStreamer::new(
            qlog::QLOG_VERSION.into(),
            self.title,
            self.description,
            None,
            self.start_time,
            trace,
            qlog::events::EventImportance::Core,
            writer,
        );

        match streamer.start_log() {
            Ok(()) => Some(QlogStream(Arc::new(Mutex::new(streamer)))),
            Err(e) => {
                warn!("could not initialize endpoint qlog streamer: {e}");
                None
            }
        }
    }
}

#[cfg(feature = "qlog")]
impl Default for QlogConfig {
    fn default() -> Self {
        Self {
            writer: None,
            title: None,
            description: None,
            start_time: Instant::now(),
        }
    }
}

/// Parameters governing MTU discovery.
///
/// # The why of MTU discovery
///
/// By design, QUIC ensures during the handshake that the network path between the client and the
/// server is able to transmit unfragmented UDP packets with a body of 1200 bytes. In other words,
/// once the connection is established, we know that the network path's maximum transmission unit
/// (MTU) is of at least 1200 bytes (plus IP and UDP headers). Because of this, a QUIC endpoint can
/// split outgoing data in packets of 1200 bytes, with confidence that the network will be able to
/// deliver them (if the endpoint were to send bigger packets, they could prove too big and end up
/// being dropped).
///
/// There is, however, a significant overhead associated to sending a packet. If the same
/// information can be sent in fewer packets, that results in higher throughput. The amount of
/// packets that need to be sent is inversely proportional to the MTU: the higher the MTU, the
/// bigger the packets that can be sent, and the fewer packets that are needed to transmit a given
/// amount of bytes.
///
/// Most networks have an MTU higher than 1200. Through MTU discovery, endpoints can detect the
/// path's MTU and, if it turns out to be higher, start sending bigger packets.
///
/// # MTU discovery internals
///
/// Quinn implements MTU discovery through DPLPMTUD (Datagram Packetization Layer Path MTU
/// Discovery), described in [section 14.3 of RFC
/// 9000](https://www.rfc-editor.org/rfc/rfc9000.html#section-14.3). This method consists of sending
/// QUIC packets padded to a particular size (called PMTU probes), and waiting to see if the remote
/// peer responds with an ACK. If an ACK is received, that means the probe arrived at the remote
/// peer, which in turn means that the network path's MTU is of at least the packet's size. If the
/// probe is lost, it is sent another 2 times before concluding that the MTU is lower than the
/// packet's size.
///
/// MTU discovery runs on a schedule (e.g. every 600 seconds) specified through
/// [`MtuDiscoveryConfig::interval`]. The first run happens right after the handshake, and
/// subsequent discoveries are scheduled to run when the interval has elapsed, starting from the
/// last time when MTU discovery completed.
///
/// Since the search space for MTUs is quite big (the smallest possible MTU is 1200, and the highest
/// is 65527), Quinn performs a binary search to keep the number of probes as low as possible. The
/// lower bound of the search is equal to [`TransportConfig::initial_mtu`] in the
/// initial MTU discovery run, and is equal to the currently discovered MTU in subsequent runs. The
/// upper bound is determined by the minimum of [`MtuDiscoveryConfig::upper_bound`] and the
/// `max_udp_payload_size` transport parameter received from the peer during the handshake.
///
/// # Black hole detection
///
/// If, at some point, the network path no longer accepts packets of the detected size, packet loss
/// will eventually trigger black hole detection and reset the detected MTU to 1200. In that case,
/// MTU discovery will be triggered after [`MtuDiscoveryConfig::black_hole_cooldown`] (ignoring the
/// timer that was set based on [`MtuDiscoveryConfig::interval`]).
///
/// # Interaction between peers
///
/// There is no guarantee that the MTU on the path between A and B is the same as the MTU of the
/// path between B and A. Therefore, each peer in the connection needs to run MTU discovery
/// independently in order to discover the path's MTU.
#[derive(Clone, Debug)]
pub struct MtuDiscoveryConfig {
    pub(crate) interval: Duration,
    pub(crate) upper_bound: u16,
    pub(crate) minimum_change: u16,
    pub(crate) black_hole_cooldown: Duration,
}

impl MtuDiscoveryConfig {
    /// Specifies the time to wait after completing MTU discovery before starting a new MTU
    /// discovery run.
    ///
    /// Defaults to 600 seconds, as recommended by [RFC
    /// 8899](https://www.rfc-editor.org/rfc/rfc8899).
    pub fn interval(&mut self, value: Duration) -> &mut Self {
        self.interval = value;
        self
    }

    /// Specifies the upper bound to the max UDP payload size that MTU discovery will search for.
    ///
    /// Defaults to 1452, to stay within Ethernet's MTU when using IPv4 and IPv6. The highest
    /// allowed value is 65527, which corresponds to the maximum permitted UDP payload on IPv6.
    ///
    /// It is safe to use an arbitrarily high upper bound, regardless of the network path's MTU. The
    /// only drawback is that MTU discovery might take more time to finish.
    pub fn upper_bound(&mut self, value: u16) -> &mut Self {
        self.upper_bound = value.min(MAX_UDP_PAYLOAD);
        self
    }

    /// Specifies the amount of time that MTU discovery should wait after a black hole was detected
    /// before running again. Defaults to one minute.
    ///
    /// Black hole detection can be spuriously triggered in case of congestion, so it makes sense to
    /// try MTU discovery again after a short period of time.
    pub fn black_hole_cooldown(&mut self, value: Duration) -> &mut Self {
        self.black_hole_cooldown = value;
        self
    }

    /// Specifies the minimum MTU change to stop the MTU discovery phase.
    /// Defaults to 20.
    pub fn minimum_change(&mut self, value: u16) -> &mut Self {
        self.minimum_change = value;
        self
    }
}

impl Default for MtuDiscoveryConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(600),
            upper_bound: 1452,
            black_hole_cooldown: Duration::from_secs(60),
            minimum_change: 20,
        }
    }
}

/// Maximum duration of inactivity to accept before timing out the connection
///
/// This wraps an underlying [`VarInt`], representing the duration in milliseconds. Values can be
/// constructed by converting directly from `VarInt`, or using `TryFrom<Duration>`.
///
/// ```
/// # use std::{convert::TryFrom, time::Duration};
/// # use quinn_proto::{IdleTimeout, VarIntBoundsExceeded, VarInt};
/// # fn main() -> Result<(), VarIntBoundsExceeded> {
/// // A `VarInt`-encoded value in milliseconds
/// let timeout = IdleTimeout::from(VarInt::from_u32(10_000));
///
/// // Try to convert a `Duration` into a `VarInt`-encoded timeout
/// let timeout = IdleTimeout::try_from(Duration::from_secs(10))?;
/// # Ok(())
/// # }
/// ```
#[derive(Default, Copy, Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct IdleTimeout(VarInt);

impl From<VarInt> for IdleTimeout {
    fn from(inner: VarInt) -> Self {
        Self(inner)
    }
}

impl std::convert::TryFrom<Duration> for IdleTimeout {
    type Error = VarIntBoundsExceeded;

    fn try_from(timeout: Duration) -> Result<Self, Self::Error> {
        let inner = VarInt::try_from(timeout.as_millis())?;
        Ok(Self(inner))
    }
}

impl fmt::Debug for IdleTimeout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
