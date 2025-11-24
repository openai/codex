QUIC API Overview
=================

This document sets out the objectives of the QUIC API design process, describes
the new and changed APIs, and the design constraints motivating those API
designs and the relevant design decisions.

- [QUIC API Overview](#quic-api-overview)
  * [Overview and Implementation Status](#overview-and-implementation-status)
  * [Objectives](#objectives)
  * [SSL Objects](#ssl-objects)
    + [Structure of Documentation](#structure-of-documentation)
    + [Existing APIs](#existing-apis)
      - [`SSL_set_connect_state`](#-ssl-set-connect-state-)
      - [`SSL_set_accept_state`](#-ssl-set-accept-state-)
      - [`SSL_is_server`](#-ssl-is-server-)
      - [`SSL_connect`](#-ssl-connect-)
      - [`SSL_accept`](#-ssl-accept-)
      - [`SSL_do_handshake`](#-ssl-do-handshake-)
      - [`SSL_read`, `SSL_read_ex`, `SSL_peek`, `SSL_peek_ex`](#-ssl-read----ssl-read-ex----ssl-peek----ssl-peek-ex-)
      - [`SSL_write`, `SSL_write_ex`](#-ssl-write----ssl-write-ex-)
      - [`SSL_pending`](#-ssl-pending-)
      - [`SSL_has_pending`](#-ssl-has-pending-)
      - [`SSL_shutdown`](#-ssl-shutdown-)
      - [`SSL_clear`](#-ssl-clear-)
      - [`SSL_free`](#-ssl-free-)
      - [`SSL_set0_rbio`, `SSL_set0_wbio`, `SSL_set_bio`](#-ssl-set0-rbio----ssl-set0-wbio----ssl-set-bio-)
      - [`SSL_set_[rw]fd`](#-ssl-set--rw-fd-)
      - [`SSL_get_[rw]fd`](#-ssl-get--rw-fd-)
      - [`SSL_CTRL_MODE`, `SSL_CTRL_CLEAR_MODE`](#-ssl-ctrl-mode----ssl-ctrl-clear-mode-)
      - [SSL Modes](#ssl-modes)
    + [New APIs for Single-Stream Operation](#new-apis-for-single-stream-operation)
      - [`SSL_handle_events`](#-ssl-handle-events-)
      - [`SSL_get_event_timeout`](#-ssl-get-event-timeout-)
      - [`SSL_set_blocking_mode`, `SSL_get_blocking_mode`](#-ssl-set-blocking-mode----ssl-get-blocking-mode-)
      - [`SSL_get_rpoll_descriptor`, `SSL_get_wpoll_descriptor`](#-ssl-get-rpoll-descriptor----ssl-get-wpoll-descriptor-)
      - [`SSL_net_read_desired`, `SSL_net_write_desired`](#-ssl-want-net-read----ssl-want-net-write-)
      - [`SSL_want`, `SSL_want_read`, `SSL_want_write`](#-ssl-want----ssl-want-read----ssl-want-write-)
      - [`SSL_set1_initial_peer_addr`](#-ssl-set-initial-peer-addr-)
      - [`SSL_shutdown_ex`](#-ssl-shutdown-ex-)
      - [`SSL_stream_conclude`](#-ssl-stream-conclude-)
      - [`SSL_stream_reset`](#-ssl-stream-reset-)
      - [`SSL_get_stream_state`](#-ssl-get-stream-state-)
      - [`SSL_get_stream_read_error_code`, `SSL_get_stream_write_error_code`](#-ssl-get-stream-read-error-code----ssl-get-stream-write-error-code-)
      - [`SSL_get_conn_close_info`](#-ssl-get-conn-close-info-)
    + [New APIs for Multi-Stream Operation](#new-apis-for-multi-stream-operation)
      - [Notes on Multi-Threaded Operation](#notes-on-multi-threaded-operation)
      - [Notes on Blocking](#notes-on-blocking)
      - [Notes on Application-Level Polling](#notes-on-application-level-polling)
      - [`SSL_get0_connection`](#-ssl-get0-connection-)
      - [`SSL_is_connection`](#-ssl-is-connection-)
      - [`SSL_get_stream_type`](#-ssl-get-stream-type-)
      - [`SSL_get_stream_id`](#-ssl-get-stream-id-)
      - [`SSL_is_stream_local`](#-ssl-is-stream-local-)
      - [`SSL_new_stream`](#-ssl-new-stream-)
      - [`SSL_accept_stream`](#-ssl-accept-stream-)
      - [`SSL_get_accept_stream_queue_len`](#-ssl-get-accept-stream-queue-len-)
      - [`SSL_set_incoming_stream_policy`](#-ssl-set-incoming-stream-policy-)
      - [`SSL_set_default_stream_mode`](#-ssl-set-default-stream-mode-)
    + [Future APIs](#future-apis)
  * [BIO Objects](#bio-objects)
    + [Existing APIs](#existing-apis-1)
      - [`BIO_s_connect`, `BIO_new_ssl_connect`, `BIO_set_conn_hostname`](#-bio-s-connect----bio-new-ssl-connect----bio-set-conn-hostname-)
      - [`BIO_new_bio_pair`](#-bio-new-bio-pair-)
      - [Interactions with `BIO_f_buffer`](#interactions-with--bio-f-buffer-)
      - [MTU Signalling](#mtu-signalling)
    + [New APIs](#new-apis)
      - [`BIO_sendmmsg` and `BIO_recvmmsg`](#-bio-sendmmsg--and--bio-recvmmsg-)
      - [Truncation Mode](#truncation-mode)
      - [Capability Negotiation](#capability-negotiation)
      - [Local Address Support](#local-address-support)
      - [`BIO_s_dgram_pair`](#-bio-s-dgram-pair-)
      - [`BIO_POLL_DESCRIPTOR`](#-bio-poll-descriptor-)
      - [`BIO_s_dgram_mem`](#-bio-s-dgram-mem-)
      - [`BIO_err_is_non_fatal`](#-bio-err-is-non-fatal-)
  * [Q & A](#q---a)

Overview and Implementation Status
----------------------------------

A listing of all SSL object APIs and their implications for QUIC, including
current implementation status, can be found in
[quic-api-ssl-funcs.md](./quic-api-ssl-funcs.md).

Non-SSL object APIs which are new or changed, or otherwise discussed in this
document are listed below, along with their implementation status. SSL object
APIs are not listed here; see [quic-api-ssl-funcs.md](./quic-api-ssl-funcs.md)
for details on SSL object APIs.

| Semantics | API                             | Status |
|-----------|---------------------------------|--------|
| Changed   | `BIO_s_connect`                 | Done  |
| Unchanged | `BIO_set_conn_hostname`         | Done   |
| N/A       | `BIO_new_bio_pair`              | N/A (see `BIO_new_bio_dgram_pair`)   |
| New       | `BIO_s_dgram_pair`              | Done   |
| Unchanged | `BIO_dgram_get_mtu`             | Done   |
| Unchanged | `BIO_dgram_set_mtu`             | Done   |
| New       | `BIO_sendmmsg`                  | Done   |
| New       | `BIO_recvmmsg`                  | Done   |
| New       | `BIO_dgram_set_no_trunc`        | Done   |
| New       | `BIO_dgram_get_no_trunc`        | Done   |
| New       | `BIO_dgram_set_caps`            | Done   |
| New       | `BIO_dgram_get_caps`            | Done   |
| New       | `BIO_dgram_get_effective_caps`  | Done   |
| New       | `BIO_dgram_get_local_addr_cap`  | Done   |
| New       | `BIO_dgram_set_local_addr_enable` | Done  |
| New       | `BIO_dgram_get_local_addr_enable` | Done  |
| New       | `BIO_get_rpoll_descriptor`      | Done   |
| New       | `BIO_get_wpoll_descriptor`      | Done   |
| New       | `BIO_err_is_non_fatal`          | Done   |

Objectives
----------

The objectives of the QUIC API design are:

- to provide an API suitable for use with QUIC, now and in the future;

- to reuse the existing libssl APIs to the extent feasible;

- to enable existing applications to adapt to using QUIC with only
  minimal API changes.

SSL Objects
-----------

### Structure of Documentation

Each API listed below has an information table with the following fields:

- **Semantics**: This can be one of:

    - **Unchanged**: The semantics of this existing libssl API call are
      unchanged.
    - **Changed**: The semantics are changed for QUIC.
    - **New**: The API is new for QUIC.

- `SSL_get_error`: Can this API, when used with QUIC, change the
  state returned by `SSL_get_error`? This can be any combination of:

    - **Never**: Does not interact with `SSL_get_error`.
    - **Error**: Non-`WANT_READ`/`WANT_WRITE` errors can be raised.
    - **Want**: `WANT_READ`/`WANT_WRITE` can be raised.

- **Can Tick?**: Whether this function is allowed to perform event processing
  for the QUIC state machine and potentially perform network I/O.

- **CSHL:** Connection/Stream/Handshake Layer classification.
  This can be one of:

    - **HL:** This is a handshake layer related call. It should be supported
      on a QUIC connection SSL object, forwarding to the handshake layer
      SSL object. QUIC stream SSL objects do not allow these calls to be
      forwarded.

    - **HL-Forbidden:** This is a handshake layer related call, but it is
      inapplicable to QUIC, so it is not supported.

    - **C:** Not handshake-layer related. QUIC connection SSL object usage only.
      Fails on a QUIC stream SSL object.

    - **CS:** Not handshake-layer related. Can be used on any QUIC SSL object.

    - **S**: Requires a QUIC stream SSL object or a QUIC connection SSL object
      with a default stream attached.

### Existing APIs

#### `SSL_set_connect_state`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| Unchanged | Never         | No        | HL            |

#### `SSL_set_accept_state`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| Unchanged | Never         | No        | HL            |

**Note:** Attempting to proceed in this state will not function for now because
we do not implement server support at this time. However, the semantics of this
function as such are unchanged.

#### `SSL_is_server`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| Unchanged | Never         | No        | HL            |

#### `SSL_connect`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| Unchanged | Error/Want    | Yes       | HL            |

Simple composition of `SSL_set_connect_state` and `SSL_do_handshake`.

#### `SSL_accept`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| Unchanged | Error/Want    | Yes       | HL            |

Simple composition of `SSL_set_accept_state` and `SSL_do_handshake`.

#### `SSL_do_handshake`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| Unchanged | Error/Want    | Yes       | HL            |

**Note:** Idempotent if handshake already completed.

**Blocking Considerations:** Blocks until handshake completed if in blocking
mode.

#### `SSL_read`, `SSL_read_ex`, `SSL_peek`, `SSL_peek_ex`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| Unchanged | Error/Want    | Yes       | CS            |

**Blocking Considerations:** Blocks until at least one byte is available or an
error occurs if in blocking mode (including the peek functions).

If the read part of the stream has been finished by the peer, calls to
`SSL_read` will fail with `SSL_ERROR_ZERO_RETURN`.

If a stream has terminated in a non-normal fashion (for example because the
stream has been reset, or the connection has terminated), calls to `SSL_read`
will fail with `SSL_ERROR_SSL`.

`SSL_get_stream_read_state` can be used to clarify the stream state when an
error occurs.

#### `SSL_write`, `SSL_write_ex`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| Unchanged | Error/Want    | Yes       | CS            |

We have to implement all of the following modes:

- `SSL_MODE_ENABLE_PARTIAL_WRITE` on or off
- `SSL_MODE_ACCEPT_MOVING_WRITE_BUFFER` on or off
- Blocking mode on or off

**Blocking Considerations:** Blocks until libssl has accepted responsibility for
(i.e., copied) all data provided, or an error occurs, if in blocking mode. In
other words, it blocks until it can buffer the data. This does not necessarily
mean that the data has actually been sent.

`SSL_get_stream_write_state` can be used to clarify the stream state when an
error occurs.

#### `SSL_pending`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| Unchanged | Never         | No        | CS            |

#### `SSL_has_pending`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| Unchanged | Never         | No        | CS            |

**TBD.** Options:

  - Semantics unchanged or approximated (essentially, `SSL_pending() || any RXE
    queued || any URXE queued`).
  - Change semantics to only determine the return value based on if there is
    data in the stream receive buffer.

#### `SSL_shutdown`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| Unchanged | Error         | Yes       | CS            |

See `SSL_shutdown_ex` below for discussion of how this will work for QUIC.

Calling `SSL_shutdown` is always exactly identical in function to calling
`SSL_shutdown_ex` with `flags` set to 0 and `args` set to `NULL`.

#### `SSL_clear`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| TBD       | TBD           | No        | C             |

There are potential implementation hazards:

>SSL_clear() resets the SSL object to allow for another connection. The reset
>operation however keeps several settings of the last sessions (some of these
>settings were made automatically during the last handshake). It only makes sense
>for a new connection with the exact same peer that shares these settings, and
>may fail if that peer changes its settings between connections.

**TBD:** How should `SSL_clear` be implemented? Either:

  - Modernised implementation which resets everything, handshake layer
    re-instantiated (safer);
  - Preserve `SSL_clear` semantics at the handshake layer, reset all QUIC state
    (`QUIC_CHANNEL` torn down, CSM reset).

**TBD:** Semantics of this on stream objects.

#### `SSL_free`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| Changed   | Never         | No        | CS            |

**QUIC stream SSL objects.** When used on a QUIC stream SSL object, parts of the
stream state may continue to exist internally, managed inside the QUIC
connection SSL object, until they can be correctly torn down, or until the QUIC
connection SSL object is freed.

If a QUIC stream SSL object is freed for a stream which has not reached a
terminal state for all of its parts (both send and receive, as applicable), the
stream is automatically reset (non-normal termination) with an application error
code of 0. To explicitly reset a stream with a different application error code,
call `SSL_stream_reset` before calling this function.

If the peer continues to send data on the stream before it processes the
notification of the stream's termination, that incoming data will be discarded.
However, the peer will be reliably notified of the non-normal termination of the
stream assuming that the connection remains healthy.

When freeing a QUIC stream SSL object which was terminated in a non-normal
fashion, or which was terminated automatically due to a call to this function,
any data which was appended to the stream via `SSL_write` may or may not have
already been transmitted, and even if already transmitted, may or may not be
retransmitted in the event of loss.

When freeing a QUIC stream SSL object which was terminated normally (for example
via `SSL_stream_conclude`), data appended to the stream via `SSL_write` will
still be transmitted or retransmitted as necessary, assuming that the QUIC
connection SSL object is not freed and that the connection remains healthy.

**QUIC connection SSL objects.** `SSL_free` is largely unchanged for QUIC
connection SSL objects on the client side. When freeing a QUIC connection SSL
object being used in client mode, there is immediate termination of any QUIC
network I/O processing as the resources needed to handle the connection are
immediately freed. This means that, if a QUIC connection SSL object which has
not been shutdown properly is freed using this function:

- Any data which was pending transmission or retransmission will not be
  transmitted, including in streams which were terminated normally;

- The connection closure process will not function correctly or in an
  RFC-compliant manner. Connection closure will not be signalled to the peer
  and the connection will simply disappear from the perspective of the peer. The
  connection will appear to remain active until the connection's idle timeout
  (if negotiated) takes effect.

  For further discussion of this issue, see `SSL_shutdown_ex` and the Q&A.

#### `SSL_set0_rbio`, `SSL_set0_wbio`, `SSL_set_bio`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| Changed   | Never         | No        | C             |

Sets network-side BIO.

The changes to the semantics of these calls are as follows:

  - The BIO MUST be a BIO with datagram semantics (this is a change relative to
    TLS, though not to DTLS).

  - If the BIO is non-pollable (see below), application-level blocking mode will
    be forced off.

#### `SSL_set_[rw]fd`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| Changed   | Never         | No        | C             |

Sets network-side socket FD.

Existing behaviour: Instantiates a `BIO_s_socket`, sets an FD on it, and sets it
as the BIO.

New proposed behaviour:

- Instantiate a `BIO_s_dgram` instead for a QUIC connection SSL object.
- Fails (no-op) for a QUIC stream SSL object.

#### `SSL_get_[rw]fd`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| Unchanged | Never         | No        | C             |

Should not require any changes.

#### `SSL_CTRL_MODE`, `SSL_CTRL_CLEAR_MODE`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| Unchanged | Never         | No        | CS            |

#### SSL Modes

- `SSL_MODE_ENABLE_PARTIAL_WRITE`: Implemented. If this mode is set during a
  non-partial-write `SSL_write` operation spanning multiple `SSL_write` calls,
  this mode does not take effect until the non-partial write operation is
  completed.

- `SSL_MODE_ACCEPT_MOVING_WRITE_BUFFER`: Implemented.

- `SSL_MODE_AUTO_RETRY`: TBD.

- `SSL_MODE_RELEASE_BUFFERS`: Ignored. This is an optimization and if it has
  any sensible semantic correspondence to QUIC, this can be considered later.

- `SSL_MODE_SEND_FALLBACK_SCSV`: TBD: Either ignore or fail if the client
  attempts to set this prior to handshake. The latter is probably safer.

  Ignored if set after handshake (existing behaviour).

- `SSL_MODE_ASYNC`: TBD.

### New APIs for Single-Stream Operation

#### `SSL_handle_events`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| New       | Never         | Yes       | CS            |

Advances the QUIC state machine to the extent feasible, potentially performing
network I/O. Also compatible with DTLSv1 and supersedes `DTLSv1_handle_timeout`
for all use cases.

#### `SSL_get_event_timeout`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| New       | Never         | No        | CS            |

Gets the time until the QUIC state machine next wants to receive a timeout
event, if any.

This is similar to the existing `DTLSv1_get_timeout` function, but it is not
specific to DTLSv1. It is also usable for DTLSv1 and can become a
protocol-agnostic API for this purpose, superseding `DTLSv1_get_timeout` for all
use cases.

The design is similar to that of `DTLSv1_get_timeout` and uses a `struct
timeval`. However, this function can also output an infinite timeout using the
`is_infinite` argument, whereas whereas `DTLSv1_get_timeout` represents an
infinite timeout using a 0 return value, which does not allow a failure
condition to be distinguished.

#### `SSL_set_blocking_mode`, `SSL_get_blocking_mode`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| New       | Never         | No        | CS            |

Turns blocking mode on or off. This is necessary because up until now libssl has
operated in blocking or non-blocking mode automatically as an emergent
consequence of whether the underlying network socket is blocking. For QUIC, this
is no longer viable, thus blocking semantics at the application level must be
explicitly configured.

Use on stream objects: It may be feasible to implement this such that different
QUIC stream SSL objects can have different settings for this option.

Not supported for non-QUIC SSL objects.

#### `SSL_get_rpoll_descriptor`, `SSL_get_wpoll_descriptor`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| New       | Never         | No        | CS            |

These functions output poll descriptors which can be used to determine when the
QUIC state machine next needs to have events handled. `SSL_get_rpoll_descriptor`
is relevant if `SSL_net_read_desired` returns 1, and `SSL_get_wpoll_descriptor`
is relevant if `SSL_net_write_desired` returns 1.

The implementation of these functions is a simple forward to
`BIO_get_rpoll_descriptor` and `BIO_get_wpoll_descriptor` on the underlying
network BIOs.

TODO: Support these for non-QUIC SSL objects

#### `SSL_net_read_desired`, `SSL_net_write_desired`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| New       | Never         | No        | CS            |

These calls return 1 if the QUIC state machine is interested in receiving
further data from the network, or writing to the network, respectively. The
return values of these calls should be used to determine which wakeup events
should cause an application to call `SSL_handle_events`. These functions do not
mutate any state, and their return values may change after a call to any SSL
function other than `SSL_net_read_desired`, `SSL_net_write_desired`,
`SSL_get_rpoll_descriptor`, `SSL_get_wpoll_descriptor` and
`SSL_get_event_timeout`.

TODO: Support these for non-QUIC SSL objects, turning this into a unified
replacement for `SSL_want`

#### `SSL_want`, `SSL_want_read`, `SSL_want_write`

The existing API `SSL_want`, and the macros defined in terms of it, are
traditionally used to determine if the SSL state machine has exited in
non-blocking mode due to a desire to read from or write to the underlying
network BIO. However, this API is unsuitable for use with QUIC because the
return value of `SSL_want` can only express one I/O direction at a time (read or
write), not both. This call will not be implemented for QUIC (e.g. always
returns `SSL_NOTHING`) and `SSL_net_read_desired` and `SSL_net_write_desired`
will be used instead.

#### `SSL_set1_initial_peer_addr`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| New       | Never         | No        | CS            |

`SSL_set1_initial_peer_addr` sets the initial L4 UDP peer address for an outgoing
QUIC connection.

The initial peer address may be autodetected if no peer address has already been
set explicitly and the QUIC connection SSL object is provided with a
`BIO_s_dgram` with a peer set.

`SSL_set1_initial_peer_addr` cannot be called after a connection is established.

#### `SSL_shutdown_ex`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| New       | Error         | Yes       | C             |

```c
typedef struct  ssl_shutdown_ex_args_st {
    /* These arguments pertain only to QUIC connections. */
    uint64_t    quic_error_code; /* [0, 2**62-1] */
    const char *quic_reason;
} SSL_SHUTDOWN_EX_ARGS;

#define SSL_SHUTDOWN_FLAG_RAPID         (1U << 0)
#define SSL_SHUTDOWN_FLAG_IMMEDIATE     (1U << 1)

int SSL_shutdown_ex(SSL *ssl,
                    uint64_t flags,
                    const SSL_SHUTDOWN_EX_ARGS *args,
                    size_t args_len);
```

`SSL_shutdown_ex` is an extended version of `SSL_shutdown`.

`args` specifies arguments which control how the SSL object is shut down. `args`
are read only on the first call to `SSL_shutdown_ex` for a given SSL object and
subsequent calls to `SSL_shutdown_ex` ignore the `args` argument. `args_len`
should be set to `sizeof(*args)`. This function is idempotent; once the shutdown
process for an SSL object is complete, further calls are a no-op and return 1.

Calling `SSL_shutdown_ex` on a QUIC connection SSL object causes the immediate
close of the QUIC connection. “Immediate close” is as defined by RFC 9000.

If no QUIC connection attempt was ever initiated using the given SSL object, the
QUIC connection transitions immediately to the Terminated state. Otherwise, the
connection closure process is initiated if it has not already begun.

Any application stream data on a non-terminated or normally terminated stream
which has yet to be transmitted is flushed to the network before the termination
process begins. This ensures that where an application which calls `SSL_write`
and performs a connection closure in a way which is considered normal to the
application protocol being used, all of the data written is delivered to the
peer. This behaviour may be skipped by setting the `SSL_SHUTDOWN_FLAG_IMMEDIATE`
flag, in which case any data appended to streams via `SSL_write` (or any
end-of-stream conditions) may not be transmitted to the peer. This flag may be
useful where a non-normal application condition has occurred and the delivery of
data written to streams via `SSL_write` is no longer relevant. Application
stream data on streams which were terminated non-normally (for example via
`SSL_stream_reset`) is not transmitted by this function.

A QUIC connection can be shut down using this function in two different ways:

- **RFC compliant mode.** In this mode, which provides the most robust
  operation, the shutdown process may take a period of time up to three times
  the current estimated RTT to the peer. It is possible for the closure process
  to complete much faster in some circumstances but this cannot be relied upon.

  In blocking mode, the function will return once the closure process is
  complete. In non-blocking mode, `SSL_shutdown_ex` should be called until it
  returns 1, indicating the closure process is complete and the connection is
  now terminated.

- **Rapid mode.** In this mode, a `CONNECTION_CLOSE` frame is sent in a
  best-effort manner and the connection is terminated immediately. If the
  `CONNECTION_CLOSE` frame sent is lost, the peer will not know that the
  connection has terminated until the negotiated idle timeout (if any) expires.

  This will generally return 0 on success, indicating that the connection has
  not yet reached the Terminating state (unless it has already done so, in which
  case it will return 1).

  In blocking mode, this blocks until at least one `CONNECTION_CLOSE` frame is
  sent but does not otherwise block. In non-blocking mode, this should be called
  until it returns a non-negative value. A negative value indicates failure or
  an I/O would-block condition.

It is permissible for an application to implement a hybrid approach, for example
by initiating a rapid or non-blocking shutdown and continuing to call
`SSL_handle_events` for a duration it chooses.

If `SSL_SHUTDOWN_FLAG_RAPID` is specified in `flags`, a rapid shutdown is
performed, otherwise an RFC-compliant shutdown is performed. The principal
effect of this flag is to partially disable blocking behaviour in blocking mode,
and the QUIC implementation will still attempt to implement the Terminating
state semantics if the application happens to call `SSL_handle_events`, until it
reaches the Terminated state or is freed. An application can change its mind
about performing a rapid shutdown by making a subsequent call to
`SSL_shutdown_ex` without the flag set.

Calling `SSL_shutdown_ex` on a QUIC stream SSL object is not valid; such a call
will fail and has no effect. The rationale for this is that an application may
well want to pass around SSL objects for individual QUIC streams to existing
parts of its own code which expect something which behaves like a typical SSL
object (i.e., a single bytestream); those components may well already call
`SSL_shutdown` and it is not desired for such calls to affect the whole
connection.

The `args->quic_error_code` and `args->reason` fields allow the application
error code and reason string for the closure of a QUIC connection to be
specified. If `args` or `args->reason` is `NULL`, a zero-length string is used
for the reason. If `args` is `NULL`, an error code of 0 is used.
`args->quic_error_code` must be in the range `[0, 2**62-1]`, else this function
fails. These fields are ignored for SSL objects which do not represent QUIC
connections.

#### `SSL_stream_conclude`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| New       | Error         | Yes       | S             |

```c
int SSL_stream_conclude(SSL *ssl, uint64_t flags);
```

`SSL_stream_conclude` signals the normal end-of-stream condition to the send
part of a QUIC stream. If called on a QUIC connection SSL object with a default
stream, it signals the end of that stream to the peer. If called on a QUIC
stream SSL object, it signals the end of that stream to the peer.

This function may only be called for bidirectional streams and for outgoing
unidirectional streams. It is a no-op if it has already been called for a given
stream, or if either the stream or connection have entered an error state.

Any data already queued for transmission via a call to `SSL_write()` will still
be written in a reliable manner before the end-of-stream is signalled, assuming
the connection remains healthy. This function can be thought of as appending a
logical end-of-stream marker after any data which has previously been written to
the stream via calls to `SSL_write`. Further attempts to call `SSL_write` after
calling this function will fail.

When calling this on a bidirectional stream, the receive part of the stream
remains unaffected, and the peer may continue to send data via it until the peer
also signals the end of the stream. Thus, `SSL_read()` can still be used.

This function is used to conclude the send part of a stream in a normal manner.
To perform non-normal termination of both the sending and receiving parts of a
stream, see `SSL_stream_reset`.

`flags` is reserved and should be set to 0.

#### `SSL_stream_reset`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| New       | Error         | Yes       | S             |

```c
typedef struct ssl_stream_reset_args_st {
    uint64_t quic_error_code; /* [0, 2**62-1] */
} SSL_STREAM_RESET_ARGS;

int SSL_stream_reset(SSL *ssl,
                     const SSL_STREAM_RESET_ARGS *args,
                     size_t args_len);
```

Conducts a non-normal termination of a bidirectional or outgoing unidirectional
stream. For QUIC, this corresponds to a stream reset using a `RESET_STREAM`
frame.

It may be called on either a QUIC stream SSL object or a QUIC connection SSL
object with a default stream; the given stream is reset. The QUIC connection is
not affected.

For bidirectional streams, this terminates both sending and receiving parts of
the stream. It may not be called on an incoming unidirectional stream.

If `args` is `NULL`, an application error code of 0 is used. Otherwise, the
application error code to use is specified in `args->quic_error_code`, which
must be in the range `[0, 2**62-1]`. `args_len` must be set to `sizeof(*args)`
if `args` is non-NULL.

Only the first call to this function has any effect; subsequent calls are
no-ops. This is considered a success case.

#### `SSL_get_stream_state`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| New       | Never         | No        | S             |

```c
/*
 * e.g. Non-QUIC SSL object, or QUIC connection SSL object without a default
 * stream.
 */
#define SSL_STREAM_STATE_NONE                   0

/*
 * The read or write part of the stream is still available and has not been
 * terminated in a normal or non-normal manner.
 */
#define SSL_STREAM_STATE_OK                     1

/*
 * The stream is a unidirectional stream and this direction cannot be used; for
 * example, a remotely initiated unidirectional stream where
 * SSL_get_stream_write_state is called, or a locally initiated unidirectional
 * stream where SSL_get_stream_read_state is
 called.
 */
#define SSL_STREAM_STATE_WRONG_DIR              2

/*
 * The read or write part of the stream has been finished in a normal manner.
 *
 * For SSL_get_stream_read_state, this means that there is no more data to read,
 * and that any future SSL_read calls will return any residual data waiting to
 * be read followed by an SSL_ERROR_ZERO_RETURN condition.
 *
 * For SSL_get_stream_write_state, this means that the local application has
 * already indicated the end of the stream by calling SSL_stream_conclude,
 * and that future calls to SSL_write will fail.
 */
#define SSL_STREAM_STATE_FINISHED               3

/*
 * The stream was reset by the local party.
 *
 * For SSL_get_stream_read_state, this means that the stream was aborted using a
 * locally transmitted STOP_SENDING frame. Attempts to read from the stream via
 * SSL_read will fail, though SSL_read may allow any  residual data waiting to
 * be  read to be  read first.
 *
 * For SSL_get_stream_write_state, this means that the stream was aborted
 * using a locally transmitted RESET_STREAM frame. Attempts to write to
 * the stream will fail.
 */
#define SSL_STREAM_STATE_RESET_LOCAL            4

/*
 * The stream was reset by the remote party.
 *
 * For SSL_get_stream_read_state, this means the peer sent a STREAM_RESET
 * frame for the stream.
 *
 * For SSL_get_stream_write_state, this means the peer sent a STOP_SENDING
 * frame for the stream.
 */
#define SSL_STREAM_STATE_RESET_REMOTE           5

/*
 * The underlying connection supporting the stream has closed or otherwise
 * failed.
 *
 * For SSL_get_stream_read_state, this means that attempts to read from the
 * stream via SSL_read will fail, though SSL_read may allow any residual
 * data waiting to be read to be read first.
 *
 * For SSL_get_stream_write_state, this means that attempts to write to the
 * stream will fail.
 */
#define SSL_STREAM_STATE_CONN_CLOSED            6

int SSL_get_stream_read_state(SSL *ssl);
int SSL_get_stream_write_state(SSL *ssl);
```

This API allows the current state of a stream to be queried. This allows an
application to determine whether a stream is still usable and why a stream has
reached an error state.

#### `SSL_get_stream_read_error_code`, `SSL_get_stream_write_error_code`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| New       | Never         | No        | S             |

```c
int SSL_get_stream_read_error_code(SSL *ssl, uint64_t *app_error_code);
int SSL_get_stream_write_error_code(SSL *ssl, uint64_t *app_error_code);
```

`SSL_get_stream_read_error_code` gets the error code for the read part of the
stream.

`SSL_get_stream_write_error_code` gets the error code for the write part of
the stream.

If a stream has been terminated normally, returns 0.

If a stream has been terminated non-normally, returns 1 and writes the
applicable application error code to `*app_error_code`.

If a stream is still healthy, or was healthy at the time the connection was
closed, or the respective part of the stream does not exist (e.g. for a
unidirectional stream), returns -1.

#### `SSL_get_conn_close_info`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| New       | Never         | No        | C             |

```c
#define SSL_CONN_CLOSE_FLAG_LOCAL
#define SSL_CONN_CLOSE_FLAG_TRANSPORT

typedef struct ssl_conn_close_info_st {
    uint64_t error_code;
    char     *reason;
    size_t   reason_len;
    uint32_t flags;
} SSL_CONN_CLOSE_INFO;

int SSL_get_conn_close_info(SSL *ssl,
                            SSL_CONN_CLOSE_INFO *info,
                            size_t info_len);
```

If a connection is still healthy, returns 0. Otherwise, fills `*info` with
information about the error causing connection termination and returns 1.
`info_len` must be set to `sizeof(*info)`. Returns -1 if called on a non-QUIC
SSL object or if the connection status cannot be determined.

`info->reason` is set to point to a buffer containing a reason string. The
buffer is valid for the lifetime of the SSL object. The reason string will
always be zero terminated, but since it is received from a potentially untrusted
peer, may also contain zero bytes. `info->reason_len` is the true length of the
reason string in bytes.

`info->flags` has `SSL_CONN_CLOSE_FLAG_LOCAL` set if the connection closure was
locally initiated.

`info->flags` has `SSL_CONN_CLOSE_FLAG_TRANSPORT` if the connection closure was
initiated by QUIC, and 0 if it was initiated by the application. The namespace
of `info->error_code` is determined by this parameter.

### New APIs for Multi-Stream Operation

The above new APIs are built on constructively to facilitate multi-stream
operation.

The concept of a QUIC stream SSL object is introduced. A QUIC SSL object is
either a QUIC connection SSL object or a QUIC stream SSL object. A QUIC stream
SSL object belongs to a QUIC connection SSL object. A QUIC connection SSL object
may or may not have an associated default stream. There may only be at most one
default stream for a QUIC connection SSL object. Reading or writing application
data to a QUIC connection SSL object with a default stream is equivalent to
reading or writing to that stream. It is an error to attempt to read or write
application data, or perform other stream-specific operations, on a QUIC
connection SSL object without a default stream associated.

#### Notes on Multi-Threaded Operation

Initially these APIs will not be thread safe over the same connection, but in
the longer term we intend to support multiple threads using different QUIC
stream SSL objects on different threads over the same connection without the
application having to do any locking. This is referred to as multi-stream
multi-thread (MSMT) operation. Only APIs explicitly denoted below will
eventually be MSMT-safe.

#### Notes on Blocking

The blocking mode can be configured on each SSL object individually. When a QUIC
stream SSL object is created it inherits its blocking state from the currently
configured blocking state of the QUIC connection SSL object at the time the
stream is created. This can be changed independently. For example, a QUIC
connection SSL object can be in blocking mode to allow for blocking
`SSL_accept_stream` calls, yet have some or all QUIC stream SSL objects be in
non-blocking mode concurrently.

#### Notes on Application-Level Polling

An API may be added in the future to allow applications to poll multiple QUIC
connection SSL objects efficiently for new stream and stream readability events.
This is not yet urgent but will be more relevant for concurrent server
applications.

#### `SSL_get0_connection`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| New       | Never         | No        | CS            |

```c
/*
 * Get the SSL object representing the connection associated with this object.
 *
 * If the SSL object represents a non-QUIC method or a QUIC connection, this
 * returns the same object passed.
 *
 * If the SSL object represents a QUIC stream returns the QUIC connection
 * object.
 */
SSL *SSL_get0_connection(SSL *ssl);
```

#### `SSL_is_connection`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| New       | Never         | No        | CS            |

```c
/*
 * Returns 1 if the object represents a connection. This always returns 1 for
 * non-QUIC methods, but returns 0 for SSL objects for QUIC streams which are
 * not also the QUIC connection object.
 *
 * This is exactly equivalent to (SSL_get0_connection(ssl) == ssl).
 */
int SSL_is_connection(SSL *ssl);
```

#### `SSL_get_stream_type`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| New       | Never         | No        | S             |

```c
/*
 * If the object represents a stream, returns an SSL_STREAM_TYPE value
 * designating whether the stream can be used for transmission, reception,
 * or both.
 *
 * This always returns SSL_STREAM_TYPE_BIDI for non-QUIC methods.
 *
 * It returns SSL_STREAM_TYPE_NONE for a QUIC connection object if it
 * does not have a default stream.
 */
#define SSL_STREAM_TYPE_NONE    0
#define SSL_STREAM_TYPE_READ    1
#define SSL_STREAM_TYPE_WRITE   2
#define SSL_STREAM_TYPE_BIDI    (SSL_STREAM_TYPE_READ | SSL_STREAM_TYPE_WRITE)
__owur int SSL_get_stream_type(SSL *ssl);
```

#### `SSL_get_stream_id`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| New       | Never         | No        | S             |

```c
/*
 * QUIC: Returns the unique stream ID for the stream, an integer in range [0, 2**62-1],
 * or UINT64_MAX if the stream ID is not available. If called on a QUIC
 * connection, returns the unique stream ID for the default stream if there is
 * one, and otherwise returns UINT64_MAX.
 *
 * TLS, DTLS: Returns UINT64_MAX.
 */
__owur uint64_t SSL_get_stream_id(SSL *ssl);
```

#### `SSL_is_stream_local`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| New       | Never         | No        | S             |

```c
/*
 * QUIC: Returns 1 if the stream was locally initiated, or 0 otherwise.
 *
 * TLS, DTLS: Returns -1.
 */
__owur int SSL_is_stream_local(SSL *ssl);
```

#### `SSL_new_stream`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| New       | Never         | No        | C             |

```c
/*
 * Create a new SSL object representing a single additional stream.
 *
 * There is no need to call SSL_connect on the resulting object, and
 * any such call is a no-op.
 *
 * For QUIC:
 *   Creates a new stream. Must be called only on a QUIC connection SSL object.
 *   Can be used on client or server. If the SSL_STREAM_FLAG_UNI flag is set,
 *   the created stream is unidirectional, otherwise it is bidirectional.
 *
 *   To be MSMT-safe.
 *
 * For TLS and DTLS SSL objects:
 *   Always fails.
 */
#define SSL_STREAM_FLAG_UNI    1

SSL *SSL_new_stream(SSL *ssl, uint64_t flags);
```

#### `SSL_accept_stream`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| New       | Never         | Yes       | C             |

```c
/*
 * Create a new SSL object representing an additional stream which was created
 * by the peer.
 *
 * There is no need to call SSL_accept on the resulting object, and
 * any such call is a no-op.
 *
 * For QUIC:
 *   Must be called only on a QUIC connection SSL object. Fails if called on a
 *   stream object. Checks if a new stream has been created by the peer. If it
 *   has, creates a new SSL object to represent it and returns it. Otherwise,
 *   returns NULL. If multiple streams are available to be accepted, the oldest
 *   stream (that is, the stream with the lowest stream ID) is accepted.
 *
 * For all other methods:
 *   Returns NULL.
 *
 * The flags argument is unused and should be set to zero.
 *
 * To be MSMT-safe (i.e., can be called from multiple threads).
 *
 * If the QUIC connection SSL object is configured in blocking mode, this
 * function will block unless the SSL_ACCEPT_STREAM_NO_BLOCK flag is passed.
 *
 * This function returns NULL if the effective incoming stream reject policy is
 * `REJECT`.
 */
#define SSL_ACCEPT_STREAM_NO_BLOCK      1

SSL *SSL_accept_stream(SSL *ssl, uint64_t flags);
```

#### `SSL_get_accept_stream_queue_len`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| New       | Never         | No        | C             |

```c
/*
 * Determine the number of streams waiting to be returned on a subsequent call
 * to SSL_accept_stream. If this returns a non-zero value, the next call to
 * SSL_accept_stream (on any thread) is guaranteed to work. Returns 0 for
 * non-QUIC objects, or for QUIC stream objects.
 *
 * To be MSMT-safe.
 */
size_t SSL_get_accept_stream_queue_len(SSL *ssl);
```

#### `SSL_set_incoming_stream_policy`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| New       | Never         | No        | C             |

```c
/*
 * Sets the policy for incoming streams. If `policy` is `AUTO` (the default):
 *
 *   - if the default stream mode is
 *     `SSL_DEFAULT_STREAM_MODE_AUTO_BIDI` or
 *     `SSL_DEFAULT_STREAM_MODE_AUTO_UNI`, this is equivalent to `REJECT`;
 *
 *   - otherwise, this is equivalent to `ACCEPT`.
 *
 * If configured to `ACCEPT`, incoming streams are placed on the accept queue
 * for application consumption. `aec` is ignored in this case.
 *
 * If configured to `REJECT`, incoming streams automatically have both their
 * receiving and sending parts handled via non-normal termination. `aec` is an
 * application error code used for the `STOP_SENDING` and `RESET_STREAM` frames
 * used for the purposes of this termination. The default AEC value used if this
 * function is never called is 0.
 */
#define SSL_INCOMING_STREAM_POLICY_AUTO      0
#define SSL_INCOMING_STREAM_POLICY_ACCEPT    1
#define SSL_INCOMING_STREAM_POLICY_REJECT    2

int SSL_set_incoming_stream_policy(SSL *ssl, int policy, uint64_t aec);
```

#### `SSL_set_default_stream_mode`

| Semantics | `SSL_get_error` | Can Tick? | CSHL          |
| --------- | ------------- | --------- | ------------- |
| New       | Never         | No        | C             |

```c
/*
 * Used to control single stream operation. Calling this function determines the
 * nature of the default stream which will automatically be created on the QUIC
 * connection SSL object.
 *
 * The default mode is `SSL_DEFAULT_STREAM_MODE_AUTO_BIDI`.
 *
 * The modes are as follows:
 *
 *   - `SSL_DEFAULT_STREAM_MODE_NONE`: No default stream will ever be created.
 *     The application is assumed to understand multi-stream operation.
 *     Remotely-initiated streams are placed in the accept queue for application
 *     consumption. `SSL_read` and `SSL_write` calls must be made on a QUIC
 *     stream SSL object, not the QUIC connection SSL object, as no default
 *     stream will be associated with it.
 *
 *   - `SSL_DEFAULT_STREAM_MODE_AUTO_BIDI`: "First stream wins" mode of
 *     operation for single-stream usage. If `SSL_write` is called before the
 *     peer opens a remotely-initiated stream, a locally-initiated bidirectional
 *     stream is created and bound as the default stream. If the peer opens a
 *     remotely-initiated stream before the local application calls `SSL_write`
 *     (with `len > 0`) for the first time, that stream is bound as the default
 *     stream, which may be bidirectional or unidirectional; if it is
 *     unidirectional, calls to `SSL_write` will fail. Attempts to create
 *     additional streams by the peer are automatically rejected unless
 *     the application opts in (API TBD).
 *
 *   - `SSL_DEFAULT_STREAM_MODE_AUTO_UNI`: "First stream wins" mode of
 *     operation for single-stream usage, with a unidirectional stream. This
 *     functions identically to `SSL_DEFAULT_STREAM_MODE_AUTO_BIDI`, but if the
 *     local application calls `SSL_write` prior to the peer creating a
 *     remotely-initiated stream, a unidirectional TX-only stream is created and
 *     bound as the default stream. Thereafter, calls to `SSL_read` will fail.
 *     If the peer creates a remotely-initiated stream prior to the first call
 *     to `SSL_write` (with `len > 0`), that stream will be bound as the default
 *     stream; note that a bidirectional stream may be bound in this case.
 *     Attempts to create additional streams by the peer are automatically
 *     rejected unless the application opts in (API TBD).
 *
 * This function must be called before a default stream object is created, for
 * example before initiating a connection. If the function is too late to have
 * an effect, this function fails and returns 0.
 */
#define SSL_DEFAULT_STREAM_MODE_NONE                0
#define SSL_DEFAULT_STREAM_MODE_AUTO_BIDI           1
#define SSL_DEFAULT_STREAM_MODE_AUTO_UNI            2

__owur int SSL_set_default_stream_mode(SSL *ssl, uint32_t mode);
```

### Future APIs

A custom poller interface may be provided in the future. For more information,
see the QUIC I/O Architecture design document.

BIO Objects
-----------

### Existing APIs

#### `BIO_s_connect`, `BIO_new_ssl_connect`, `BIO_set_conn_hostname`

We are aiming to support use of the existing `BIO_new_ssl_connect` API with only
minimal changes. This will require internal changes to `BIO_s_connect`, which
should automatically detect when it is being used with a QUIC `SSL_CTX` and act
accordingly.

#### `BIO_new_bio_pair`

Unsuitable for use with QUIC on the network side; instead, applications can
make use of the new `BIO_s_dgram_pair` which provides equivalent functionality
with datagram semantics.

#### Interactions with `BIO_f_buffer`

Existing applications sometimes combine a network socket BIO with a
`BIO_f_buffer`. This is problematic because the datagram semantics of writes are
not preserved, therefore the BIO provided to libssl is, as provided, unusable
for the purposes of implementing QUIC. Moreover, output buffering is not a
relevant or desirable performance optimisation for the transmission of UDP
datagrams and will actually undermine QUIC performance by causing incorrect
calculation of ACK delays and consequently inaccurate RTT calculation.

Options:

  - Require applications to be changed to not use QUIC with a `BIO_f_buffer`.
  - Detect when a `BIO_f_buffer` is part of a BIO stack and bypass it
    (yucky and surprising).

#### MTU Signalling

**See also:**
[BIO_s_dgram_pair(3)](https://www.openssl.org/docs/manmaster/man3/BIO_s_dgram_pair.html)

`BIO_dgram_get_mtu` (`BIO_CTRL_DGRAM_GET_MTU`) and `BIO_dgram_set_mtu`
(`BIO_CTRL_DGRAM_SET_MTU`) already exist for `BIO_s_dgram` and are implemented
on a `BIO_s_dgram_pair` to allow the MTU to be determined and configured. One
side of a pair can configure the MTU to allow the other side to detect it.

`BIO_s_dgram` also has pre-existing support for getting the correct MTU value
from the OS using `BIO_CTRL_DGRAM_QUERY_MTU`.

### New APIs

#### `BIO_sendmmsg` and `BIO_recvmmsg`

**See also:**
[BIO_sendmmsg(3)](https://www.openssl.org/docs/manmaster/man3/BIO_sendmmsg.html)

The BIO interface features a new high-performance API for the execution of
multiple read or write operations in a single system call, on supported OSes. On
other OSes, a compatible fallback implementation is used.

Unlike all other BIO APIs, this API is intended for concurrent threaded use and
as such operates in a stateless fashion with regards to a BIO. This means, for
example, that retry indications are made using explicit API inputs and outputs
rather than setting an internal flag on the BIO.

This new BIO API includes:

- Local address support (getting the destination address of an incoming
  packet; setting the source address of an outgoing packet), where support
  for this is available;
- Peer address support (setting the destination address of an outgoing
  packet; getting the source address of an incoming packet), where support
  for this is available.

The following functionality was intentionally left out of this design because
not all OSes can provide support:

- Iovecs (which have also been determined not to be necessary for a
  performant QUIC implementation);
- Features such as `MSG_DONTWAIT`, etc.

This BIO API is intended to be extensible. For more information on this API, see
BIO_sendmmsg(3) and BIO_recvmmsg(3).

Custom BIO implementers may set their own implementation of these APIs via
corresponding `BIO_meth` getter/setter functions.

#### Truncation Mode

**See also:**
[BIO_s_dgram_pair(3)](https://www.openssl.org/docs/manmaster/man3/BIO_s_dgram_pair.html)

The controls `BIO_dgram_set_no_trunc` (`BIO_CTRL_DGRAM_SET_NO_TRUNC`) and
`BIO_dgram_get_no_trunc` (`BIO_CTRL_DGRAM_GET_NO_TRUNC`) are introduced. This is
a boolean value which may be implemented by BIOs with datagram semantics. When
enabled, attempting to receive a datagram such that the datagram would
ordinarily be truncated (as per the design of the Berkeley sockets API) instead
results in a failure. This is intended for implementation by `BIO_s_dgram_pair`.
For compatibility, the default behaviour is off.

#### Capability Negotiation

**See also:**
[BIO_s_dgram_pair(3)](https://www.openssl.org/docs/manmaster/man3/BIO_s_dgram_pair.html)

Where a `BIO_s_dgram_pair` is used, there is the potential for such a memory BIO
to be used by existing application code which is being adapted for use with
QUIC. A problem arises whereby one end of a `BIO_s_dgram_pair` (for example, the
side being used by OpenSSL's QUIC implementation) may assume that the other end
supports certain capabilities (for example, specifying a peer address), when in
actual fact the opposite end of the `BIO_s_dgram_pair` does not.

A capability signalling mechanism is introduced which allows one end of a
`BIO_s_dgram_pair` to indicate to the user of the opposite BIO the following
capabilities and related information:

- Whether source addresses the peer specifies will be processed.
- Whether destination addresses the peer specifies will be processed.
- Whether source addresses will be provided to the opposite BIO when it
  receives datagrams.
- Whether destination addresses will be provided to the opposite BIO
  when it receives datagrams.

The usage is as follows:

- One side of a BIO pair calls `BIO_dgram_set_caps` with zero or
  more of the following flags to advertise its capabilities:
  - `BIO_DGRAM_CAP_HANDLES_SRC_ADDR`
  - `BIO_DGRAM_CAP_HANDLES_DST_ADDR`
  - `BIO_DGRAM_CAP_PROVIDES_SRC_ADDR`
  - `BIO_DGRAM_CAP_PROVIDES_DST_ADDR`
- The other side of the BIO pair calls `BIO_dgram_get_effective_caps`
  to learn the effective capabilities of the BIO. These are the capabilities set
  by the opposite BIO.
- The above process can also be repeated in the opposite direction.

#### Local Address Support

**See also:**
[BIO_s_dgram_pair(3)](https://www.openssl.org/docs/manmaster/man3/BIO_s_dgram_pair.html)

Support for local addressing (the reception of destination addresses for
incoming packets, and the specification of source addresses for outgoing
packets) varies by OS. Thus, it may not be available in all circumstances. A
feature negotiation mechanism is introduced to facilitate this.

`BIO_dgram_get_local_addr_cap` (`BIO_CTRL_DGRAM_GET_LOCAL_ADDR_CAP`) determines
if a BIO is potentially capable of supporting local addressing on the current
platform. If it determines that support is available, local addressing support
must then be explicitly enabled via `BIO_dgram_set_local_addr_enable`
(`BIO_CTRL_DGRAM_SET_LOCAL_ADDR_ENABLE`). If local addressing support has not
been enabled, attempts to use local addressing (for example via `BIO_sendmmsg`
or `BIO_recvmmsg` with a `BIO_MSG` with a non-NULL `local` field) fails.

An explicit enablement call is required because setting up local addressing
support requires system calls on most operating systems prior to sending or
receiving packets and we do not wish to do this automatically inside the
`BIO_sendmmsg`/`BIO_recvmmsg` fastpaths, particularly since the process of
enabling support could fail due to lack of OS support, etc.

`BIO_dgram_get_local_addr_enable` (`BIO_CTRL_DGRAM_GET_LOCAL_ADDR_ENABLE`) is
also available.

It is important to note that `BIO_dgram_get_local_addr_cap` is entirely distinct
from the application capability negotiation mechanism discussed above. Whereas
the capability negotiation mechanism discussed above allows *applications* to
signal what they are capable of handling in their usage of a given BIO,
`BIO_dgram_local_addr_cap` allows a *BIO implementation* to indicate to the
users of that BIO whether it is able to support local addressing (where
enabled).

#### `BIO_s_dgram_pair`

**See also:**
[BIO_s_dgram_pair(3)](https://www.openssl.org/docs/manmaster/man3/BIO_s_dgram_pair.html)

A new BIO implementation, `BIO_s_dgram_pair`, is provided. This is similar to
the existing BIO pair but provides datagram semantics. It provides full support
for the new APIs `BIO_sendmmsg`, `BIO_recvmmsg`, the capability negotiation
mechanism described above, local address support and the MTU signalling
mechanism described above.

It can be instantiated using the new API `BIO_new_dgram_pair`.

#### `BIO_POLL_DESCRIPTOR`

The concept of *poll descriptors* are introduced. A poll descriptor is a tagged
union structure which represents an abstraction over some unspecified kind of OS
descriptor which can be used for synchronization and waiting.

The most commonly used kind of poll descriptor is one which describes a network
socket (i.e., on POSIX-like platforms, a file descriptor), however other kinds
of poll descriptor may be defined.

A BIO may be queried for whether it has a poll descriptor for read or write
operations respectively:

- Where `BIO_get_rpoll_descriptor` (`BIO_CTRL_GET_RPOLL_DESCRIPTOR`) is called,
  the BIO should output a poll descriptor which describes a resource which can
  be used to determine when the BIO will next become readable via a call to
  `BIO_read` or, if supported by the BIO, `BIO_recvmmsg`.
- Where
  `BIO_get_wpoll_descriptor` (`BIO_CTRL_GET_WPOLL_DESCRIPTOR`) is called, the
  BIO should output a poll descriptor which describes a resource which can be
  used to determine when the BIO will next become writeable via a call to
  `BIO_write` or, if supported by the BIO, `BIO_sendmmsg`.

A BIO may not necessarily be able to provide a poll descriptor. For example,
memory-based BIOs such as `BIO_s_dgram_pair` do not correspond to any OS
synchronisation resource, and thus the `BIO_get_rpoll_descriptor` and
`BIO_get_wpoll_descriptor` calls are not supported for such BIOs.

A BIO which supports these functions is known as pollable, and a BIO which does
not is known as non-pollable. `BIO_s_dgram` supports these functions.

The implementation of these functions for a `BIO_f_ssl` forwards to
`SSL_get_rpoll_descriptor` and `SSL_get_wpoll_descriptor` respectively. The

#### `BIO_s_dgram_mem`

This is a basic memory buffer BIO with datagram semantics. Unlike
`BIO_s_dgram_pair`, it is unidirectional and does not support peer addressing or
local addressing.

#### `BIO_err_is_non_fatal`

A new predicate function `BIO_err_is_non_fatal` is defined which determines if
an error code represents a non-fatal or transient error. For details, see
[BIO_sendmmsg(3)](https://www.openssl.org/docs/manmaster/man3/BIO_sendmmsg.html).

Q & A
-----

To assist in understanding, when a “TBD” listed above is removed, or when a
relevant question is raised, the resolution to the question will be placed here.

**Q. Should `SSL_do_handshake` wait until the handshake is completed, or until it
is confirmed?**

**Note:** [The terms *handshake complete* and *handshake confirmed* are defined
in RFC 9001 and have specific
meanings.](https://www.rfc-editor.org/rfc/rfc9001.html#name-handshake-complete)

A. `SSL_do_handshake` should wait until the handshake is completed, because
handshake completion represents the completion of the cryptographic
authentication of the connection. When a connection's handshake is completed,
TLS 1.3 Finished messages have been exchanged by both parties, even if the
handshake has not yet been *confirmed*. Moreover, RFC 9001 s. 4.1.2 states:

>Additionally, a client MAY consider the handshake to be confirmed when it
>receives an acknowledgment for a 1-RTT packet.

This logically implies that it is OK for a client to start transmitting 1-RTT
packets prior to handshake confirmation, otherwise there would be no in-flight
1-RTT packets for the client to receive ACKs for.

**Q. Does `ENABLE_PARTIAL_WRITE` interact with blocking mode?**

A. No; this mode is only relevant to non-blocking mode. In blocking mode,
`SSL_write` always waits until all data is written unless an error occurs. The
semantics of `SSL_write` are preserved unchanged.

**Q. Does `SSL_write` block until data is written to the network, or simply
until it is buffered?**

A. `SSL_write` blocks until it has accepted responsibility for the data passed
to it, just like `write(2)` or `send(2)`. In other words, it blocks until it can
buffer the data. This does not necessarily mean that the data has actually been
sent.

**Q. How should connection closure work?**

A. **RFC requirements.** After we begin terminating the connection by sending a
`CONNECTION_CLOSE` frame, QUIC requires that we continue to process network I/O
for a certain period of time so that any further traffic from the peer results
in generation of a further `CONNECTION_CLOSE` frame. This is necessary to handle
the possibility that the `CONNECTION_CLOSE` frame which was initially sent may
be lost.

**API issues.** This creates a complication because it implies that the
connection closure process may take a fair amount of time, whereas existing API
users will generally expect to be able to call `SSL_shutdown` and then
immediately free the SSL object.

However, if the caller immediately frees the SSL object, this precludes
our implementing the applicable logic, at least on the client side. Moreover,
existing API users are likely to tear down underlying network BIOs immediately
after calling `SSL_free` anyway. In other words, any implementation based on
secretly keeping QUIC state around after a call to `SSL_free` does not seem
particularly workable on the client side.

**Server side considerations.** There is more of a prospect here on the server
side, since multiple connections will share the same socket, which will
presumably be associated with some kind of enduring listener object. Thus when
server support is implemented in the future connection teardown could be handled
internally by maintaining the state of connections undergoing termination inside
the listener object. However, similar caveats to those discussed here arise when
the listener object itself is to be town down. (It is also possible we could
optionally allow use of the server-style API to make multiple outgoing client
connections with a non-zero-length client-side CID on the same underlying
network BIO.)

There are only really two ways to handle this:

- **RFC conformant mode.** `SSL_shutdown` only indicates that shutdown is
  complete once the entire connection closure process is complete.

  This process consists of the Closing and Draining states. In some cases the
  Closing state may last only briefly, namely if the peer chooses to respond to
  our `CONNECTION_CLOSE` frame with a `CONNECTION_CLOSE` frame of its own. This
  allows immediate progression to the Draining state. However, a peer is *not*
  required to respond with such a frame. Thus in the worst case, this state can
  be as long as `3*PTO`; for example a peer with a high estimated RTT of 300ms
  would have us wait for 900ms.

  In the Draining state we simply ignore all incoming traffic and do not
  generate outgoing traffic. The purpose of this state is to simply tie up the
  socket and ensure any data still in flight is discarded. However, RFC 9000
  states:

    Disposing of connection state prior to exiting the closing or draining state
    could result in an endpoint generating a Stateless Reset unnecessarily when
    it receives a late-arriving packet. Endpoints that have some alternative
    means to ensure that late-arriving packets do not induce a response, such as
    those that are able to close the UDP socket, MAY end these states earlier to
    allow for faster resource recovery. Servers that retain an open socket for
    accepting new connections SHOULD NOT end the closing or draining state early

  Because our client mode implementation uses one socket per connection, it
  appears to be reasonable based on the above text to omit the implementation of
  the draining state (the same may not be the case for the server role when
  implemented in the future).

  Thus, in general, `SSL_shutdown` can be expected to take about one round
  trip's time to complete when dealing with a peer whose QUIC implementation
  happens to respond to a `CONNECTION_CLOSE` frame with a `CONNECTION_CLOSE`
  frame of its own, and about three round trips otherwise.

- **Rapid shutdown mode.** `SSL_shutdown` sends a `CONNECTION_CLOSE` frame once
  and completes immediately. The Closing and Draining states are not used, and
  if the `CONNECTION_CLOSE` frame was lost, the peer will have to wait for idle
  timeout to determine that the connection is gone (there is also the
  possibility that, if the socket is closed by the application after teardown, a
  peer will make something of ICMP Port Unreachable messages, but this is
  unlikely to be reliable and since this message is not authenticated, QUIC
  implementations probably shouldn't pay much attention to it anyway.)

There is little problem with `SSL_shutdown` taking as long as it needs to for
some long-running applications, but for others it poses a real issue. For
example, a command-line tool which makes one connection, performs one
application-specific transaction, and then tears down the connection. In this
case an RFC-conformant connection termination would essentially require the
process to hang around for a substantial amount of time after the work of the
process is done.

For this reason, it is concluded that both of these shutdown modes need to be
offered.

Where connection closure is initiated remotely rather than locally, only the
draining state is relevant. Since we conclude above that we do not need to
implement the draining state on the client side, this means that connection
closure can be completed immediately in the case of a remote closure.

**Q. Should we just map `SSL_handle_events` to `DTLS_CTRL_HANDLE_TIMEOUT`
internally?**

A. No, since the infinite time representation is different between the two
calls.

**Q. How should `STOP_SENDING` be supported?**

We trigger `STOP_SENDING` automatically if an application frees the associated
QUIC stream SSL object.

**Q. Can data be received on a locally initiated bidirectional stream before any
data is sent on that stream?**

This is an interesting question without a clear answer to be found in the QUIC
RFCs. A close reading of RFC 9000 suggests that the answer is, in principle,
yes; however the RFC also grants explicit permission to make design choices in
implementations which would preclude this:

>An implementation might choose to defer allocating a stream ID to a stream until
>it sends the first STREAM frame and enters this state, which can allow for
>better stream prioritization.

If an ID has not been allocated to a stream, obviously incoming data cannot be
addressed to it. However, supposing that an implementation does not do this,
RFC 9000 seems basically clear that it is valid for an application to create a
stream locally, then receive data on it before sending anything:

>The sending part of a stream that the endpoint initiates (types 0 and 2 for
>clients, 1 and 3 for servers) is opened by the application. The "Ready"
>state represents a newly created stream that is able to accept data from the
>application.
>
>[...]
>
>For a bidirectional stream, the receiving part enters the "Recv" state when
>the sending part initiated by the endpoint (type 0 for a client, type 1 for
>a server) enters the "Ready" state.

A peer is not generally notified of the creation of a stream which has not sent
any data yet, since the creation of a stream is signalled only implicitly via
the transmission of data in `STREAM` frames. However, a zero-length STREAM frame
could presumably be used to effect such a notification. RFC 9000 contains no
specific discussion of this possibility but does not preclude it. As such, in
order to receive data on a locally-initiated bidirectional stream before sending
any data on that stream, it would be necessary to either

- Use a QUIC implementation which signals a bidirectional stream which has
  not yet sent any data via a zero-length stream frame, or

- Use an application protocol which can inform the peer of the stream ID
  of the created stream in some application protocol-specific way.
  This is somewhat less plausible because it would require an API between
  the application and its QUIC library to inform the QUIC library
  that the peer has in fact created a stream with a given ID and to
  take its word for it. This is unlikely to be commonly available, especially as
  application errors in usage of such an API would lead to internal
  inconsistencies in QUIC connection state.

Of course this discussion is somewhat esoteric as it is unclear why an
application would want to create a locally-initiated stream and then have the
peer transmit on it first, rather than simply use a remotely-initiated stream.
Thus this discussion of this edge case is more of a curiosity, however for
completeness it needs to be thought about in the API design.

**Q. How should single-stream operation support locally and remotely-initiated
streams?**

Note that the ID of a stream depends on whether it is bidirectional and whether
it is initiated by the client or server. Therefore, in single stream operation,
it is necessary to know whether single-stream QUIC is being used with
client-initiated or server-initiated stream initiation, and whether a
bidirectional or unidirectional stream is being used; otherwise, we do not know
which stream ID to bind to.

The object of single stream operation is to support simple uses cases for simple
applications. There seems no need to support esoteric usage of streams such as
receiving first on a locally initiated stream here, thus we avoid supporting
this to simplify the API.

As such, an application which calls `SSL_write` on a QUIC connection SSL object
before it calls `SSL_read` by definition is using a locally-initiated stream,
and an application which does the opposite is using a remotely-initiated stream.
We can use the ordering of initial calls to `SSL_read` and `SSL_write` to infer
the desired stream type.

Supporting locally-initiated streams (`SSL_write` called first) is simple;
we automatically create the stream and queue data for transmission.

Supporting remotely-initiated streams (`SSL_read` called first) is a little
stranger. We could create the stream with the correct ID when cued to by the
initial call to `SSL_read` implying use of a remotely-initiated stream. However,
this would mean we are creating state tracking a remotely-initiated stream
before the peer has signalled it. This would work in the happy case where the
client is connected to a compatible server but may result in strange
inconsistencies of QUIC internal state if a client is accidentally connected to
an incompatible peer. Since the peer ought to be the authority on the streams it
creates, this seems like an undesirable approach.

Ergo, creation of a default remotely-initiated stream needs to be deferred
until the *peer* signals such a stream.

This leads naturally to a "first stream wins" model of implementation:

- When a QUIC connection SSL object is created, default stream mode is
  enabled, meaning that a default stream will be bound to the QUIC connection
  SSL object at the earliest available opportunity. However, no default
  stream is bound yet.

- One of the following events happened — whichever happens first wins:

  - The local application calls `SSL_write()` (`len > 0`). A locally-initiated
    stream with ordinal 0 is created. The stream is bidirectional by default but
    this can be changed. This stream is bound as the default stream.

  - The peer creates a stream. This stream is bound as the default stream.

If the local application calls `SSL_read()` before either of the above
occur, `SSL_read()` fails as though no data is available until one
of the above events occurs.

Once one of the above events occurs, any additional stream created by the peer
is automatically terminated using both `STOP_SENDING` and `STREAM_RESET` frames
(to terminate both the receiving and sending parts respectively) and there is no
API-visible effect to the local application (unless the application explicitly
opts into supporting additional streams).
