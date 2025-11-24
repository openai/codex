RX depacketizer
===============

This component takes a QUIC packet and parses the frames contained therein,
to be forwarded to appropriate other components for further processing.

In the [overview], this is called the "RX Frame Handler".  The name "RX
depacketizer" was chosen to reflect the kinship with the [TX packetizer].

Main structures
---------------

### Connection

Represented by an `QUIC_CONNECTION` object, defined in
[`include/internal/quic_ssl.h`](../../../include/internal/quic_ssl.h).

### Stream

Represented by an `QUIC_STREAM` object (yet to be defined).

### Packets

Represented by the `OSSL_QRX_PKT` structure, defined in
`include/internal/quic_record_rx.h` in [QUIC Demuxer and Record Layer (RX+TX)].

Interactions
------------

The RX depacketizer receives a packet from the QUIC Read Record Layer, and
then processes frames in two phases:

1.  [Collect information for the ACK Manager](#collect-information-for-the-ack-manager)
2.  [Pass frame data](#pass-frame-data)

### Other components

There are a number of other components that the RX depacketizer wants to
interact with:

-   [ACK manager]
-   Handshake manager, which is currently unspecified.  It's assumed that
    this will wrap around what is called the "TLS Handshake Record Layer",
    in the [overview]
-   Session manager, which is currently unspecified for QUIC, but may very
    well be the existing `SSL_SESSION` functionality, extended to fit QUIC
    purposes.
-   Flow control, which is currently unspecified.  In the [overview], it's
    called the "Flow Controller And Statistics Collector"
-   Connection manager, which is currently unspecified.  In the [overview],
    there's a "Connection State Machine" that the "RX Frame Handler" isn't
    talking directly with, so it's possible that the Connection manager will
    turn out to be the Handshake manager.
-   Stream SSL objects, to pass the stream data to.

### Read and process a packet

Following how things are designed elsewhere, the depacketizer is assumed to
be called "from above" using the following function:

``` C
__owur int ossl_quic_depacketize(QUIC_CONNECTION *connection);
```

This function would create an `OSSL_QRX_PKT` and call the QUIC Read Record
Layer with a pointer to it, leaving it to the QUIC Read Record Layer to fill
in the data.

This uses the `ossl_qrx_read_pkt()` packet reading function from
[QUIC Demuxer and Record Layer (RX+TX)].
(the `OSSL_QRX_PKT` structure / sub-structure needs to be extended to take
an `OSSL_TIME`, possibly by reference, which should be filled in with the
packet reception time)

### Collect information for the [ACK manager]

This collects appropriate data into a `QUIC_ACKM_RX_PKT` structure:

-   The packet number (`packet->packet_number`)
-   The packet receive time (`received`)
-   The packet space, which is always:
    -   `QUIC_PN_SPACE_INITIAL` when `packet->packet_type == pkt_initial`
    -   `QUIC_PN_SPACE_HANDSHAKE` when `packet->packet_type == pkt_handshake`
    -   `QUIC_PN_SPACE_APP` for all other packet types
-   The ACK eliciting flag.  This is calculated by looping through all
    frames and noting those that are ACK eliciting, as determined from
    [Table 1](#table-1) below)

### Passing frame data

This loops through all the frames, extracts data where there is any
and calls diverse other components as shown in the Passed to column in
[Table 1](#table-1) below.

#### Table 1

Taken from [RFC 9000 12.4 Frames and Frame Types]

| Type | Name                    | Passed to                | ACK eliciting | I        | H        | 0        | 1        |
|------|-------------------------|--------------------------|---------------|----------|----------|----------|----------|
| 0x00 | [padding]               | -                        |               | &#10004; | &#10004; | &#10004; | &#10004; |
| 0x01 | [ping]                  | -                        | &#10004;      | &#10004; | &#10004; | &#10004; | &#10004; |
| 0x02 | [ack 0x02]              | [ACK manager] [^1]       |               | &#10004; | &#10004; |          | &#10004; |
| 0x03 | [ack 0x03]              | [ACK manager] [^1]       |               | &#10004; | &#10004; |          | &#10004; |
| 0x04 | [reset_stream]          | - [^2]                   | &#10004;      |          |          | &#10004; | &#10004; |
| 0x05 | [stop_sending]          | - [^3]                   | &#10004;      |          |          | &#10004; | &#10004; |
| 0x06 | [crypto]                | Handshake manager        | &#10004;      | &#10004; | &#10004; |          | &#10004; |
| 0x07 | [new_token]             | Session manager          | &#10004;      |          |          |          | &#10004; |
| 0x08 | [stream 0x08]           | Apprioriate stream [^4]  | &#10004;      |          |          | &#10004; | &#10004; |
| 0x09 | [stream 0x09]           | Apprioriate stream [^4]  | &#10004;      |          |          | &#10004; | &#10004; |
| 0x0A | [stream 0x0A]           | Apprioriate stream [^4]  | &#10004;      |          |          | &#10004; | &#10004; |
| 0x0B | [stream 0x0B]           | Apprioriate stream [^4]  | &#10004;      |          |          | &#10004; | &#10004; |
| 0x0C | [stream 0x0C]           | Apprioriate stream [^4]  | &#10004;      |          |          | &#10004; | &#10004; |
| 0x0D | [stream 0x0D]           | Apprioriate stream [^4]  | &#10004;      |          |          | &#10004; | &#10004; |
| 0x0E | [stream 0x0E]           | Apprioriate stream [^4]  | &#10004;      |          |          | &#10004; | &#10004; |
| 0x0F | [stream 0x0F]           | Apprioriate stream [^4]  | &#10004;      |          |          | &#10004; | &#10004; |
| 0x10 | [max_data]              | Flow control [^5]        | &#10004;      |          |          | &#10004; | &#10004; |
| 0x11 | [max_stream_data]       | Flow control [^5]        | &#10004;      |          |          | &#10004; | &#10004; |
| 0x12 | [max_streams 0x12]      | Connection manager? [^6] | &#10004;      |          |          | &#10004; | &#10004; |
| 0x13 | [max_streams 0x13]      | Connection manager? [^6] | &#10004;      |          |          | &#10004; | &#10004; |
| 0x14 | [data_blocked]          | Flow control [^5]        | &#10004;      |          |          | &#10004; | &#10004; |
| 0x15 | [stream_data_blocked]   | Flow control [^5]        | &#10004;      |          |          | &#10004; | &#10004; |
| 0x16 | [streams_blocked 0x16]  | Connection manager? [^6] | &#10004;      |          |          | &#10004; | &#10004; |
| 0x17 | [streams_blocked 0x17]  | Connection manager? [^6] | &#10004;      |          |          | &#10004; | &#10004; |
| 0x18 | [new_connection_id]     | Connection manager       | &#10004;      |          |          | &#10004; | &#10004; |
| 0x19 | [retire_connection_id]  | Connection manager       | &#10004;      |          |          | &#10004; | &#10004; |
| 0x1A | [path_challenge]        | Connection manager? [^7] | &#10004;      |          |          | &#10004; | &#10004; |
| 0x1B | [path_response]         | Connection manager? [^7] | &#10004;      |          |          |          | &#10004; |
| 0x1C | [connection_close 0x1C] | Connection manager       |               | &#10004; | &#10004; | &#10004; | &#10004; |
| 0x1D | [connection_close 0x1D] | Connection manager       |               |          |          | &#10004; | &#10004; |
| 0x1E | [handshake_done]        | Handshake manager        | &#10004;      |          |          |          | &#10004; |
| ???? | *[Extension Frames]*    | - [^8]                   | &#10004;      |          |          |          |          |

The I, H, 0, and 1 columns are validity in different packet types, with this meaning:

| Pkts | Description                |
|:----:|----------------------------|
| I    | Valid in Initial packets   |
| H    | Valid in Handshake packets |
| 0    | Valid in 0-RTT packets     |
| 1    | Valid in 1-RTT packets     |

Notes:

[^1]: This creates and populates an `QUIC_ACKM_ACK` structure, then calls
    `QUIC_ACKM_on_rx_ack_frame()`, with the appropriate context
    (`QUIC_ACKM`, the created `QUIC_ACKM_ACK`, `pkt_space` and `rx_time`)
[^2]: Immediately terminates the appropriate receiving stream `QUIC_STREAM`
    object.
    This includes discarding any buffered application data.
    For a stream that's send-only, the error `STREAM_STATE_ERROR` is raised,
    and the `QUIC_CONNECTION` object is terminated.
[^3]: Immediately terminates the appropriate sending stream `QUIC_STREAM`
    object.
    For a stream that's receive-only, the error `STREAM_STATE_ERROR` is
    raised, and the `QUIC_CONNECTION` object is terminated.
[^4]: The frame payload (Stream Data) is passed as is to the `QUIC_STREAM`
    object, along with available metadata (offset and length, as determined
    to be available from the lower 3 bits of the frame type).
[^5]: The details of what flow control will need are yet to be determined
[^6]: I imagine that `max_streams` and `streams_blocked` concern a Connection
    manager before anything else.
[^7]: I imagine that path challenge/response concerns a Connection manager
    before anything else.
[^8]: We have no idea what extension frames there will be.  However, we
    must at least acknowledge their presence, so much is clear from the RFC.

[overview]: https://github.com/openssl/openssl/blob/master/doc/designs/quic-design/quic-overview.md
[TX packetizer]: https://github.com/openssl/openssl/pull/18570
[SSL object refactoring using SSL_CONNECTION object]: https://github.com/openssl/openssl/pull/18612
[QUIC Demuxer and Record Layer (RX+TX)]: https://github.com/openssl/openssl/pull/18949
[ACK manager]: https://github.com/openssl/openssl/pull/18564
[RFC 9000 12.4 Frames and Frame Types]: https://datatracker.ietf.org/doc/html/rfc9000#section-12.4
[padding]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.1
[ping]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.2
[ack 0x02]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.3
[ack 0x03]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.3
[reset_stream]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.4
[stop_sending]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.5
[crypto]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.6
[new_token]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.7
[stream 0x08]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.8
[stream 0x09]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.8
[stream 0x0A]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.8
[stream 0x0B]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.8
[stream 0x0C]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.8
[stream 0x0D]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.8
[stream 0x0E]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.8
[stream 0x0F]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.8
[max_data]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.9
[max_stream_data]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.10
[max_streams 0x12]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.11
[max_streams 0x13]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.11
[data_blocked]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.12
[stream_data_blocked]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.13
[streams_blocked 0x16]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.14
[streams_blocked 0x17]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.14
[new_connection_id]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.15
[retire_connection_id]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.16
[path_challenge]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.17
[path_response]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.18
[connection_close 0x1C]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.19
[connection_close 0x1D]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.19
[handshake_done]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.20
[Extension Frames]: https://datatracker.ietf.org/doc/html/rfc9000#section-19.21
