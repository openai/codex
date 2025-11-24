Flow Control
============

Introduction to QUIC Flow Control
---------------------------------

QUIC flow control acts at both connection and stream levels. At any time,
transmission of stream data could be prevented by connection-level flow control,
by stream-level flow control, or both. Flow control uses a credit-based model in
which the relevant flow control limit is expressed as the maximum number of
bytes allowed to be sent on a stream, or across all streams, since the beginning
of the stream or connection. This limit may be periodically bumped.

It is important to note that both connection and stream-level flow control
relate only to the transmission of QUIC stream data. QUIC flow control at stream
level counts the total number of logical bytes sent on a given stream. Note that
this does not count retransmissions; thus, if a byte is sent, lost, and sent
again, this still only counts as one byte for the purposes of flow control. Note
that the total number of logical bytes sent on a given stream is equivalent to
the current “length” of the stream. In essence, the relevant quantity is
`max(offset + len)` for all STREAM frames `(offset, len)` we have ever sent for
the stream.

(It is essential that this be determined correctly, as deadlock may occur if we
believe we have exhausted our flow control credit whereas the peer believes we
have not, as the peer may wait indefinitely for us to send more data before
advancing us more flow control credit.)

QUIC flow control at connection level is based on the sum of all the logical
bytes transmitted across all streams since the start of the connection.

Connection-level flow control is controlled by the `MAX_DATA` frame;
stream-level flow control is controlled by the `MAX_STREAM_DATA` frame.

The `DATA_BLOCKED` and `STREAM_DATA_BLOCKED` frames defined by RFC 9000 are less
important than they first appear, as peers are not allowed to rely on them. (For
example, a peer is not allowed to wait until we send `DATA_BLOCKED` to increase
our connection-level credit, and a conformant QUIC implementation can choose to
never generate either of these frame types.) These frames rather serve two
purposes: to enhance flow control performance, and as a debugging aid.
However, their implementation is not critical.

Note that it follows from the above that the CRYPTO-frame stream is not subject
to flow control.

Note that flow control and congestion control are completely separate
mechanisms. In a given circumstance, either or both mechanisms may restrict our
ability to transmit application data.

Consider the following diagram:

    RWM   SWM           SWM'   CWM         CWM'
     |     |             |      |           |
     |     |<--    credit|   -->|           |
     |   <-|- threshold -|----->|           |
                          ----------------->
                                 window size

We introduce the following terminology:

- **Controlled bytes** refers to any byte which counts for purposes of flow
  control. A controlled byte is any byte of application data in a STREAM frame
  payload, the first time it is sent (retransmissions do not count).

- (RX side only) **Retirement**, which refers to where we dequeue one or more
  controlled bytes from a QUIC stream and hand them to the application, meaning
  we are no longer responsible for them.

  Retirement is an important factor in our RX flow control design, as we want
  peers to transmit not just at the rate that our QUIC implementation can
  process incoming data, but also at a rate the application can handle.

- (RX side only) The **Retired Watermark** (RWM), the total number of retired
  controlled bytes since the beginning of the connection or stream.

- The **Spent Watermark** (SWM), which is the number of controlled bytes we have
  sent (for the TX side) or received (for the RX side). This represents the
  amount of flow control budget which has been spent. It is a monotonic value
  and never decreases. On the RX side, such bytes have not necessarily been
  retired yet.

- The **Credit Watermark** (CWM), which is the number of bytes which have
  been authorized for transmission so far. This count is a cumulative count
  since the start of the connection or stream and thus is also monotonic.

- The available **credit**, which is always simply the difference between
  the SWM and the CWM.

- (RX side only) The **threshold**, which is how close we let the RWM
  get to the CWM before we choose to extend the peer more credit by bumping the
  CWM. The threshold is relative to (i.e., subtracted from) the CWM.

- (RX side only) The **window size**, which is the amount by which we or a peer
  choose to bump the CWM each time, as we reach or exceed the threshold. The new
  CWM is calculated as the SWM plus the window size (note that it added to the
  SWM, not the old CWM.)

Note that:

- If the available credit is zero, the TX side is blocked due to a lack of
  credit.

- If any circumstance occurs which would cause the SWM to exceed the CWM,
  a flow control protocol violation has occurred and the connection
  should be terminated.

Connection-Level Flow Control - TX Side
---------------------------------------

TX side flow control is exceptionally simple. It can be modelled as the
following state machine:

        ---> event: On TX (numBytes)
        ---> event: On TX Window Updated (numBytes)
        <--- event: On TX Blocked
        Get TX Window() -> numBytes

The On TX event is passed to the state machine whenever we send a packet.
`numBytes` is the total number of controlled bytes we sent in the packet (i.e.,
the number of bytes of STREAM frame payload which are not retransmissions). This
value is added to the TX-side SWM value. Note that this may be zero, though
there is no need to pass the event in this case.

The On TX Window Updated event is passed to the state machine whenever we have
our CWM increased. In other words, it is passed whenever we receive a `MAX_DATA`
frame, with the integer value contained in that frame (or when we receive the
`initial_max_data` transport parameter).

The On TX Window Updated event expresses the CWM (that is, the cumulative
number of controlled bytes we are allowed to send since the start of the
connection), thus it is monotonic and may never regress. If an On TX Window
Update event is passed to the state machine with a value lower than that passed
in any previous such event, it indicates a peer protocol error or a local
programming error.

The Get TX Window function returns our credit value (that is, it returns the
number of controlled bytes we are allowed to send). This value is reduced by the
On TX event and increased by the On TX Window Updated event. In fact, it is
simply the difference between the last On TX Window Updated value and the sum of
the `numBytes` arguments of all On TX events so far; it is that simple.

