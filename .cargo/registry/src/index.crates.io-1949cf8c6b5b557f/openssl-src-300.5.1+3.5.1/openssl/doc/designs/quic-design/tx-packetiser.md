TX Packetiser
=============

This module creates frames from the application data obtained from
the application.  It also receives CRYPTO frames from the TLS Handshake
Record Layer and ACK frames from the ACK Handling And Loss Detector
subsystem.

The packetiser also deals with the flow and congestion controllers.

Creation & Destruction
----------------------

```c
typedef struct quic_tx_packetiser_args_st {
    /* Configuration Settings */
    QUIC_CONN_ID    cur_scid;   /* Current Source Connection ID we use. */
    QUIC_CONN_ID    cur_dcid;   /* Current Destination Connection ID we use. */
    BIO_ADDR        peer;       /* Current destination L4 address we use. */
    /* ACK delay exponent used when encoding. */
    uint32_t        ack_delay_exponent;

    /* Injected Dependencies */
    OSSL_QTX        *qtx;       /* QUIC Record Layer TX we are using */
    QUIC_TXPIM      *txpim;     /* QUIC TX'd Packet Information Manager */
    QUIC_CFQ        *cfq;       /* QUIC Control Frame Queue */
    OSSL_ACKM       *ackm;      /* QUIC Acknowledgement Manager */
    QUIC_STREAM_MAP *qsm;       /* QUIC Streams Map */
    QUIC_TXFC       *conn_txfc; /* QUIC Connection-Level TX Flow Controller */
    QUIC_RXFC       *conn_rxfc; /* QUIC Connection-Level RX Flow Controller */
    const OSSL_CC_METHOD *cc_method; /* QUIC Congestion Controller */
    OSSL_CC_DATA    *cc_data;   /* QUIC Congestion Controller Instance */
    OSSL_TIME       (*now)(void *arg);  /* Callback to get current time. */
    void            *now_arg;

    /*
     * Injected dependencies - crypto streams.
     *
     * Note: There is no crypto stream for the 0-RTT EL.
     *       crypto[QUIC_PN_SPACE_APP] is the 1-RTT crypto stream.
     */
    QUIC_SSTREAM    *crypto[QUIC_PN_SPACE_NUM];
} QUIC_TX_PACKETISER_ARGS;

_owur typedef struct ossl_quic_tx_packetiser_st OSSL_QUIC_TX_PACKETISER;

OSSL_QUIC_TX_PACKETISER *ossl_quic_tx_packetiser_new(QUIC_TX_PACKETISER_ARGS *args);
void ossl_quic_tx_packetiser_free(OSSL_QUIC_TX_PACKETISER *tx);
```

Structures
----------

### Connection

Represented by an QUIC_CONNECTION object.

### Stream

Represented by an QUIC_STREAM object.

As per [RFC 9000 2.3 Stream Prioritization], streams should contain a priority
provided by the calling application.  For MVP, this is not required to be
implemented because only one stream is supported.  However, packets being
retransmitted should be preferentially sent as noted in
[RFC 9000 13.3 Retransmission of Information].

```c
void SSL_set_priority(SSL *stream, uint32_t priority);
uint32_t SSL_get_priority(SSL *stream);
```

For protocols where priority is not meaningful, the set function is a noop and
the get function returns a constant value.

Interactions
------------

The packetiser interacts with the following components, the APIs for which
can be found in their respective design documents and header files:

- SSTREAM: manages application stream data for transmission.
- QUIC_STREAM_MAP: Maps stream IDs to QUIC_STREAM objects and tracks which
  streams are active (i.e., need servicing by the TX packetiser).
- Crypto streams for each EL other than 0-RTT (each is one SSTREAM).
- CFQ: queried for generic control frames
- QTX: record layer which completed packets are written to.
- TXPIM: logs information about transmitted packets, provides information to
  FIFD.
- FIFD: notified of transmitted packets.
- ACKM: loss detector.
- Connection and stream-level TXFC and RXFC instances.
- Congestion controller (not needed for MVP).

