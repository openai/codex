//! Adds Contexts to Sentry Events.
//!
//! This integration is enabled by default in `sentry` and adds `device`, `os`
//! and `rust` contexts to Events, and also sets a `server_name` if it is not
//! already defined.
//!
//! See the [Contexts Interface] documentation for more info.
//!
//! # Examples
//!
//! ```rust
//! let integration = sentry_contexts::ContextIntegration::new().add_os(false);
//! let _sentry = sentry::init(sentry::ClientOptions::new().add_integration(integration));
//! ```
//!
//! [Contexts Interface]: https://develop.sentry.dev/sdk/event-payloads/contexts/

#![doc(html_favicon_url = "https://sentry-brand.storage.googleapis.com/favicon.ico")]
#![doc(html_logo_url = "https://sentry-brand.storage.googleapis.com/sentry-glyph-black.png")]
#![warn(missing_docs)]

mod integration;
/// Contains functions to retrieve various contexts that can be useful
/// to attach to events
pub mod utils;

pub use integration::ContextIntegration;
