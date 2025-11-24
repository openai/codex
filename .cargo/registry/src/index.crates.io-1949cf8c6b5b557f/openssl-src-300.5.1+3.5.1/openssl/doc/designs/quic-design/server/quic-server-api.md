QUIC Server API Design
======================

Requirements
------------

Essential:

- Support server mode.
- Support multiple clients on one UDP socket/BIO.
- Accept incoming clients and get a `SSL *` object which works just like a
  client QCSO.

Maybe:

- Binding multiple BIOs (UDP sockets) to one listener?

Implied:

- Support connection migration and preferred addressing in the future.
- Maximise reuse of existing APIs.

Approach
--------

Two design sketches are shown below, their relative merits are evaluated, and
one is chosen.

Sketch A: `SSL_LISTENER`
------------------------

A new type `SSL_LISTENER` is introduced to the public API. This represents an
entity which listens for incoming connections. It is **not** intended to be QUIC
specific and provides an abstract interface which could be adapted to other
cases in the future. For example, it could be used for TLS over TCP to make
accepting incoming TCP connections easy.

Internally, an `SSL_LISTENER` has a common header in much the same way that
`SSL` objects now do. For example, if called using a QUIC `SSL_CTX`, the
internal structure might be a `QUIC_LISTENER`, etc. If called using a TLS
`SSL_CTX`, this might be a `TCP_LISTENER` doing something very ordinary. We can
support DTLS in the future in the same way as QUIC, etc.

Internally, an `SSL_LISTENER` stores any common state that needs to be held
collectively for all connections created by that listener.

`SSL_LISTENER` objects are refcounted and referenced by any connection which
descends from them. Thus, an application may free the `SSL_LISTENER`, but
it will be retained until it is no longer depended on by any connection.

The operation of an `SSL_LISTENER` is simple: call
`SSL_LISTENER_accept_connection`. For QUIC, this returns a new QCSO, which is
considered a child of the listener.

```c
SSL_LISTENER *SSL_LISTENER_new(SSL_CTX *ctx, BIO *net_rbio, BIO *net_wbio);
int SSL_LISTENER_up_ref(SSL_LISTENER *l);
int SSL_LISTENER_free(SSL_LISTENER *l);

BIO *SSL_LISTENER_get0_net_rbio(SSL_LISTENER *l);
BIO *SSL_LISTENER_get0_net_wbio(SSL_LISTENER *l);
int SSL_LISTENER_set1_net_rbio(SSL_LISTENER *l, BIO *net_rbio);
int SSL_LISTENER_set1_net_wbio(SSL_LISTENER *l, BIO *net_wbio);

SSL *SSL_LISTENER_accept_connection(SSL_LISTENER *l, uint64_t flags);
size_t SSL_LISTENER_get_accept_queue_len(SSL_LISTENER *l);
```

### Discussion

However, there are a number of disadvantages to introducing a new object type.
The concept of a listener shares many of the same concepts inherent to
supporting non-blocking network I/O which our SSL API for QUIC connections has
been carefully designed to support:

- Poll descriptors (`SSL_get_[rw]poll_descriptor`);
- `SSL_net_(read|write)_desired`;
- `SSL_handle_events`;
- `SSL_get_event_timeout`;
- BIO configuration (`SSL_set_[rw]bio`);
- Refcounting (`SSL_up_ref`, `SSL_free`).

We would need to duplicate all of these APIs. Moreover application developers
would need to use both and the distinction between the two would become
confusing.

Sketch B: New SSL Object Type: Listener
---------------------------------------

Instead of the above, the definition of a new SSL object subtype — a Listener —
is proposed. A QLSO (QUIC Listener SSL Object) is defined in much the same way
that QCSOs and QSSOs are currently defined, using the common SSL structure
header.

Many existing APIs around the handling of non-blocking I/O can then be supported
and work in exactly the same way.

