Report on the Conclusions of the QUIC DDD Process
=================================================

The [QUIC Demo-Driven Design process](README.md) was undertaken to meet the OMC
requirement to develop a QUIC API that required only minimal changes to existing
applications to be able to adapt their code to use QUIC. The demo-driven design
process developed a set of representative demos modelling a variety of common
OpenSSL usage patterns based on analysis of a broad spectrum of open source
software projects using OpenSSL.

As part of this process, a set of proposed diffs were produced. These proposed
diffs were the expected changes which would be needed to the baseline demos to
support QUIC based on theoretical analysis of the minimum requirements to be
able to support QUIC. This analysis concluded that the changes needed to
applications could be kept very small in many circumstances, with only minimal
diff sizes to the baseline demos.

Following the development of QUIC MVP, these demos have been revisited and the
correspondence of our actual final API and usage patterns with the planned diffs
have been reviewed.

This document discusses the planned changes and the actual changes for each demo
and draws conclusions on the level of disparity.

Since tracking a set of diffs separately is unwieldy, both the planned and
unplanned changes have been folded into the original baseline demo files guarded
with `#ifdef USE_QUIC`. Viewing these files therefore is informative to
application writers as it provides a clear view of what is different when using
QUIC. (The originally planned changes, and the final changes, are added in
separate, clearly-labelled commits; to view the originally planned changes only,
view the commit history for a given demo file.)

ddd-01-conn-blocking
--------------------

This demo exists to demonstrate the simplest possible usage of OpenSSL, whether
with TLS or QUIC.

### Originally planned changes

The originally planned change to enable applications for QUIC amounted to just a
single line:

```diff
+    ctx = SSL_CTX_new(QUIC_client_method());
-    ctx = SSL_CTX_new(TLS_client_method());
```

### Actual changes

The following additional changes needed to be made:

- `QUIC_client_method` was renamed to `OSSL_QUIC_client_method` for namespacing
  reasons.

- A call to `SSL_set_alpn_protos` to configure ALPN was added. This is necessary
  because QUIC mandates the use of ALPN, and this was not noted during the
  DDD process.

ddd-02-conn-nonblocking
-----------------------

This demo exists to demonstrate simple non-blocking usage. As with
ddd-01-conn-blocking, the name resolution process is managed by `BIO_s_connect`.

It also arbitrarily adds a `BIO_f_buffer` pushed onto the BIO stack
as this is a common application usage pattern.

### Originally planned changes

The originally planned changes to enable applications for QUIC amounted to:

- Change of method (as for ddd-01-conn-blocking);

- Use of a `BIO_f_dgram_buffer` BIO method instead of a `BIO_f_buffer`;

- Use of a `BIO_get_poll_fd` function to get the FD to poll rather than
  `BIO_get_fd`;

- A change to how the `POLLIN`/`POLLOUT`/`POLLERR` flags to pass to poll(2)
  need to be determined.

- Additional functions in application code to determine event handling
  timeouts related to QUIC (`get_conn_pump_timeout`) and to pump
  the QUIC event loop (`pump`).

- Timeout computation code which involves merging and comparing different
  timeouts and calling `pump` as needed, based on deadlines reported
  by libssl.

Note that some of these changes are unnecessary when using the thread assisted
mode (see the variant ddd-02-conn-nonblocking-threads below).

### Actual changes

The following additional changes needed to be made:

- Change of method name (as for ddd-01-conn-blocking);

- Use of ALPN (as for ddd-01-conn-blocking);

- The strategy for how to expose pollable OS resource handles
  to applications to determine I/O readiness has changed substantially since the
  original DDD process. As such, applications now use `BIO_get_rpoll_descriptor`
  and `BIO_get_wpoll_descriptor` to determine I/O readiness, rather than the
  originally hypothesised `SSL_get_poll_fd`.

- The strategy for how to determine when to poll for `POLLIN`, when to
  poll for `POLLOUT`, etc. has changed since the original DDD process.
  This information is now exposed via `SSL_net_read_desired` and
  `SSL_net_write_desired`.

- The API to expose the event handling deadline for the QUIC engine
  has evolved since the original DDD process. The new API
  `SSL_get_event_timeout` is used, rather than the originally hypothesised
  `BIO_get_timeout`/`SSL_get_timeout`.

