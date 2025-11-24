Glossary of QUIC Terms
======================

**ACKM:** ACK Manager. Responsible for tracking packets in flight and generating
notifications when they are lost, or when they are successfully delivered.

**Active Stream:** A stream which has data or control frames ready for
transmission. Active stream status is managed by the QSM.

**AEC:** Application error code. An error code provided by a local or remote
application to be signalled as an error code value by QUIC. See QUIC RFCs
(`STOP_SENDING`, `RESET_STREAM`, `CONNECTION_CLOSE`).

**APL:** API Personality Layer. The QUIC API Personality Layer lives in
`quic_impl.c` and implements the libssl API personality (`SSL_read`, etc.) in
terms of an underlying `QUIC_CHANNEL` object.

**Bidi:** Abbreviation of bidirectional, referring to a QUIC bidirectional
stream.

**CC:** Congestion controller. Estimates network channel capacity and imposes
limits on transmissions accordingly.

**CFQ:** Control Frame Queue. Considered part of the FIFM, this implements the
GCR strategy for frame in flight management. For details, see FIFM design
document.

**Channel:** See `QUIC_CHANNEL`.

**CID:** Connection ID.

**CMPL:** Coalescing Maximum Packet Length. The maximum number of bytes left to
serialize another QUIC packet into the same datagram as one or more previous
packets. This is just the MDPL minus the total size of all previous packets
already serialized into to the same datagram.

**CMPPL:** Coalescing Maximum Packet Payload Length. The maximum number of
payload bytes we can put in the payload of another QUIC packet which is to be
coalesced with one or more previous QUIC packets and placed into the same
datagram. Essentially, this is the room we have left for another packet payload.

**CSM:** Connection state machine. Refers to some aspects of a QUIC channel. Not
implemented as an explicit state machine.

**DCID:** Destination Connection ID. Found in all QUIC packet headers.

**DEMUX:** The demuxer routes incoming packets to the correct connection QRX by
DCID.

**DGRAM:** (UDP) datagram.

**DISPATCH:** Refers to the QUIC-specific dispatch code in `ssl_lib.c`. This
dispatches calls to libssl public APIs to the APL.

**EL:** Encryption level. See RFC 9000.

**Engine:** See `QUIC_ENGINE`.

**Event Leader:** The QSO which is is the top-level QSO in a hierarchy of QSOs,
and which is responsible for event processing for all QSOs in that hierarchy.
This may be a QLSO or QCSO. See [the server API
design](server/quic-server-api.md).

**FC:** Flow control. Comprises TXFC and RXFC.

**FIFD:** Frame-in-flight dispatcher. Ties together the CFQ and TXPIM to handle
frame tracking and retransmission on loss.

**FIFM:** Frame-in-flight manager. Tracks frames in flight until their
containing packets are deemed delivered or lost, so that frames can be
retransmitted if necessary. Comprises the CFQ, TXPIM and FIFD.

**GCR:** Generic Control Frame Retransmission. A strategy for regenerating lost
frames. Stores raw frame in a queue so that it can be retransmitted if lost. See
FIFM design document for details. Implemented by the CFQ.

**Key epoch:** Non-negative number designating a generation of QUIC keys used to
encrypt or decrypt packets, starting at 0. This increases by 1 when a QUIC key
update occurs.

**Key Phase:** Key phase bit in QUIC packet headers. See RFC 9000.

**Keyslot**: A set of cryptographic state used to encrypt or decrypt QUIC
packets by a QRX or QTX. Due to the QUIC key update mechanism, multiple keyslots
may be maintained at a given time. See `quic_record_rx.h` for details.

**KP:** See Key Phase.

**KS:** See Keyslot.

**KU:** Key update. See also TXKU, RXKU.

**LCID:** Local CID. Refers to a CID which will be recognised as identifying a
connection if found in the DCID field of an incoming packet. See also RCID.

**LCIDM:** Local CID Manager. Tracks LCIDs which have been advertised to a peer.
See also RCIDM.

**Locally-initiated:** Refers to a QUIC stream which was initiated by the local
application rather than the remote peer.

**MDPL:** Maximum Datagram Payload Length. The maximum number of UDP payload
bytes we can put in a UDP packet. This is derived from the applicable PMTU. This
is also the maximum size of a single QUIC packet if we place only one packet in
a datagram. The MDPL may vary based on both local source IP and destination IP
due to different path MTUs.

**MinDPL:** Minimum Datagram Payload Length. In some cases we must ensure a
datagram has a minimum size of a certain number of bytes. This does not need to
be accomplished with a single packet, but we may need to add PADDING frames to
the final packet added to a datagram in this case.

**MinPL:** Minimum Packet Length. The minimum serialized packet length we are
using while serializing a given packet. May often be 0. Used to meet MinDPL
requirements, and thus equal to MinDPL minus the length of any packets we have
already encoded into the datagram.