### SSTREAM

Each application or crypto stream has a SSTREAM object for the sending part.
This manages the buffering of data written to the stream, frees that data when
the packet it was sent in was acknowledged, and can return the data for
retransmission on loss. It receives loss and acknowledgement notifications from
the FIFD without direct TX packetiser involvement.

### QUIC Stream Map

The TX packetiser queries the QUIC stream map for a list of active streams
(QUIC_STREAM), which are iterated on a rotating round robin basis. Each
QUIC_STREAM provides access to the various components, such as a QUIC_SSTREAM
instance (for streams with a send part). Streams are marked inactive when
they no longer have any need to generate frames at the present time.

### Crypto Streams

The crypto streams for each EL (other than 0-RTT, which does not have a crypto
stream) are represented by SSTREAM instances. The TX packetiser queries SSTREAM
instances provided to it as needed when generating packets.

### CFQ

Many control frames do not require special handling and are handled by the
generic CFQ mechanism. The TX packetiser queries the CFQ for any frames to be
sent and schedules them into a packet.

### QUIC Write Record Layer

Coalesced frames are passed to the QUIC record layer for encryption and sending.
To send accumulated frames as packets to the QUIC Write Record Layer:

```c
int ossl_qtx_write_pkt(OSSL_QTX *qtx, const OSSL_QTX_PKT *pkt);
```

The packetiser will attempt to maximise the number of bytes in a packet.
It will also attempt to create multiple packets to send simultaneously.

The packetiser should also implement a wait time to allow more data to
accumulate before exhausting it's supply of data.  The length of the wait
will depend on how much data is queued already and how much space remains in
the packet being filled.  Once the wait is finished, the packets will be sent
by calling:

```c
void ossl_qtx_flush_net(OSSL_QTX *qtx);
```

The write record layer is responsible for coalescing multiple QUIC packets
into datagrams.

### TXPIM, FIFD, ACK Handling and Loss Detector

ACK handling and loss detection is provided by the ACKM and FIFD. The FIFD uses
the per-packet information recorded by the TXPIM to track which frames are
contained within a packet which was lost or acknowledged, and generates
callbacks to the TX packetiser, SSTREAM instances and CFQ to allow it to
regenerate those frames as needed.

1. When a packet is sent, the packetiser informs the FIFD, which also informs
   the ACK Manager.
2. When a packet is ACKed, the FIFD notifies applicable SSTREAMs and the CFQ
   as appropriate.
3. When a packet is lost, the FIFD notifies the TX packetiser of any frames
   which were in the lost packet for which the Regenerate strategy is
   applicable.
4. Currently, no notifications to the TX packetiser are needed when packets
   are discarded (e.g. due to an EL being discarded).

### Flow Control

The packetiser interacts with connection and stream-level TXFC and RXFC
instances. It interacts with RXFC instances to know when to generate flow
control frames, and with TXFC instances to know how much stream data it is
allowed to send in a packet.

### Congestion Control

The packetiser is likely to interact with the congestion controller in the
future. Currently, congestion control is a no-op.

Packets
-------

Packet formats are defined in [RFC 9000 17.1 Packet Formats].

### Packet types

QUIC supports a number of different packets. The combination of packets of
different encryption levels as per [RFC 9000 12.2 Coalescing Packets], is done
by the record layer. Non-encrypted packets are not handled by the TX Packetiser
and callers may send them by direct calls to the record layer.

#### Initial Packet

Refer to [RFC 9000 17.2.2 Initial Packet].

#### Handshake Packet

Refer to [RFC 9000 17.2.4 Handshake Packet].

#### App Data 0-RTT Packet

Refer to [RFC 9000 17.2.3 0-RTT].

#### App Data 1-RTT Packet

Refer to [RFC 9000 17.3.1 1-RTT].

Packetisation and Processing
----------------------------

