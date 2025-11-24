QUIC Requirements
=================

There have been various sources of requirements for the OpenSSL QUIC
implementation. The following sections summarise the requirements obtained from
each of these sources.

Original OMC Requirements
-------------------------

The OMC have specified an initial set of requirements for QUIC as well as other
requirements for the coming releases. The remainder of this section summarises
the OMC requirements that were originally
[posted](https://mta.openssl.org/pipermail/openssl-project/2021-October/002764.html)
and that were specific to QUIC

* The focus for the next releases is QUIC, with the objective of providing a
  fully functional QUIC implementation over a series of releases (2-3).

* The current libssl record layer includes support for TLS, DTLS and KTLS. QUIC
  will introduce another variant and there may be more over time. The OMC requires
  a pluggable record layer interface to be implemented to enable this to be less
  intrusive, more maintainable, and to harmonize the existing record layer
  interactions between TLS, DTLS, KTLS and the planned QUIC protocols. The pluggable
  record layer interface will be internal only for MVP and be public in a future
  release.

* The application must have the ability to be in control of the event loop without
  requiring callbacks to process the various events. An application must also have
  the ability to operate in “blocking” mode.

* The QUIC implementation must include at least one congestion control algorithm.
  The fully functional release will provide the ability to plug in more
  implementations (via a provider).

* The minimum viable product (MVP) for the next release is a pluggable record
  layer interface and a single stream QUIC client in the form of s_client that
  does not require significant API changes. In the MVP, interoperability should be
  prioritized over strict standards compliance.

* The MVP will not contain a library API for an HTTP/3 implementation (it is a
  non-goal of the initial release). Our expectation is that other libraries will
  be able to use OpenSSL to build an HTTP/3 client on top of OpenSSL for the
  initial release.

* Once we have a fully functional QUIC implementation (in a subsequent release),
  it should be possible for external libraries to be able to use the pluggable
  record layer interface and it should offer a stable ABI (via a provider).

* The next major release number is intended to be reserved for the fully
  functional QUIC release (this does not imply we expect API breakage to occur as
  part of this activity - we can change major release numbers even if APIs remain
  compatible).

* PR#8797 will not be merged and compatibility with the APIs proposed in that PR
  is a non-goal.

* We do not plan to place protocol versions themselves in separate providers at
  this stage.

* For the MVP a single interop target (i.e. the server implementation list):

  1. [Cloudfare](https://cloudflare-quic.com/)

* Testing against other implementations is not a release requirement for the MVP.

### Non-QUIC OpenSSL Requirements

In addition to the QUIC requirements, the OMC also required that:

* The objective is to have shorter release timeframes, with releases occurring
  every six months.

* The platform policy, covering the primary and secondary platforms, should be
  followed. (Note that this includes testing of primary and secondary platforms
  on project CI)

OMC Blog post requirements
--------------------------

The OMC additionally published a
[blog post](https://www.openssl.org/blog/blog/2021/11/25/openssl-update/) which
also contained some requirements regarding QUIC. Statements from that blog post
have been extracted, paraphrased and summarised here as requirements:

* The objective is to have APIs that allow applications to support any of our
  existing (or future) security protocols and to select between them with minimal
  effort.

* In TLS/DTLS each connection represents a single stream and each connection is
  treated separately by our APIs. In the context of QUIC, APIs to be able to
  handle a collection of streams will be necessary for many applications. With the
  objective of being able to select which security protocol is used, APIs that
  encompass that capability for all protocols will need to be added.

* The majority of existing applications operate using a single connection (i.e.
  effectively they are single stream in nature) and this fundamental context of
  usage needs to remain simple.

* We need to enable the majority of our existing user’s applications to be able
  to work in a QUIC environment while expanding our APIs to enable future
  application usage to support the full capabilities of QUIC.

* We will end up with interfaces that allow other QUIC implementations
  (outside of OpenSSL) to be able to use the TLS stack within OpenSSL – however
  that is not the initial focus of the work.

* A long term supported core API for external QUIC library implementation usage
  in a future OpenSSL release will be provided.

* Make it easy for our users to communicate securely, flexibly and performantly
  using whatever security protocol is most appropriate for the task at hand.

* We will provide unified, consistent APIs to our users that work for all types
  of applications ranging from simple single stream clients up to optimised high
  performance servers

Additional OTC analysis
-----------------------

An OTC document provided the following analysis.

There are different types of application that we need to cater for:

* Simple clients that just do basic SSL_read/SSL_write or BIO_read/BIO_write
  interactions. We want to be able to enable them to transfer to using single
  stream QUIC easily. (MVP)

* Simple servers that just do basic SSL_read/SSL_write or BIO_read/BIO_write
  interactions. We want to be able to enable them to transfer to using single
  stream QUIC easily. More likely to want to do multi-stream.

* High performance applications (primarily server based) using existing libssl
  APIs; using custom network interaction BIOs in order to get the best performance
  at a network level as well as OS interactions (IO handling, thread handling,
  using fibres). Would prefer to use the existing APIs - they don’t want to throw
  away what they’ve got. Where QUIC necessitates a change they would be willing to
  make minor changes.

* New applications. Would be willing to use new APIs to achieve their goals.

Other requirements
------------------

The following section summarises requirements obtained from other sources and
discussions.

* The differences between QUIC, TLS, DTLS etc, should be minimised at an API
  level - the structure of the application should be the same. At runtime
  applications should be able to pick whatever protocol they want to use

* It shouldn’t be harder to do single stream just because multi stream as a
  concept exists.

* It shouldn’t be harder to do TLS just because you have the ability to do DTLS
  or QUIC.

* Application authors will need good documentation, demos, examples etc.

* QUIC performance should be comparable (in some future release - not MVP) with
  other major implementations and measured by a) handshakes per second
  b) application data throughput (bytes per second) for a single stream/connection

* The internal architecture should allow for the fact that we may want to
  support "single copy" APIs in the future:

  A single copy API would make it possible for application data being sent or
  received via QUIC to only be copied from one buffer to another once. The
  "single" copy allowed is to allow for the implicit copy in an encrypt or decrypt
  operation.

  Single copy for sending data occurs when the application supplies a buffer of
  data to be sent. No copies of that data are made until it is encrypted. Once
  encrypted no further copies of the encrypted data are made until it is provided
  to the kernel for sending via a system call.

  Single copy for receiving data occurs when a library supplied buffer is filled
  by the kernel via a system call from the socket. No further copies of that data
  are made until it is decrypted. It is decrypted directly into a buffer made
  available to (or supplied by) the application with no further internal copies
  made.

MVP Requirements (3.2)
----------------------

This section summarises those requirements from the above that are specific to
the MVP.

* a pluggable record layer (not public for MVP)

* a single stream QUIC client in the form of s_client that does not require
  significant API changes.

* interoperability should be prioritized over strict standards compliance.

* Single interop target for testing (cloudflare)

* Testing against other implementations is not a release requirement for the MVP.

* Support simple clients that just do basic SSL_read/SSL_write or BIO_read/BIO_write
  interactions. We want to be able to enable them to transfer to using single
  stream QUIC easily. (MVP)
