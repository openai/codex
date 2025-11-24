//! This crate provides support for logging events and errors / panics to the
//! [Sentry] error logging service. It integrates with the standard panic
//! system in Rust as well as a few popular error handling setups.
//!
//! [Sentry]: https://sentry.io/
//!
//! # Quickstart
//!
//! The most convenient way to use this library is via the [`sentry::init`] function,
//! which starts a sentry client with a default set of integrations, and binds
//! it to the current [`Hub`]. More Information on how to use Sentry in parallel,
//! concurrent and async scenarios can be found on the [`Hub`] docs as well.
//!
//! The [`sentry::init`] function returns a guard that when dropped will flush Events that were not
//! yet sent to the sentry service. It has a two second deadline for this so shutdown of
//! applications might slightly delay as a result of this. Keep the guard around or sending events
//! will not work.
//!
//! ```rust
//! let _guard = sentry::init("https://key@sentry.io/42");
//! sentry::capture_message("Hello World!", sentry::Level::Info);
//! // when the guard goes out of scope here, the client will wait up to two
//! // seconds to send remaining events to the service.
//! ```
//!
//! More complex examples on how to use sentry can also be found in [examples]. Extended instructions
//! may also be found on [Sentry itself].
//!
//! [`sentry::init`]: fn.init.html
//! [`Hub`]: struct.Hub.html
//! [examples]: https://github.com/getsentry/sentry-rust/tree/master/sentry/examples
//! [Sentry itself]: https://docs.sentry.io/platforms/rust
//!
//! # Integrations
//!
//! What makes this crate useful are its various integrations. Some of them are enabled by
//! default; See [Features]. Uncommon integrations or integrations for deprecated parts of
//! the ecosystem require a feature flag. For available integrations and how to use them, see
//! [integrations] and [apply_defaults].
//!
//! [Features]: #features
//! [integrations]: integrations/index.html
//! [apply_defaults]: fn.apply_defaults.html
//!
//! # Minimal API
//!
//! This crate comes fully-featured. If the goal is to instrument libraries for usage
//! with sentry, or to extend sentry with a custom [`Integration`] or a [`Transport`],
//! one should use the [`sentry-core`] crate instead.
//!
//! [`Integration`]: trait.Integration.html
//! [`Transport`]: trait.Transport.html
//! [`sentry-core`]: https://crates.io/crates/sentry-core
//!
//! # Features
//!
//! Additional functionality and integrations are enabled via feature flags. Some features require
//! extra setup to function properly.
//!
//! | Feature           | Default | Is Integration | Deprecated | Additional notes                                                                         |
//! | --------------    | ------- | -------------- | ---------- | ---------------------------------------------------------------------------------------- |
//! | `backtrace`       | ✅      | 🔌             |            |                                                                                          |
//! | `contexts`        | ✅      | 🔌             |            |                                                                                          |
//! | `panic`           | ✅      | 🔌             |            |                                                                                          |
//! | `transport`       | ✅      |                |            |                                                                                          |
//! | `anyhow`          |         | 🔌             |            |                                                                                          |
//! | `test`            |         |                |            |                                                                                          |
//! | `debug-images`    | ✅      | 🔌             |            |                                                                                          |
//! | `log`             |         | 🔌             |            | Requires extra setup; See [`sentry-log`]'s documentation.                                |
//! | `debug-logs`      |         |                | ❗         | Requires extra setup; See [`sentry-log`]'s documentation.                                |
//! | `slog`            |         | 🔌             |            | Requires extra setup; See [`sentry-slog`]'s documentation.                               |
//! | `reqwest`         | ✅      |                |            |                                                                                          |
//! | `native-tls`      | ✅      |                |            | `reqwest` must be enabled.                                                               |
//! | `rustls`          |         |                |            | `reqwest` must be enabled. `native-tls` must be disabled via `default-features = false`. |
//! | `curl`            |         |                |            |                                                                                          |
//! | `surf`            |         |                |            |                                                                                          |
//! | `tower`           |         | 🔌             |            | Requires extra setup; See [`sentry-tower`]'s documentation.                              |
//! | `ureq`            |         |                |            | `ureq` transport support using `rustls` by default                                       |
//! | `ureq-native-tls` |         |                |            |                                                                                          |
//!
//! [`sentry-log`]: https://crates.io/crates/sentry-log
//! [`sentry-slog`]: https://crates.io/crates/sentry-slog
//! [`sentry-tower`]: https://crates.io/crates/sentry-tower
//!
//! ## Default features
//! - `backtrace`: Enables backtrace support.
//! - `contexts`: Enables capturing device, OS, and Rust contexts.
//! - `panic`: Enables support for capturing panics.
//! - `transport`: Enables the default transport, which is currently `reqwest` with `native-tls`.
//! - `debug-images`: Enables capturing metadata about the loaded shared libraries.
//!
//! ## Debugging/Testing
//! - `anyhow`: Enables support for the `anyhow` crate.
//! - `test`: Enables testing support.
//!
//! ## Logging
//! - `log`: Enables support for the `log` crate.
//! - `slog`: Enables support for the `slog` crate.
//! - `debug-logs`: **Deprecated**. Uses the `log` crate for internal logging.
//!
//! ## Transports
//! - `reqwest`: **Default**. Enables the `reqwest` transport.
//! - `native-tls`: **Default**. Uses the `native-tls` crate. This only affects the `reqwest` transport.
//! - `rustls`: Enables `rustls` support for `reqwest`. Please note that `native-tls` is a default
//!   feature, and `default-features = false` must be set to completely disable building `native-tls`
//!   dependencies.
//! - `curl`: Enables the `curl` transport.
//! - `surf`: Enables the `surf` transport.
//! - `ureq`: Enables the `ureq` transport using `rustls`.
//! - `ureq-native-tls`: Enables the `ureq` transport using `native-tls`.
//!
//! ## Integrations
//! - `tower`: Enables support for the `tower` crate and those using it.

