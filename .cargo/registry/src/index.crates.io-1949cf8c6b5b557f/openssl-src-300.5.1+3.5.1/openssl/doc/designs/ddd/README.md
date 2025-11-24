Demo-Driven Design
==================

The OpenSSL project from time to time must evolve its public API surface in
order to support new functionality and deprecate old functionality. When this
occurs, the changes to OpenSSL's public API must be planned, discussed and
agreed. One significant dimension which must be considered when considering any
proposed API change is how a broad spectrum of real-world OpenSSL applications
uses the APIs which exist today, as this determines the ways in which those
applications will be affected by any proposed changes, the extent to which they
will be affected, and the extent of any changes which will need to be made by
codebases using OpenSSL to remain current with best practices for OpenSSL API
usage.

As such, it is useful for the OpenSSL project to have a good understanding of
the usage patterns common in codebases which use OpenSSL, so that it can
anticipate the impact of any evolution of its API on those codebases. This
directory seeks to maintain a set of **API usage demos** which demonstrate a
full spectrum of ways in which real-world applications use the OpenSSL APIs.
This allows the project to discuss any proposed API changes in terms of the
changes that would need to be made to each demo. Since the demos are
representative of a broad spectrum of real-world OpenSSL-based applications,
this ensures that API evolution is made both with reference to real-world API
usage patterns and with reference to the impact on existing applications.

As such, these demos are maintained in the OpenSSL repository because they are
useful both to current and any future proposed API changes. The set of demos may
be expanded over time, and the demos in this directory at any one time constitute
a present body of understanding of API usage patterns, which can be used to plan
API changes.

For further background information on the premise of this approach, see [API
long-term evolution](https://github.com/openssl/openssl/issues/17939).

Scope
-----

The current emphasis is on client demos. Server support for QUIC is deferred to
subsequent OpenSSL releases, and therefore is (currently) out of scope for this
design exercise.

The demos also deliberately focus on aspects of libssl usage which are likely to
be relevant to QUIC and require changes; for example, how varied applications
have libssl perform network I/O, and how varied applications create sockets and
connections for use with libssl. The libssl API as a whole has a much larger
scope and includes numerous functions and features; the intention is
not to demonstrate all of these, because most of them will not be touched by
QUIC. For example, while many users of OpenSSL may make use of APIs for client
certificates or other TLS functionality, the use of QUIC is unlikely to have
implications for these APIs and demos demonstrating such functionality are
therefore out of scope.

[A report is available](REPORT.md) on the results of the DDD process following
the completion of the development of the QUIC MVP (minimum viable product).

Background
----------

These demos were developed after analysis of the following open source
applications to determine libssl API usage patterns. The commonly occurring usage
patterns were determined and used to determine categories into which to classify
the applications:

|                  | Blk? | FD |
|------------------|------|----|
| mutt             | S |      AOSF  |
| vsftpd           | S |      AOSF  |
| exim             | S |      AOSFx |
| wget             | S |      AOSF  |
| Fossil           | S |      BIOc  |
| librabbitmq      | A |      BIOx  |
| ngircd           | A |      AOSF  |
| stunnel          | A |      AOSFx |
| Postfix          | A |      AOSF  |
| socat            | A |      AOSF  |
| HAProxy          | A |      BIOx  |
| Dovecot          | A |      BIOm  |
| Apache httpd     | A |      BIOx  |
| UnrealIRCd       | A |      AOSF  |
| wpa_supplicant   | A |      BIOm  |
| icecast          | A |      AOSF  |
| nginx            | A |      AOSF  |
| curl             | A |      AOSF  |
| Asterisk         | A |      AOSF  |
| Asterisk (DTLS)  | A |      BIOm/x |
| pgbouncer        | A |      AOSF, BIOc  |

* Blk: Whether the application uses blocking or non-blocking I/O.
  * S: Blocking (Synchronous)
  * A: Nonblocking (Asynchronous)
* FD: Whether the application creates and owns its own FD.
  * AOSF: Application owns, calls SSL_set_fd.
  * AOSFx: Application owns, calls SSL_set_[rw]fd, different FDs for read/write.
  * BIOs: Application creates a socket/FD BIO and calls SSL_set_bio.
    Application created the connection.
  * BIOx: Application creates a BIO with a custom BIO method and calls SSL_set_bio.
  * BIOm: Application creates a memory BIO and does its own
    pumping to/from actual socket, treating libssl as a pure state machine which
    does no I/O itself.
  * BIOc: Application uses BIO_s_connect-based methods such as BIO_new_ssl_connect
    and leaves connection establishment to OpenSSL.

Demos
-----

The demos found in this directory are:

|                 | Type  | Description |
|-----------------|-------|-------------|
| [ddd-01-conn-blocking](ddd-01-conn-blocking.c) | S-BIOc | A `BIO_s_connect`-based blocking example demonstrating exemplary OpenSSL API usage |
| [ddd-02-conn-nonblocking](ddd-02-conn-nonblocking.c) | A-BIOc | A `BIO_s_connect`-based nonblocking example demonstrating exemplary OpenSSL API usage, with use of a buffering BIO |
| [ddd-03-fd-blocking](ddd-03-fd-blocking.c) | S-AOSF | A `SSL_set_fd`-based blocking example demonstrating real-world OpenSSL API usage (corresponding to S-AOSF applications above) |
| [ddd-04-fd-nonblocking](ddd-04-fd-nonblocking.c) | A-AOSF | A `SSL_set_fd`-based non-blocking example demonstrating real-world OpenSSL API usage (corresponding to A-AOSF applications above) |
| [ddd-05-mem-nonblocking](ddd-05-mem-nonblocking.c) | A-BIOm | A non-blocking example based on use of a memory buffer to feed OpenSSL encrypted data (corresponding to A-BIOm applications above) |
| [ddd-06-mem-uv](ddd-06-mem-uv.c) | A-BIOm | A non-blocking example based on use of a memory buffer to feed OpenSSL encrypted data; uses libuv, a real-world async I/O library |

On Ubuntu, libuv can be obtained by installing the package "libuv1-dev".

Availability of a default certificate store is assumed. `SSL_CERT_DIR` may be
set when running the demos if necessary.
