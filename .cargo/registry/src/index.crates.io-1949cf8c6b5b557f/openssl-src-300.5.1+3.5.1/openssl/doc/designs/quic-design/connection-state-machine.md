QUIC Connection State Machine
=============================

FSM Model
---------

QUIC client-side connection state can be broken down into five coarse phases of
a QUIC connection:

- The Idle substate (which is simply the state before we have started trying to
  establish a connection);
- The Active state, which comprises two substates:
    - The Establishing state, which comprises many different substates;
    - The Open state;
- The Terminating state, which comprises several substates;
- The Terminated state, which is the terminal state.

There is monotonic progression through these phases.

These names have been deliberately chosen to use different terminology to common
QUIC terms such as 'handshake' to avoid confusion, as they are not the same
concepts. For example, the Establishing state uses Initial, Handshake and 1-RTT
packets.

This discussion is (currently) given from the client side perspective only.
State machine considerations only relevant to servers are not mentioned.
0-RTT is also not currently modelled in this analysis.

The synthesis of this FSM is not suggested by the QUIC RFCs but has been
discerned from the requirements imposed. This does not mean that the
implementation of this FSM as literally presented below is an optimal or
advisable implementation strategy, and a cursory examination of existing QUIC
implementations suggests that such an approach is not common. Moreover, excess
attention should not be given to the Open state, as 1-RTT application
communication can occur even still in the Establishing state (for example, when
the handshake has been completed but not yet confirmed).

However, the state machine described herein is helpful as an aid to
understanding and broadly captures the logic which our implementation will
embody. The design of the actual implementation is discussed further below.

The above states and their substates are defined as follows:

- The Establishing state involves the use of Initial and Handshake
  packets. It is terminated when the handshake is confirmed.

  Handshake confirmation is not the same as handshake completion.
  Handshake confirmation occurs on the client when it receives
  a `HANDSHAKE_DONE` frame (which occurs in a 1-RTT packet, thus
  1-RTT packets are also invoked in the Establishing state).
  On the server, handshake confirmation occurs as soon as
  the handshake is considered completed (see RFC 9001 s. 4.1).

  The Establishing state is subdivided into the following substates:

   - Proactive Version Negotiation (optional): The client sends
     a Version Negotiation packet with a reserved version number
     to forcibly elicit a list of the server's supported versions.
     This is not expected to be commonly used, as it adds a round trip.

     If it is used, the time spent in this state is based on waiting for
     the server to respond, and potentially retransmitting after a
     timeout.

   - Pre-Initial: The client has completed proactive version negotiation
     (if it performed it), but has not yet sent any encrypted packet. This
     substate is included for exposition; no time will generally be spent in it
     and there is immediate transmission of the first encrypted packet and
     transition to Initial Exchange A.

   - Initial Exchange A: The client has sent at least one Initial
     packet to the server attempting to initiate a connection.

     The client is waiting for a server response, which might
     be:
       - a Version Negotiation packet (leading to the Reactive Version
                                       Negotiation state);
       - a Retry packet     (leading to Initial Exchange B); or
       - an Initial packet  (leading to the Initial Exchange Confirmed state).

   - Reactive Version Negotiation: The server has rejected the client's
     proposed version. If proactive version negotiation was used, this
     can be considered an error. Otherwise, we return to the Pre-Initial
     state and proceed as though proactive version negotiation was
     performed using the information in the version negotiation packet.

   - Initial Exchange B: The client has been asked to perform a Retry.
     It sends at least one Initial packet to the server attempting to
     initiate a connection. Every Initial packet contains the quoted Retry
     Token. Any data sent in `CRYPTO` frames in Initial Exchange A must be
     retransmitted, but PNs MUST NOT be reset. Note that this is still
     considered part of the same connection, and QUIC Transport Parameters are
     later used to cryptographically bind the established connection state to
     the original DCIDs used as part of the Retry process. A server is not
     allowed to respond to a Retry-triggered Initial exchange with another
     Retry, and if it does we ignore it, which is the major distinction of this
     state from Initial Exchange A.

     The client is waiting for a server response, which might be:
       - a Version Negotiation packet (invalid, ignored);
       - a Retry packet               (invalid, ignored);
       - an Initial packet    (leading to the Initial Exchange Continued
                               state);

   - Initial Exchange Continued: The client has sent at least one
     Initial packet to the server and received at least one valid Initial packet
     from the server. There is no longer any possibility of a Retry (any such
     packet is ignored) and communications may continue via Initial packets for
     an arbitrarily long period until the handshake layer indicates the
     Handshake EL is ready.

     The client is waiting for server packets, until one of those packets
     causes the handshake layer (whether it is TLS 1.3 or some other
     hypothetical handshake layer) to emit keys for the Handshake EL.
     This will generally occur due to incoming Initial packets containing
     crypto stream segments (in the form of `CRYPTO` frames) which deliver
     handshake layer protocol messages to the handshake layer in use.

   - Handshake: The Handshake EL is now available to the client.
     Either client or server may send the first Handshake packet.

     The client is waiting to receive a Handshake packet from the server.

   - Handshake Continued: The client has received and successfully
     decrypted at least one Handshake packet. The client now discards
     the Initial EL. Communications via the handshake EL may continue for
     an arbitrary period of time.

     The client is waiting to receive more Handshake packets from the
     server to advance the handshake layer and cause it to transition
     to the Handshake Completed state.

   - Handshake Completed: The handshake layer has indicated that it
     considers the handshake completed. For TLS 1.3, this means both
     parties have sent and received (and verified) TLS 1.3 Finished
     messages. The handshake layer must emit keys for the 1-RTT EL
     at this time.

     Though the handshake is not yet confirmed, the client can begin
     sending 1-RTT packets.

     The QUIC Transport Parameters sent by the peer are now authenticated.
     (Though the peer's QUIC Transport Parameters may have been received
      earlier in the handshake process, they are only considered
      authenticated at this point.)

     The client transitions to Handshake Confirmed once either
       - it receives a `HANDSHAKE_DONE` frame in a 1-RTT packet, or
       - it receives acknowledgement of any 1-RTT packet it sent.

     Though this discussion only covers the client state machine, it is worth
     noting that on the server, the handshake is considered confirmed as soon as
     it is considered completed.

   - Handshake Confirmed: The client has received confirmation from
     the server that the handshake is confirmed.

     The principal effect of moving to this state is that the Handshake
     EL is discarded. Key Update is also now permitted for the first
     time.

     The Establishing state is now done and there is immediate transition
     to the Open state.

- The Open state is the steady state of the connection. It is a single state.

  Application stream data is exchanged freely. Only 1-RTT packets are used. The
  Initial, Handshake (and 0-RTT) ELs have been discarded, transport parameters
  have been exchanged, and the handshake has been confirmed.

  The client transitions to

   - the Terminating — Closing state if the local application initiates an
     immediate close (a `CONNECTION_CLOSE` frame is sent);
   - the Terminating — Draining state if the remote peer initiates
     an immediate close (i.e., a `CONNECTION_CLOSE` frame is received);
   - the Terminated state if the idle timeout expires; a `CONNECTION_CLOSE`
     frame is NOT sent;
   - the Terminated state if the peer triggers a stateless reset; a
     `CONNECTION_CLOSE` frame is NOT sent.

- The Terminating state is used when closing the connection.
  This may occur due to an application request or a transport-level
  protocol error.

  Key updates may not be initiated in the Terminating state.

  This state is divided into two substates:

   - The Closing state, used for a locally initiated immediate close. In
     this state, a packet containing a `CONNECTION_CLOSE` frame is
     transmitted again in response to any packets received. This ensures
     that a `CONNECTION_CLOSE` frame is received by the peer even if the
     initially transmitted `CONNECTION_CLOSE` frame was lost. Note that
     these `CONNECTION_CLOSE` frames are not governed by QUIC's normal loss
     detection mechanisms; this is a bespoke mechanism unique to this
     state, which exists solely to ensure delivery of the `CONNECTION_CLOSE`
     frame.

     The endpoint progresses to the Terminated state after a timeout
     interval, which should not be less than three times the PTO interval.

     It is also possible for the endpoint to transition to the Draining
     state instead, if it receives a `CONNECTION_CLOSE` frame prior
     to the timeout expiring. This indicates that the peer is also
     closing.

   - The Draining state, used for a peer initiated immediate close.

     The local endpoint may not send any packets of any kind in this
     state. It may optionally send one `CONNECTION_CLOSE` frame immediately
     prior to entering this state.

     The endpoint progresses to the Terminated state after a timeout
     interval, which should not be less than three times the PTO interval.