### Definitions

 - Maximum Datagram Payload Length (MDPL): The maximum number of UDP payload
   bytes we can put in a UDP packet. This is derived from the applicable PMTU.
   This is also the maximum size of a single QUIC packet if we place only one
   packet in a datagram. The MDPL may vary based on both local source IP and
   destination IP due to different path MTUs.

 - Maximum Packet Length (MPL): The maximum size of a fully encrypted
   and serialized QUIC packet in bytes in some given context. Typically
   equal to the MDPL and never greater than it.

 - Maximum Plaintext Payload Length (MPPL): The maximum number of plaintext
   bytes we can put in the payload of a QUIC packet. This is related to
   the MDPL by the size of the encoded header and the size of any AEAD
   authentication tag which will be attached to the ciphertext.

 - Coalescing MPL (CMPL): The maximum number of bytes left to serialize
   another QUIC packet into the same datagram as one or more previous
   packets. This is just the MDPL minus the total size of all previous
   packets already serialized into to the same datagram.

 - Coalescing MPPL (CMPPL): The maximum number of payload bytes we can put in
   the payload of another QUIC packet which is to be coalesced with one or
   more previous QUIC packets and placed into the same datagram. Essentially,
   this is the room we have left for another packet payload.

 - Remaining CMPPL (RCMPPL): The number of bytes left in a packet whose payload
   we are currently forming. This is the CMPPL minus any bytes we have already
   put into the payload.

 - Minimum Datagram Length (MinDPL): In some cases we must ensure a datagram
   has a minimum size of a certain number of bytes. This does not need to be
   accomplished with a single packet, but we may need to add PADDING frames
   to the final packet added to a datagram in this case.

 - Minimum Packet Length (MinPL): The minimum serialized packet length we
   are using while serializing a given packet. May often be 0. Used to meet
   MinDPL requirements, and thus equal to MinDPL minus the length of any packets
   we have already encoded into the datagram.

 - Minimum Plaintext Payload Length (MinPPL): The minimum number of bytes
   which must be placed into a packet payload in order to meet the MinPL
   minimum size when the packet is encoded.

 - Active Stream: A stream which has data or flow control frames ready for
   transmission.

### Frames

Frames are taken from [RFC 9000 12.4 Frames and Frame Types].

| Type | Name                  | I       | H       | 0       | 1       | N       | C       | P       | F       |
|------|-----------------------|---------|---------|---------|---------|---------|---------|---------|---------|
| 0x00 | padding               | &check; | &check; | &check; | &check; | &check; |         | &check; |         |
| 0x01 | ping                  | &check; | &check; | &check; | &check; |         |         |         |         |
| 0x02 | ack 0x02              | &check; | &check; |         | &check; | &check; | &check; |         |         |
| 0x03 | ack 0x03              | &check; | &check; |         | &check; | &check; | &check; |         |         |
| 0x04 | reset_stream          |         |         | &check; | &check; |         |         |         |         |
| 0x05 | stop_sending          |         |         | &check; | &check; |         |         |         |         |
| 0x06 | crypto                | &check; | &check; |         | &check; |         |         |         |         |
| 0x07 | new_token             |         |         |         | &check; |         |         |         |         |
| 0x08 | stream 0x08           |         |         | &check; | &check; |         |         |         | &check; |
| 0x09 | stream 0x09           |         |         | &check; | &check; |         |         |         | &check; |
| 0x0A | stream 0x0A           |         |         | &check; | &check; |         |         |         | &check; |
| 0x0B | stream 0x0B           |         |         | &check; | &check; |         |         |         | &check; |
| 0x0C | stream 0x0C           |         |         | &check; | &check; |         |         |         | &check; |
| 0x0D | stream 0x0D           |         |         | &check; | &check; |         |         |         | &check; |
| 0x0E | stream 0x0E           |         |         | &check; | &check; |         |         |         | &check; |
| 0x0F | stream 0x0F           |         |         | &check; | &check; |         |         |         | &check; |
| 0x10 | max_data              |         |         | &check; | &check; |         |         |         |         |
| 0x11 | max_stream_data       |         |         | &check; | &check; |         |         |         |         |
| 0x12 | max_streams 0x12      |         |         | &check; | &check; |         |         |         |         |
| 0x13 | max_streams 0x13      |         |         | &check; | &check; |         |         |         |         |
| 0x14 | data_blocked          |         |         | &check; | &check; |         |         |         |         |
| 0x15 | stream_data_blocked   |         |         | &check; | &check; |         |         |         |         |
| 0x16 | streams_blocked 0x16  |         |         | &check; | &check; |         |         |         |         |
| 0x17 | streams_blocked 0x17  |         |         | &check; | &check; |         |         |         |         |
| 0x18 | new_connection_id     |         |         | &check; | &check; |         |         | &check; |         |
| 0x19 | retire_connection_id  |         |         | &check; | &check; |         |         |         |         |
| 0x1A | path_challenge        |         |         | &check; | &check; |         |         | &check; |         |
| 0x1B | path_response         |         |         |         | &check; |         |         | &check; |         |
| 0x1C | connection_close 0x1C | &check; | &check; | &check; | &check; | &check; |         |         |         |
| 0x1D | connection_close 0x1D |         |         | &check; | &check; | &check; |         |         |         |
| 0x1E | handshake_done        |         |         |         | &check; |         |         |         |         |