**MinPPL:** Minimum Packet Payload Length. The minimum number of bytes which
must be placed into a packet payload in order to meet the MinPL minimum size
when the packet is encoded.

**MPL:** Maximum Packet Length. The maximum size of a fully encrypted and
serialized QUIC packet in bytes in some given context. Typically equal to the
MDPL and never greater than it.

**MPPL:** Maximum Packet Payload Length. The maximum number of plaintext bytes
we can put in the payload of a QUIC packet. This is related to the MDPL by the
size of the encoded header and the size of any AEAD authentication tag which
will be attached to the ciphertext.

**MSMT:** Multi-stream multi-thread. Refers to a type of multi-stream QUIC usage
in which API calls can be made on different threads.

**MSST:** Multi-stream single-thread. Refers to a type of multi-stream QUIC
usage in which API calls must not be made concurrently.

**NCID:** New Connection ID. Refers to a QUIC `NEW_CONNECTION_ID` frame.

**Numbered CID:** Refers to a Connection ID which has a sequence number assigned
to it. All CIDs other than Initial ODCIDs and Retry ODCIDs have a sequence
number assigned. See also Unnumbered CID.

**ODCID:** Original Destination CID. This is the DCID found in the first Initial
packet sent by a client, and is used to generate the secrets for encrypting
Initial packets. It is only used temporarily.

**PN:** Packet number. Most QUIC packet types have a packet number (PN); see RFC
9000.

**Port:** See `QUIC_PORT`.

**Port Leader:** The QSO which is responsible for servicing a given UDP socket.
This may be a QCSO or a QLSO. See [the server API design](server/quic-server-api.md).

**PTO:** Probe timeout. See RFC 9000.

**QC:** See `QUIC_CONNECTION`.

**QCSO:** QUIC Connection SSL Object. This is an SSL object created using
`SSL_new` using a QUIC method, or by a QLSO.

**QCTX**: QUIC Context. This is a utility object defined within the QUIC APL
which helps to unwrap an SSL object pointer (a QLSO, QCSO or QSSO) into the
relevant structure pointers such as `QUIC_LISTENER`, `QUIC_CONNECTION` or
`QUIC_XSO`.

**QL:** See `QUIC_LISTENER`.

**QLSO:** QUIC Listener SSL Object. An object created to represent a socket
which can accept one or more incoming QUIC connections and/or make multiple
outgoing QUIC connections. Parent of zero or more QCSOs.

**QUIC_LISTENER:** QUIC listener. This is the APL object representing a QUIC
listener (QLSO) in the APL.

**QP:** See `QUIC_PORT`.

**QUIC_PORT:** Internal object owning the network socket BIO which services a
QLSO. This is the QUIC core object corresponding to the APL's `QUIC_LISTENER`
object (a QLSO). Owns zero more `QUIC_CHANNEL` instances.

**QRL:** QUIC record layer. Refers collectively to the QRX and QTX.

**QRX:** QUIC Record Layer RX. Receives incoming datagrams and decrypts the
packets contained in them. Manages decrypted packets in a queue pending
processing by upper layers.

**QS:** See `QUIC_STREAM`.

**QSM:** QUIC Streams Mapper. Manages internal `QUIC_STREAM` objects and maps
IDs to those objects. Allows iteration of active streams.

**QSO:** QUIC SSL Object. May be a QLSO, QCSO or QSSO.

**QSSO:** QUIC Stream SSL Object. This is an SSL object which is subsidiary to a
given QCSO, obtained using (for example) `SSL_new_stream` or
`SSL_accept_stream`.

**QTLS**, **QUIC_TLS**: Implements the QUIC handshake layer using TLS 1.3,
wrapping libssl TLS code to implement the QUIC-specific aspects of QUIC TLS.

**QTX:** QUIC Record Layer TX. Encrypts and sends packets in datagrams.

**QUIC_CHANNEL:** Internal object in the QUIC core implementation corresponding
to a QUIC connection. Ties together other components and provides connection
handling and state machine implementation. Belongs to a `QUIC_PORT` representing
a UDP socket/BIO, which in turn belongs to a `QUIC_ENGINE`. Owns some number of
`QUIC_STREAM` instances. The `QUIC_CHANNEL` code is fused tightly with the RXDP.

**QUIC_CONNECTION:** QUIC connection. This is the object representing a QUIC
connection in the APL. It internally corresponds to a `QUIC_CHANNEL` object in
the QUIC core implementation.

**QUIC_ENGINE:** Internal object in the QUIC core implementation constituting
the top-level object of a QUIC event and I/O processing domain. Owns zero or
more `QUIC_PORT` instances, each of which owns zero or more `QUIC_CHANNEL`
objects representing QUIC connections.

**QUIC_PORT:** Internal object in the QUIC core implementation corresponding to
a listening port/network BIO. Has zero or more child `QUIC_CHANNEL` objects
associated with it and belongs to a `QUIC_ENGINE`.