- The Terminated state is the terminal state of a connection.
  Regardless of how a connection ends (local or peer-initiated immediate close,
  idle timeout, stateless reset), a connection always ultimately ends up in this
  state. There is no longer any requirement to send or receive any packet. No
  timer events related to the connection will ever need fire again. This is a
  totally quiescent state. The state associated with the connection may now be
  safely freed.

We express this state machine in more concrete form in the form of a table,
which makes the available transitions clear:

† Except where superseded by a more specific transition

ε means “where no other transition is applicable”.

Where an action is specified in the Transition/Action column but no new state,
no state change occurs.

<table>
<tr><th>State</th><th>Action On Entry/Exit</th><th>Event</th><th>Transition/Action</th></tr>
<tr>
  <td rowspan="2"><tt>IDLE</tt></td>
  <td rowspan="2"></td>
  <td>—<tt>APP:CONNECT</tt>→</td>
  <td><tt>ACTIVE.ESTABLISHING.PROACTIVE_VER_NEG</tt> (if used), else
  <tt>ACTIVE.ESTABLISHING.PRE_INITIAL</tt></td>
</tr>
<tr>
  <td>—<tt>APP:CLOSE</tt>→</td>
  <td><tt>TERMINATED</tt></td>
</tr>
<tr>
  <td rowspan="5"><tt>ACTIVE</tt></td>
  <td rowspan="5"></td>
  <td>—<tt>IDLE_TIMEOUT</tt>→</td>
  <td><tt>TERMINATED</tt></td>
</tr>
<tr>
  <td>—<tt>PROBE_TIMEOUT</tt>→ †</td>
  <td><tt>SendProbeIfAnySentPktsUnacked()</tt></td>
</tr>
<tr>
  <td>—<tt>APP:CLOSE</tt>→ †</td>
  <td><tt>TERMINATING.CLOSING</tt></td>
</tr>
<tr>
  <td>—<tt>RX:ANY[CONNECTION_CLOSE]</tt>→</td>
  <td><tt>TERMINATING.DRAINING</tt></td>
</tr>
<tr>
  <td>—<tt>RX:STATELESS_RESET</tt>→</td>
  <td><tt>TERMINATED</tt></td>
</tr>

<tr>
  <td rowspan="3"><tt>ACTIVE.ESTABLISHING.PROACTIVE_VER_NEG</tt></td>
  <td rowspan="3"><tt>enter:SendReqVerNeg</tt></td>
  <td>—<tt>RX:VER_NEG</tt>→</td>
  <td><tt>ACTIVE.ESTABLISHING.PRE_INITIAL</tt></td>
</tr>
<tr>
  <td>—<tt>PROBE_TIMEOUT</tt>→</td>
  <td><tt>ACTIVE.ESTABLISHING.PROACTIVE_VER_NEG</tt> (retransmit)</td>
</tr>
<tr>
  <td>—<tt>APP:CLOSE</tt>→</td>
  <td><tt>TERMINATED</tt></td>
</tr>
<tr>
  <td rowspan="1"><tt>ACTIVE.ESTABLISHING.PRE_INITIAL</tt></td>
  <td rowspan="1"></td>
  <td>—ε→</td>
  <td><tt>ACTIVE.ESTABLISHING.INITIAL_EXCHANGE_A</tt></td>