The various fields are as defined in RFC 9000.

#### Pkts

_Pkts_ are defined as:

| Pkts | Description|
| :---: | --- |
| I | Valid in Initial packets|
| H | Valid in Handshake packets|
| 0 | Valid in 0-RTT packets|
| 1 | Valid in 1-RTT packets|

#### Spec

_Spec_ is defined as:

| Spec | Description |
| :---: | --- |
| N | Not ack-eliciting. |
| C | does not count toward bytes in flight for congestion control purposes. |
| P | Can be used to probe new network paths during connection migration. |
| F | The contents of frames with this marking are flow controlled. |

For `C`, `N` and `P`, the entire packet must consist of only frames with the
marking for the packet to qualify for it.  For example, a packet with an ACK
frame and a _stream_ frame would qualify for neither the `C` or `N` markings.

#### Notes

- Do we need the distinction between 0-rtt and 1-rtt when both are in
  the Application Data number space?
- 0-RTT packets can morph into 1-RTT packets and this needs to be handled by
  the packetiser.

### Frame Type Prioritisation

The frame types listed above are reordered below in the order of priority with
which we want to serialize them. We discuss the motivations for this priority
ordering below. Items without a line between them have the same priority.

```plain
HANDSHAKE_DONE          GCR / REGEN
----------------------------
MAX_DATA                      REGEN
DATA_BLOCKED                  REGEN
MAX_STREAMS                   REGEN
STREAMS_BLOCKED               REGEN
----------------------------


NEW_CONNECTION_ID             GCR
RETIRE_CONNECTION_ID          GCR
----------------------------
PATH_CHALLENGE                  -
PATH_RESPONSE                   -
----------------------------
ACK                             -     (non-ACK-eliciting)
----------------------------
CONNECTION_CLOSE              ***     (non-ACK-eliciting)
----------------------------
NEW_TOKEN                     GCR

----------------------------
CRYPTO                        GCR/*q

============================          ]  priority group, repeats per stream
RESET_STREAM                  GCR*    ]
STOP_SENDING                  GCR*    ]
----------------------------          ]
MAX_STREAM_DATA               REGEN   ]
STREAM_DATA_BLOCKED           REGEN   ]
----------------------------          ]
STREAM                        *q      ]
============================          ]

----------------------------
PING                           -
----------------------------
PADDING                        -      (non-ACK-eliciting)
```

(See [Frame in Flight Manager](quic-fifm.md) for information on the meaning of
the second column, which specifies the retransmission strategy for each frame
type.)

- `PADDING`: For obvious reasons, this frame type is the lowest priority. We only
  add `PADDING` frames at the very end after serializing all other frames if we
  have been asked to ensure a non-zero MinPL but have not yet met that minimum.

