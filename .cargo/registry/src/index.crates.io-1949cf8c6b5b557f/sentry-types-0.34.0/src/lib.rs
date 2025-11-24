//! This crate provides common types for working with the Sentry protocol or the
//! Sentry server.  It's used by the Sentry Relay infrastructure as well as the
//! rust Sentry client.
//!
//! Most of the types in this crate are serializable in one form or another.
//! The types in the `protocol` module are generally really only serializable
//! to JSON as other formats are not supported by Sentry at this date.
//!
//! ## Contents
//!
//! The crate provides a bunch of common types for working with Sentry as
//! such (DSN, ProjectIDs, authentication headers) as well as types for
//! the Sentry event protocol.
//!
//! Right now only `v7` of the protocol is implemented but it's versioned
//! so later versions might be added later.
//!
//! ## API Concepts
//!
//! Most types are directly serializable or deserializable and try to implement
//! the `Default` type.  This means that objects can be created conviently
//! and missing attributes can be filled in:
//!
//! ```rust
//! use sentry_types::protocol::v7;
//!
//! let event = v7::Event {
//!     message: Some("Hello World!".to_string()),
//!     culprit: Some("foo in bar".to_string()),
//!     level: v7::Level::Info,
//!     ..Default::default()
//! };
//! ```

#![doc(html_favicon_url = "https://sentry-brand.storage.googleapis.com/favicon.ico")]
#![doc(html_logo_url = "https://sentry-brand.storage.googleapis.com/sentry-glyph-black.png")]
#![warn(missing_docs)]

#[macro_use]
mod macros;

mod auth;
mod crontab_validator;
mod dsn;
mod project_id;
pub mod protocol;
pub(crate) mod utils;

pub use crate::auth::*;
pub use crate::dsn::*;
pub use crate::project_id::*;

// Re-export external types and traits for convenience
pub use debugid::*;
pub use uuid::{Uuid, Variant as UuidVariant, Version as UuidVersion};

/// Generates a random [Uuid] using a thread local RNG.
pub fn random_uuid() -> Uuid {
    uuid::Builder::from_random_bytes(rand::random()).into_uuid()
}
