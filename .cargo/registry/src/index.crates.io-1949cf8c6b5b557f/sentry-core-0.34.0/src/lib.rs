//! This crate provides the core of the [Sentry] SDK, which can be used to log
//! events and errors.
//!
//! `sentry-core` is meant for integration authors and third-party library authors
//! that want to instrument their code for sentry.
//!
//! Regular users who wish to integrate sentry into their applications should
//! instead use the [`sentry`] crate, which comes with a default transport and
//! a large set of integrations for various third-party libraries.
//!
//! # Core Concepts
//!
//! This crate follows the [Unified API] guidelines and is centered around
//! the concepts of [`Client`], [`Hub`] and [`Scope`], as well as the extension
//! points via the [`Integration`], [`Transport`] and [`TransportFactory`] traits.
//!
//! # Parallelism, Concurrency and Async
//!
//! The main concurrency primitive is the [`Hub`]. In general, all concurrent
//! code, no matter if multithreaded parallelism or futures concurrency, needs
//! to run with its own copy of a [`Hub`]. Even though the [`Hub`] is internally
//! synchronized, using it concurrently may lead to unexpected results up to
//! panics.
//!
//! For threads or tasks that are running concurrently or outlive the current
//! execution context, a new [`Hub`] needs to be created and bound for the computation.
//!
//! ```rust
//! # let rt = tokio::runtime::Runtime::new().unwrap();
//! # rt.block_on(async {
//! use rayon::prelude::*;
//! use sentry::{Hub, SentryFutureExt};
//! use std::sync::Arc;
//!
//! // Parallel multithreaded code:
//! let outer_hub = Hub::current();
//! let results: Vec<_> = [1_u32, 2, 3]
//!     .into_par_iter()
//!     .map(|num| {
//!         let thread_hub = Arc::new(Hub::new_from_top(&outer_hub));
//!         Hub::run(thread_hub, || num * num)
//!     })
//!     .collect();
//!
//! assert_eq!(&results, &[1, 4, 9]);
//!
//! // Concurrent futures code:
//! let futures = [1_u32, 2, 3]
//!     .into_iter()
//!     .map(|num| async move { num * num }.bind_hub(Hub::new_from_top(Hub::current())));
//! let results = futures::future::join_all(futures).await;
//!
//! assert_eq!(&results, &[1, 4, 9]);
//! # });
//! ```
//!
//! For tasks that are not concurrent and do not outlive the current execution
//! context, no *new* [`Hub`] needs to be created, but the current [`Hub`] has
//! to be bound.
//!
//! ```rust
//! # let rt = tokio::runtime::Runtime::new().unwrap();
//! # rt.block_on(async {
//! use sentry::{Hub, SentryFutureExt};
//!
//! // Spawned thread that is being joined:
//! let hub = Hub::current();
//! let result = std::thread::spawn(|| Hub::run(hub, || 1_u32)).join();
//!
//! assert_eq!(result.unwrap(), 1);
//!
//! // Spawned future that is being awaited:
//! let result = tokio::spawn(async { 1_u32 }.bind_hub(Hub::current())).await;
//!
//! assert_eq!(result.unwrap(), 1);
//! # });
//! ```
//!
//! # Minimal API
//!
//! By default, this crate comes with a so-called "minimal" mode. This mode will
//! provide all the APIs needed to instrument code with sentry, and to write
//! sentry integrations, but it will blackhole a lot of operations.
//!
//! In minimal mode some types are restricted in functionality. For instance
//! the [`Client`] is not available and the [`Hub`] does not retain all API
//! functionality.
//!
//! # Features
//!
//! - `feature = "client"`: Activates the [`Client`] type and certain
//!   [`Hub`] functionality.
//! - `feature = "test"`: Activates the [`test`] module, which can be used to
//!   write integration tests. It comes with a test transport which can capture
//!   all sent events for inspection.
//! - `feature = "debug-logs"`: Uses the `log` crate for debug output, instead
//!   of printing to `stderr`. This feature is **deprecated** and will be
//!   replaced by a dedicated log callback in the future.
//!
//! [Sentry]: https://sentry.io/
//! [`sentry`]: https://crates.io/crates/sentry
//! [Unified API]: https://develop.sentry.dev/sdk/unified-api/
//! [`test`]: test/index.html

#![doc(html_favicon_url = "https://sentry-brand.storage.googleapis.com/favicon.ico")]
#![doc(html_logo_url = "https://sentry-brand.storage.googleapis.com/sentry-glyph-black.png")]
#![warn(missing_docs)]

// macros; these need to be first to be used by other modules
#[macro_use]
mod macros;

mod api;
mod breadcrumbs;
mod clientoptions;
mod constants;
mod error;
mod futures;
mod hub;
mod integration;
mod intodsn;
mod performance;
mod scope;
mod transport;

// public api or exports from this crate
pub use crate::api::*;
pub use crate::breadcrumbs::IntoBreadcrumbs;
pub use crate::clientoptions::{ClientOptions, SessionMode};
pub use crate::error::{capture_error, event_from_error, parse_type_from_debug};
pub use crate::futures::{SentryFuture, SentryFutureExt};
pub use crate::hub::Hub;
pub use crate::integration::Integration;
pub use crate::intodsn::IntoDsn;
pub use crate::performance::*;
pub use crate::scope::{Scope, ScopeGuard};
pub use crate::transport::{Transport, TransportFactory};

#[cfg(all(feature = "client", feature = "metrics-cadence1"))]
pub mod cadence;
// client feature
#[cfg(feature = "client")]
mod client;
#[cfg(feature = "client")]
mod hub_impl;
#[cfg(all(feature = "client", feature = "metrics"))]
pub mod metrics;
#[cfg(feature = "client")]
mod session;
#[cfg(all(feature = "client", feature = "metrics"))]
mod units;
#[cfg(feature = "client")]
pub use crate::client::Client;

// test utilities
#[cfg(feature = "test")]
pub mod test;

// public api from other crates
#[doc(inline)]
pub use sentry_types as types;
pub use sentry_types::protocol::v7 as protocol;
pub use sentry_types::protocol::v7::{Breadcrumb, Envelope, Level, User};
