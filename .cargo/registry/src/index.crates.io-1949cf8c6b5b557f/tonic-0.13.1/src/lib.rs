//! A Rust implementation of [gRPC], a high performance, open source, general
//! RPC framework that puts mobile and HTTP/2 first.
//!
//! [`tonic`] is a gRPC over HTTP/2 implementation focused on **high
//! performance**, **interoperability**, and **flexibility**. This library was
//! created to have first class support of async/await and to act as a core building
//! block for production systems written in Rust.
//!
//! # Examples
//!
//! Examples can be found in the [`tonic-examples`] crate.
//!
//! # Getting Started
//!
//! Follow the instructions in the [`tonic-build`] crate documentation.
//!
//! # Feature Flags
//!
//! - `transport`: Enables the fully featured, batteries included client and server
//!   implementation based on [`hyper`], [`tower`] and [`tokio`]. This enables `server`
//!   and `channel` features. Enabled by default.
//! - `server`: Enables just the full featured server portion of the `transport` feature.
//! - `channel`: Enables just the full featured channel portion of the `transport` feature.
//! - `router`: Enables the [`axum`] based service router. Enabled by default.
//! - `codegen`: Enables all the required exports and optional dependencies required
//!   for [`tonic-build`]. Enabled by default.
//! - `tls-ring`: Enables the [`rustls`] based TLS options for the `transport` feature using
//!   the [`ring`] libcrypto provider. Not enabled by default.
//! - `tls-aws-lc`: Enables the [`rustls`] based TLS options for the `transport` feature using
//!   the [`aws-lc-rs`] libcrypto provider. Not enabled by default.
//! - `tls-native-roots`: Adds system trust roots to [`rustls`]-based gRPC clients using the
//!   [`rustls-native-certs`] crate. Not enabled by default.
//! - `tls-webpki-roots`: Add the standard trust roots from the [`webpki-roots`] crate to
//!   `rustls`-based gRPC clients. Not enabled by default.
//! - `prost`: Enables the [`prost`] based gRPC [`Codec`] implementation. Enabled by default.
//! - `gzip`: Enables compressing requests, responses, and streams. Depends on [`flate2`].
//!   Not enabled by default.
//! - `deflate`: Enables compressing requests, responses, and streams. Depends on [`flate2`].
//!   Not enabled by default.
//! - `zstd`: Enables compressing requests, responses, and streams. Depends on [`zstd`].
//!   Not enabled by default.
//!
//! # Structure
//!
//! ## Generic implementation
//!
//! The main goal of [`tonic`] is to provide a generic gRPC implementation over HTTP/2
//! framing. This means at the lowest level this library provides the ability to use
//! a generic HTTP/2 implementation with different types of gRPC encodings formats. Generally,
//! some form of codegen should be used instead of interacting directly with the items in
//! [`client`] and [`server`].
//!
//! ## Transport
//!
//! The [`transport`] module contains a fully featured HTTP/2.0 [`Channel`] (gRPC terminology)
//! and [`Server`]. These implementations are built on top of [`tokio`], [`hyper`] and [`tower`].
//! It also provides many of the features that the core gRPC libraries provide such as load balancing,
//! tls, timeouts, and many more. This implementation can also be used as a reference implementation
//! to build even more feature rich clients and servers. This module also provides the ability to
//! enable TLS using [`rustls`], via the `tls` feature flag.
//!
//! # Code generated client/server configuration
//!
//! ## Max Message Size
//!
//! Currently, both servers and clients can be configured to set the max message encoding and
//! decoding size. This will ensure that an incoming gRPC message will not exhaust the systems
//! memory. By default, the decoding message limit is `4MB` and the encoding limit is `usize::MAX`.
//!
//! [gRPC]: https://grpc.io
//! [`tonic`]: https://github.com/hyperium/tonic
//! [`tokio`]: https://docs.rs/tokio
//! [`prost`]: https://docs.rs/prost
//! [`hyper`]: https://docs.rs/hyper
//! [`tower`]: https://docs.rs/tower
//! [`tonic-build`]: https://docs.rs/tonic-build
//! [`ring`]: https://docs.rs/ring
//! [`tonic-examples`]: https://github.com/hyperium/tonic/tree/master/examples
//! [`Codec`]: codec/trait.Codec.html
//! [`Channel`]: transport/struct.Channel.html
//! [`Server`]: transport/struct.Server.html
//! [`rustls`]: https://docs.rs/rustls
//! [`client`]: client/index.html
//! [`transport`]: transport/index.html
//! [`rustls-native-certs`]: https://docs.rs/rustls-native-certs
//! [`webpki-roots`]: https://docs.rs/webpki-roots
//! [`flate2`]: https://docs.rs/flate2
//! [`zstd`]: https://docs.rs/zstd

#![recursion_limit = "256"]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/tokio-rs/website/master/public/img/icons/tonic.svg"
)]
#![doc(issue_tracker_base_url = "https://github.com/hyperium/tonic/issues/")]
#![doc(test(no_crate_inject, attr(deny(rust_2018_idioms))))]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

pub mod body;
pub mod client;
pub mod codec;
pub mod metadata;
pub mod server;
pub mod service;

#[cfg(any(feature = "server", feature = "channel"))]
pub mod transport;

mod extensions;
mod macros;
mod request;
mod response;
mod status;
mod util;

/// A re-export of [`async-trait`](https://docs.rs/async-trait) for use with codegen.
#[cfg(feature = "codegen")]
pub use async_trait::async_trait;

#[doc(inline)]
pub use codec::Streaming;
pub use extensions::GrpcMethod;
pub use http::Extensions;
pub use request::{IntoRequest, IntoStreamingRequest, Request};
pub use response::Response;
pub use status::{Code, ConnectError, Status, TimeoutExpired};

pub(crate) type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[doc(hidden)]
#[cfg(feature = "codegen")]
pub mod codegen;

/// `Result` is a type that represents either success ([`Ok`]) or failure ([`Err`]).
/// By default, the Err value is of type [`Status`] but this can be overridden if desired.
pub type Result<T, E = Status> = std::result::Result<T, E>;