- The API to perform QUIC event processing has been renamed to be
  more descriptive. It is now called `SSL_handle_events` rather than
  the originally hypothesised `BIO_pump`/`SSL_pump`.

The following changes were foreseen to be necessary, but turned out to actually
not be necessary:

- The need to change code which pushes a `BIO_f_buffer()` after an SSL BIO
  was foreseen as use of buffering on the network side is unworkable with
  QUIC. This turned out not to be necessary since we can just reject the
  BIO_push() call. The buffer should still be freed eventually when the
  SSL BIO is freed. The buffer is not used and is unnecessary, so it is
  still desirable for applications to remove this code.

ddd-02-conn-nonblocking-threads
-------------------------------

This is a variant of the ddd-02-conn-nonblocking demo. The base is the same, but
the changes made are different. The use of thread-assisted mode, in which an
internal assist thread is used to perform QUIC event handling, enables an
application to make fewer changes than are needed in the ddd-02-conn-nonblocking
demo.

### Originally planned changes

The originally planned changes to enable applications for QUIC amounted to:

- Change of method, this time using method `QUIC_client_thread_method` rather
  than `QUIC_client_method`;

- Use of a `BIO_get_poll_fd` function to get the FD to poll rather than
  `BIO_get_fd`;

- A change to how the `POLLIN`/`POLLOUT`/`POLLERR` flags to pass to poll(2)
  need to be determined.

  Note that this is a substantially smaller list of changes than for
  ddd-02-conn-nonblocking.

### Actual changes

The following additional changes needed to be made:

- Change of method name (`QUIC_client_thread_method` was renamed to
  `OSSL_QUIC_client_thread_method` for namespacing reasons);

- Use of ALPN (as for ddd-01-conn-blocking);

- Use of `BIO_get_rpoll_descriptor` rather than `BIO_get_poll_fd` (as for
  ddd-02-conn-nonblocking).

- Use of `SSL_net_read_desired` and `SSL_net_write_desired` (as for
  ddd-02-conn-nonblocking).

ddd-03-fd-blocking
------------------

This demo is similar to ddd-01-conn-blocking but uses a file descriptor passed
directly by the application rather than BIO_s_connect.

### Originally planned changes

- Change of method (as for ddd-01-conn-blocking);

- The arguments to the `socket(2)` call are changed from `(AF_INET, SOCK_STREAM,
  IPPROTO_TCP)` to `(AF_INET, SOCK_DGRAM, IPPROTO_UDP)`.

### Actual changes

The following additional changes needed to be made:

- Change of method name (as for ddd-01-conn-blocking);

- Use of ALPN (as for ddd-01-conn-blocking).

ddd-04-fd-nonblocking
---------------------

This demo is similar to ddd-01-conn-nonblocking but uses a file descriptor
passed directly by the application rather than BIO_s_connect.

### Originally planned changes

- Change of method (as for ddd-01-conn-blocking);

- The arguments to the `socket(2)` call are changed from `(AF_INET, SOCK_STREAM,
  IPPROTO_TCP)` to `(AF_INET, SOCK_DGRAM, IPPROTO_UDP)`;

- A change to how the `POLLIN`/`POLLOUT`/`POLLERR` flags to pass to poll(2)
  need to be determined.

- Additional functions in application code to determine event handling
  timeouts related to QUIC (`get_conn_pump_timeout`) and to pump
  the QUIC event loop (`pump`).

- Timeout computation code which involves merging and comparing different
  timeouts and calling `pump` as needed, based on deadlines reported
  by libssl.

### Actual changes

The following additional changes needed to be made:

- Change of method name (as for ddd-01-conn-blocking);

- Use of ALPN (as for ddd-01-conn-blocking);

- `SSL_get_timeout` replaced with `SSL_get_event_timeout` (as for
  ddd-02-conn-nonblocking);

- `SSL_pump` renamed to `SSL_handle_events` (as for ddd-02-conn-nonblocking);