```c
/*
 * SSL_new_listener
 * ----------------
 *
 * Creates a QLSO, assuming a QUIC SSL_CTX.
 *
 * TCP and DTLS can be supported in the future; not supported for non-QUIC
 * SSL_CTXs for now.
 */
SSL *SSL_new_listener(SSL_CTX *ctx, uint64_t flags);

/*
 * A QLSO supports the following existing APIs:
 *
 *   - Poll descriptors (`SSL_get_[rw]poll_descriptor`)
 *   - `SSL_net_(read|write)_desired`
 *   - `SSL_handle_events`
 *   - `SSL_get_event_timeout`
 *   - BIO configuration (`SSL_set_[rw]bio`)
 *   - Refcounting (`SSL_up_ref`, `SSL_free`)
 */

/*
 * New API for QLSOs: SSL_accept_connection
 * ----------------------------------------
 *
 * These APIs are exactly the same as  SSL_accept_stream, but made a separate
 * function to avoid confusion. Supported on QLSOs only.
 */
SSL *SSL_accept_connection(SSL *ssl, uint64_t flags);
size_t SSL_get_accept_connection_queue_len(SSL *ssl);

/*
 * New API for QLSOs: SSL_listen
 * -----------------------------
 *
 * This is not usually needed as SSL_accept_connection will implicitly trigger
 * this if it has not been called yet. It is only needed if a server wishes to
 * ensure it has started to accept incoming connections but does not wish to
 * actually call SSL_accept_connection yet. In this regard it is similar to
 * listen(2).
 */
int SSL_listen(SSL *ssl);

/*
 * New API for QCSOs: SSL_get_peer_addr
 * ------------------------------------
 *
 * An API to retrieve a L4 peer address. Supported on QCSOs only.
 */
int SSL_get_peer_addr(SSL *ssl, BIO_ADDR *peer_addr);

/* SSL_get_local_addr may be introduced once it is relevant (migration etc.) */

/* Returns 1 for QLSOs (or future TLS/DTLS listeners) only. */
int SSL_is_listener(SSL *ssl);

/* Returns the QLSO (if any) from which a QCSO or QSSO is descended. */
SSL *SSL_get0_listener(SSL *ssl);

/* SSL_is_quic returns 1 */
```

Selection of Design
-------------------

We could also consider using an `SSL_CTX` as the listener type. Since a
`SSL_CTX` is intended to span the lifespan of multiple TLS connections there is
a degree of logic here. However, this does not match up naturally because an
`SSL_CTX` is not designed to hold network resources or manage network I/O. It
does not have the facilities for polling and event handling that the `SSL`
object API has been enhanced with. It would also be a fairly major paradigm
shift for existing server applications and make things difficult if a server
wanted to use different `SSL_CTX` instances to use different e.g. TLS settings
within the same listener.

Based on the merits of the above, Sketch B is chosen for adoption.

Discussion of Relevant Issues
-----------------------------

### Notes on Internal Refactoring to Support Multiple Connections

Currently a client connection is a QCSO
(`QUIC_CONNECTION`) which contains a `QUIC_CHANNEL`. The `QUIC_CONNECTION`
provides API Personality Layer (APL) implementation and wraps the APL-agnostic
`QUIC_CHANNEL`. We currently have two APLs, the libssl API (“the” APL) and
`QUIC_TSERVER` for internal testing use. `QUIC_XSO` is the APL object
representing a QUIC stream and wraps our APL-agnostic channel-owned
`QUIC_STREAM` object. There is no specific object in the `QUIC_TSERVER` APL for
a stream as they are referenced in that API by stream ID only.

`QUIC_CHANNEL` corresponds exactly to a QUIC connection. In a similar manner,
`QUIC_STREAM` corresponds exactly to a QUIC stream. In server operation,
multiple `QUIC_CHANNEL` instances will exist, one per accepted connection.

Above we introduce the notion of a QUIC Listener SSL Object (QLSO), which
internally will be represented by the type `QUIC_LISTENER`. This forms a new
object in the libssl APL.

With server operation, multiple connections will share a single UDP
socket/network BIO. Certain resources currently tied 1:1 into a `QUIC_CHANNEL`
will be broken out, such as the network BIO pointers and `QUIC_DEMUX` instance.
A `QUIC_CHANNEL` continues to own e.g. the QRX/QTX as these are
connection-specific.

Because preserving the separation of the APL(s) and the QUIC core is desirable,
the concept of a `QUIC_PORT` is introduced. This is the internal object tracking
a demux and network BIOs which a `QUIC_CHANNEL` will belong to. In other words,
the new situation is as follows:

| Abbr. | libssl APL Object | QUIC Core Object |
|-------|-------------------|------------------|
| QLSO  | `QUIC_LISTENER`   | `QUIC_PORT`      |
| QCSO  | `QUIC_CONNECTION` | `QUIC_CHANNEL`   |
| QSSO  | `QUIC_XSO`        | `QUIC_STREAM`    |

