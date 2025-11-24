Stream Receive Buffers
======================

This is a QUIC specific module that retains the received stream data
until the application reads it with SSL_read() or any future stream read
calls.

Receive Buffers requirements for MVP
------------------------------------

These are the requirements that were identified for MVP:

- As packets with stream frames are received in arbitrary frames the
  received data must be stored until all the data with earlier offsets
  are received.
- As packets can be received before application calls SSL_read() to read
  the data the data must be stored.
- The application should be able to set the limit on how much data should
  be stored. The flow controller should be used to limit the peer to not send
  more data. Without the flow control limit a rogue peer could trigger
  a DoS via unlimited flow of incoming stream data frames.
- After the data is passed via SSL_read() to the application the stored
  data can be released and flow control limit can be raised.
- As the peer can recreate stream data frames when resending them, the
  implementation must be able to handle properly frames with partially
  or fully overlapping data with previously received frames.

Optional Receive Buffers requirements
-------------------------------------

These are optional features of the stream receive buffers implementation.
They are not required for MVP but they are otherwise desirable:

- To support a single copy operation with a future stream read call
  the received data should not be copied out of the decrypted packets to
  store the data. The only information actually stored would be a list
  of offset, length, and pointers to data, along with a pointer to the
  decrypted QUIC packet that stores the actual frame.

Proposed new public API calls
-----------------------------

```C
int SSL_set_max_stored_stream_data(SSL *stream, size_t length);
```

This function adjusts the current data flow control limit on the `stream`
to allow storing `length` bytes of quic stream data before it is read by
the application.

OpenSSL handles sending MAX_STREAM_DATA frames appropriately when the
application reads the stored data.

```C
int SSL_set_max_unprocessed_packet_data(SSL *connection,
                                        size_t length);
```

This sets the limit on unprocessed quic packet data `length` in bytes that
is allowed to be allocated for the `connection`.
See the [Other considerations](#other-considerations) section below.

Interfaces to other QUIC implementation modules
-----------------------------------------------

### Front End I/O API

SSL_read() copies data out of the stored buffers if available and
eventually triggers release of stored unprocessed packet(s).

SSL_peek(), SSL_pending(), SSL_has_pending() peek into the stored
buffers for any information about the stored data.

### RX Depacketizer

The Receive Buffers module obtains the stream data via the ssl_queue_data()
callback.

The module uses ossl_qrx_pkt_wrap_up_ref() and ossl_qrx_pkt_wrap_release()
functions to keep and release decrypted packets with unprocessed data.

### Flow Control

The Receive Buffers module provides an appropriate value for the Flow
Control module to send MAX_DATA and MAX_STREAM_DATA frames. Details
TBD.

### QUIC Read Record Layer

The Receive Buffers module needs to know whether it should stop holding
the decrypted quic packets and start copying the stream data due to
the limit reached. See the `SSL_set_max_unprocessed_quic_packet_data()`
function above and the [Other considerations](#other-considerations) section
below. Details TBD.

Implementation details
----------------------

The QUIC_RSTREAM object holds the received stream data in the SFRAME_LIST
structure. This is a sorted list of partially (never fully) overlapping
data frames. Each list item holds a pointer to the received packet
wrapper for refcounting and proper release of the received packet
data once the stream data is read by the application.

Each SFRAME_LIST item has range.start and range.end values greater
than the range.start and range.end values of the previous item in the list.
This invariant is ensured on the insertion of overlapping stream frames.
Any redundant frames are released. Insertion at the end of the list
is optimised as in the ideal situation when no packets are lost we
always just append new frames.

See `include/internal/quic_stream.h` and `include/internal/quic_sf_list.h`
for internal API details.

Other considerations
--------------------

The peer is allowed to recreate the stream data frames. As we aim for
a single-copy operation a rogue peer could use this to override the stored
data limits by sending duplicate frames with only slight changes in the
offset. For example: 1st frame - offset 0 length 1000, 2nd frame -
offset 1 length 1000, 3rd frame - offset 2 length 1000, and so on. We
would have to keep the packet data for all these frames which would
effectively raise the stream data flow control limit quadratically.

And this is not the only way how a rogue peer could make us occupy much
more data than what is allowed by the stream data flow control limit
in the single-copy scenario.

Although intuitively the MAX_DATA flow control limit might be used to
somehow limit the allocated packet buffer size, it is defined as sum
of allowed data to be sent across all the streams in the connection instead.
The packet buffer will contain much more data than just the stream frames
especially with a rogue peer, that means MAX_DATA limit cannot be used
to limit the memory occupied by packet buffers.

To resolve this problem, we fall back to copying the data off the
decrypted packet buffer once we reach a limit on unprocessed decrypted
packets. We might also consider falling back to copying the data in case
we receive stream data frames that are partially overlapping and one frame
not being a subrange of the other.

Because in MVP only a single bidirectional stream to receive
any data will be supported, the MAX_DATA flow control limit should be equal
to MAX_STREAM_DATA limit for that stream.