</tr>
<tr>
  <td rowspan="4"><tt>ACTIVE.ESTABLISHING.INITIAL_EXCHANGE_A</tt></td>
  <td rowspan="4"><tt>enter:SendPackets()</tt> (First Initial)</td>
  <td>—<tt>RX:RETRY</tt>→</td>
  <td><tt>ACTIVE.ESTABLISHING.INITIAL_EXCHANGE_B</tt></td>
</tr>
<tr>
  <td>—<tt>RX:INITIAL</tt>→</td>
  <td><tt>ACTIVE.ESTABLISHING.INITIAL_EXCHANGE_CONTINUED</tt></td>
</tr>
<tr>
  <td>—<tt>RX:VER_NEG</tt>→</td>
  <td><tt>ACTIVE.ESTABLISHING.REACTIVE_VER_NEG</tt></td>
</tr>
<tr>
  <td>—<tt>CAN_SEND</tt>→</td>
  <td><tt>SendPackets()</tt></td>
</tr>
<tr>
  <td rowspan="1"><tt>ACTIVE.ESTABLISHING.REACTIVE_VER_NEG</tt></td>
  <td rowspan="1"></td>
  <td>—ε→</td>
  <td><tt>ACTIVE.ESTABLISHING.PRE_INITIAL</tt></td>
</tr>
<tr>
  <td rowspan="3"><tt>ACTIVE.ESTABLISHING.INITIAL_EXCHANGE_B</tt></td>
  <td rowspan="3"><tt>enter:SendPackets()</tt><br/>
    (First Initial, with token)<br/>
    (*All further Initial packets contain the token)<br/>(*PN is not reset)</td>
  <td>—<tt>RX:INITIAL</tt>→</td>
  <td><tt>ACTIVE.ESTABLISHING.INITIAL_EXCHANGE_CONTINUED</tt></td>
</tr>
<tr>
  <td>—<tt>PROBE_TIMEOUT</tt>→</td>
  <td>TODO: Tail loss probe for initial packets?</td>
</tr>
<tr>
  <td>—<tt>CAN_SEND</tt>→</td>
  <td><tt>SendPackets()</tt></td>
</tr>
<tr>
  <td rowspan="2"><tt>ACTIVE.ESTABLISHING.INITIAL_EXCHANGE_CONTINUED</tt></td>
  <td rowspan="2"><tt>enter:SendPackets()</tt></td>
  <td>—<tt>RX:INITIAL</tt>→</td>
  <td>(packet processed, no change)</td>
</tr>
<tr>
  <td>—<tt>TLS:HAVE_EL(HANDSHAKE)</tt>→</td>
  <td><tt>ACTIVE.ESTABLISHING.HANDSHAKE</tt></td>
</tr>
<tr>
  <td rowspan="3"><tt>ACTIVE.ESTABLISHING.HANDSHAKE</tt></td>
  <td rowspan="3"><tt>enter:ProvisionEL(Handshake)</tt><br/>
  <tt>enter:SendPackets()</tt> (First Handshake packet, if pending)</td>
  <td>—<tt>RX:HANDSHAKE</tt>→</td>
  <td><tt>ACTIVE.ESTABLISHING.HANDSHAKE_CONTINUED</tt></td>
</tr>
<tr>
  <td>—<tt>RX:INITIAL</tt>→</td>
  <td>(packet processed if EL is not dropped)</td>
</tr>
<tr>
  <td>—<tt>CAN_SEND</tt>→</td>
  <td><tt>SendPackets()</tt></td>
</tr>
<tr>
  <td rowspan="3"><tt>ACTIVE.ESTABLISHING.HANDSHAKE_CONTINUED</tt></td>
  <td rowspan="3"><tt>enter:DropEL(Initial)</tt><br/><tt>enter:SendPackets()</tt></td>
  <td>—<tt>RX:HANDSHAKE</tt>→</td>
  <td>(packet processed, no change)</td>
</tr>
<tr>
  <td>—<tt>TLS:HANDSHAKE_COMPLETE</tt>→</td>
  <td><tt>ACTIVE.ESTABLISHING.HANDSHAKE_COMPLETE</tt></td>