**QUIC_STREAM**: Internal object tracking a QUIC stream. Unlike an XSO this is
not part of the APL. An XSO wraps a QUIC_STREAM once that stream is exposed as
an API object. As such, a `QUIC_CONNECTION` is to a `QUIC_CHANNEL` what a
`QUIC_XSO` is to a `QUIC_STREAM`.

**RCID:** Remote CID. Refers to a CID which has been provided to us by a peer
and which we can place in the DCID field of an outgoing packet. See also LCID,
Unnumbered CID and Numbered CID.

**RCIDM:** Remote CID Manager. Tracks RCIDs which have been provided to us by a
peer. See also LCIDM.

**REGEN:** A strategy for regenerating lost frames. This strategy regenerates
the frame from canonical data sources without having to store a copy of the
frame which was transmitted. See FIFM design document for details.

**Remotely-initiated:** Refers to a QUIC stream which was initiated by the
remote peer, rather than by the local application.

**RIO:** Reactive I/O subsystem. Refers to the generic, non-QUIC specific parts
of the asynchronous I/O handling code which the OpenSSL QUIC stack is built on.

**RSTREAM:** Receive stream. Internal receive buffer management object used to
store data which has been RX'd but not yet read by the application.

**RTT:** Round trip time. Time for a datagram to reach a given peer and a reply
to reach the local machine, assuming the peer responds immediately.

**RXDP:** RX depacketiser. Handles frames in packets which have been decrypted
by a QRX.

**RXE:** RX entry. Structure containing decrypted received packets awaiting
processing. Stored in a queue known as the RXL. These structures belong to a
QRX.

**RXFC:** RX flow control. This determines how much a peer may send to us and
provides indication of when flow control frames increasing a peer's flow control
budget should be generated. Exists in both connection-level and stream-level
instances.

**RXKU:** RX key update. The detected condition whereby a received packet
has a flipped Key Phase bit, meaning the peer has initiated a key update.
Causes a solicited TXKU. See also TXKU.

**RXL:** RXE list. See RXE.

**RCMPPL:** Remaining CMPPL. The number of bytes left in a packet whose payload
we are currently forming. This is the CMPPL minus any bytes we have already put
into the payload.

**SCID:** Source Connection ID. Found in some QUIC packet headers.

**SRT:** Stateless reset token.

**SRTM:** Stateless reset token manager. Object which tracks SRTs we have
received.

**SSTREAM:** Send stream. Internal send buffer management object used to store
data which has been passed to libssl for sending but which has not yet been
transmitted, or not yet been acknowledged.

**STATM:** Statistics manager. Measures estimated connection RTT.

**TA:** Thread assisted mode.

**TPARAM:** Transport parameter. See RFC 9000.

**TSERVER:** Test server. Internal test server object built around a channel.

**TXE:** TX entry. Structure containing encrypted data pending transmission.
Owned by the QTX.

**TXFC:** TX flow control. This determines how much can be transmitted to the
peer. Exists in both connection-level and stream-level instances.

**TXKU:** TX key update. This refers to when a QTX signals a key update for the
TX direction by flipping the Key Phase bit in an outgoing packet. A TXKU can be
either spontaneous (locally initiated) or in solicited (in response to receiving
an RXKU). See also RXKU.

**TXL:** TXE list. See TXE.

**TXP:** TX packetiser. This is responsible for generating yet-unencrypted
packets and passing them to a QTX for encryption and transmission. It must
decide how to spend the space available in a datagram.

**TXPIM:** Transmitted Packet Information Manager. Stores information about
transmitted packets and the frames contained within them. This information
is needed to facilitate retransmission of frames if the packets they are in
are lost. Note that the ACKM records only receipt or loss of entire packets,
whereas TXPIM tracks information about individual frames in those packets.

**TX/RX v. Send/Receive:** The terms *TX*  and *RX* are used for *network-level*
communication, whereas *send* and *receive* are used for application-level
communication. An application *sends*  on a stream (causing data to be appended
to a *send stream buffer*, and that data is eventually TX'd by the TXP and QTX.)

**Uni:** Abbreviation of unidirectional, referring to a QUIC unidirectional
stream.

**Unnumbered CID:** Refers to a CID which does not have a sequence number
associated with it and therefore cannot be referred to by a `NEW_CONNECTION_ID`
or `RETIRE_CONNECTION_ID` frame's sequence number fields. The only unnumbered
CIDs are Initial ODCIDs and Retry ODCIDs. These CIDs are exceptionally retired
automatically during handshake confirmation. See also Numbered CID.

**URXE:** Unprocessed RX entry. Structure containing yet-undecrypted received
datagrams pending processing. Stored in a queue known as the URXL.
Ownership of URXEs is shared between DEMUX and QRX.

**URXL:** URXE list. See URXE.

**XSO:** External Stream Object. This is the API object representing a QUIC
stream in the APL. Internally, it is the `QUIC_XSO` structure, externally it is
a `SSL *` (and is a QSSO).