### Listener Flexibility

A listener allows multiple connections to be received on a UDP socket. However,
there is no intrinsic requirement in QUIC that a socket only be used for client
use, or only for server use. Therefore, it should be possible for an application
to create a QLSO, use it to accept connections, and also use it to initiate
outgoing connections.

This can be supported in a simple manner:

```c
SSL *SSL_new_from_listener(SSL *ssl);
```

This creates an unconnected (idle) QCSO from a QLSO. Use of
`SSL_set1_initial_peer_addr` is mandatory in this case as peer address
autodetection is inhibited.

The created QCSO otherwise works in a completely normal fashion.

### Client-Only Listeners

In fact, the realisation of this hybrid capability is simply an emergent
consequence of our internal refactor. Since a QUIC server will be split into an
internal `QUIC_PORT` and zero or more `QUIC_CONNECTION` instances, so too will a
client connection become simply a degenerate case where there is a `QUIC_PORT`
and only one `QUIC_CONNECTION`. So the client case basically just becomes a
simplified API where we spin up a `QUIC_PORT`/listener automatically to support
a single connection.

It is potentially desirable for a client to be able to use the listener
API even if it is only initiating connections as a client and never accepting
connections. There are several reasons why an application might want to do this:

- it allows the application to centralise its I/O polling and networking arrangements;
- it allows the application to handle the TERMINATING phase of connections in a
  better (and more RFC-compliant) way.

For the latter reason, consider how connection termination works. Once a QUIC
Immediate Close is initiated using a `CONNECTION_CLOSE` frame, a connection
enters the `TERMINATING` state and is expected to stay there for some period of
time until the possibility of various contingencies (such as loss of a
`CONNECTION_CLOSE` frame and its necessary retransmission) has been accounted
for. However, many applications may not wish to hang around for potentially
several seconds or longer while tearing down a QUIC connection.

Allowing clients to use a listener resolves this problem, if the lifetime of the
application exceeds the lifetime of most client connections. The listener can
continue to hold the `QUIC_PORT` internally and respond to enquiries for
terminating and terminated connections, even if (as far as the application is
concerned) no client connections currently exist and all prior client
connections have been freed using `SSL_free`. The internal `QUIC_CHANNEL` can be
kept around by the `QUIC_PORT` until it reaches the `TERMINATED` state.

A client-only listener can be created one of two ways:

1. by calling `SSL_new_listener` while passing this flag:

    ```
    #define SSL_LISTENER_FLAG_NO_ACCEPT  (1U << 0)
    ```

2. by using a client `SSL_METHOD`, which implies `NO_ACCEPT`.

This flag simply indicates that this listener is not to be used to accept
incoming connections. A client can then use `SSL_new_from_listener` as usual to
create an outgoing QCSO.

### CID Length Determination

Currently a client connection uses a zero-length local CID. Since we want to
support multiple outgoing client connections on one socket these will need to
support a non-zero-length local CID. Equally servers will need to support CIDs.

By default when a QLSO is used, we will use a real, non-zero-length CID. If the
existing QUIC client API is used (with no QLSO) we will likely spin up a
`QUIC_PORT` internally but specially choose to use a zero-length local CID since
we know this port will never be used for more than one connection.

If an application creates a QLSO explicitly but knows it will not need to
create more than one connection, it can pass the following flag when creating
the listener:

```c
#define SSL_LISTENER_FLAG_SINGLE_CONN   (1U << 1)
```

Failure to do so is nonfatal and simply increases packet overheads.

### Future Compatibility: TLS over TCP Support

Assuming we choose to use the SSL object type as the basis of a new listener
object type, let us consider how this API could also be used to support TLS over
TCP, at least in a server role:

```c
/* Blocking example: */
{
    SSL *l, *conn;
    BIO *tcp_listener = get_bio_sock_for_listening_tcp_socket();
    SSL_CTX *ctx = create_ssl_ctx(TLS_server_method());
    BIO_ADDR addr;

    l = SSL_new_listener(ctx);
    SSL_set_bio(l, tcp_listener, tcp_listener);

    for (;;) {
        /* standard SSL_CONNECTION is returned */
        conn = SSL_accept_connection(l, 0);

        spawn_thread_to_process_conn(conn, &addr);
    }
}
```