- `PING`: The `PING` frame is encoded as a single byte. It is used to make a packet
  ACK-eliciting if it would not otherwise be ACK-eliciting. Therefore we only
  need to send it if

  a. we have been asked to ensure the packet is ACK-eliciting, and
  b. we do not have any other ACK-eliciting frames in the packet.

  Thus we wait until the end before adding the PING frame as we may end up
  adding other ACK-eliciting frames and not need to add it. There is never
  a need to add more than one PING frame. If we have been asked to ensure
  the packet is ACK-eliciting and we do not know for sure up front if we will
  add any other ACK-eliciting packet, we must reserve one byte of our CMPPL
  to ensure we have room for this. We can cancel this reservation if we
  add an ACK-eliciting frame earlier. For example:

  - We have been asked to ensure a packet is ACK-eliciting and the CMPPL is
    1000 (we are coalescing with another packet).
  - We allocate 999 bytes for non-PING frames.
  - While adding non-PING frames, we add a STREAM frame, which is
    ACK-eliciting, therefore the PING frame reservation is cancelled
    and we increase our allocation for non-PING frames to 1000 bytes.

- `HANDSHAKE_DONE`: This is a single byte frame with no data which is used to
  indicate handshake completion. It is only ever sent once. As such, it can be
  implemented as a single flag, and there is no risk of it outcompeting other
  frames. It is therefore trivially given the highest priority.

- `MAX_DATA`, `DATA_BLOCKED`: These manage connection-level flow control. They
  consist of a single integer argument, and, as such, take up little space, but
  are also critical to ensuring the timely expansion of the connection-level
  flow control window. Thus there is a performance reason to include them in
  packets with high priority and due to their small size and the fact that there
  will only ever be at most one per packet, there is no risk of them
  outcompeting other frames.

- `MAX_STREAMS`, `STREAMS_BLOCKED`: Similar to the frames above for
  connection-level flow control, but controls rate at which new streams are
  opened. The same arguments apply here, so they are prioritised equally.

- `STREAM`: This is the bread and butter of a QUIC packet, and contains
  application-level stream data. As such these frames can usually be expected to
  consume most of our packet's payload budget. We must generally assume that

  - there are many streams, and
  - several of those streams have much more data waiting to be sent than
    can be sent in a single packet.

  Therefore we must ensure some level of balance between multiple competing
  streams. We refer to this as stream scheduling. There are many strategies that
  can be used for this, and in the future we might even support
  application-signalled prioritisation of specific streams. We discuss
  stream scheduling further below.

  Because these frames are expected to make up the bulk of most packets, we
  consider them low priority, higher only than `PING` and `PADDING` frames.
  Moreover, we give priority to control frames as unlike `STREAM` frames, they
  are vital to the maintenance of the health of the connection itself. Once we
  have serialized all other frame types, we can reserve the rest of the packet
  for any `STREAM` frames. Since all `STREAM` frames are ACK-eliciting, if we
  have any `STREAM` frame to send at all, it cancels any need for any `PING`
  frame, and may be able to partially or wholly obviate our need for any
  `PADDING` frames which we might otherwise have needed. Thus once we start
  serializing STREAM frames, we are limited only by the remaining CMPPL.

- `MAX_STREAM_DATA`, `STREAM_DATA_BLOCKED`: Stream-level flow control. These
  contain only a stream ID and integer value used for flow control, so they are
  not large. Since they are critical to the management and health of a specific
  stream, and because they are small and have no risk of stealing too many bytes
  from the `STREAM` frames they follow, we always serialize these before any
  corresponding `STREAM` frames for a given stream ID.

- `RESET_STREAM`, `STOP_SENDING`: These terminate a given stream ID and thus are
  also associated with a stream. They are also small. As such, we consider these
  higher priority than both `STREAM` frames and the stream-level flow control
  frames.

- `NEW_CONNECTION_ID`, `RETIRE_CONNECTION_ID`: These are critical for connection
  management and are not particularly large, therefore they are given a high
  priority.

- `PATH_CHALLENGE`, `PATH_RESPONSE`: Used during connection migration, these
  are small and are given a high priority.

