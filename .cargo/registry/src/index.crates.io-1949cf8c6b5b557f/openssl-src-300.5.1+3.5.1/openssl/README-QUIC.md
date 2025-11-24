Using OpenSSL with QUIC
=======================

From OpenSSL 3.2, OpenSSL features support for making QUIC connections as a
client. Starting with OpenSSL 3.5, server-side QUIC support has also been added.

Users interested in using the new QUIC functionality are encouraged to look at
some of the following resources:

- The new [OpenSSL Guide], which provides introductory guides on the use of TLS,
  QUIC, and other OpenSSL functionality.
- The [OpenSSL Guide] incorporates various code samples. The complete source
  for these can be [found in the source tree under `demos/guide`](./demos/guide/).
- The [openssl-quic(7) manual page], which provides a basic reference overview
  of QUIC functionality and how use of QUIC differs from use of TLS with regard
  to our API.
- The [Demo-Driven Design (DDD)][DDD] demos, which demonstrate the use of QUIC
  using simple examples. These can be [found in the source tree under
  `doc/designs/ddd`].
- The [demo found in `demos/http3`], which provides an HTTP/3 client example
  using the nghttp3 HTTP/3 library.

FAQ
---

### Why would I want to use QUIC, and what functionality does QUIC offer relative to TLS or DTLS?

QUIC is a state-of-the-art secure transport protocol carried over UDP. It can
serve many of the use cases of SSL/TLS as well as those of DTLS.

QUIC delivers a number of advantages such as support for multiple streams of
communication; it is the basis for HTTP/3 [RFC 9114]; fast connection
initiation; and connection migration (enabling a connection to survive IP
address changes). For a more complete description of what QUIC is and its
advantages see the [QUIC Introduction] in the [OpenSSL Guide].

For a comprehensive overview of OpenSSL's QUIC implementation, see the
[openssl-quic(7) manual page].

### How can I use HTTP/3 with OpenSSL?

There are many HTTP/3 implementations in C available. The use of one such HTTP/3
library with OpenSSL QUIC is demonstrated via the [demo found in `demos/http3`].

### How can I use OpenSSL QUIC in my own application for a different protocol?

The [OpenSSL Guide] provides introductory examples for how to make use of
OpenSSL QUIC.

The [openssl-quic(7) manual page] and the [Demo-Driven Design (DDD)][DDD] demos
may also be helpful to illustrate the changes needed if you are trying to adapt
an existing application.

### How can I test QUIC using `openssl s_client`?

There is basic support for single-stream QUIC using `openssl s_client`:

```shell
$ openssl s_client -quic -alpn myalpn -connect host:port
```

In the above example replace `host` with the hostname of the server (e.g.
`www.example.com`) and `port` with the port for the server (e.g. `443`). Replace
`myalpn` with the Application Layer Protocol to use (e.g.`h3` represents
HTTP/3). IANA maintains a standard list of [ALPN ids] that can be used.

This example connects to a QUIC server and opens a single bidirectional stream.
Data can be passed via stdin/stdout as usual. This allows test usage of QUIC
using simple TCP/TLS-like usage. Note that OpenSSL has no direct support for
HTTP/3 so connecting to an HTTP/3 server should be possible but sending an
HTTP/3 request or receiving any response data is not.

### How can I create a QUIC server with OpenSSL?

Starting with OpenSSL 3.5, you can create a QUIC server. OpenSSL provides a server
implementation example that you can use as a reference:

The example QUIC server implementation can be found in the source tree under
[`demos/quic/server`](./demos/quic/server/). This demonstrates how to implement a
basic QUIC server using the OpenSSL API.

To run the example QUIC server:

```shell
$ ./demos/quic/server/server <port-number> <certificate-file> <key-file>
```

For example:

```shell
$ ./demos/quic/server/server 4433 server.pem server.key
```

Replace `server.pem` and `server.key` with your certificate and private key files.
Note that the standard `openssl s_server` command does NOT support QUIC - you must
use this dedicated server example instead.

For more information about implementing QUIC servers with OpenSSL, refer to the
[OpenSSL Guide] and the [openssl-quic(7) manual page].

[openssl-quic(7) manual page]: https://www.openssl.org/docs/manmaster/man7/openssl-quic.html
[OpenSSL Guide]: https://www.openssl.org/docs/manmaster/man7/ossl-guide-introduction.html
[DDD]: https://github.com/openssl/openssl/tree/master/doc/designs/ddd
[found in the source tree under `doc/designs/ddd`]: ./doc/designs/ddd/
[demo found in `demos/http3`]: ./demos/http3/
[QUIC Introduction]: https://www.openssl.org/docs/manmaster/man7/ossl-guide-quic-introduction.html
[RFC 9114]: https://tools.ietf.org/html/rfc9114
[ALPN ids]: https://www.iana.org/assignments/tls-extensiontype-values/tls-extensiontype-values.xhtml#alpn-protocol-ids