Essentially, when `SSL_new_listener` is given a `SSL_CTX` which is using a TLS
method, it returns some different kind of object, say a `TCP_LISTENER`, which
also is a kind of SSL object. `SSL_accept_connection` calls `accept(2)` via some
BIO abstraction on the provided BIO, and constructs a new BIO on the resulting
socket, wrapping it via standard `SSL_new` using the same `SSL_CTX`.

Thus, this API should be compatible with any future adaptation to also support
TCP.

### Future Compatibility: DTLS over UDP Support

The relevant concerns regarding DTLS are likely to be highly similar to QUIC,
so this API should also be adaptable to support DTLS in the future, providing
a unified server-side API. `SSL_new_listener` will return e.g. a
`DTLS_LISTENER`.

### Supporting Multiple Sockets

There are three means that we can provide to support a server which wants to
listen on multiple UDP sockets:

1. Allow a QLSO to bind multiple BIOs;
2. Create multiple QLSOs and use some polling mechanism to service all of them;
3. Create some higher level object which can aggregate multiple QLSOs.

(1) would require more extensive API changes and should only be done if a
compelling reason to do so arises. (3) is a more advanced design which is
theoretically possible in future and is discussed in the next section.
For now, we will support only (2) (which we can support via our planned polling
API) until a compelling need arises.

### Roles and Domains: Concepts and Future Evolution

A client can use QUIC using a single QCSO without ever creating a QLSO.
Conversely a client or server can use QUIC by creating a QLSO and then one or
more subsidiary QCSOs. This implies that, at least in terms of the publicly
visible API, the roles of certain essential aspects of QUIC can belong to
different publicly visible API objects.

The following concepts are introduced:

- **Port Leader:** A Port Leader is the APL object which caused the creation
  of a `QUIC_PORT`, which is responsible for servicing a particular pair
  of network BIOs. Currently maps to one UDP socket.

  In the simple client API, the QCSO is the port leader.
  In a client using a QLSO, the QLSO is the port leader.
  In a server, the QLSO is the port leader.

- **Event Leader:** An Event Leader is the APL object which handles event
  processing. Calls to SSL_handle_events() for any subsidiary object is
  equivalent to a call to SSL_handle_events() on the Event Leader, which handles
  state machine processing for that object and all subsidiary objects.

  The event leader is the highest-level APL object in a QUIC APL object
  hierarchy:

  In the simple client API, the QCSO is the event leader.
  In a client using a QLSO, the QLSO is the event leader.
  In a server, the QLSO is the event leader.

- **Domain:** A QUIC domain is a set of APL objects rooted in an Event Leader.
  In other words, requests to handle events on subsidiary objects in a domain
  are channeled up to the Event Leader and processed there. A QUIC domain is a
  set of logically grouped QUIC objects which share a single I/O reactor and
  event processing context.

  A domain could contain multiple QLSOs (i.e., multiple UDP sockets) in future.
  In this case QLSOs may cease to be the most senior APL object type and there
  may be some new hypothetical SSL_new_quic_domain() call.

  However, there is no plan to implement domains at this time; for the time
  being, this is purely conceptual and ensures our architecture can grow to
  supporting multiple sockets and listeners in a given QUIC context if need be.

  If ever implemented, a domain will be reified by a `QUIC_DOMAIN` structure as
  part of the APL and a `QUIC_ENGINE` structure as part of the QUIC core. The
  SSL object type will be abbreviated QDSO (QUIC Domain SSL Object).

The introduction of the concept of a domain demonstrates the value of the Port
Leader and Domain Leader concepts going forward:

Example 1 — Explicitly Created Domain:

```text
QDSO (QUIC_DOMAIN/QUIC_ENGINE)              Event Leader
    QLSO (QUIC_LISTENER/QUIC_PORT)          Port Leader
        QCSO (QUIC_CONNECTION/QUIC_CHANNEL)
            QSSO (QUIC_XSO/QUIC_STREAM)
```

Example 2 - Explicitly Created QLSO, Implicitly Created Domain:

```text
QLSO (QUIC_LISTENER/QUIC_PORT)              Event Leader, Port Leader
    [QUIC_ENGINE]
    QCSO (QUIC_CONNECTION/QUIC_CHANNEL)
        QSSO (QUIC_XSO/QUIC_STREAM)
```

