#![warn(
    missing_debug_implementations,
    missing_docs,
    rust_2018_idioms,
    unreachable_pub
)]
#![forbid(unsafe_code)]
#![allow(elided_lifetimes_in_paths, clippy::type_complexity)]
#![cfg_attr(test, allow(clippy::float_cmp))]
#![cfg_attr(docsrs, feature(doc_auto_cfg, doc_cfg))]
// `rustdoc::broken_intra_doc_links` is checked on CI

//! `async fn(Request) -> Result<Response, Error>`
//!
//! # Overview
//!
//! Tower is a library of modular and reusable components for building
//! robust networking clients and servers.
//!
//! Tower provides a simple core abstraction, the [`Service`] trait, which
//! represents an asynchronous function taking a request and returning either a
//! response or an error. This abstraction can be used to model both clients and
//! servers.
//!
//! Generic components, like [`timeout`], [rate limiting], and [load balancing],
//! can be modeled as [`Service`]s that wrap some inner service and apply
//! additional behavior before or after the inner service is called. This allows
//! implementing these components in a protocol-agnostic, composable way. Typically,
//! such services are referred to as _middleware_.
//!
//! An additional abstraction, the [`Layer`] trait, is used to compose
//! middleware with [`Service`]s. If a [`Service`] can be thought of as an
//! asynchronous function from a request type to a response type, a [`Layer`] is
//! a function taking a [`Service`] of one type and returning a [`Service`] of a
//! different type. The [`ServiceBuilder`] type is used to add middleware to a
//! service by composing it with multiple [`Layer`]s.
//!
//! ## The Tower Ecosystem
//!
//! Tower is made up of the following crates:
//!
//! * [`tower`] (this crate)
//! * [`tower-service`]
//! * [`tower-layer`]
//! * [`tower-test`]
//!
//! Since the [`Service`] and [`Layer`] traits are important integration points
//! for all libraries using Tower, they are kept as stable as possible, and
//! breaking changes are made rarely. Therefore, they are defined in separate
//! crates, [`tower-service`] and [`tower-layer`]. This crate contains
//! re-exports of those core traits, implementations of commonly-used
//! middleware, and [utilities] for working with [`Service`]s and [`Layer`]s.
//! Finally, the [`tower-test`] crate provides tools for testing programs using
//! Tower.
//!
//! # Usage
//!
//! Tower provides an abstraction layer, and generic implementations of various
//! middleware. This means that the `tower` crate on its own does *not* provide
//! a working implementation of a network client or server. Instead, Tower's
//! [`Service` trait][`Service`] provides an integration point between
//! application code, libraries providing middleware implementations, and
//! libraries that implement servers and/or clients for various network
//! protocols.
//!
//! Depending on your particular use case, you might use Tower in several ways:
//!
//! * **Implementing application logic** for a networked program. You might
//!   use the [`Service`] trait to model your application's behavior, and use
//!   the middleware [provided by this crate](#modules) and by other libraries
//!   to add functionality to clients and servers provided by one or more
//!   protocol implementations.
//! * **Implementing middleware** to add custom behavior to network clients and
//!   servers in a reusable manner. This might be general-purpose middleware
//!   (and if it is, please consider releasing your middleware as a library for
//!   other Tower users!) or application-specific behavior that needs to be
//!   shared between multiple clients or servers.
//! * **Implementing a network protocol**. Libraries that implement network
//!   protocols (such as HTTP) can depend on `tower-service` to use the
//!   [`Service`] trait as an integration point between the protocol and user
//!   code. For example, a client for some protocol might implement [`Service`],
//!   allowing users to add arbitrary Tower middleware to those clients.
//!   Similarly, a server might be created from a user-provided [`Service`].
//!
//!   Additionally, when a network protocol requires functionality already
//!   provided by existing Tower middleware, a protocol implementation might use
//!   Tower middleware internally, as well as as an integration point.
//!
//! ## Library Support
//!
//! A number of third-party libraries support Tower and the [`Service`] trait.
//! The following is an incomplete list of such libraries:
//!
//! * [`hyper`]: A fast and correct low-level HTTP implementation.
//! * [`tonic`]: A [gRPC-over-HTTP/2][grpc] implementation built on top of
//!   [`hyper`]. See [here][tonic-examples] for examples of using [`tonic`] with
//!   Tower.
//! * [`warp`]: A lightweight, composable web framework. See
//!   [here][warp-service] for details on using [`warp`] with Tower.
//! * [`tower-lsp`]: implementations of the [Language
//!   Server Protocol][lsp] based on Tower.
//!
//! [`hyper`]: https://crates.io/crates/hyper
//! [`tonic`]: https://crates.io/crates/tonic
//! [tonic-examples]: https://github.com/hyperium/tonic/tree/master/examples/src/tower
//! [grpc]: https://grpc.io
//! [`warp`]: https://crates.io/crates/warp
//! [warp-service]: https://docs.rs/warp/0.2.5/warp/fn.service.html
//! [`tower-lsp`]: https://crates.io/crates/tower-lsp
//! [lsp]: https://microsoft.github.io/language-server-protocol/
//!
//! If you're the maintainer of a crate that supports Tower, we'd love to add
//! your crate to this list! Please [open a PR] adding a brief description of
//! your library!
//!
//! ## Getting Started
//!
//! If you're brand new to Tower and want to start with the basics, we recommend you
//! check out some of our [guides].
//!
//! The various middleware implementations provided by this crate are feature
//! flagged, so that users can only compile the parts of Tower they need. By
//! default, all the optional middleware are disabled.
//!
//! To get started using all of Tower's optional middleware, add this to your
//! `Cargo.toml`:
//!
//! ```toml
//! tower = { version = "0.4", features = ["full"] }
//! ```
//!
//! Alternatively, you can only enable some features. For example, to enable
//! only the [`retry`] and [`timeout`] middleware, write:
//!
//! ```toml
//! tower = { version = "0.4", features = ["retry", "timeout"] }
//! ```
//!
//! See [here](#modules) for a complete list of all middleware provided by
//! Tower.
//!
//!
//! ## Supported Rust Versions
//!
//! Tower will keep a rolling MSRV (minimum supported Rust version) policy of **at
//! least** 6 months. When increasing the MSRV, the new Rust version must have been
//! released at least six months ago. The current MSRV is 1.64.0.
//!
//! [`Service`]: crate::Service
//! [`Layer`]: crate::Layer
//! [rate limiting]: crate::limit::rate
//! [load balancing]: crate::balance
//! [`ServiceBuilder`]: crate::ServiceBuilder
//! [utilities]: crate::ServiceExt
//! [`tower`]: https://crates.io/crates/tower
//! [`tower-service`]: https://crates.io/crates/tower-service
//! [`tower-layer`]: https://crates.io/crates/tower-layer
//! [`tower-test`]: https://crates.io/crates/tower-test
//! [`retry`]: crate::retry
//! [open a PR]: https://github.com/tower-rs/tower/compare
//! [guides]: https://github.com/tower-rs/tower/tree/master/guides

