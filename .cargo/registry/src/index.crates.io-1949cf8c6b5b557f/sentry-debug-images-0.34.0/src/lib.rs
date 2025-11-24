//! The Sentry Debug Images integration.
//!
//! The [`DebugImagesIntegration`] adds metadata about the loaded shared
//! libraries to Sentry [`Event`]s.
//!
//! # Configuration
//!
//! The integration by default attaches this information to all [`Event`]s, but
//! a custom filter can be defined as well.
//!
//! ```rust
//! use sentry_core::Level;
//! let integration = sentry_debug_images::DebugImagesIntegration::new()
//!     .filter(|event| event.level >= Level::Warning);
//! ```
//!
//! [`Event`]: sentry_core::protocol::Event

#![doc(html_favicon_url = "https://sentry-brand.storage.googleapis.com/favicon.ico")]
#![doc(html_logo_url = "https://sentry-brand.storage.googleapis.com/sentry-glyph-black.png")]
#![warn(missing_docs)]
#![deny(unsafe_code)]

mod images;
mod integration;

pub use images::debug_images;
pub use integration::DebugImagesIntegration;