- `CRYPTO`: These frames generate the logical crypto stream, which is a logical
  bidirectional bytestream used to transport TLS records for connection
  handshake and management purposes. As such, the crypto stream is viewed as
  similar to application streams but of a higher priority. We are willing to let
  `CRYPTO` frames outcompete all application stream-related frames if need be,
  as `CRYPTO` frames are more important to the maintenance of the connection and
  the handshake layer should not generate an excessive amount of data.

- `CONNECTION_CLOSE`, `NEW_TOKEN`: The `CONNECTION_CLOSE` frame can contain a
  user-specified reason string. The `NEW_TOKEN` frame contains an opaque token
  blob. Both can be arbitrarily large but for the fact that they must fit in a
  single packet and are thus ultimately limited by the MPPL. However, these
  frames are important to connection maintenance and thus are given a priority
  just above that of `CRYPTO` frames. The `CONNECTION_CLOSE` frame has higher
  priority than `NEW_TOKEN`.

- `ACK`: `ACK` frames are critical to avoid needless retransmissions by our peer.
  They can also potentially become large if a large number of ACK ranges needs
  to be transmitted. Thus `ACK` frames are given a fairly high priority;
  specifically, their priority is higher than all frames which have the
  potential to be large but below all frames which contain only limited data,
  such as connection-level flow control. However, we reserve the right to adapt
  the size of the ACK frames we transmit by chopping off some of the PN ranges
  to limit the size of the ACK frame if its size would be otherwise excessive.
  This ensures that the high priority of the ACK frame does not starve the
  packet of room for stream data.

### Stream Scheduling

**Stream budgeting.** When it is time to add STREAM frames to a packet under
construction, we take our Remaining CMPPL and call this value the Streams
Budget. There are many ways we could make use of this Streams Budget.

For the purposes of stream budgeting, we consider all bytes of STREAM frames,
stream-level flow control frames, RESET_STREAM and STOP_SENDING frames to
“belong” to their respective streams, and the encoded sizes of these frames are
accounted to those streams for budgeting purposes. If the total number of bytes
of frames necessary to serialize all pending data from all active streams is
less than our Streams Budget, there is no need for any prioritisation.
Otherwise, there are a number of strategies we could employ. We can categorise
the possible strategies into two groups to begin with:

  - **Intrapacket muxing (IRPM)**. When the data available to send across all
    streams exceeds the Streams Budget for the packet, allocate an equal
    portion of the packet to each stream.

  - **Interpacket muxing (IXPM).** When the data available to send across all
    streams exceeds the Streams Budget for the packet, try to fill the packet
    using as few streams as possible, and multiplex by using different
    streams in different packets.

Though obvious, IRPM does not appear to be a widely used strategy [1] [2],
probably due to a clear downside: if a packet is lost and it contains data for
multiple streams, all of those streams will be held up. This undermines a key
advantage of QUIC, namely the ability of streams to function independently of
one another for the purposes of head-of-line blocking. By contrast, with IXPM,
if a packet is lost, typically only a single stream is held up.

Suppose we choose IXPM. We must now choose a strategy for deciding when to
schedule streams on packets. [1] establishes that there are two basic
strategies found in use:

  - A round robin (RR) strategy in which the frame scheduler switches to
    the next active stream every n packets (where n ≥ 1).

  - A sequential (SEQ) strategy in which a stream keeps being transmitted
    until it is no longer active.

The SEQ strategy does not appear to be suitable for general-purpose
applications as it presumably starves other streams of bandwidth. It appears
that this strategy may be chosen in some implementations because it can offer
greater efficiency with HTTP/3, where there are performance benefits to
completing transmission of one stream before beginning the next. However, it
does not seem like a suitable choice for an application-agnostic QUIC
implementation. Thus the RR strategy is the better choice and the popular choice
in a survey of implementations.