</tr>
 <tr>
  <td>—<tt>CAN_SEND</tt>→</td>
  <td><tt>SendPackets()</tt></td>
</tr>
<tr>
  <td rowspan="3"><tt>ACTIVE.ESTABLISHING.HANDSHAKE_COMPLETED</tt></td>
  <td rowspan="3"><tt>enter:ProvisionEL(1RTT)</tt><br/><tt>enter:HandshakeComplete()</tt><br/><tt>enter[server]:Send(HANDSHAKE_DONE)</tt><br/><tt>enter:SendPackets()</tt></td>
  <td>—<tt>RX:1RTT[HANDSHAKE_DONE]</tt>→</td>
  <td><tt>ACTIVE.ESTABLISHING.HANDSHAKE_CONFIRMED</tt></td>
</tr>
<tr>
  <td>—<tt>RX:1RTT</tt>→</td>
  <td>(packet processed, no change)</td>
</tr>
 <tr>
  <td>—<tt>CAN_SEND</tt>→</td>
  <td><tt>SendPackets()</tt></td>
</tr>
<tr>
  <td rowspan="1"><tt>ACTIVE.ESTABLISHING.HANDSHAKE_CONFIRMED</tt></td>
  <td rowspan="1"><tt>enter:DiscardEL(Handshake)</tt><br/><tt>enter:Permit1RTTKeyUpdate()</tt></td>
  <td>—ε→</td>
  <td><tt>ACTIVE.OPEN</tt></td>
</tr>
<tr>
  <td rowspan="2"><tt>ACTIVE.OPEN</tt></td>
  <td rowspan="2"></td>
  <td>—<tt>RX:1RTT</tt>→</td>
  <td>(packet processed, no change)</td>
</tr>
<tr>
  <td>—<tt>CAN_SEND</tt>→</td>
  <td><tt>SendPackets()</tt></td>
</tr>
<tr>
  <td rowspan="2"><tt>TERMINATING</tt></td>
  <td rowspan="2"></td>
  <td>—<tt>TERMINATING_TIMEOUT</tt>→</td>
  <td><tt>TERMINATED</tt></td>
</tr>
<tr>
  <td>—<tt>RX:STATELESS_RESET</tt>→</td>
  <td><tt>TERMINATED</tt></td>
</tr>
<tr>
  <td rowspan="3"><tt>TERMINATING.CLOSING</tt></td>
  <td rowspan="3"><tt>enter:QueueConnectionCloseFrame()</tt><br/><tt>enter:SendPackets()</tt></td>
  <td>—<tt>RX:ANY[CONNECTION_CLOSE]</tt>→</td>
  <td><tt>TERMINATING.DRAINING</tt></td>
</tr>
<tr>
  <td>—<tt>RX:ANY</tt>→</td>
  <td><tt>QueueConnectionCloseFrame()</tt><br/><tt>SendPackets()</tt></td>
</tr>
<tr>
  <td>—<tt>CAN_SEND</tt>→</td>
  <td><tt>SendPackets()</tt></td>
</tr>
<tr>
  <td rowspan="1"><tt>TERMINATING.DRAINING</tt></td>
  <td rowspan="1"></td>
  <td></td>
  <td></td>
</tr>
<tr>
  <td rowspan="1"><tt>TERMINATED</tt></td>
  <td rowspan="1"></td>
  <td>[terminal state]</td>
  <td></td>
</tr>
</table>

Notes on various events:

- `CAN_SEND` is raised when transmission of packets has been unblocked after previously
  having been blocked. There are broadly two reasons why transmission of packets
  may not have been possible:

  - Due to OS buffers or network-side write BIOs being full;
  - Due to limits imposed by the chosen congestion controller.

  `CAN_SEND` is expected to be raised due to a timeout prescribed by the
  congestion controller or in response to poll(2) or similar notifications, as
  abstracted by the BIO system and how the application has chosen to notify
  libssl of network I/O readiness.

  It is generally implied that processing of a packet as mentioned above
  may cause new packets to be queued and sent, so this is not listed
  explicitly in the Transition column except for the `CAN_SEND` event.