The On TX Blocked event is emitted at the time of any edge transition where the
value which would be returned by the Get TX Window function changes from
non-zero to zero. This always occurs during processing of an On TX event. (This
event is intended to assist in deciding when to generate `DATA_BLOCKED`
frames.)

We must not exceed the flow control limits, else the peer may terminate the
connection with an error.

An initial connection-level credit is communicated by the peer in the
`initial_max_data` transport parameter. All other credits occur as a result of a
`MAX_DATA` frame.

Stream-Level Flow Control - TX Side
-----------------------------------

Stream-level flow control works exactly the same as connection-level flow
control for the TX side.

The On TX Window Updated event occurs in response to the `MAX_STREAM_DATA`
frame, or based on the relevant transport parameter
(`initial_max_stream_data_bidi_local`, `initial_max_stream_data_bidi_remote`,
`initial_max_stream_data_uni`).

The On TX Blocked event can be used to decide when to generate
`STREAM_DATA_BLOCKED` frames.

Note that the number of controlled bytes we can send in a stream is limited by
both connection and stream-level flow control; thus the number of controlled
bytes we can send is the lesser value of the values returned by the Get TX
Window function on the connection-level and stream-level state machines,
respectively.

Connection-Level Flow Control - RX Side
---------------------------------------

        ---> event: On RX Controlled Bytes (numBytes)       [internal event]
        ---> event: On Retire Controlled Bytes (numBytes)
        <--- event: Increase Window (numBytes)
        <--- event: Flow Control Error

RX side connection-level flow control provides an indication of when to generate
`MAX_DATA` frames to bump the peer's connection-level transmission credit. It is
somewhat more involved than the TX side.

The state machine receives On RX Controlled Bytes events from stream-level flow
controllers. Callers do not pass the event themselves. The event is generated by
a stream-level flow controller whenever we receive any controlled bytes.
`numBytes` is the number of controlled bytes we received. (This event is
generated by stream-level flow control as retransmitted stream data must be
counted only once, and the stream-level flow control is therefore in the best
position to determine how many controlled bytes (i.e., new, non-retransmitted
stream payload bytes) have been received).

If we receive more controlled bytes than we authorized, the state machine emits
the Flow Control Error event. The connection should be terminated with a
protocol error in this case.

The state machine emits the Increase Window event when it thinks that the peer
should be advanced more flow control credit (i.e., when the CWM should be
bumped). `numBytes` is the new CWM value, and is monotonic with regard to all
previous Increase Window events emitted by the state machine.

The state machine is passed the On Retire Controlled bytes event when one or
more controlled bytes are dequeued from any stream and passed to the
application.

The state machine uses the cadence of the On Retire Controlled Bytes events it
receives to determine when to increase the flow control window. Thus, the On
Retire Controlled Bytes event should be sent to the state machine when
processing of the received controlled bytes has been *completed* (i.e., passed
to the application).

Stream-Level Flow Control - RX Side
-----------------------------------

RX-side stream-level flow control works similarly to RX-side connection-level
flow control. There are a few differences:

- There is no On RX Controlled Bytes event.

- The On Retire Controlled Bytes event may optionally pass the same event
  to a connection-level flow controller (an implementation decision), as these
  events should always occur at the same time.

- An additional event is added, which replaces the On RX Controlled Bytes event:

        ---> event: On RX Stream Frame (offsetPlusLength, isFin)

  This event should be passed to the state machine when a STREAM frame is
  received. The `offsetPlusLength` argument is the sum of the offset field of
  the STREAM frame and the length of the frame's payload in bytes. The isFin
  argument should specify whether the STREAM frame had the FIN flag set.

  This event is used to generate the internal On RX Controlled Bytes event to
  the connection-level flow controller. It is also used by stream-level flow
  control to determine if flow control limits are violated by the peer.

  The state machine handles `offsetPlusLength` monotonically and ignores the
  event if a previous such event already had an equal or greater value. The
  reason this event is used instead of a `On RX (numBytes)` style event is that
  this API can be monotonic and thus easier to use (the caller does not need to
  remember if they have already counted a specific controlled byte in a STREAM
  frame, which may after all duplicate some of the controlled bytes in a
  previous STREAM frame).

RX Window Sizing
----------------

For RX flow control we must determine our window size. This is the value we add
to the peer's current SWM to determine the new CWM each time as RWM reaches the
threshold. The window size should be adapted dynamically according to network
conditions.

Many implementations choose to have a mechanism for increasing the window size
but not decreasing it, a simple approach which we adopt here.

The common algorithm is a so-called auto-tuning approach in which the rate of
window consumption (i.e., the rate at which RWM approaches CWM after CWM is
bumped) is measured and compared to the measured connection RTT. If the time it
takes to consume one window size exceeds a fixed multiple of the RTT, the window
size is doubled, up to an implementation-chosen maximum window size.

Auto-tuning occurs in 'epochs'. At the end of each auto-tuning epoch, a decision
is made on whether to double the window size, and a new auto-tuning epoch is
started.

For more information on auto-tuning, see [Flow control in
QUIC](https://docs.google.com/document/d/1F2YfdDXKpy20WVKJueEf4abn_LVZHhMUMS5gX6Pgjl4/edit#heading=h.hcm2y5x4qmqt)
and [QUIC Flow
Control](https://docs.google.com/document/d/1SExkMmGiz8VYzV3s9E35JQlJ73vhzCekKkDi85F1qCE/edit#).