Here we see that the QLSO now has the Event Leader role, as it is the most
senior object visible publicly as a constructed API object. Note that a
QUIC_ENGINE will be created internally to allow the QLSO to service the Event
Leader role, but the corresponding QUIC_DOMAIN APL object will not exist, as
this is not API-visible.

Example 3 - Explicitly Created Single Client QCSO:

```text
QCSO (QUIC_CONNECTION/QUIC_CHANNEL)         Event Leader, Port Leader
    [QUIC_ENGINE]
    [QUIC_PORT]
    QSSO (QUIC_XSO/QUIC_STREAM)
```

Here we see that the QCSO now has the Port Leader role. Note that a QUIC_PORT
will be created internally to allow the QCSO to service the Port Leader role,
but the corresponding QUIC_LISTENER APL object will not exist, as this is not
API-visible. Similarly a QUIC_ENGINE is spun up to handle the Event Leader role,
as above.

It should be emphasised that everything in this section is largely academic and
intended to demonstrate potential future directions and the adaptability of the
design to future evolution. There is no immediate plan to implement a QDSO
object type in the APL. However it will be viable to do so if we want to support
multiple sockets per domain someday.

| Abbr. | libssl APL Object | QUIC Core Object | Seniority   | Potential Roles |
|-------|-------------------|------------------|-------------|-----------------|
| QDSO  | `QUIC_DOMAIN`     | `QUIC_ENGINE`    | 1 (Highest) | EL              |
| QLSO  | `QUIC_LISTENER`   | `QUIC_PORT`      | 2           | EL PL           |
| QCSO  | `QUIC_CONNECTION` | `QUIC_CHANNEL`   | 3           | EL PL           |
| QSSO  | `QUIC_XSO`        | `QUIC_STREAM`    | 4           |                 |

### Polling

We will support polling all QUIC object types (QLSOs, QCSOs, QSSOs) using a new
polling API. This is the subject of its own design and will be discussed in more
detail in a separate design document.

### Impact of `SSL_METHOD` selection

The interaction of our various QCSO and QLSO construction APIs in the APL,
and the choice of the `SSL_METHOD` for a `SSL_CTX`, must also be considered.
The following interactions are chosen:

```text
SSL_new(QUIC_client_method)             → QCSO(Client)
SSL_new(QUIC_client_thread_method)      → QCSO(Client)
SSL_new(QUIC_server_method)             → (not supported; error)

SSL_new_listener(QUIC_client_method)        → QLSO(Client)   (implicit NO_ACCEPT)
SSL_new_listener(QUIC_client_thread_method) → QLSO(Client)   (implicit NO_ACCEPT)
SSL_new_listener(QUIC_server_method)        → QLSO(Server + Client)

SSL_new_from_listener(QUIC_client_method)           → QCSO (Client)
SSL_new_from_listener(QUIC_client_thread_method)    → QCSO (Client)
SSL_new_from_listener(QUIC_server_method)           → QCSO (Client)
```

Observations:

- It makes no sense to try and make a single QCSO proactively as a server,
  so this just results in an error.

- A server QLSO can also make outgoing client connections.

- By contrast, if a client `SSL_METHOD` is specified when creating a listener,
  it is assumed to be uninterested in accepting connections and this implies the
  `NO_ACCEPT` flag. In this regard the functionality of a server is a superset
  of the functionality of a client.

- `SSL_new_from_listener` always creates a client QCSO.

- `SINGLE_CONN` is implied automatically only when calling `SSL_new` and not
  using a QLSO.

Usage Example
-------------

The following is a blocking example, where `SSL_accept_connection` waits until a
new connection is available.

```c
{
    SSL *l, *conn;
    SSL_CTX *ctx = create_ssl_ctx(QUIC_server_method());
    BIO *dgram_bio = get_dgram_bio();

    l = SSL_new_listener(ctx);
    SSL_set_bio(l, dgram_bio, dgram_bio);

    for (;;) {
        /* automatically starts and calls SSL_listen on first call */
        conn = SSL_accept_connection(l, 0); /* standard QCSO is returned */

        spawn_thread_to_process_conn(conn);
    }
}
```

In non-blocking mode, `SSL_accept_connection` will simply act in a non-blocking
fashion and return immediately with NULL if no new incoming connection is
available, just like `SSL_accept_stream`. The details of how polling and waiting
for incoming stream events, and other events, will work will be discussed in the
subsequent polling design document.
