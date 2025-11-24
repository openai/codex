# Tower

Tower is a library of modular and reusable components for building robust
networking clients and servers.

[![Crates.io][crates-badge]][crates-url]
[![Documentation][docs-badge]][docs-url]
[![Documentation (master)][docs-master-badge]][docs-master-url]
[![MIT licensed][mit-badge]][mit-url]
[![Build Status][actions-badge]][actions-url]
[![Discord chat][discord-badge]][discord-url]

[crates-badge]: https://img.shields.io/crates/v/tower.svg
[crates-url]: https://crates.io/crates/tower
[docs-badge]: https://docs.rs/tower/badge.svg
[docs-url]: https://docs.rs/tower
[docs-master-badge]: https://img.shields.io/badge/docs-master-blue
[docs-master-url]: https://tower-rs.github.io/tower/tower
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: LICENSE
[actions-badge]: https://github.com/tower-rs/tower/workflows/CI/badge.svg
[actions-url]:https://github.com/tower-rs/tower/actions?query=workflow%3ACI
[discord-badge]: https://img.shields.io/discord/500028886025895936?logo=discord&label=discord&logoColor=white
[discord-url]: https://discord.gg/EeF3cQw
## Overview

Tower aims to make it as easy as possible to build robust networking clients and
servers. It is protocol agnostic, but is designed around a request / response
pattern. If your protocol is entirely stream based, Tower may not be a good fit.

Tower provides a simple core abstraction, the [`Service`] trait, which
represents an asynchronous function taking a request and returning either a
response or an error. This abstraction can be used to model both clients and
servers.

Generic components, like [timeouts], [rate limiting], and [load balancing],
can be modeled as [`Service`]s that wrap some inner service and apply
additional behavior before or after the inner service is called. This allows
implementing these components in a protocol-agnostic, composable way. Typically,
such services are referred to as _middleware_.

An additional abstraction, the [`Layer`] trait, is used to compose
middleware with [`Service`]s. If a [`Service`] can be thought of as an
asynchronous function from a request type to a response type, a [`Layer`] is
a function taking a [`Service`] of one type and returning a [`Service`] of a
different type. The [`ServiceBuilder`] type is used to add middleware to a
service by composing it with multiple [`Layer`]s.

### The Tower Ecosystem

Tower is made up of the following crates:

* [`tower`] (this crate)
* [`tower-service`]
* [`tower-layer`]
* [`tower-test`]

Since the [`Service`] and [`Layer`] traits are important integration points
for all libraries using Tower, they are kept as stable as possible, and
breaking changes are made rarely. Therefore, they are defined in separate
crates, [`tower-service`] and [`tower-layer`]. This crate contains
re-exports of those core traits, implementations of commonly-used
middleware, and [utilities] for working with [`Service`]s and [`Layer`]s.
Finally, the [`tower-test`] crate provides tools for testing programs using
Tower.

## Usage

Tower provides an abstraction layer, and generic implementations of various
middleware. This means that the `tower` crate on its own does *not* provide
a working implementation of a network client or server. Instead, Tower's
[`Service` trait][`Service`] provides an integration point between
application code, libraries providing middleware implementations, and
libraries that implement servers and/or clients for various network
protocols.

Depending on your particular use case, you might use Tower in several ways:

* **Implementing application logic** for a networked program. You might
  use the [`Service`] trait to model your application's behavior, and use
  the middleware [provided by this crate][all_layers] and by other libraries
  to add functionality to clients and servers provided by one or more
  protocol implementations.
* **Implementing middleware** to add custom behavior to network clients and
  servers in a reusable manner. This might be general-purpose middleware
  (and if it is, please consider releasing your middleware as a library for
  other Tower users!) or application-specific behavior that needs to be
  shared between multiple clients or servers.
* **Implementing a network protocol**. Libraries that implement network
  protocols (such as HTTP) can depend on `tower-service` to use the
  [`Service`] trait as an integration point between the protocol and user
  code. For example, a client for some protocol might implement [`Service`],
  allowing users to add arbitrary Tower middleware to those clients.
  Similarly, a server might be created from a user-provided [`Service`].

  Additionally, when a network protocol requires functionality already
  provided by existing Tower middleware, a protocol implementation might use
  Tower middleware internally, as well as an integration point.