- The strategy for how to determine when to poll for `POLLIN`, when to
  poll for `POLLOUT`, etc. has changed since the original DDD process.
  This information is now exposed via `SSL_net_read_desired` and
  `SSL_net_write_desired` (as for ddd-02-conn-nonblocking).

ddd-05-mem-nonblocking
----------------------

This demo is more elaborate. It uses memory buffers created and managed by an
application as an intermediary between libssl and the network, which is a common
usage pattern for applications. Managing this pattern for QUIC is more elaborate
since datagram semantics on the network channel need to be maintained.

### Originally planned changes

- Change of method (as for ddd-01-conn-blocking);

- Call to `BIO_new_bio_pair` is changed to `BIO_new_dgram_pair`, which
  provides a bidirectional memory buffer BIO with datagram semantics.

- A change to how the `POLLIN`/`POLLOUT`/`POLLERR` flags to pass to poll(2)
  need to be determined.

- Potential changes to buffer sizes used by applications to buffer
  datagrams, if those buffers are smaller than 1472 bytes.

- The arguments to the `socket(2)` call are changed from `(AF_INET, SOCK_STREAM,
  IPPROTO_TCP)` to `(AF_INET, SOCK_DGRAM, IPPROTO_UDP)`;

### Actual changes

The following additional changes needed to be made:

- Change of method name (as for ddd-01-conn-blocking);

- Use of ALPN (as for ddd-01-conn-blocking);

- The API to construct a `BIO_s_dgram_pair` ended up being named
  `BIO_new_bio_dgram_pair` rather than `BIO_new_dgram_pair`;

- Use of `SSL_net_read_desired` and `SSL_net_write_desired` (as for
  ddd-02-conn-nonblocking).

ddd-06-mem-uv
-------------

This demo is the most elaborate of the set. It uses a real-world asynchronous
I/O reactor, namely libuv (the engine used by Node.js). In doing so it seeks to
demonstrate and prove the viability of our API design with a real-world
asynchronous I/O system. It operates wholly in non-blocking mode and uses memory
buffers on either side of the QUIC stack to feed data to and from the
application and the network.

### Originally planned changes

- Change of method (as for ddd-01-conn-blocking);

- Various changes to use of libuv needed to switch to using UDP;

- Additional use of libuv to configure a timer event;

- Call to `BIO_new_bio_pair` is changed to `BIO_new_dgram_pair`
  (as for ddd-05-mem-nonblocking);

- Some reordering of code required by the design of libuv.

### Actual changes

The following additional changes needed to be made:

- Change of method name (as for ddd-01-conn-blocking);

- Use of ALPN (as for ddd-01-conn-blocking);

- `BIO_new_dgram_pair` renamed to `BIO_new_bio_dgram_pair` (as for
  ddd-05-mem-nonblocking);

- `SSL_get_timeout` replaced with `SSL_get_event_timeout` (as for
  ddd-02-conn-nonblocking);

- `SSL_pump` renamed to `SSL_handle_events` (as for ddd-02-conn-nonblocking);

- Fixes to use of libuv based on a corrected understanding
  of its operation, and changes that necessarily ensue.

Conclusions
-----------

The DDD process has successfully delivered on the objective of delivering a QUIC
API which can be used with only minimal API changes. The additional changes on
top of those originally planned which were required to successfully execute the
demos using QUIC were highly limited in scope and mostly constituted only minor
changes. The sum total of the changes required for each demo (both planned and
additional), as denoted in each DDD demo file under `#ifdef USE_QUIC` guards,
are both minimal and limited in scope.

“Minimal” and “limited” are distinct criteria. If inexorable technical
requirements dictate, an enormous set of changes to an application could be
considered “minimal”. The changes required to representative applications, as
demonstrated by the DDD demos, are not merely minimal but also limited.

For example, while the extent of these necessary changes varies by the
sophistication of each demo and the kind of application usage pattern it
represents, some demos in particular demonstrate exceptionally small changesets;
for example, ddd-01-conn-blocking and ddd-02-conn-nonblocking-threads, with
ddd-01-conn-blocking literally being enabled by a single line change assuming
ALPN is already configured.

This report concludes the DDD process for the single-stream QUIC client API
design process, which sought to validate our API design and API ease of use for
existing applications seeking to adopt QUIC.
