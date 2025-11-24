<h1 align="center"><img width="500" src="https://raw.githubusercontent.com/quinn-rs/quinn/51a3cea225670757cb844a342428e4e1341d9f13/docs/thumbnail.svg" /></h1>

[![Documentation](https://docs.rs/quinn/badge.svg)](https://docs.rs/quinn/)
[![Crates.io](https://img.shields.io/crates/v/quinn.svg)](https://crates.io/crates/quinn)
[![Build status](https://github.com/quinn-rs/quinn/workflows/CI/badge.svg)](https://github.com/djc/quinn/actions?query=workflow%3ACI)
[![codecov](https://codecov.io/gh/quinn-rs/quinn/branch/main/graph/badge.svg)](https://codecov.io/gh/quinn-rs/quinn)
[![Chat](https://img.shields.io/badge/chat-%23quinn:matrix.org-%2346BC99?logo=matrix)](https://matrix.to/#/#quinn:matrix.org)
[![Chat](https://img.shields.io/discord/976380008299917365?logo=discord)](https://discord.gg/SGPEcDfVzh)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE-MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE-APACHE)

Quinn is a pure-Rust, async-compatible implementation of the IETF [QUIC][quic] transport protocol.
The project was founded by [Dirkjan Ochtman](https://github.com/djc) and
[Benjamin Saunders](https://github.com/Ralith) as a side project in 2018, and has seen more than
30 releases since then. If you're using Quinn in a commercial setting, please consider
[sponsoring](https://opencollective.com/quinn-rs) the project.

## Features

- Simultaneous client/server operation
- Ordered and unordered stream reads for improved performance
- Works on stable Rust, tested on Linux, macOS and Windows
- Pluggable cryptography, with a standard implementation backed by
  [rustls][rustls] and [*ring*][ring]
- Application-layer datagrams for small, unreliable messages
- Future-based async API
- Minimum supported Rust version of 1.74.1

## Overview

- **quinn:** High-level async API based on tokio, see [examples][examples] for usage. This will be used by most developers. (Basic benchmarks are included.)
- **quinn-proto:** Deterministic state machine of the protocol which performs [**no** I/O][sans-io] internally and is suitable for use with custom event loops (and potentially a C or C++ API).
- **quinn-udp:** UDP sockets with ECN information tuned for the protocol.
- **bench:** Benchmarks without any framework.
- **fuzz:** Fuzz tests.

# Getting Started

**Examples**

```sh
$ cargo run --example server ./
$ cargo run --example client https://localhost:4433/Cargo.toml
```

This launches an HTTP 0.9 server on the loopback address serving the current
working directory, with the client fetching `./Cargo.toml`. By default, the
server generates a self-signed certificate and stores it to disk, where the
client will automatically find and trust it.

**Links**

- Talk at [RustFest Paris (May 2018) presentation][talk]; [slides][slides]; [YouTube][youtube]
- Usage [examples][examples]
- Guide [book][documentation]

## Usage Notes

<details>
<summary>
Click to show the notes
</summary>

### Buffers

A Quinn endpoint corresponds to a single UDP socket, no matter how many
connections are in use. Handling high aggregate data rates on a single endpoint
can require a larger UDP buffer than is configured by default in most
environments. If you observe erratic latency and/or throughput over a stable
network link, consider increasing the buffer sizes used. For example, you could
adjust the `SO_SNDBUF` and `SO_RCVBUF` options of the UDP socket to be used
before passing it in to Quinn. Note that some platforms (e.g. Linux) require
elevated privileges or modified system configuration for a process to increase
its UDP buffer sizes.

### Certificates

By default, Quinn clients validate the cryptographic identity of servers they
connect to. This prevents an active, on-path attacker from intercepting
messages, but requires trusting some certificate authority. For many purposes,
this can be accomplished by using certificates from [Let's Encrypt][letsencrypt]
for servers, and relying on the default configuration for clients.

For some cases, including peer-to-peer, trust-on-first-use, deliberately
insecure applications, or any case where servers are not identified by domain
name, this isn't practical. Arbitrary certificate validation logic can be
implemented by enabling the `dangerous_configuration` feature of `rustls` and
constructing a Quinn `ClientConfig` with an overridden certificate verifier by
hand.

When operating your own certificate authority doesn't make sense, [rcgen][rcgen]
can be used to generate self-signed certificates on demand. To support
trust-on-first-use, servers that automatically generate self-signed certificates
should write their generated certificate to persistent storage and reuse it on
future runs.

</details>
<p></p>

## Contribution

All feedback welcome. Feel free to file bugs, requests for documentation and
any other feedback to the [issue tracker][issues].

The quinn-proto test suite uses simulated IO for reproducibility and to avoid
long sleeps in certain timing-sensitive tests. If the `SSLKEYLOGFILE`
environment variable is set, the tests will emit UDP packets for inspection
using external protocol analyzers like Wireshark, and NSS-compatible key logs
for the client side of each connection will be written to the path specified in
the variable.

The minimum supported Rust version for published releases of our
crates will always be at least 6 months old at the time of release.

[quic]: https://quicwg.github.io/
[issues]: https://github.com/djc/quinn/issues
[rustls]: https://github.com/ctz/rustls
[ring]: https://github.com/briansmith/ring
[talk]: https://paris.rustfest.eu/sessions/a-quic-future-in-rust
[slides]: https://github.com/djc/talks/blob/ff760845b51ba4836cce82e7f2c640ecb5fd59fa/2018-05-26%20A%20QUIC%20future%20in%20Rust/Quinn-Speaker.pdf
[animation]: https://dirkjan.ochtman.nl/files/head-of-line-blocking.html
[youtube]: https://www.youtube.com/watch?v=EHgyY5DNdvI
[letsencrypt]: https://letsencrypt.org/
[rcgen]: https://crates.io/crates/rcgen
[examples]: https://github.com/djc/quinn/tree/main/quinn/examples
[documentation]: https://quinn-rs.github.io/quinn/networking-introduction.html
[sans-io]: https://sans-io.readthedocs.io/how-to-sans-io.html