### Library Support

A number of third-party libraries support Tower and the [`Service`] trait.
The following is an incomplete list of such libraries:

* [`hyper`]: A fast and correct low-level HTTP implementation.
* [`tonic`]: A [gRPC-over-HTTP/2][grpc] implementation built on top of
  [`hyper`]. See [here][tonic-examples] for examples of using [`tonic`] with
  Tower.
* [`axum`]: Ergonomic and modular web framework built with Tokio, Tower, and Hyper.
* [`tower-lsp`]: implementations of the [Language
  Server Protocol][lsp] based on Tower.
* [`kube`]: Kubernetes client and futures controller runtime. [`kube::Client`]
  makes use of the Tower ecosystem: [`tower`], [`tower-http`], and
  [`tower-test`]. See [here][kube-example-minimal] and
  [here][kube-example-trace] for examples of using [`kube`] with Tower.

[`hyper`]: https://crates.io/crates/hyper
[`tonic`]: https://crates.io/crates/tonic
[tonic-examples]: https://github.com/hyperium/tonic/tree/master/examples/src/tower
[grpc]: https://grpc.io
[`axum`]: https://crates.io/crates/axum
[`tower-lsp`]: https://crates.io/crates/tower-lsp
[lsp]: https://microsoft.github.io/language-server-protocol/
[`kube`]: https://crates.io/crates/kube
[`kube::Client`]: https://docs.rs/kube/latest/kube/struct.Client.html
[kube-example-minimal]: https://github.com/clux/kube-rs/blob/master/examples/custom_client.rs
[kube-example-trace]: https://github.com/clux/kube-rs/blob/master/examples/custom_client_trace.rs
[`tower-http`]: https://crates.io/crates/tower-http

If you're the maintainer of a crate that supports Tower, we'd love to add
your crate to this list! Please [open a PR] adding a brief description of
your library!

### Getting Started

The various middleware implementations provided by this crate are feature
flagged, so that users can only compile the parts of Tower they need. By
default, all the optional middleware are disabled.

To get started using all of Tower's optional middleware, add this to your
`Cargo.toml`:

```toml
tower = { version = "0.5.1", features = ["full"] }
```

Alternatively, you can only enable some features. For example, to enable
only the [`retry`] and [`timeout`][timeouts] middleware, write:

```toml
tower = { version = "0.5.1", features = ["retry", "timeout"] }
```

See [here][all_layers] for a complete list of all middleware provided by
Tower.

[`Service`]: https://docs.rs/tower/latest/tower/trait.Service.html
[`Layer`]: https://docs.rs/tower/latest/tower/trait.Layer.html
[all_layers]: https://docs.rs/tower/latest/tower/#modules
[timeouts]: https://docs.rs/tower/latest/tower/timeout/
[rate limiting]: https://docs.rs/tower/latest/tower/limit/rate
[load balancing]: https://docs.rs/tower/latest/tower/balance/
[`ServiceBuilder`]: https://docs.rs/tower/latest/tower/struct.ServiceBuilder.html
[utilities]: https://docs.rs/tower/latest/tower/trait.ServiceExt.html
[`tower`]: https://crates.io/crates/tower
[`tower-service`]: https://crates.io/crates/tower-service
[`tower-layer`]: https://crates.io/crates/tower-layer
[`tower-test`]: https://crates.io/crates/tower-test
[`retry`]: https://docs.rs/tower/latest/tower/retry
[open a PR]: https://github.com/tower-rs/tower/compare


## Supported Rust Versions

Tower will keep a rolling MSRV (minimum supported Rust version) policy of **at
least** 6 months. When increasing the MSRV, the new Rust version must have been
released at least six months ago. The current MSRV is 1.64.0.

## License

This project is licensed under the [MIT license](LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Tower by you, shall be licensed as MIT, without any additional
terms or conditions.