#[macro_use]
pub(crate) mod macros;
#[cfg(feature = "balance")]
pub mod balance;
#[cfg(feature = "buffer")]
pub mod buffer;
#[cfg(feature = "discover")]
pub mod discover;
#[cfg(feature = "filter")]
pub mod filter;
#[cfg(feature = "hedge")]
pub mod hedge;
#[cfg(feature = "limit")]
pub mod limit;
#[cfg(feature = "load")]
pub mod load;
#[cfg(feature = "load-shed")]
pub mod load_shed;

#[cfg(feature = "make")]
pub mod make;
#[cfg(feature = "ready-cache")]
pub mod ready_cache;
#[cfg(feature = "reconnect")]
pub mod reconnect;
#[cfg(feature = "retry")]
pub mod retry;
#[cfg(feature = "spawn-ready")]
pub mod spawn_ready;
#[cfg(feature = "steer")]
pub mod steer;
#[cfg(feature = "timeout")]
pub mod timeout;
#[cfg(feature = "util")]
pub mod util;

pub mod builder;
pub mod layer;

#[cfg(feature = "util")]
#[doc(inline)]
#[cfg_attr(docsrs, doc(cfg(feature = "util")))]
pub use self::util::{service_fn, ServiceExt};

#[doc(inline)]
pub use crate::builder::ServiceBuilder;

#[cfg(feature = "make")]
#[doc(inline)]
#[cfg_attr(docsrs, doc(cfg(feature = "make")))]
pub use crate::make::MakeService;

#[doc(inline)]
pub use tower_layer::Layer;

#[doc(inline)]
pub use tower_service::Service;

#[allow(unreachable_pub)]
#[cfg(any(feature = "balance", feature = "discover", feature = "make"))]
mod sealed {
    pub trait Sealed<T> {}
}

/// Alias for a type-erased error type.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;
