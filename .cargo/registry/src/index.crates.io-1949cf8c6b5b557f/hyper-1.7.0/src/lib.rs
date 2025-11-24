#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![cfg_attr(test, deny(rust_2018_idioms))]
#![cfg_attr(all(test, feature = "full"), deny(unreachable_pub))]
#![cfg_attr(all(test, feature = "full"), deny(warnings))]
#![cfg_attr(all(test, feature = "nightly"), feature(test))]
#![cfg_attr(docsrs, feature(doc_cfg))]

//! # hyper
//!
//! hyper is a **fast** and **correct** HTTP implementation written in and for Rust.
//!
//! ## Features
//!
//! - HTTP/1 and HTTP/2
//! - Asynchronous design
//! - Leading in performance
//! - Tested and **correct**
//! - Extensive production use
//! - [Client](client/index.html) and [Server](server/index.html) APIs
//!
//! If just starting out, **check out the [Guides](https://hyper.rs/guides/1/)
//! first.**
//!
//! ## "Low-level"
//!
//! hyper is a lower-level HTTP library, meant to be a building block
//! for libraries and applications.
//!
//! If looking for just a convenient HTTP client, consider the
//! [reqwest](https://crates.io/crates/reqwest) crate.
//!
//! # Optional Features
//!
//! hyper uses a set of [feature flags] to reduce the amount of compiled code.
//! It is possible to just enable certain features over others. By default,
//! hyper does not enable any features but allows one to enable a subset for
//! their use case. Below is a list of the available feature flags. You may
//! also notice above each function, struct and trait there is listed one or
//! more feature flags that are required for that item to be used.
//!
//! If you are new to hyper it is possible to enable the `full` feature flag
//! which will enable all public APIs. Beware though that this will pull in
//! many extra dependencies that you may not need.
//!
//! The following optional features are available:
//!
//! - `http1`: Enables HTTP/1 support.
//! - `http2`: Enables HTTP/2 support.
//! - `client`: Enables the HTTP `client`.
//! - `server`: Enables the HTTP `server`.
//!
//! [feature flags]: https://doc.rust-lang.org/cargo/reference/manifest.html#the-features-section
//!
//! ## Unstable Features
//!
//! hyper includes a set of unstable optional features that can be enabled through the use of a
//! feature flag and a [configuration flag].
//!
//! The following is a list of feature flags and their corresponding `RUSTFLAG`:
//!
//! - `ffi`: Enables C API for hyper `hyper_unstable_ffi`.
//! - `tracing`: Enables debug logging with `hyper_unstable_tracing`.
//!
//! For example:
//!
//! ```notrust
//! RUSTFLAGS="--cfg hyper_unstable_tracing" cargo build
//! ```
//!
//! [configuration flag]: https://doc.rust-lang.org/reference/conditional-compilation.html
//!
//! # Stability
//!
//! It's worth talking a bit about the stability of hyper. hyper's API follows
//! [SemVer](https://semver.org). Breaking changes will only be introduced in
//! major versions, if ever. New additions to the API, such as new types,
//! methods, or traits will only be added in minor versions.
//!
//! Some parts of hyper are documented as NOT being part of the stable API. The
//! following is a brief list, you can read more about each one in the relevant
//! part of the documentation.
//!
//! - Downcasting error types from `Error::source()` is not considered stable.
//! - Private dependencies use of global variables is not considered stable.
//!   So, if a dependency uses `log` or `tracing`, hyper doesn't promise it
//!   will continue to do so.
//! - Behavior from default options is not stable. hyper reserves the right to
//!   add new options that are enabled by default which might alter the
//!   behavior, for the purposes of protection. It is also possible to _change_
//!   what the default options are set to, also in efforts to protect the
//!   most people possible.
#[doc(hidden)]
pub use http;

#[cfg(all(test, feature = "nightly"))]
extern crate test;

#[doc(no_inline)]
pub use http::{header, HeaderMap, Method, Request, Response, StatusCode, Uri, Version};

pub use crate::error::{Error, Result};

#[macro_use]
mod cfg;

#[macro_use]
mod trace;

pub mod body;
mod common;
mod error;
pub mod ext;
#[cfg(test)]
mod mock;
pub mod rt;
pub mod service;
pub mod upgrade;

#[cfg(feature = "ffi")]
#[cfg_attr(docsrs, doc(cfg(all(feature = "ffi", hyper_unstable_ffi))))]
pub mod ffi;

cfg_proto! {
    mod headers;
    mod proto;
}

cfg_feature! {
    #![feature = "client"]

    pub mod client;
}

cfg_feature! {
    #![feature = "server"]

    pub mod server;
}