- `PROBE_TIMEOUT` is raised after the PTO interval and stimulates generation
  of a tail loss probe.

- `IDLE_TIMEOUT` is raised after the connection idle timeout expires.
  Note that the loss detector only makes a determination of loss due to an
  incoming ACK frame; if a peer becomes totally unresponsive, this is the only
  mechanism available to terminate the connection (other than the local
  application choosing to close it).

- `RX:STATELESS_RESET` indicates receipt of a stateless reset, but note
  that it is not guaranteed that we are able to recognise a stateless reset
  that we receive, thus this event may not always be raised.

- `RX:ANY[CONNECTION_CLOSE]` denotes a `CONNECTION_CLOSE` frame received
  in any non-discarded EL.

- Any circumstance where `RX:RETRY` or `RX:VER_NEG` are not explicitly
  listed means that these packets are not allowed and will be ignored.

- Protocol errors, etc. can be handled identically to `APP:CLOSE` events
  as indicated in the above table if locally initiated. Protocol errors
  signalled by the peer are handled as `RX:ANY[CONNECTION_CLOSE]` events.

Notes on various actions:

- `SendPackets()` sends packets if we have anything pending for transmission,
  and only to the extent we are able to with regards to congestion control and
  available BIO buffer space, etc.

Non-FSM Model
-------------

Common QUIC implementations appear to prefer modelling connection state as a set
of flags rather than as a FSM. It can be observed above that there is a fair
degree of commonality between many states. This has been modelled above using
hierarchical states with default handlers for common events. [The state machine
can be viewed as a diagram here (large
image).](./images/connection-state-machine.png)

We transpose the above table to sort by events rather than states, to discern
the following list of events:

- `APP:CONNECT`: Supported in `IDLE` state only.

- `RX:VER_NEG`: Handled in `ESTABLISHING.PROACTIVE_VER_NEG` and
  `ESTABLISHING.INITIAL_EXCHANGE_A` only, otherwise ignored.

- `RX:RETRY`: Handled in `ESTABLISHING.INITIAL_EXCHANGE_A` only.

- `PROBE_TIMEOUT`: Applicable to `OPEN` and all (non-ε) `ESTABLISHING`
  substates. Handled via `SendProbeIfAnySentPktsUnacked()` except in the
  `ESTABLISHING.PROACTIVE_VER_NEG` state, which reenters that state to trigger
  retransmission of a Version Negotiation packet.

- `IDLE_TIMEOUT`: Applicable to `OPEN` and all (non-ε) `ESTABLISHING` substates.
  Action: immediate transition to `TERMINATED` (no `CONNECTION_CLOSE` frame
  is sent).

- `TERMINATING_TIMEOUT`: Timeout used by the `TERMINATING` state only.

- `CAN_SEND`: Applicable to `OPEN` and all (non-ε) `ESTABLISHING`
  substates, as well as `TERMINATING.CLOSING`.
  Action: `SendPackets()`.

- `RX:STATELESS_RESET`: Applicable to all `ESTABLISHING` and `OPEN` states and
  the `TERMINATING.CLOSING` substate.
  Always causes a direct transition to `TERMINATED`.

- `APP:CLOSE`: Supported in `IDLE`, `ESTABLISHING` and `OPEN` states.
  (Reasonably a no-op in `TERMINATING` or `TERMINATED.`)

- `RX:ANY[CONNECTION_CLOSE]`: Supported in all `ESTABLISHING` and `OPEN` states,
  as well as in `TERMINATING.CLOSING`. Transition to `TERMINATING.DRAINING`.

- `RX:INITIAL`, `RX:HANDSHAKE`, `RX:1RTT`: Our willingness to process these is
  modelled on whether we have an EL provisioned or discarded, etc.; thus
  this does not require modelling as additional state.

  Once we successfully decrypt a Handshake packet, we stop processing Initial
  packets and discard the Initial EL, as required by RFC.