#![doc(html_favicon_url = "https://sentry-brand.storage.googleapis.com/favicon.ico")]
#![doc(html_logo_url = "https://sentry-brand.storage.googleapis.com/sentry-glyph-black.png")]
#![warn(missing_docs)]
// Only enables the `doc_cfg` feature when the `doc_cfg` configuration attribute
// is defined. Used to expose docs for feature-locked integrations, and other
// feature-gated documentation.
#![cfg_attr(doc_cfg, feature(doc_cfg))]

mod defaults;
mod init;
pub mod transports;

// re-export from core
#[doc(inline)]
pub use sentry_core::*;

// added public API
pub use crate::defaults::apply_defaults;
pub use crate::init::{init, ClientInitGuard};

/// Available Sentry Integrations.
///
/// Integrations extend the functionality of the SDK for some common frameworks and
/// libraries. There are two different kinds of integrations:
/// - Event sources
/// - Event processors
///
/// Integrations which are **event sources** such as
/// [`sentry::integrations::anyhow`] typically provide one or more functions to
/// create new events. These integrations will have an extension trait which exposes
/// a new method on the [`Hub`].
///
/// Integrations which **process events** in some way usually implement the
/// [`Integration`] trait and need to be installed when sentry is initialised.
///
/// # Installing Integrations
///
/// Processing integrations which implement [`Integration`] need to be installed
/// when sentry is initialised. This can be accomplished by using
/// [`ClientOptions::add_integration()`].
///
/// For example, if one were to disable the default integrations (see below) but
/// still wanted to use [`sentry::integrations::debug_images`]:
///
/// ```
/// # #[cfg(feature = "debug-images")] {
/// use sentry::integrations::debug_images::DebugImagesIntegration;
/// use sentry::ClientOptions;
///
/// let options = ClientOptions {
///     default_integrations: false,
///     ..Default::default()
/// }
/// .add_integration(DebugImagesIntegration::new());
/// let _guard = sentry::init(options);
/// # }
/// ```
///
/// # Default Integrations
///
/// The [`ClientOptions::default_integrations`] option is a boolean field that
/// when enabled will enable all of the default integrations via
/// [`apply_defaults()`] **before** any integrations provided by
/// [`ClientOptions::integrations`] are installed. Those interested in a list
/// of default integrations and how they are applied are advised to visit
/// [`apply_defaults()`]'s implementation.
///
/// [`sentry::integrations::anyhow`]: integrations::anyhow
/// [`Integration`]: crate::Integration
/// [`ClientOptions::add_integration()`]: crate::ClientOptions::add_integration
/// [`sentry::integrations::debug_images`]: integrations::debug_images
/// [`ClientOptions::default_integrations`]: crate::ClientOptions::default_integrations
/// [`apply_defaults()`]: ../fn.apply_defaults.html
pub mod integrations {
    #[cfg(feature = "anyhow")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "anyhow")))]
    #[doc(inline)]
    pub use sentry_anyhow as anyhow;
    #[cfg(feature = "backtrace")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "backtrace")))]
    #[doc(inline)]
    pub use sentry_backtrace as backtrace;
    #[cfg(feature = "contexts")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "contexts")))]
    #[doc(inline)]
    pub use sentry_contexts as contexts;
    #[cfg(feature = "debug-images")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "debug_images")))]
    #[doc(inline)]
    pub use sentry_debug_images as debug_images;
    #[cfg(feature = "log")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "log")))]
    #[doc(inline)]
    pub use sentry_log as log;
    #[cfg(feature = "panic")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "panic")))]
    #[doc(inline)]
    pub use sentry_panic as panic;
    #[cfg(feature = "slog")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "slog")))]
    #[doc(inline)]
    pub use sentry_slog as slog;
    #[cfg(feature = "tower")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "tower")))]
    #[doc(inline)]
    pub use sentry_tower as tower;
    #[cfg(feature = "tracing")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "tracing")))]
    #[doc(inline)]
    pub use sentry_tracing as tracing;
}

#[doc(inline)]
pub use sentry_core::types::protocol::latest as protocol;