The choice of `n` for the RR strategy is most trivially 1 but there are
suggestions [1] that a higher value of `n` may lead to greater performance due
to packet loss in typical networks occurring in small durations affecting small
numbers of consecutive packets. Thus, if `n` is greater than 1, fewer streams
will be affected by packet loss and held up on average. However, implementing
different values of `n` poses no non-trivial implementation concerns, so it is
not a major concern for discussion here. Such a parameter can easily be made
configurable.

Thus, we choose what active stream to select to fill in a packet on a
revolving round robin basis, moving to the next stream in the round robin
every `n` packets. If the available data in the active stream is not enough to
fill a packet, we do also move to the next stream, so IRPM can still occur in
this case.

When we fill a packet with a stream, we start with any applicable `RESET_STREAM`
or `STOP_SENDING` frames, followed by stream-level flow control frames if
needed, followed by `STREAM` frames.

(This means that `RESET_STREAM`, `STOP_SENDING`, `MAX_STREAM_DATA`,
 `STREAM_DATA_BLOCKED` and `STREAM` frames are interleaved rather than occurring
 in a fixed priority order; i.e., first there could be a `STOP_SENDING` frame
 for one stream, then a `STREAM` frame for another, then another `STOP_SENDING`
 frame for another stream, etc.)

[1] [Same Standards; Different Decisions: A Study of QUIC and HTTP/3
Implementation Diversity (Marx et al. 2020)](https://qlog.edm.uhasselt.be/epiq/files/QUICImplementationDiversity_Marx_final_11jun2020.pdf)
[2] [Resource Multiplexing and Prioritization in HTTP/2 over TCP versus HTTP/3
over QUIC (Marx et al. 2020)](https://h3.edm.uhasselt.be/files/ResourceMultiplexing_H2andH3_Marx2020.pdf)

### Packets with Special Requirements

Some packets have special requirements which the TX packetiser must meet:

- **Padded Initial Datagrams.**
  A datagram must always be padded to at least 1200 bytes if it contains an
  Initial packet. (If there are multiple packets in the datagram, the padding
  does not necessarily need to be part of the Initial packet itself.) This
  serves to confirm that the QUIC minimum MTU is met.

- **Token in Initial Packets.**
  Initial packets may need to contain a token. If used, token is contained in
  all further Initial packets sent by the client, not just the first Initial
  packet.

- **Anti-amplification Limit.** Sometimes a lower MDPL may be imposed due to
  anti-amplification limits. (Only a concern for servers, so not relevant to
  MVP.)

  Note: It has been observed that a lot of implementations are not fastidious
  about enforcing the amplification limit in terms of precise packet sizes.
  Rather, they just use it to determine if they can send another packet, but not
  to determine what size that packet must be. Implementations with 'precise'
  anti-amplification implementations appear to be rare.

- **MTU Probes.** These packets have a precisely crafted size for the purposes
  of probing a path MTU. Unlike ordinary packets, they are routinely expected to
  be lost and this loss should not be taken as a signal for congestion control
  purposes. (Not relevant for MVP.)

- **Path/Migration Probes.** These packets are sent to verify a new path
  for the purposes of connection migration.

- **ACK Manager Probes.** Packets produced because the ACK manager has
  requested a probe be sent. These MUST be made ACK-eliciting (using a PING
  frame if necessary). However, these packets need not be reserved exclusively
  for ACK Manager purposes; they SHOULD contain new data if available, and MAY
  contain old data.

We handle the need for different kinds of packet via a notion of “archetypes”.
The TX packetiser is requested to generate a datagram via the following call:

```c
/* Generate normal packets containing most frame types. */
#define TX_PACKETISER_ARCHETYPE_NORMAL      0
/* Generate ACKs only. */
#define TX_PACKETISER_ARCHETYPE_ACK_ONLY    1

int ossl_quic_tx_packetiser_generate(OSSL_QUIC_TX_PACKETISER *txp,
                                     uint32_t archetype);
```

More archetypes can be added in the future as required. The archetype limits
what frames can be placed into the packets of a datagram.

### Encryption Levels

A QUIC connection progresses through Initial, Handshake, 0-RTT and 1-RTT
encryption levels (ELs). The TX packetiser decides what EL to use to send a
packet; or rather, it would be more accurate to say that the TX packetiser
decides what ELs need a packet generating. Many resources are instantiated per
EL, and can only be managed using a packet of that EL, therefore a datagram will
frequently need to contain multiple packets to manage the resources of different
ELs. We can thus view datagram construction as a process of determining if an EL
needs to produce a packet for each EL, and concatenating the resulting packets.

The following EL-specific resources exist:

- The crypto stream, a bidirectional byte stream abstraction provided
  to the handshake layer. There is one crypto stream for each of the Initial,
  Handshake and 1-RTT ELs. (`CRYPTO` frames are prohibited in 0-RTT packets,
  which is to say the 0-RTT EL has no crypto stream of its own.)

- Packet number spaces and acknowledgements. The 0-RTT and 1-RTT ELs
  share a PN space, but Initial and Handshake ELs both have their own
  PN spaces. Thus, Initial packets can only be acknowledged using an `ACK`
  frame sent in an Initial packet, etc.

Thus, a fully generalised datagram construction methodology looks like this:

- Let E be the set of ELs which are not discarded and for which `pending(el)` is
  true, where `pending()` is a predicate function determining if the EL has data
  to send.

- Determine if we are limited by anti-amplification restrictions.
  (Not relevant for MVP since this is only needed on the server side.)

- For each EL in E, construct a packet bearing in mind the Remaining CMPPL
  and append it to the datagram.

  For the Initial EL, we attach a token if we have been given one.

  If Initial is in E, the total length of the resulting datagram must be at
  least 1200, but it is up to us to which packets of which ELs in E we add
  padding to.

- Send the datagram.

### TX Key Update

The TX packetiser decides when to tell the QRL to initiate a TX-side key update.
It decides this using information provided by the QRL.

### Restricting packet sizes

Two factors impact the size of packets that can be sent:

* The maximum datagram payload length (MDPL)
* Congestion control

The MDPL limits the size of an entire datagram, whereas congestion control
limits how much data can be in flight at any given time, which may cause a lower
limit to be imposed on a given packet.

### Stateless Reset

Refer to [RFC 9000 10.3 Stateless Reset].  It's entirely reasonable for
the state machine to send this directly and immediately if required.

[RFC 9000 2.3 Stream Prioritization]: https://datatracker.ietf.org/doc/html/rfc9000#section-2.3
[RFC 9000 4.1 Data Flow Control]: https://datatracker.ietf.org/doc/html/rfc9000#section-4.1
[RFC 9000 10.3 Stateless Reset]: https://datatracker.ietf.org/doc/html/rfc9000#section-10.3
[RFC 9000 12.2 Coalescing Packets]: https://datatracker.ietf.org/doc/html/rfc9000#section-12.2
[RFC 9000 12.4 Frames and Frame Types]: https://datatracker.ietf.org/doc/html/rfc9000#section-12.4
[RFC 9000 13.3 Retransmission of Information]: https://datatracker.ietf.org/doc/html/rfc9000#section-13.3
[RFC 9000 17.1 Packet Formats]: https://datatracker.ietf.org/doc/html/rfc9000#section-17
[RFC 9000 17.2.1 Version Negotiation Packet]: https://datatracker.ietf.org/doc/html/rfc9000#section-17.2.1
[RFC 9000 17.2.2 Initial Packet]: https://datatracker.ietf.org/doc/html/rfc9000#section-17.2.2
[RFC 9000 17.2.3 0-RTT]: https://datatracker.ietf.org/doc/html/rfc9000#section-17.2.3
[RFC 9000 17.2.4 Handshake Packet]: https://datatracker.ietf.org/doc/html/rfc9000#section-17.2.4
[RFC 9000 17.2.5 Retry Packet]: https://datatracker.ietf.org/doc/html/rfc9000#section-17.2.5
[RFC 9000 17.3.1 1-RTT]: https://datatracker.ietf.org/doc/html/rfc9000#section-17.3.1
[RFC 9002]: https://datatracker.ietf.org/doc/html/rfc9002
