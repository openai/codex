QUIC Polling API Design
=======================

- [QUIC Polling API Design](#quic-polling-api-design)
  * [Background](#background)
  * [Requirements](#requirements)
  * [Reflections on Past Mistakes in Poller Interface Design](#reflections-on-past-mistakes-in-poller-interface-design)
  * [Example Use Cases](#example-use-cases)
    + [Use Case A: Simple Blocking or Non-Blocking Application](#use-case-a--simple-blocking-or-non-blocking-application)
    + [Use Case B: Application-Controlled Hierarchical Polling](#use-case-b--application-controlled-hierarchical-polling)
  * [Use of Poll Descriptors](#use-of-poll-descriptors)
  * [Event Types and Representation](#event-types-and-representation)
  * [Designs](#designs)
    + [Sketch A: One-Shot/Immediate Mode API](#sketch-a--one-shot-immediate-mode-api)
    + [Sketch B: Registered/Retained Mode API](#sketch-b--registered-retained-mode-api)
      - [Use Case Examples](#use-case-examples)
  * [Proposal](#proposal)
  * [Custom Poller Methods](#custom-poller-methods)
    + [Translation](#translation)
    + [Custom Poller Methods API](#custom-poller-methods-api)
    + [Internal Polling: Usage within SSL Objects](#internal-polling--usage-within-ssl-objects)
    + [External Polling: Usage over SSL Objects](#external-polling--usage-over-ssl-objects)
    + [Future Adaptation to Internal Pollable Resources](#future-adaptation-to-internal-pollable-resources)
  * [Worked Examples](#worked-examples)
    + [Internal Polling — Default Poll Method](#internal-polling---default-poll-method)
    + [Internal Polling — Custom Poll Method](#internal-polling---custom-poll-method)
    + [External Polling — Immediate Mode](#external-polling---immediate-mode)
    + [External Polling — Retained Mode](#external-polling---retained-mode)
    + [External Polling — Immediate Mode Without Event Handling](#external-polling---immediate-mode-without-event-handling)
  * [Change Notification Callback Mechanism](#change-notification-callback-mechanism)
  * [Q&A](#q-a)
  * [Windows support](#windows-support)
  * [Extra features on QUIC objects](#extra-features-on-quic-objects)
    + [Low-watermark functionality](#low-watermark-functionality)
    + [Timeouts](#timeouts)
    + [Autotick control](#autotick-control)

Background
----------

An application can create multiple QLSOs (see the [server API design
document](quic-server-api.md)), each bound to a single read/write network BIO
pair. Therefore an application needs to be able to poll:

- a QLSO for new incoming connection events;
- a QCSO for new incoming stream events;
- a QCSO for new incoming datagram events (when we support the datagram
  extension);
- a QCSO for stream creatability events;
- a QCSO for new connection error events;
- a QSSO (or QCSO with a default stream attached) for readability events;
- a QSSO (or QCSO with a default stream attached) for writeability events;
- non-OpenSSL objects, such as OS socket handles.

Observations:

- There are a large number of event types an application might want to poll on.

- There are different object types we might want to poll on.

- These object types are currently all SSL objects, though we should not assume
  that this will always be the case.

- The nature of a polling interface means that it must be possible to
  poll (i.e., block) on all desired objects in a single call. i.e., polling
  cannot really be composed using multiple sequential calls. Thus, it must be
  able for an application to request wakeup on the first of an arbitrary subset
  of any of the above kinds of events in a single polling call.

Requirements
------------

- **Universal cross-pollability.** Ability to poll on any combination of the above
  event types and pollable objects in a single poller call.

- **Support external polling.** An application must be able to be in control
  of its own polling if desired. This means no libssl code does any blocking I/O
  or poll(2)-style calls; the application handles all poll(2)-like calls to the
  OS. The application must thereafter be able to find out from us what QUIC
  objects are ready to be serviced.

- **Support internal polling.** Support a blocking poll(2)-like call provided
  by libssl for applications that want us to arrange OS polling.

- **Timeouts.** Support for optional timeouts.

- **Multi-threading.** The API must have semantics suitable for performant
  multi-threaded use, including for concurrent access to the same QUIC objects
  where supported by our API contract. This includes in particular
  avoidance of the thundering herd problem.

Desirable:

- Avoid needless impedance discontinuities with COTS polling interfaces (e.g.
  select(2), poll(2)).

- Efficient and performant design.

- Future extensibility.

Reflections on Past Mistakes in Poller Interface Design
-------------------------------------------------------

The deficiencies of select(2) are fairly evident and essentially attested to by
its replacement with poll(2) in POSIX operating systems. To the extent that
poll(2) has been replaced, it is largely due to the performance issues it poses
when evaluating large numbers of file descriptors. However, this design
is also unable to address the thundering herd problem, which we discuss
subsequently.

The replacements for poll(2) include Linux's epoll(2) and BSD's kqueue(2).

The design of Linux's epoll(2) interface in particular has often been noted to
contain a large number of design issues:

- It is designed to poll only FDs; this is probably a partial cause behind
  Linux's adaptation of everything into a FD (PIDs, signals, timers, eventfd,
  etc.)

- Events registered with epoll are associated with the underlying kernel
  object (file description), rather than a file descriptor; therefore events can
  still be received for a FD after the FD is closed(!) by a process, even
  quoting an incorrect FD in the reported events, unless a process takes care to
  unregister the FD prior to calling close(2).

- There are separate `EPOLL_CTL_ADD` and `EPOLL_CTL_MOD` calls which are needed
  to add a new FD registration and modify an existing FD registration, when
  most of the time what is desired is an “upsert” (update or insert) call. Thus
  callers have to track whether an FD has already been added or not.

- Only one FD can be registered, modified, or unregistered per syscall, rather
  than several FDs at once (syscall overhead).

- The design is poorly equipped to handle multithreaded use due to the
  thundering herd issue. If a single UDP datagram arrives and multiple threads
  are polling for such an event, only one of these threads should be woken up.

BSD's kqueue(2) has generally been regarded as a good, well thought out design,
and avoids most or all of these issues.

Example Use Cases
-----------------

Suppose there exists a hypothetical poll(2)-like API called `SSL_poll`. We
explore various possible use cases below:

### Use Case A: Simple Blocking or Non-Blocking Application

An application has two QCSOs open each with one QSSO. The QCSOs and QSSOs might
be in blocking or non-blocking mode. It wants to block until any of these have
data ready to read (or a connection error) and wants to know which SSL object is
ready and for what reason. It also wants to timeout after 1 second.

```text
SSL_poll([qcso0, qcso1, qsso0, qsso1],
         [READ|ERR, READ|ERR, READ|ERR, READ|ERR], timeout=1sec)
    → (OK, [qcso0], [READ])
    | Timeout
```

### Use Case B: Application-Controlled Hierarchical Polling

An application has two QCSOs open each with one QSSO, all in non-blocking mode.
It wants to block until any of these have data ready to read (or a connection
error) and wants to know which SSL object is ready and for what reason, but also
wants to block until various other application-specific non-QUIC events occur.
As such, it wants to handle its own polling.

This usage pattern is supported via hierarchical polling:

- An application collects file descriptors and event flags to poll from our QUIC
  implementation, either by using `SSL_get_[rw]poll_descriptor` and
  `SSL_net_(read|write)_desired` on each QCSO and deduplicating the results, or
  using those calls on each QLSO. It also determines the QUIC event handling
  timeout using `SSL_get_event_timeout`.

- An application does its own polling and timeout handling.

- An application calls `SSL_handle_events` if the polling process indicated
  an event for either of the QUIC poll descriptors or the QUIC event handling
  timeout has expired. The call need be made only on an Event Leader but can
  be made on any QUIC SSL object in the hierarchy.

- An application calls `SSL_poll` similarly to the above example, but with
  timeout set to 0 (and possibly with some kind of `NO_HANDLE_EVENTS` flag). The
  purpose of this call is **not** to block but to narrow down what QUIC objects
  are now ready for servicing.

This demonstrates the principle of hierarchical polling, whereby an application
can do its own polling and then use a poller in a mode where it always returns
immediately to narrow things down to specific QUIC objects. This is necessary as
one QCSO may obviously service many QSSOs, etc.

The requirement implied by this use case are:

- An application must be able to use our polling interface without blocking and
  without having `SSL_handle_events` or OS polling APIs be called, if desired.

Use of Poll Descriptors
-----------------------

As discussed in the [I/O Architecture Design Document](../quic-io-arch.md), the
notion of poll descriptors is used to provide an abstraction over arbitrary
pollable resources. A `BIO_POLL_DESCRIPTOR` is a tagged union structure which
can contain different kinds of handles.

This concept maps directly to our capacity for application-level polling of the
QUIC stack defined in this document, so it is used here. This creates a
consistent interface around polling.

To date, `BIO_POLL_DESCRIPTOR` structures have been used to contain an OS socket
file descriptor (`int` for POSIX, `SOCKET` for Win32), which can be used with
APIs such as `select(2)`. The tagged union structure is extended to support
specifying a SSL object pointer:

```c
#define BIO_POLL_DESCRIPTOR_SSL 2   /* (SSL *) */

typedef struct bio_poll_descriptor_st {
    uint32_t type;
    union {
        ...
        SSL     *ssl;
    } value;
} BIO_POLL_DESCRIPTOR;
```

Event Types and Representation
------------------------------

Regardless of the API design chosen, event types can first be defined.

### Summary of Event Types

We define the following event types:

- **R (Readable):** There is application data available to be read.

- **W (Writable):** It is currently possible to write more application data.

- **ER (Exception on Read):** The receive part of a stream has been remotely
  reset via a `RESET_STREAM` frame.

- **EW (Exception on Write):** The send part of a stream has been remotely
  reset via a `STOP_SENDING` frame.

- **EC (Exception on Connection):** A connection has started terminating
  (Terminating or Terminated states).

- **EL (Exception on Listener):** A QUIC listener SSL object has failed,
  for example due to a permanent error on an underlying network BIO.

- **ECD (Exception on Connection Drained):** A connection has *finished*
  terminating (Terminated state).

- **IC (Incoming Connection):** There is at least one incoming connection
  available to be popped using `SSL_accept_connection()`.

- **ISB (Incoming Stream — Bidirectional):** There is at least one
  bidirectional stream incoming and available to be popped using
  `SSL_accept_stream()`.

- **ISU (Incoming Stream — Unidirectional):** There is at least one
  unidirectional stream incoming and available to be popped using
  `SSL_accept_stream()`.

- **OSB (Outgoing Stream — Bidirectional):** It is currently possible
  to create at least one additional bidirectional stream.

- **OSU (Outgoing Stream — Unidirectional):** It is currently possible
  to create at least one additional unidirectional stream.

- **F (Failure):** Identifies failure of the `SSL_poll()` mechanism itself.

While this is a fairly large number of event types, there are valid use cases
for all of these and reasons why they need to be separate from one another. The
following dialogue explores the various design considerations.

### General Principles

From our discussion below we derive some general principles:

- It is important to provide an adequate granularity of event types so as to
  ensure an application can avoid wakeups it doesn't want.

- Event types which are not given by a particular object are simply ignored
  if requested by the application and never raised, similar to `poll(2)`.

- While not all event masks may make sense (e.g. `R` but not `ER`), we do not
  seek to prescribe combinations at this time. This is dissimilar to `poll(2)`
  which makes some event types “mandatory”. We may evolve this in future.

- Exception events on some successfully polled resource are not the same as the
  failure of the `SSL_poll()` mechanism itself (`SSL_poll()` returning 0).

### Header File Definitions

```c
#define SSL_POLL_EVENT_NONE         0

/*
 * Fundamental Definitions
 * -----------------------
 */

/* F (Failure) */
#define SSL_POLL_EVENT_F            (1U << 0)

/* EL (Exception on Listener) */
#define SSL_POLL_EVENT_EL           (1U << 1)

/* EC (Exception on Connection) */
#define SSL_POLL_EVENT_EC           (1U << 2)

/* ECD (Exception on Connection Drained) */
#define SSL_POLL_EVENT_ECD          (1U << 3)

/* ER (Exception on Read) */
#define SSL_POLL_EVENT_ER           (1U << 4)

/* EW (Exception on Write) */
#define SSL_POLL_EVENT_EW           (1U << 5)

/* R (Readable) */
#define SSL_POLL_EVENT_R            (1U << 6)

/* W (Writable) */
#define SSL_POLL_EVENT_W            (1U << 7)

/* IC (Incoming Connection) */
#define SSL_POLL_EVENT_IC           (1U << 8)

/* ISB (Incoming Stream: Bidirectional) */
#define SSL_POLL_EVENT_ISB          (1U << 9)

/* ISU (Incoming Stream: Unidirectional) */
#define SSL_POLL_EVENT_ISU          (1U << 10)

/* OSB (Outgoing Stream: Bidirectional) */
#define SSL_POLL_EVENT_OSB          (1U << 11)

/* OSU (Outgoing Stream: Unidirectional) */
#define SSL_POLL_EVENT_OSU          (1U << 12)

/*
 * Composite Definitions
 * ---------------------
 */

/* Read/write. */
#define SSL_POLL_EVENT_RW           (SSL_POLL_EVENT_R  | SSL_POLL_EVENT_W)

/* Read/write and associated exception event types. */
#define SSL_POLL_EVENT_RE           (SSL_POLL_EVENT_R  | SSL_POLL_EVENT_ER)
#define SSL_POLL_EVENT_WE           (SSL_POLL_EVENT_W  | SSL_POLL_EVENT_EW)
#define SSL_POLL_EVENT_RWE          (SSL_POLL_EVENT_RE | SSL_POLL_EVENT_WE)

/* All exception event types. */
#define SSL_POLL_EVENT_E            (SSL_POLL_EVENT_EL | SSL_POLL_EVENT_EC \
                                     | SSL_POLL_EVENT_ER | SSL_POLL_EVENT_EW)

/* Streams and connections. */
#define SSL_POLL_EVENT_IS           (SSL_POLL_EVENT_ISB | SSL_POLL_EVENT_ISU)
#define SSL_POLL_EVENT_I            (SSL_POLL_EVENT_IS  | SSL_POLL_EVENT_IC)
#define SSL_POLL_EVENT_OS           (SSL_POLL_EVENT_OSB | SSL_POLL_EVENT_OSU)
```

### Discussion

#### `EL`: Exception on Listener

**Q. When is this event type raised?**

A. This event type is raised only on listener (port) failure, which occurs when
an underlying network BIO encounters a permanent error.

**Q. Does `EL` imply `EC` and `ECD` on all child connections?**

A. Yes. A permanent network BIO failure causes immediate failure of all
connections dependent on it without first going through `TERMINATING` (except
possibly in the future with multipath for connections which aren't exclusively
reliant on that port).

**Q. What SSL object types can raise this event type?**

A. The event type is raised on a QLSO only. This may be revisited in future
(e.g. having it also be raised on child QCSOs.)

**Q. Why does this event type need to be distinct from `EC`?**

A. An application which is not immediately concerned by the failure of an
individual connection likely still needs to be notified if an entire port fails.

#### `EC`, `ECD`: Exception on Connection (/Drained)

**Q. Should this event be reported when a connection begins shutdown, begins
terminating, or finishes terminating?**

A.

- There is a use case to learn when we finish terminating because that is when
  we can throw away our port safely (raised on `TERMINATED`);

- there is a use case for learning as soon as we start terminating (raised on
  `TERMINATING` or `TERMINATED`);

- shutdown (i.e., waiting for streams to be done transmitting and then
  terminating, as per `SSL_shutdown_ex()`) is always initiated by the local
  application, thus there is no valid need for an application to poll on it.

As such, separate event types must be available both for the start of the
termination process and the conclusion of the termination process. `EC`
corresponds to `TERMINATING` or `TERMINATED` and `ECD` corresponds to
`TERMINATED` only.

**Q. What happens in the event of idle timeout?**

A. Idle timeout is an immediate transition to `TERMINATED` as per the channel
code.

**Q. Does `ECD` imply `EC`?**

A. Yes, as `EC` is raised in both the `TERMINATING` and `TERMINATED` states.

**Q. Can `ECD` occur without `EC` also occurring?**

A. No, this is not possible.

**Q. Does it make sense for an application to be able to mask this?**

A. Possibly not, though there is nothing particularly requiring us to prevent
this at this time.

**Q. Does it make sense for an application to be able to listen for this but not
`EL`?**

A. Yes, since `EL` implies `EC`, it is valid for an application to handle
port/listener failure purely in terms of the emergent consequence of all
connections failing.

#### `R`: Readable

Application data or FIN is available for popping via `SSL_read`. Never raised
after a stream FIN has been retired.

**Q. Is this raised on `RESET_STREAM`?**

A. No. Applications which wish to know of receive stream part failure should
listen for `ER`.

**Q. Should this be reported if the connection fails?**

A. If there is still application data that can be read, yes. Otherwise, no.

**Q. Should this be reported if shutdown has commenced?**

A. Potentially — if there is still data to be read or more data arrives at the
last minute.

**Q. What happens if this event is enabled on a send-only stream?**

A. The event is never raised.

**Q. Can this event be received before a connection has been (fully)
established?**

A. Potentially on the server side in the future due to incoming 0-RTT data.

#### `ER`: Error on Read

Raised only when the receive part of a stream has been reset by the remote peer
using a `RESET_STREAM` frame.

**Q. Should this be reported if a stream has already been concluded normally and
that FIN has been retired by the application by calling `SSL_read()`?**

A. No. We consider FIN retirement a success condition for our purposes here, so
normal stream conclusion and the retirement of that event does not cause ER.

**Q. Should this be reported if the connection fails?**

A. No, because that can be separately determined via the `EC` event and this
provides greater clarity as to what event is occurring and why. Also, it is
possible that a connection could fail and some application data is still
buffered to be read by the application, so `EC` does not imply `!R`.

**Q. Should this be reported if shutdown has been commenced?**

A. No — so long as the connection is alive more data could still be received at
the last minute.

**Q. What happens if this event is enabled on a send-only stream?**

A. The event is never raised.

**Q. What happens if this event is enabled on a QCSO?**

A. The event is applicable if the QCSO has a default stream attached. Otherwise,
it is never raised.

**Q. Why is this event separate from `R`?**

A. If an application receives an `R` event, this means more application data is
available to be read but this may be a business-as-usual circumstance which the
application does not feel obliged to handle urgently; therefore, it might mask
`R` in some circumstances.

If a stream reset is triggered by a peer, this needs to be notifiable to an
application immediately even if the application would not care about more
ordinary application data arriving on a stream for now.

Therefore, `ER` *must* be separate from `R`, otherwise such applications would
be unable to prevent spurious wakeups due to normal application data when they
only care about the possibility of a stream reset.

**Q. Should applications be able to listen on `R` but not `ER`?**

A. This would enable an application to listen for more application data but not
care about stream resets. This can be permitted for now even if it raises some
questions about the robustness of such applications.

**Q. How will the future reliable stream resets extension be handled?**

A. `R` will be raised until all data up to the reliable reset point has been
retired by the application, then `ER` is raised and `R` is never again raised.

**Q. What happens if a stream is reset after the FIN has been retired by the
application?**

A. The reset is ignored; as per RFC 9000 s. 3.2, the Data Read state is terminal
and has no `RESET_STREAM` transition. Moreover, after an application is done
with a stream it can free the QSSO, which means a post-FIN-retirement reset
cannot be reliably received anyway.

Note that this does not preclude handling of `RESET_STREAM` in the normal way
for a stream which was concluded normally but where the application has *not*
yet read all data, which is potentially useful.

#### `W`: Writable

Raised when send buffer space is available, so that it is possible to write
application data via `SSL_write`.

**Q. Is this raised on `STOP_SENDING`?**

A. No. Applications which wish to know of remotely-triggered send stream part
reset should listen for `EW`.

**Q. Should this be reported if the connection fails?**

A. No.

**Q. Should this be reported if shutdown has commenced?**

A. No.

**Q. What happens if this event is enabled on a concluded send part?**

A. The event is never raised after the stream is concluded.

**Q. What happens if this event is enabled on a receive-only stream?**

A. The event is never raised.

**Q. What happens if this event is enabled on a QCSO?**

A. The event is applicable if the QCSO has a default stream attached. Otherwise,
it is never raised.

**Q. Can this event be raised before a connection has been established?**

A. Potentially in the future, if 0-RTT is in use and we have a cached 0-RTT
session including flow control budgets which establish we have room to write
more data for 0-RTT.

#### `ER`: Error on Write

Raised only when the send part of a stream has been reset by the remote peer via
`STOP_SENDING`.

**Q. Should this be raised if a stream's send part has been concluded
normally?**

A. No. We consider that a success condition for our purposes here.

**Q. Should this be reported if the connection fails?**

A. No, because that can be separately determined via the `EC` event and this
provides greater clarity as to what event is occurring and why.

**Q. What happens if this event is enabled on a receive-only stream?**

A. The event is never raised.

**Q. Should this be reported if the send part was reset locally via
`SSL_reset_stream()`?**

A. There is no need for this since the application knows what it did, though
there is no particular harm in doing so. Current decision: do not report it.

**Q. What if the send part was reset locally and then we also received a
`STOP_SENDING` frame for it?**

A. If the local application has reset a stream locally, it knows about this fact
therefore there is no need to raise `EW`. The local reset takes precedence.

**Q. Should this be reported if shutdown has commenced?**

A. Probably not, since shutdown is under local application control and so if an
application does this it already knows about it. Therefore there is no reason to
poll for it.

**Q. Why is this event separate from `W`?**

A. It is useful for an application to be able to determine if something odd has
happened on a stream (like it being reset remotely via `STOP_SENDING`) even if
it does not currently want to write anything (and therefore is not listening for
`W`). Since stream resets can occur asynchronously and have application
protocol-defined semantics, it is important an application can be notified of
them immediately.

**Q. Should applications be able to listen on `W` but not `EW`?**

A. This would enable an application to listen for the opportunity to write but
not care about `STOP_SENDING` events. This is probably valid even if it raises
some questions about the robustness of such applications. It can be allowed,
even if not recommended (see the General Principles section below).

**Q. How will the future reliable stream resets extension be handled?**

A. The extension does not offer a `STOP_SENDING` equivalent so this is not a
relevant concern.

#### `ISB`, `ISU`: Incoming Stream Availability

Indicates one or more incoming bidrectional or unidirectional streams which have
yet to be popped via `SSL_accept_stream()`.

**Q. Is this raised on `RESET_STREAM`?**

A. It is raised on anything that would cause `SSL_accept_stream()` to return a
stream. This could include a stream which was created by being reset.

**Q. What happens if this event is enabled on a QSSO or QLSO?**

A. The event is never raised.

**Q. If a stream is in the accept queue and then the connection fails, should it
still be reported?**

A. Yes. The application may be able to accept the stream and pop any application
data which was already received in future. It is the application's choice to
listen for EC and have it take priority if it wishes.

**Q. Can this event be raised before a connection has been established?**

A. Client — no. Server — no initially, except possibly during 0-RTT when a
connection is not considered fully established yet.

#### `OSB`, `OSU`: Outgoing Stream Readiness

Indicates we have the ability, based on current stream count flow control state,
to initiate an outgoing bidirectional or unidirectional stream.

**Q. Should this be reported if the connection fails?**

A. No.

**Q. Should this be reported if shutdown has commenced?**

A. No.

**Q. What happens if this event is enabled on a QLSO or QSSO?**

A. The event is never raised.

**Q. Can this event be raised before a connection has been established?**

A. Potentially in future, on the client side only, if 0-RTT is in use and we
have a cached 0-RTT session including flow control budgets which establish we
have room to write more data for 0-RTT.

#### `IC`: Incoming Connection

Indicates at least one incoming connection is available to be popped using
`SSL_accept_connection()`.

**Q. Should this be reported if the port fails?**

A. Potentially. A connection could have already been able to receive application
data prior to it being popped from the accept queue by the application calling
`SSL_accept_connection()`. Whether or not application data was received on any
stream, a successfully established connection should be reported so that the
application knows it happened.

**Q. Can this event be raised before a connection has been established?**

A. Potentially in future, if 0-RTT is in use; we could receive connection data
before the connection process is complete (handshake confirmation).

**Q. What happens if this event is enabled on a QCSO or QSSO?**

A. The event is never raised.

#### `F`: Failure

Indicates that the `SSL_poll` mechanism itself has failed. This may be due to
specifying an unsupported `BIO_POLL_DESCRIPTOR` type, or an unsupported `SSL`
object, or so on. This indicates a caller usage error. It is wholly distinct
from an exception condition on a successfully polled resource (e.g. `ER`, `EW`,
`EC`, `EP`).

**Q. Can this event type be masked?**

A. No — this event type may always be raised even if not requested. Requesting
it is a no-op (similar to `poll(2)` `POLLERR`). This is the only non-maskable
event type.

**Q. What happens if an `F` event is raised?**

The `F` event is reported in one or more elements of the items array. The
`result_count` output value reflects the number of items in the items array with
non-zero `revents` fields, as always. This includes any `F` events (there may be
multiple), and any non-`F` events which were output for earlier entries in the
items array (where a `F` event occurs for a subsequent entry in the items
array).

`SSL_poll()` then returns 0. The ERR stack *always* has at least one entry
placed on it, which reflects the first `F` event which was output. Any
subsequent `F` events do not have error information available.

Designs
-------

Two designs are considered here:

- Sketch A: An “immediate-mode” poller interface similar to poll(2).

- Sketch B: A “registered” poller interface similar to BSD's kqueue(2) (or Linux's
  epoll(2)).

Sketch A is simpler but is likely to be less performant. Sketch B is a bit more
elaborate but can offer more performance. It is possible to offer both APIs if
desired.

### Sketch A: One-Shot/Immediate Mode API

We define a common structure for representing polled events:

```c
typedef struct ssl_poll_item_st {
    BIO_POLL_DESCRIPTOR desc;
    uint64_t            events, revents;
} SSL_POLL_ITEM;
```

This structure works similarly to the `struct pollfd` structure used by poll(2).
`desc` describes the object to be polled, `events` is a bitmask of
`SSL_POLL_EVENT` values describing what events to listen for, and `revents` is
a bitmask of zero or more events which are actually raised.

Polling implementations are only permitted to modify the `revents` field in a
`SSL_POLL_ITEM` structure passed by the caller.

```c
/*
 * SSL_poll
 * --------
 *
 * SSL_poll evaluates each of the items in the given array of SSL_POLL_ITEMs
 * and determines which poll items have relevant readiness events raised. It is
 * similar to POSIX poll(2).
 *
 * The events field of each item specifies the events the caller is interested
 * in and is the sum of zero or more SSL_POLL_EVENT_* values. When using
 * SSL_poll in a blocking fashion, only the occurrence of one or more events
 * specified in the events field, or a timeout or failure of the polling
 * mechanism, will cause SSL_poll to return.
 *
 * When SSL_poll returns, the revents field is set to the events actually active
 * on an item. This may or may not also include events which were not requested
 * in the events field.
 *
 * Specifying an item with an events field of zero is a no-op; the array entry
 * is ignored. Unlike poll(2), error events are not automatically included
 * and it is the application's responsibility to request them.
 *
 * Each item to be polled is described by a BIO_POLL_DESCRIPTOR. A
 * BIO_POLL_DESCRIPTOR is an extensible tagged union structure which describes
 * some kind of object which SSL_poll might (or might not) know how to poll.
 * Currently, SSL_poll can poll the following kinds of BIO_POLL_DESCRIPTOR:
 *
 *   BIO_POLL_DESCRIPTOR_TYPE_SOCK_FD   (int fd)    -- OS-pollable sockets only
 *      Note: Some OSes consider sockets to be a different kind of handle type
 *            to ordinary file handles. Therefore, this type is used
 *            specifically for OS socket handles only (e.g. SOCKET on Win32).
 *            It cannot be used to poll other OS handle types.
 *
 *   BIO_POLL_DESCRIPTOR_TYPE_SSL       (SSL *ssl)  -- QUIC SSL objects only
 *
 * num_items is the number of items in the passed array.
 *
 * stride must be set to sizeof(SSL_POLL_ITEM).
 *
 * timeout specifies how long to wait for at least one passed SSL_POLL_ITEM to
 * have at least one event to report. If it is set to NULL, this function does
 * not time out and waits forever. Otherwise, it is a timeout value expressing a
 * timeout duration in microseconds. The value expresses a duration, not a
 * deadline.
 *
 * This function can be used in a non-blocking mode where it will provide
 * information on readiness for each of the items and then return immediately,
 * even if no item is ready. To facilitate this, pass a zero-value timeout
 * structure.
 *
 * If num_items is set to zero, this function returns with a timeout condition
 * after the specified timeout, or immediately with failure if no timeout
 * was requested (as otherwise it would logically deadlock).
 *
 * flags must be zero or more SSL_POLL_FLAG values:
 *
 *   - SSL_POLL_FLAG_NO_HANDLE_EVENTS:
 *       This may be used only when a zero timeout is specified (non-blocking
 *       mode). Ordinarily in this case, relevant SSL objects have internal
 *       event processing performed as this may help them to become ready.
 *       This may also cause network I/O to occur. If this flag is specified,
 *       no such processing will be performed. This means that SSL_poll
 *       will only report pre-existing readiness events for the specified objects.
 *
 *       If timeout is NULL or non-zero, specifying this flag is an error.
 *
 * Regardless of whether this function succeeds, times out, or fails for other
 * reasons, the revents field of each item is set to a valid value reflecting
 * the current readiness, or to 0, and *result_count (if non-NULL) is written
 * with the total number of items having an revents field, which,
 * when masked with the corresponding events field, is nonzero at the time the
 * function returns. Note that these entries in the items array may not be
 * consecutive or at the start of the array.
 *
 * There is a distinction between exception conditions on a resource which is
 * polled (such as a connection being terminated) and an failure in the polling
 * code itself. A mere exception condition is not considered a failure of
 * the polling mechanism itself and does not call SSL_poll to return 0. If
 * the polling mechanism itself fails (for example, because an unsupported
 * BIO_POLL_DESCRIPTOR type or SSL object type is passed), the F event type
 * is raised on at least one poll item and the function returns 0. At least
 * one ERR stack entry will be raised describing the cause of the first F event
 * for the input items. Any additional F events do not have their error
 * information reported.
 *
 * Returns 1 on success or timeout, and 0 on failure. Timeout conditions can
 * be distinguished by the *result_count field being written as 0.
 *
 * This function does not modify any item's events or desc field.
 * The initial value of an revents field when this function is called is of no
 * consequence.
 *
 * This is a "one-shot" API; greater performance may be obtained from using
 * an API which requires advanced registration of pollables.
 */
#define SSL_POLL_FLAG_NO_HANDLE_EVENTS      (1U << 0)

int SSL_poll(SSL_POLL_ITEM *item,
             size_t num_items, size_t stride,
             const struct timeval *timeout,
             uint64_t flags,
             size_t *result_count);
```

**Performance and thundering-herd issues.** There are two intrinsic performance
issues with this design:

- Because it does not involve advance registration of things being polled,
  the entire object list needs to be scanned in each call, and there is
  no real opportunity to maintain internal state which would make polling
  more efficient.

- Because this design is inherently “stateless”, it cannot really solve
  the thundering herd problem in any reasonable way. In other words, if n
  threads are all calling `SSL_poll` on the same set of objects and events,
  there is no way for an event to be efficiently distributed to just one of
  those threads.

  This limitation is intrinsic to the design of `poll(2)` and poll-esque APIs.
  It is not necessarily a reason not to offer this rather simple API, as use of
  poll(2) and poll(2)-like APIs is widespread and users are likely to appreciate
  an API which does not provide significant impedance discontinuities to
  applications which use select/poll, even if those applications suffer impaired
  performance as a result.

### Sketch B: Registered/Retained Mode API

Alternatively, an API which requires advance registration of pollable objects is
proposed.

Attention is called to certain design features:

- This design can solve the thundering herd problem, achieving efficient
  distribution of work to threads by auto-disabling an event mask bit after
  distribution of the readiness event to one thread currently calling the poll
  function.

- The fundamental call, `SSL_POLL_GROUP_change_poll`, combines the operations
  of adding/removing/changing registered events and actually polling. This is
  important as due to the herd-avoidance design above, events can be and are
  automatically disarmed and need rearming as frequently as the poll function is
  called. This streamlined design therefore enhances efficiency. This design
  aspect is inspired directly by kqueue.

- Addition of registered events and mutation of existing events uses an
  idempotent upsert-type operation, which is what most applications actually
  want (unlike e.g. epoll).

```c
typedef struct ssl_poll_group_st SSL_POLL_GROUP;

/*
 * The means of obtaining an SSL_POLL_GROUP instance is discussed
 * subsequently. For now, you can imagine the following strawman function:
 *
 *     SSL_POLL_GROUP *SSL_POLL_GROUP_new(void);
 *
 */

void SSL_POLL_GROUP_free(SSL_POLL_GROUP *pg);

#define SSL_POLL_EVENT_FLAG_NONE            0

/*
 * Registered event is deleted (not disabled) after one event fires.
 */
#define SSL_POLL_EVENT_FLAG_ONESHOT         (1U << 0)

/*
 * Work queue dispatch (anti-thundering herd) - dispatch to one concurrent call
 * and set DISABLED.
 */
#define SSL_POLL_EVENT_FLAG_DISPATCH        (1U << 1)

/* Registered event is disabled and will not return events. */
#define SSL_POLL_EVENT_FLAG_DISABLED        (1U << 2)

/* Delete a registered event. */
#define SSL_POLL_EVENT_FLAG_DELETE          (1U << 3)

/* Change previous cookie value. Cookie is normally only set on initial add. */
#define SSL_POLL_EVENT_FLAG_UPDATE_COOKIE   (1U << 4)

/*
 * A structure to request registration, deregistration or modification of a
 * registered event.
 */
typedef struct ssl_poll_change_st {
    /* The pollable object to be polled. */
    BIO_POLL_DESCRIPTOR desc;
    size_t              instance;

    /* An opaque application value passed through in any reported event. */
    void                *cookie;

    /*
     * Disables and enables event types. Any events in disable_mask are
     * disabled, and then any events in enable_events are enabled. disable_events
     * is processed before enable_events, therefore the enabled event types may
     * be set (ignoring any previous value) by setting disable_events to
     * UINT64_MAX and enable_events to the desired event types. Non-existent
     * event types are ignored.
     */
    uint64_t            disable_events, enable_events;

    /*
     * Enables and disables registered event flags in the same vein as
     * disable_events and enable_events manages registered event types.
     * This is used to disable and enable SSL_POLL_EVENT_FLAG bits.
     */
    uint64_t            disable_flags, enable_flags;
} SSL_POLL_CHANGE;

typedef struct ssl_poll_event_st {
    BIO_POLL_DESCRIPTOR desc;
    size_t              instance;
    void                *cookie;
    uint64_t            revents;
} SSL_POLL_EVENT;

/*
 * SSL_POLL_GROUP_change_poll
 * --------------------------
 *
 * This function performs the following actions:
 *
 *   - firstly, if num_changes is non-zero, it updates registered events on the
 *     specified poll group, adding, removing and modifying registered events as
 *     specified by the changes in the array given in changes;
 *
 *   - secondly, if num_events is non-zero, it polls for any events that have
 *     arisen that match the registered events, and places up to num_events such
 *     events in the array given in events.
 *
 * This function may be used for either of these effects, or both at the same
 * time. Changes to event registrations are applied before events are returned.
 *
 * If num_changes is non-zero, change_stride must be set to
 * sizeof(SSL_POLL_CHANGE).
 *
 * If num_events is non-zero, event_stride must be set to
 * sizeof(SSL_POLL_EVENT).
 *
 * If timeout is NULL, this function blocks forever until an applicable event
 * occurs. If it points to a zero value, this function never blocks and will
 * apply given changes, return any applicable events, if any, and then return
 * immediately. Note that any requested changes are always applied regardless of
 * timeout outcome.
 *
 * flags must be zero or more SSL_POLL_FLAGS. If SSL_POLL_FLAG_NO_HANDLE_EVENTS
 * is set, polled objects do not automatically have I/O performed which might
 * enable them to raise applicable events. If SSL_POLL_FLAG_NO_POLL is set,
 * changes are processed but no polling is performed. This is useful if it is
 * desired to provide an event array to allow errors when processing changes
 * to be received. Passing SSL_POLL_FLAG_NO_POLL forces a timeout of 0
 * (non-blocking mode); the timeout argument is ignored.
 *
 * The number of events written to events is written to *num_events_out,
 * regardless of whether this function succeeds or fails.
 *
 * Returns 1 on success or 0 on failure. A timeout is considered a success case
 * which returns 0 events; thus in this case, the function returns 1 and
 * *num_events_out is written as 0.
 *
 * This function differs from poll-style interfaces in that the events reported
 * in the events array bear no positional relationship to the registration
 * changes indicated in changes. Thus the length of these arrays is unrelated.
 *
 * An error may occur when processing a change. If this occurs, an entry
 * describing the error is written out as an event to the event array. The
 * function still returns success, unless there is no room in the events array
 * for the error (for example, if num_events is 0), in which case failure is
 * returned.
 *
 * When an event is output from this function, desc is set to the original
 * registered poll descriptor, cookie is set to the cookie value which was
 * passed in when registering the event, and revents is set to any applicable
 * events, which might be a superset of the events which were actually asked
 * for. (However, only events actually asked for at registration time will
 * cause a blocking call to SSL_POLL_GROUP_change_poll to return.)
 *
 * An event structure which represents a change processing error will have the
 * psuedo-event SSL_POLL_EVENT_POLL_ERROR set, with copies of the desc and
 * cookie provided. This is not a real event and cannot be requested in a
 * change.
 *
 * The 'primary key' for any registered event is the tuple (poll descriptor,
 * instance). Changing an existing event is done by passing a change structure
 * with the same values for the poll descriptor and instance. The instance field
 * can be used to register multiple separate registered events on the same
 * poll descriptor. Many applications will be able to use a instance field of
 * 0 in all circumstances.
 *
 * To unregister an event, pass a matching poll descriptor and instance value
 * and set DELETE in enable_flags.
 *
 * It is recommended that callers delete a registered event from a poll group
 * before freeing the underlying resource. If an object which is registered
 * inside a poll group is freed, the semantics depend on the type of the poll
 * descriptor used. For example, libssl has no safe way to detect if an OS
 * socket poll descriptor is closed, therefore it is essential callers
 * deregister such registered events prior to closing the socket handle.
 *
 * Other poll descriptor types may implement automatic deregistration from poll
 * groups which they are registered into when they are freed. This varies by
 * poll descriptor type. However, even if a poll descriptor type does implement
 * this, applications must still ensure no events in an SSL_POLL_EVENT
 * structure recorded from a previous call to this function are left over, which
 * may still reference that poll descriptor. Therefore, applications must still
 * excercise caution when freeing resources which are registered, or which were
 * previously registered in a poll group.
 */
#define SSL_POLL_FLAG_NO_HANDLE_EVENTS       (1U << 0)
#define SSL_POLL_FLAG_NO_POLL                (1U << 1)

#define SSL_POLL_EVENT_POLL_ERROR            (((uint64_t)1) << 63)

int SSL_POLL_GROUP_change_poll(SSL_POLL_GROUP *pg,

                               const SSL_POLL_CHANGE *changes,
                               size_t num_changes,
                               size_t change_stride,

                               SSL_POLL_EVENT *events,
                               size_t num_events,
                               size_t event_stride,

                               const struct timeval *timeout,
                               uint64_t flags,
                               size_t *num_events_out);

/* These macros may be used if only one function is desired. */
#define SSL_POLL_GROUP_change(pg, changes, num_changes, flags)      \
    SSL_POLL_GROUP_change_poll((pg), (changes), (num_changes),      \
                               sizeof(SSL_POLL_CHANGE),             \
                               NULL, 0, 0, NULL, (flags), NULL)

#define SSL_POLL_GROUP_poll(pg, items, num_items, timeout, flags, result_c) \
    SSL_POLL_GROUP_change_poll((pg), NULL, 0, 0,                            \
                               (items), (num_items), sizeof(SSL_POLL_ITEM), \
                               (timeout), (flags), (result_c))

/* Convenience inlines. */
static ossl_inline ossl_unused void SSL_POLL_CHANGE_set(SSL_POLL_CHANGE *chg,
                                                        BIO_POLL_DESCRIPTOR desc,
                                                        size_t instance,
                                                        void *cookie,
                                                        uint64_t events,
                                                        uint64_t flags)
{
    chg->desc           = desc;
    chg->instance       = instance;
    chg->cookie         = cookie;
    chg->disable_events = UINT64_MAX;
    chg->enable_events  = events;
    chg->disable_flags  = UINT64_MAX;
    chg->enable_flags   = flags;
}

static ossl_inline ossl_unused void SSL_POLL_CHANGE_delete(SSL_POLL_CHANGE *chg,
                                                           BIO_POLL_DESCRIPTOR desc,
                                                           size_t instance)
{
    chg->desc           = desc;
    chg->instance       = instance;
    chg->cookie.ptr     = NULL;
    chg->disable_events = 0;
    chg->enable_events  = 0;
    chg->disable_flags  = 0;
    chg->enable_flags   = SSL_POLL_EVENT_FLAG_DELETE;
}

static ossl_inline ossl_unused void
SSL_POLL_CHANGE_chevent(SSL_POLL_CHANGE *chg,
                        BIO_POLL_DESCRIPTOR desc,
                        size_t instance,
                        uint64_t disable_events,
                        uint64_t enable_events)
{
    chg->desc           = desc;
    chg->instance       = instance;
    chg->cookie.ptr     = NULL;
    chg->disable_events = disable_events;
    chg->enable_events  = enable_events;
    chg->disable_flags  = 0;
    chg->enable_flags   = 0;
}

static ossl_inline ossl_unused void
SSL_POLL_CHANGE_chflag(SSL_POLL_CHANGE *chg,
                       BIO_POLL_DESCRIPTOR desc,
                       size_t instance,
                       uint64_t disable_flags,
                       uint64_t enable_flags)
{
    chg->desc           = desc;
    chg->instance       = instance;
    chg->cookie.ptr     = NULL;
    chg->disable_events = 0;
    chg->enable_events  = 0;
    chg->disable_flags  = disable_flags;
    chg->enable_flags   = enable_flags;
}

static ossl_inline ossl_unused BIO_POLL_DESCRIPTOR
SSL_as_poll_descriptor(SSL *s)
{
    BIO_POLL_DESCRIPTOR d;

    d.type      = BIO_POLL_DESCRIPTOR_TYPE_SSL;
    d.value.ssl = s;

    return d;
}
```

#### Use Case Examples

```c
/*
 * Scenario 1: Register multiple events on different QUIC objects and
 * immediately start blocking for events.
 */
{
    int rc;

    SSL *qconn1 = get_some_quic_conn();
    SSL *qconn2 = get_some_quic_conn();
    SSL *qstream1 = get_some_quic_stream();
    SSL *qlisten1 = get_some_quic_listener();
    int socket = get_some_socket_handle();

    SSL_POLL_GROUP *pg = SSL_POLL_GROUP_new();
    SSL_POLL_CHANGE changes[32], *chg = changes;
    SSL_POLL_EVENT events[32];
    void *cookie = some_app_ptr;
    size_t i, nchanges = 0, nevents = 0;

    /* Wait for an incoming stream or conn error on conn 1 and 2. */
    SSL_POLL_CHANGE_set(chg++, SSL_as_poll_descriptor(qconn1), 0, cookie,
                        SSL_POLL_EVENT_IS | SSL_POLL_EVENT_E, 0);
    ++nchanges;

    SSL_POLL_CHANGE_set(chg++, SSL_as_poll_descriptor(qconn2), 0, cookie,
                        SSL_POLL_EVENT_IS | SSL_POLL_EVENT_E, 0);
    ++nchanges;

    /* Wait for incoming data (or reset) on stream 1. */
    SSL_POLL_CHANGE_set(chg++, SSL_as_poll_descriptor(qstream1), 0, cookie,
                        SSL_POLL_EVENT_R, 0);
    ++nchanges;

    /* Wait for an incoming connection. */
    SSL_POLL_CHANGE_set(chg++, SSL_as_poll_descriptor(qlisten1), 0, cookie,
                        SSL_POLL_EVENT_IC, 0);
    ++nchanges;

    /* Also poll on an ordinary OS socket. */
    SSL_POLL_CHANGE_set(chg++, OS_socket_as_poll_descriptor(socket), 0, cookie,
                        SSL_POLL_EVENT_RW, 0);
    ++nchanges;

    /* Immediately register all of these events and wait for an event. */
    rc = SSL_POLL_GROUP_change_poll(pg,
                                    changes, nchanges, sizeof(changes[0]),
                                    events, OSSL_NELEM(events), sizeof(events[0]),
                                    NULL, 0, &nevents);
    if (!rc)
        return 0;

    for (i = 0; i < nevents; ++i) {
        if ((events[i].revents & SSL_POLL_EVENT_POLL_ERROR) != 0)
            return 0;

        process_event(&events[i]);
    }

    return 1;
}

void process_event(const SSL_POLL_EVENT *event)
{
    APP_INFO *app = event->cookie.ptr;

    do_something(app, event->revents);
}

/*
 * Scenario 2: Test for pre-existing registered events in non-blocking mode
 * as part of a hierarchical polling strategy.
 */
{
   int rc;

   SSL_POLL_EVENT events[32],
   size_t i, nevents = 0;
   struct timeval timeout = { 0 };

   /*
    * Find out what is ready without blocking.
    * Assume application already did I/O event handling and do not tick again.
    */
   rc = SSL_POLL_GROUP_poll(pg, events, OSSL_NELEM(events),
                            &timeout, SSL_POLL_FLAG_NO_HANDLE_EVENTS,
                            &nevents);
   if (!rc)
       return 0;

   for (i = 0; i < nevents; ++i)
       process_event(&events[i]);
}

/*
 * Scenario 3: Remove one event but don't poll.
 */
{
    int rc;
    SSL_POLL_CHANGE changes[1], *chg = changes;
    size_t nchanges = 0;

    SSL_POLL_CHANGE_delete(chg++, SSL_as_poll_descriptor(qstream1), 0);
    ++nchanges;

    if (!SSL_POLL_GROUP_change(pg, changes, nchanges, 0))
        return 0;

    return 1;
}

/*
 * Scenario 4: Efficient (non-thundering-herd) multi-thread dispatch with
 * efficient rearm.
 *
 * Assume all registered events have SSL_POLL_EVENT_FLAG_DISPATCH set on them.
 *
 * Assume this function is being called concurrently from a large number of
 * threads.
 */
{
    int rc;
    SSL_POLL_CHANGE changes[32], *chg;
    SSL_POLL_EVENT events[32];
    size_t i, nchanges, nevents = 0;

    /*
     * This will block, and then the first event to occur will be returned on
     * *one* thread, and the event will be disabled. Other threads will keep
     * waiting.
     */
    if (!SSL_POLL_GROUP_poll(pg, events, OSSL_NELEM(events),
                             NULL, 0, &nevents))
        return 0;

    /* Application event loop */
    while (!app_should_stop()) {
        chg = changes;
        nchanges = 0;

        for (i = 0; i < nevents; ++i) {
            process_event(&events[i]); /* do something in application */

            /* We have processed the event so now reenable it. */
            SSL_POLL_CHANGE_chflag(chg++, events[i].desc, events[i].instance,
                                   SSL_POLL_EVENT_FLAG_DISABLE, 0);
            ++nchanges;
        }

        /* Reenable any event we processed and go to sleep again. */
        if (!SSL_POLL_GROUP_change_poll(pg, changes, nchanges, sizeof(changes[0]),
                                        events, OSSL_NELEM(events), sizeof(events[0]),
                                        NULL, 0, &nevents))
            return 0;
    }

    return 1;
}
```

Proposal
--------

It is proposed to offer both of these API sketches. The simple `SSL_poll` API is
compelling for simple use cases, and both APIs have merits and cases where they
will be highly desirable. The ability of the registered API to support
thundering herd mitigation is of particular importance.

Custom Poller Methods
---------------------

It is also desirable to support custom poller methods provided by an
application. This allows an application to support custom poll descriptor types
and provide a way to poll on those poll descriptors. For example, an application
could provide a BIO_dgram_pair (which ordinarily cannot support polling and
cannot be used with the blocking API) and a custom poller which can poll some
opaque poll descriptor handle provided by the application (which might be e.g.
based on condition variables or so on).

We therefore now discuss modifications to the above APIs to support custom
poller methods.

### Translation

When a poller polls a QUIC SSL object, it must figure out how to block on this
object. This means it must ultimately make some blocking poll(2)-like call to
the OS. Since an OS only knows how to block on resources it issues, this means
that all resources such as QUIC SSL objects must be reduced into OS resources
before polling can occur.

This process occurs via translation. Suppose `SSL_poll` is called with a QCSO,
two QSSOs on that QCSO, and an OS socket handle:

  - `SSL_poll` will convert the poll descriptors pointing to SSL objects
    to network-side poll descriptors by calling `SSL_get_[rw]poll_descriptor`,
    which calls through to `BIO_get_[rw]poll_descriptor`;

  - The yielded poll descriptors are then reduced to a set of unique poll
    descriptors (for example, both QSSOs will have the same underlying
    poll descriptor, so duplicates are removed);

  - The OS socket handle poll descriptor which was passed in is simply
    passed through as-is;

  - The resulting set of poll descriptors is then passed on to an underlying
    poller implementation, which might be based on e.g. poll(2). But it might
    also be a custom method provided by an application if one of the SSL objects
    resolved to a custom poll descriptor type.

  - When the underlying poll call returns, reverse translation occurs.
    Poll descriptors which have become ready in some aspect and which were
    translated are mapped back to the input SSL objects which they were derived
    from (since duplicates are removed, this may be multiple SSL objects per
    poll descriptor). This set of SSL objects is reduced to a unique set of
    event leaders and those event leaders are ticked. The QUIC SSL objects are
    then probed for their current state to determine current readiness and this
    information is returned.

The above scheme also means that the retained-mode polling API can be more
efficient since translation information can be retained internally rather than
being re-derived every time.

### Custom Poller Methods API

There are two kinds of polling that occur:

- Internal polling for blocking API: This is where an SSL object automatically
  polls internally to support blocking API operation. If an underlying network
  BIO cannot support a poll descriptor which we understand how to poll on, we
  cannot support blocking API operation. We can support a poll descriptor if it
  is an OS socket handle, or if a custom poller is configured that knows how to
  poll it.

- External polling support: This is where an application calls a polling API.

Firstly, the `SSL_POLL_METHOD` object is defined abstractly as follows:

```c
/* API (Psuedocode) */
#define SSL_POLL_METHOD_CAP_IMMEDIATE  (1U << 0) /* supports immediate mode */
#define SSL_POLL_METHOD_CAP_RETAINED   (1U << 1) /* supports retained mode */

interface SSL_POLL_METHOD {
    int free(void);
    int up_ref(void);

    uint64_t get_caps(void);
    int supports_poll_descriptor(const BIO_POLL_DESCRIPTOR *desc);
    int poll(/* as shown for SSL_poll */);
    SSL_POLL_GROUP *create_poll_group(const OSSL_PARAM *params);
}

interface SSL_POLL_GROUP {
    int free(void);
    int up_ref(void);

    int change_poll(/* as shown for SSL_POLL_GROUP_change_poll */);
}
```

This interface is realised as follows:

```c
typedef struct ssl_poll_method_st SSL_POLL_METHOD;
typedef struct ssl_poll_group_st SSL_POLL_GROUP;

typedef struct ssl_poll_method_funcs_st {
    int (*free)(SSL_POLL_METHOD *self);
    int (*up_ref)(SSL_POLL_METHOD *self);

    uint64_t (*get_caps)(const SSL_POLL_GROUP *self);
    int (*poll)(SSL_POLL_METHOD *self, /* as shown for SSL_poll */);
    SSL_POLL_GROUP *(*create_poll_group)(SSL_POLL_METHOD *self,
                                         const OSSL_PARAM *params);
} SSL_POLL_METHOD_FUNCS;

SSL_POLL_METHOD *SSL_POLL_METHOD_new(const SSL_POLL_METHOD_FUNCS *funcs,
                                     size_t funcs_len, size_t data_len);

void *SSL_POLL_METHOD_get0_data(const SSL_POLL_METHOD *self);

int SSL_POLL_METHOD_free(SSL_POLL_METHOD *self);
void SSL_POLL_METHOD_do_free(SSL_POLL_METHOD *self);
int SSL_POLL_METHOD_up_ref(SSL_POLL_METHOD *self);

uint64_t SSL_POLL_METHOD_get_caps(const SSL_POLL_METHOD *self);
int SSL_POLL_METHOD_supports_poll_descriptor(SSL_POLL_METHOD *self,
                                             const BIO_POLL_DESCRIPTOR *desc);
int SSL_POLL_METHOD_poll(SSL_POLL_METHOD *self, ...);
SSL_POLL_GROUP *SSL_POLL_METHOD_create_poll_group(SSL_POLL_METHOD *self,
                                                  const OSSL_PARAM *params);

typedef struct ssl_poll_group_funcs_st {
    int (*free)(SSL_POLL_GROUP *self);
    int (*up_ref)(SSL_POLL_GROUP *self);

    int (*change_poll)(SSL_POLL_GROUP *self, /* as shown for change_poll */);
} SSL_POLL_GROUP_FUNCS;

SSL_POLL_GROUP *SSL_POLL_GROUP_new(const SSL_POLL_GROUP_FUNCS *funcs,
                                     size_t funcs_len, size_t data_len);
void *SSL_POLL_GROUP_get0_data(const SSL_POLL_GROUP *self);

int SSL_POLL_GROUP_free(SSL_POLL_GROUP *self);
int SSL_POLL_GROUP_up_ref(SSL_POLL_GROUP *self);
int SSL_POLL_GROUP_change_poll(SSL_POLL_GROUP *self,
                               /* as shown for change_poll */);
```

Here is how an application might define and create a `SSL_POLL_METHOD` instance
of its own:

```c
struct app_poll_method_st {
    uint32_t refcount;
} APP_POLL_METHOD;

static int app_poll_method_free(SSL_POLL_METHOD *self)
{
    APP_POLL_METHOD *data = SSL_POLL_METHOD_get0_data(self);

    if (!--data->refcount)
        SSL_POLL_METHOD_do_free(self);

    return 1;
}

static int app_poll_method_up_ref(SSL_POLL_METHOD *self)
{
    APP_POLL_METHOD *data = SSL_POLL_METHOD_get0_data(self);

    ++data->refcount;

    return 1;
}

static uint64_t app_poll_method_get_caps(const SSL_POLL_METHOD *self)
{
    return SSL_POLL_METHOD_CAP_IMMEDIATE;
}

static int app_poll_method_supports_poll_descriptor(SSL_POLL_METHOD *self,
                                                    const BIO_POLL_DESCRIPTOR *d)
{
    return d->type == BIO_POLL_DESCRIPTOR_TYPE_SOCK_FD;
}

/* etc. */

SSL_POLL_METHOD *app_create_custom_poll_method(void)
{
    SSL_POLL_METHOD *self;
    APP_POLL_METHOD *data;

    static const SSL_POLL_METHOD_FUNCS funcs = {
        app_poll_method_free,
        app_poll_method_up_ref,
        app_poll_method_get_caps,
        app_poll_method_supports_poll_descriptor,
        app_poll_method_poll,
        NULL /* not supported by app */
    };

    self = SSL_POLL_METHOD_new(&funcs, sizeof(funcs), sizeof(APP_POLL_METHOD));
    if (self == NULL)
        return NULL;

    data = SSL_POLL_METHOD_get0_data(self);
    data->refcount = 1;
    return data;
}
```

We also provide a “default” method:

```c
BIO_POLL_METHOD *SSL_get0_default_poll_method(const OSSL_PARAM *params);
```

No params are currently defined; this is reserved for future use.

`SSL_poll` is a shorthand for using the method provided by
`SSL_get0_default_poll_method(NULL)`.

### Internal Polling: Usage within SSL Objects

To support custom pollers for internal polling, SSL objects receive an API that
allows a custom poller to be configured. To avoid confusion, custom pollers can
only be configured on an event leader, but the getter function will return the
custom poller configured on an event leader when called on any QUIC SSL object
in the hierarchy, or NULL if none is configured.

An `SSL_POLL_METHOD` can be associated with an SSL object. It can also be set
on a `SSL_CTX` object, in which case it is inherited by SSL objects created from
it:

```c
int SSL_CTX_set1_poll_method(SSL_CTX *ctx, SSL_POLL_METHOD *method);
SSL_POLL_METHOD *SSL_CTX_get0_poll_method(const SSL_CTX *ctx);

int SSL_set1_poll_method(SSL *ssl, SSL_POLL_METHOD *method);
SSL_POLL_METHOD *SSL_get0_poll_method(const SSL *ssl);
```

An SSL object created from a `SSL_CTX` which has never had
`SSL_set1_poll_method` called on it directly inherits the value set on the
`SSL_CTX`, including if the poll method set on the `SSL_CTX` is changed after
the SSL object is created. Calling `SSL_set1_poll_method(..., NULL)` overrides
this behaviour.

When a poll method is set on a QUIC domain, blocking API calls use that poller
to block as needed.

Our QUIC implementation may, if it wishes, use the provided poll method to
construct a poll group, but is not guaranteed to do so. We reserve the right to
use the immediate mode or retained mode API of the poller as desired. If we use
the retained mode, we handle state updates and teardown as needed if the caller
later changes the configured poll method by calling `SSL_set1_poll_method`
again.

If the poll method is set to NULL, we use the default poll method, which is the
same as the method provided by `SSL_get0_default_poll_method`.

Because the poll method provided is used to handle blocking on network I/O, a
poll method provided in this context only needs to handle OS socket handles,
similar to our own reactor polling in QUIC MVP.

### External Polling: Usage over SSL Objects

An application can also use an `SSL_POLL_METHOD` itself, whether via the
immediate or retained mode. In the latter case it creates one or more
`SSL_POLL_GROUP` instances.

Custom pollers are responsible for their own translation arrangements.
Retained-mode usage can be more efficient because it can allow recursive staging
of implementation-specific polling data. For example, suppose an application
enrolls a QCSO and two subsidiary QSSOs in a poll group. The reduction of these
three objects to a single pair of read/write BIO poll descriptors as provided by
an SSL object can be cached.

### Future Adaptation to Internal Pollable Resources

Suppose that in the future our QUIC implementation becomes more sophisticated
and we want to use a different kind of pollable resource to mask a more
elaborate internal reactor. For example, suppose for the sake of example we want
to switch to an internal thread-based reactor design, and signal readiness not
via an OS socket handle but via a condition variable or Linux-style `eventfd`.

Our design would hold up under these conditions as follows:

- For condition variables this would require a new poll descriptor type.
  Our default poller could be amended to support this new poll descriptor type.
  However, most OSes do not provide a way to simultaneously wait on a condition
  variable and other resources, so there are issues here unless an additional
  thread is used to adapt socket readiness to a condition variable.

- For something like `eventfd` things will work well with the existing `SOCK_FD`
  type. A QUIC SSL object simply starts returning an eventfd fd for
  `BIO_get_rpoll_descriptor` and this becomes readable when signalled by our
  internal engine. `BIO_get_wpoll_descriptor` works in the same way. (Of course
  a change on this level would probably require some sort of application
  opt-in via our API.)

- For something like Win32 Events, `WaitForSingleObject` or
  `WaitForMultipleObjects` works, but would require a new poll descriptor type.
  It is possible to plumb socket readiness into this API also, assuming Vista
  (WSAEventSelect).

Worked Examples
---------------

### Internal Polling — Default Poll Method

- Application creates a new QCSO
- Application does not set a custom poll method on it
- Application uses it in blocking mode and sets network BIOs
- Our QUIC implementation requests poll descriptors from the network BIOs
- Our QUIC implementation asks the default poller if it understands
  how to poll those poll descriptors. If not, blocking cannot be supported.
- When it needs to block, our QUIC implementation uses the default poll method
  in either immediate or retained mode based on the poll descriptors reported by
  the network BIOs provided

### Internal Polling — Custom Poll Method

- Application instantiates a custom poll method
- Application creates a new QCSO
- Application sets the custom poll method on the QCSO
- Application configures the QCSO for blocking mode and sets network BIOs
- Our QUIC implementation requests poll descriptors from the network BIOs
- Our QUIC implementation asks the custom poll method if it understands how to
- poll those poll descriptors. If not, blocking cannot be supported.
- When it needs to block, our QUIC implementation uses the custom poll method
  in either immediate or retained mode based on the poll descriptors reported
  by the network BIOs provided (internal polling)

### External Polling — Immediate Mode

- Application gets a poll method (default or custom)
- Application invokes poll() on the poll method on some number of QLSOs, QCSOs, QSSOs
  and OS sockets, etc.
- The poll method performs translation to a set of OS resources.
- The poll method asks the OS to poll/block.
- The poll method examines the results reported from the OS and performs reverse
  translation.
- The poll method poll() call reports the results and returns.

Note that custom poller methods configured on a SSL object are used for internal
polling (blocking API calls) only. Thus they have no effect on the above
scenario.

### External Polling — Retained Mode

- Application gets a poll method (default or custom)
- Application uses the poll method to create a poll group
- Application registers some number of QLSOs, QCSOs, QSSOs and OS sockets, etc.
  in the poll group.
- The poll group caches translations to a set of OS resources. It may create
  an OS device for fast polling (e.g. epoll) and register these resources
  with that method.
- Application polls using the poll group.
- The poll group asks the OS to poll/block.
- The poll group examines the results reported from the OS and performs reverse
  translation.
- The poll method reports the results and returns.

### External Polling — Immediate Mode Without Event Handling

- Application gets a poll method (default or custom)
- Application invokes poll() on the poll method on some number of QLSOs, QCSOs,
  and QSSOs with `NO_HANDLE_EVENTS` set.
- If the poll method is the default poll method, it knows how to examine
  QUIC SSL objects for readiness and does so.
- If the poll method is a custom poll method, it could choose to subdelegate
  this work to the default poll method, or implement it itself.

Change Notification Callback Mechanism
--------------------------------------

We propose to allow applications and libssl code to register callbacks for
lifecycle events on SSL objects, as discussed above. This can be used both by us
and by applications (e.g. to implement custom poller methods). The advantage
here is that an SSL object registered into a poll group can be automatically
unregistered from that poll group when it is freed.

The proposed API is as follows:

```c
/*
 * The SSL object is about to be freed (the refcount has reached zero).
 * The SSL object is still completely healthy until this call returns.
 * If the SSL object is reffed during a callback, the freeing is cancelled.
 * The callback then has full responsibility for its lifecycle.
 */
#define SSL_LIFECYCLE_EVENT_TYPE_PRE_FREE       1

/*
 * Either the read or write network BIO on an SSL object has just been changed,
 * or both. The fields in data.bio_change specify the old and new BIO pointers.
 * If a BIO reference is being set to NULL on an SSL object, the 'new' pointer
 * will be NULL; conversely, if a BIO is being set on an SSL object where
 * previously no BIO was set, the 'old' pointer will be NULL. If the applicable
 * flag (R or W) is not set, the old and new fields will be set to NULL.
 */
#define SSL_LIFECYCLE_EVENT_TYPE_BIO_CHANGE     2

#define SSL_LIFECYCLE_EVENT_FLAG_R              (1U << 0) /* read BIO changed */
#define SSL_LIFECYCLE_EVENT_FLAG_W              (1U << 1) /* write BIO changed */

typedef struct ssl_lifecycle_event_st SSL_LIFECYCLE_EVENT;
typedef struct ssl_lifecycle_cb_cookie_st *SSL_LIFECYCLE_CB_COOKIE;

/* Returns SSL_LIFECYCLE_EVENT_TYPE */
uint32_t SSL_LIFECYCLE_EVENT_get_type(const SSL_LIFECYCLE_EVENT *event);

/* Returns SSL_LIFECYCLE_EVENT_FLAG */
uint32_t SSL_LIFECYCLE_EVENT_get_flags(const SSL_LIFECYCLE_EVENT *event);

/* Returns an SSL object associated with the event (if applicable) */
SSL *SSL_LIFECYCLE_EVENT_get0_ssl(const SSL_LIFECYCLE_EVENT *event);

/*
 * For a BIO_CHANGE event, fills the passed pointers if non-NULL with the
 * applicable values. For other event types, fails.
 */
int SSL_LIFECYCLE_EVENT_get0_bios(const SSL_LIFECYCLE_EVENT *event,
                                  BIO **r_old, BIO **r_new,
                                  BIO **w_old, BIO **w_new);

/*
 * Register a lifecycle callback. Multiple lifecycle callbacks may be
 * registered. *cookie is written with an opaque value which may be used to
 * subsequently unregister the callback.
 */
int SSL_register_lifecycle_callback(SSL *ssl,
                                    void (*cb)(const SSL_LIFECYCLE_EVENT *event,
                                               void *arg),
                                    void *arg,
                                    SSL_LIFECYCLE_CB_COOKIE *cookie);

int SSL_unregister_lifecycle_callback(SSL *ssl, SSL_LIFECYCLE_CB_COOKIE cookie);
```

Q&A
---

**Q. How do we support poll methods which only support immediate mode?**

A. We simply have a fallback path for this when our QUIC implementation consumes
a custom poller. This is easy enough.

**Q. How do we support poll methods which only support retained mode?**

A. We intend to implement support for retained mode in our QUIC implementation's
internal blocking code, so this should also work OK. Remember that an external
poller method does not interact with an internal poller method (i.e., a method
set on an SSL object). In particular, no two poller methods ever interact
directly with one another. This avoids the need for recursive state shadowing
(where one poll method's retained mode API maintains state and also makes calls
to another poll method's retained mode API).

**Q. How does this design interact with hierarchical polling?**

A. We assume an application uses its own polling arrangements initially and then
uses calls to an OpenSSL external polling API (such as `SSL_poll` or a poll
method) to drill down into what is actually ready, as discussed above. There is
no issue here. An application can also use OpenSSL polling APIs instead of its
own, if desired; for example it could create a poll group from the default poll
method and use it to poll only network sockets, some of which may be from QUIC
SSL object poll descriptors, and then if needed call SSL_poll to narrow things
down once something becomes ready.

**Q. Should we support immediate and retained mode in the same API or segregate
these?**

A. They are in the same API, though we let applications use capability bits
to report support for only one of these if they wish.

**Q. How do we support extensibility of the poller interface?**

A. Using an extensible function table. An application can set a function
   pointer to NULL if it does not support it. Capability flags are used to
   advertise what is supported.

**Q. If an application sets a poll method on both an event leader and a poll
   group, what happens?**

A. Setting a poll method on an event leader provides a mechanism used for internal
blocking when making blocking calls. It is never used currently if no QUIC SSL
object in the QUIC domain isn't used in blocking mode (though this isn't a
contractual guarantee and we might do so in future for fast identification of
what we need to handle if we handle multiple OS-level sockets in future).

Setting a poll method on a poll group provides a mechanism used for polling
using that event group. Note that a custom poll method configured on a SSL
object is **not** used for the translation process performed by a poll group,
even when polling that SSL object. Translation is driven by
`SSL_get_[rw]poll_descriptor`.

**Q. What if different poll methods are configured on different event leaders
     (QUIC domains) and an application then tries to poll them all?**

A. Because the poll method configured on an event leader is ignored in favour of
the poll method directly invoked, there is no conflict here. The poll method
handles all polling when it is specifically invoked.

**Q. Where should the responsibility for poll descriptor translation lie?**

A. With the poll method or poll group being called at the time.

**Q. What method does `SSL_poll` use?**

A. It uses the default poll method. If an application wishes to use a different
poll method, it can call the `poll` method directly on that `BIO_POLL_METHOD`.

**Q. An application creates a poll group, registers an SSL object and later
changes the network BIOs set on that SSL object, or changes the poll descriptors
they return. What happens?**

A. This is solved with two design aspects:

- An application is not allowed to have the poll descriptors returned by a BIO
  change silently. If it wishes to change these, it must call `SSL_set_bio`
  again, even if with the same BIOs already set.

- We will need to either:

    - have a callback registration interface so retained mode pollers
      which have performed cached translation can be notified that a poll
      descriptor they have relied on is changing (proposed above).

    - require retained mode pollers to check for changes to translated objects
      (less efficient).

      This might cause issues with epoll because we don't have an opportunity
      to deregister an FD in this case.

  We choose the first option.

**Q. An application creates a poll group, registers a QCSO and some subsidiary
QSSOs and later frees all of these objects. What happens? (In other words, are
SSL objects auto-deregistered from poller groups?)**

A. We must assume a poll group retains an SSL object pointer if such an object
has been registered with it. Thus our options are either:

- require applications to deregister objects from any poll group they are using
  prior to freeing them; or

- add internal callback registration machinery to QUIC SSL objects so we can
  get a cleanup notification (see the above callback mechanism).

We choose the latter.

**Q. An application creates a poll group, registers a (non-QUIC-related) OS
socket handle and then closes it. What happens?**

Since OSes in general do not provide a way to get notified of these closures it
is not really possible to handle this automatically. It is essential that an
application deregisters the handle from the poll group first.

**Q. How does code using a poll method determine what poll descriptors that
method supports?**

A query method is provided which can be used to determine if the method supports
a given descriptor.

Windows support
---------------

Windows customarily poses a number of issues for supporting polling APIs. This
is largely because Windows chose an approach based around I/O *completion*
notification rather than around I/O *readiness* notification. While an implementation
of the Berkeley select(2)-style API is available, the options for higher
performance polling are largely confined to using I/O completion ports.

Because the semantics of I/O readiness and I/O completion are very different, it
has proven impossible in practice to create an I/O readiness API as an
abstraction over Windows's I/O completion API. The converse is not true; it is
fairly easy to create an I/O completion notification API over an I/O readiness
API.

It is therefore prudent to give some consideration to how Windows can be
supported:

1. We can always use `select` (or on Vista and later, `WSAPoll`).
   This may not actually be much of a problem as even in a server role, with QUIC
   we are likely to be handling a lot of clients on a relatively small number of
   OS sockets.

2. `WSAAsyncSelect` could be used with a helper thread. One thread could service
   multiple sockets, possibly even multiple poll groups.

3. `WSAEventSelect` allows a Win32 Event to be signalled on readiness,
   but this is not very useful because `WaitForMultipleObjects` is limited to 64
   objects (and even if it wasn't, poses the same issues as `select`, so back to
   where one started).

4. I/O Completion Ports are the “official” way to do high-performance I/O
   but notify on completion rather than readiness. It is impossible to build
   a poller API on top of this as such. As mentioned above, nobody has ever
   really managed to do so successfully.

5. `IOCTL_AFD_POLL`. This is an undocumented function of Winsock internals
   which allows a) epoll/kqueue-style interfaces to be built over Winsock, b)
   which are highly performant, like epoll/kqueue, and c) which use IOCPs to
   signal *readiness* rather than *completion*. In fact, this is what the
   `select` and `WSAPoll` functions use internally. Unlike those functions, this
   is based around registering sockets in advance and submits readiness
   notifications to an IOCP, so this can be quite performant.

   `IOCTL_AFD_POLL` is an internal, undocumented API. It is however widely used,
   and is now the basis of libuv (the I/O library used by Node.js), ZeroMQ, and
   Rust's entire asynchronous I/O ecosystem on Windows. In other words, while
   officially being undocumented and internal, it has in practice become widely
   used by third-party software, to the point where it cannot really be changed
   in future without breaking massive amounts of software. `IOCTL_AFD_POLL` has
   been around since at least NT 4 and is supported by Wine. Moreover it is
   worth noting that the reason why so many projects have resorted to using this
   API on Windows is due to the sheer lack of anything providing the appropriate
   functionality in the public API. The high level of reliance on this
   functionality in contemporary software doing asynchronous I/O does give
   reasonable confidence in using this API.

An immediate mode interface can be implemented using option 1.

Based on the above, options 1, 2 and 5 are viable for implementation of a
retained mode interface, with option 2 being a fairly substantial hack and
option 5 being the preferred approach for projects wanting an epoll/kqueue-style
model on Windows. The suggested approach is therefore to implement option 5,
though option 1 is also a viable fallback.

In any case, it appears the poller API as designed and proposed above
can be implemented adequately on Windows.

Extra features on QUIC objects
------------------------------

These are unlikely to be implemented initially — this is just some exploration
of features we might want to offer in future and how they would interact with
the polling design.

### Low-watermark functionality

Sometimes an application knows it does not need to do anything until at least N
bytes are available to read or write. In conventional Berkeley sockets APIs this
is known as “low-watermark” (LOWAT) functionality.

Rather than making polling interfaces more convoluted by adding fields to
polling-related structures, we propose to add a knob which can be configured on
an individual QUIC stream:

```c
#define SSL_LOWAT_FLAG_ONESHOT     (1U << 0)

int SSL_set_read_lowat(SSL *ssl, size_t lowat, uint64_t flags);
int SSL_get_read_lowat(SSL *ssl, size_t *lowat);

int SSL_set_write_lowat(SSL *ssl, size_t lowat, uint64_t flags);
int SSL_get_write_lowat(SSL *ssl, size_t *lowat);
```

If `ONESHOT` is set, the low-watermark condition is automatically cleared
after the next call to a read or write function respectively. The low-watermark
condition can also be cleared by passing a low-watermark of 0.

If low-watermark mode is configured, a poller will not report a stream as having
data ready to read, or room to write data, if the amount of room available is
less than the configured watermark.

### Timeouts

It is desirable to be able to cause blocking I/O operations to time out. For
example, an application might want to perform a blocking read from a peer but
only wait for a certain amount of time.

We support this with a configurable timeout per each type of operation.

```c
/* All operations - defined as separate bit for forward ABI compatibility */
#define SSL_OP_CLASS_ALL        (1U << 0)
/* The timeout concerns reads. */
#define SSL_OP_CLASS_R          (1U << 1)
/* The timeout concerns writes. */
#define SSL_OP_CLASS_W          (1U << 2)
/* The timeout concetns accepts. */
#define SSL_OP_CLASS_A          (1U << 3)
/* The timeout concerns new stream creation (which may be blocked on FC). */
#define SSL_OP_CLASS_N          (1U << 4)
/* The timeout concerns connects. */
#define SSL_OP_CLASS_C          (1U << 5)

/*
 * If set, t is a deadline (absolute time), otherwise it is a duration which
 * starts whenever an operation is commenced.
 */
#define SSL_TIMEOUT_FLAG_DEADLINE    (1U << 0)

/*
 * Configure a timeout for one or more operation types. At least one operation
 * type must be specified. If t is NULL, the timeout is unset for the given
 * operation. This may be called multiple times to set different timeouts
 * for different operations.
 */
int SSL_set_io_timeout(SSL *ssl, uint64_t operation,
                       const struct timeval *t, uint64_t flags);

/*
 * Retrieves a configured timeout value. operation must be a single operation
 * flag from SSL_OP_CLASS. If a timeout is configured for the operation
 * type, *is_set is written as 1 and *t is written with the configured timeout.
 * *flags is written with SSL_OP_CLASS_DEADLINE or 0 as applicable.
 * Otherwise, *is_set is written as 0, the value of *t is undefined and *flags
 * is set to 0. Returns 1 on success (including if unset) and 0 on failure (for
 * example if called on an unsupported SSL object type).
 */
int SSL_get_io_timeout(SSL *ssl, uint64_t operation,
                       struct timeval *t, int *is_set,
                       uint64_t *flags);

/*
 * Returns 1 if the last invocation of an applicable operation specified by
 * operation failed due to a timeout.
 *
 * For SSL_OP_CLASS_R, this means SSL_read or SSL_read_ex.
 * For SSL_OP_CLASS_W, this means SSL_write or SSL_write_ex.
 * For SSL_OP_CLASS_A, this means SSL_accept_stream.
 * For SSL_OP_CLASS_N, this means SSL_new_stream.
 * For SSL_OP_CLASS_C, this means SSL_do_handshake or any
 *   function which implicitly calls it, which includes any other I/O function
 *   if the connection process has not been completed yet.
 *
 * If a function is called in non-blocking mode and it cannot execute
 * immediately, this is considered to be a timeout. Therefore while timeouts are
 * not useful in non-blocking mode, this function can be used to determine if a
 * function failed because it would otherwise block.
 *
 * Invoking any operation of a given operation class clears the timeout flag
 * for that operation class regardless of the outcome of that operation.
 */
int SSL_timed_out(SSL *ssl, uint64_t operation);
```

We could consider adding a new `SSL_get_error` code also (`SSL_ERROR_TIMEOUT`).
There are no compatibility issues here because it will only be returned if an
application chooses to use the timeout functionality.

TODO: Check for duplicate existing APIs

TODO: Consider using ctrls

### Autotick control

We automatically engage in event handling when an I/O function such as
`SSL_read`, `SSL_write`, `SSL_accept_stream` or `SSL_new_stream` is called.
This is likely to be undesirable for applications in many circumstances,
so we should have a way to inhibit this.

```c
#define SSL_EVENT_FLAG_INHIBIT      (1U << 0)
#define SSL_EVENT_FLAG_INHIBIT_ONCE (1U << 1)

/*
 * operation is one or more SSL_OP_CLASS values. Inhibition can be enabled for a
 * single future call to an operation of that type (INHIBIT_ONCE), after which
 * it is disabled, or enabled persistently (INHIBIT).
 */
int SSL_set_event_flags(SSL *ssl, uint64_t operation, uint64_t flags);

/*
 * operation must specify a single operation. The flags configured are reported
 * in *flags.
 */
int SSL_get_event_flags(SSL *ssl, uint64_t operation, uint64_t *flags);
```

Autotick inhibition is only useful in non-blocking mode and it is ignored in
blocking mode. Using it in non-blocking mode carries the following implications:

- Data can be drained using `SSL_read` from existing buffers, but network I/O
  is not serviced and no new data will arrive (unless `SSL_handle_events` is
  called).

- Data can be placed into available write buffer space using `SSL_write`,
  but data will not be transmitted (unless `SSL_handle_events` is called).

- Likewise, no new incoming stream events will occur, and if calls to
  `SSL_new_stream` are currently blocked due to flow control, this
  situation will not change.

- `SSL_do_handshake` will simply report whether the handshake is done or not.