- `TLS:HAVE_EL(HANDSHAKE)`: Emitted by the handshake layer when Handshake EL
  keys are available.

- `TLS:HANDSHAKE_COMPLETE`: Emitted by the handshake layer when the handshake
  is complete. Implies connection has been authenticated. Also implies 1-RTT EL
  keys are available. Whether the handshake is complete, and also whether it is
  confirmed, is reasonably implemented as a flag.

From here we can discern state dependence of different events:

  - `APP:CONNECT`: Need to know if application has invoked this event yet,
    as if so it is invalid.

    State: Boolean: Connection initiated?

  - `RX:VER_NEG`: Only valid if we have not yet received any successfully
    processed encrypted packet from the server.

  - `RX:RETRY`: Only valid if we have sent an Initial packet to the server,
    have not yet received any successfully processed encrypted packet
    from the server, and have not previously been asked to do a Retry as
    part of this connection (and the Retry Integrity Token validates).

    Action: Note that we are now acting on a retry and start again.
    Do not reset packet numbers. The original CIDs used for the first
    connection attempt must be noted for later authentication in
    the QUIC Transport Parameters.

    State: Boolean: Retry requested?

    State: CID: Original SCID, DCID.

  - `PROBE_TIMEOUT`: If we have sent at least one encrypted packet yet,
    we can handle this via a standard probe-sending mechanism. Otherwise, we are
    still in Proactive Version Negotiation and should retransmit the Version
    Negotiation packet we sent.

    State: Boolean: Doing proactive version negotiation?

  - `IDLE_TIMEOUT`: Only applicable in `ACTIVE` states.

    We are `ACTIVE` if a connection has been initiated (see `APP:CONNECT`) and
    we are not in `TERMINATING` or `TERMINATED`.

  - `TERMINATING_TIMEOUT`: Timer used in `TERMINATING` state only.

  - `CAN_SEND`: Stimulates transmission of packets.

  - `RX:STATELESS_RESET`: Always handled unless we are in `TERMINATED`.

  - `APP:CLOSE`: Usually causes a transition to `TERMINATING.CLOSING`.

  - `RX:INITIAL`, `RX:HANDSHAKE`, `RX:1RTT`: Willingness to process
    these is implicit in whether we currently have the applicable EL
    provisioned.

  - `TLS:HAVE_EL(HANDSHAKE)`: Handled by the handshake layer
    and forwarded to the record layer to provision keys.

  - `TLS:HANDSHAKE_COMPLETE`: Should be noted as a flag and notification
    provided to various components.

We choose to model the CSM's state as follows:

  - The `IDLE`, `ACTIVE`, `TERMINATING.CLOSED`, `TERMINATING.DRAINED` and
    `TERMINATED` states are modelled explicitly as a state variable. However,
    the substates of `ACTIVE` are not explicitly modelled.

  - The following flags are modelled:
    - Retry Requested? (+ Original SCID, DCID if so)
    - Have Sent Any Packet?
    - Are we currently doing proactive version negotiation?
    - Have Successfully Received Any Encrypted Packet?
    - Handshake Completed?
    - Handshake Confirmed?

  - The following timers are modelled:
    - PTO Timeout
    - Terminating Timeout
    - Idle Timeout

Implementation Plan
-------------------

- Phase 1: “Steady state only” model which jumps to the `ACTIVE.OPEN`
  state with a hardcoded key.

  Test plan: Currently uncertain, to be determined.

- Phase 2: “Dummy handshake” model which uses a one-byte protocol
  as the handshake layer as a standin for TLS 1.3. e.g. a 0x01 byte “represents”
  a ClientHello, a 0x02 byte “represents” a ServerHello. Keys are fixed.

  Test plan: If feasible, an existing QUIC implementation will be modified to
  use this protocol and E2E testing will be performed against it. (This
  can probably be done quickly but an alternate plan may be required if
  the effort needed turns out be excessive.)

- Phase 3: Final model with TLS 1.3 handshake layer fully plumbed in.

  Test plan: Testing against real world implementations.
