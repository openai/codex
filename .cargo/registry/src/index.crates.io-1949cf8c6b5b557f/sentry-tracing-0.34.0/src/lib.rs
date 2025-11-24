//! Support for automatic breadcrumb, event, and trace capturing from `tracing` events.
//!
//! The `tracing` crate is supported in three ways. First, events can be captured as breadcrumbs for
//! later. Secondly, error events can be captured as events to Sentry. Finally, spans can be
//! recorded as structured transaction events. By default, events above `Info` are recorded as
//! breadcrumbs, events above `Error` are captured as error events, and spans above `Info` are
//! recorded as transactions.
//!
//! # Configuration
//!
//! To fully enable the tracing integration, set the traces sample rate and add a layer to the
//! tracing subscriber:
//!
//! ```
//! use tracing_subscriber::prelude::*;
//!
//! let _guard = sentry::init(sentry::ClientOptions {
//!     // Enable capturing of traces; set this a to lower value in production:
//!     traces_sample_rate: 1.0,
//!     ..sentry::ClientOptions::default()
//! });
//!
//! // Register the Sentry tracing layer to capture breadcrumbs, events, and spans:
//! tracing_subscriber::registry()
//!     .with(tracing_subscriber::fmt::layer())
//!     .with(sentry_tracing::layer())
//!     .init();
//! ```
//!
//! It is also possible to set an explicit filter, to customize which log events are captured by
//! Sentry:
//!
//! ```
//! use sentry_tracing::EventFilter;
//! use tracing_subscriber::prelude::*;
//!
//! let sentry_layer = sentry_tracing::layer().event_filter(|md| match md.level() {
//!     &tracing::Level::ERROR => EventFilter::Event,
//!     _ => EventFilter::Ignore,
//! });
//!
//! tracing_subscriber::registry()
//!     .with(tracing_subscriber::fmt::layer())
//!     .with(sentry_layer)
//!     .init();
//! ```
//!
//! # Logging Messages
//!
//! Tracing events automatically create breadcrumbs that are attached to the current scope in
//! Sentry. They show up on errors and transactions captured within this scope as shown in the
//! examples below.
//!
//! Fields passed to the event macro are automatically tracked as structured data in Sentry. For
//! breadcrumbs, they are shown directly with the breadcrumb message. For other types of data, read
//! below.
//!
//! ```
//! for i in 0..10 {
//!     tracing::debug!(number = i, "Generates a breadcrumb");
//! }
//! ```
//!
//! # Tracking Errors
//!
//! The easiest way to emit errors is by logging an event with `ERROR` level. This will create a
//! grouped issue in Sentry. To add custom information, prepend the message with fields. It is also
//! possible to add Sentry tags if a field is prefixed with `"tags."`
//!
//! ```
//! tracing::error!(
//!     field = "value",                  // will become a context field
//!     tags.custom = "value",            // will become a tag in Sentry
//!     "this is an error with a custom tag",
//! );
//! ```
//!
//! To track [error structs](std::error::Error), assign a reference to error trait object as field
//! in one of the logging macros. By convention, it is recommended to use the `ERROR` level and
//! assign it to a field called `error`, although the integration will also work with all other
//! levels and field names.
//!
//! All other fields passed to the macro are captured in a custom "Tracing Fields" context in
//! Sentry.
//!
//! ```
//! use std::error::Error;
//! use std::io;
//!
//! let custom_error = io::Error::new(io::ErrorKind::Other, "oh no");
//! tracing::error!(error = &custom_error as &dyn Error);
//! ```
//!
//! It is also possible to combine error messages with error structs. In Sentry, this creates issues
//! grouped by the message and location of the error log, and adds the passed error as nested
//! source.
//!
//! ```
//! use std::error::Error;
//! use std::io;
//!
//! let custom_error = io::Error::new(io::ErrorKind::Other, "oh no");
//! tracing::error!(error = &custom_error as &dyn Error, "my operation failed");
//! ```
//!
//! # Tracing Spans
//!
//! The integration automatically tracks `tracing` spans as spans in Sentry. A convenient way to do
//! this is with the `#[instrument]` attribute macro, which creates a transaction for the function
//! in Sentry.
//!
//! Function arguments are added as context fields automatically, which can be configured through
//! attribute arguments. Refer to documentation of the macro for more information.
//!
//! ```
//! use std::time::Duration;
//!
//! use tracing_subscriber::prelude::*;
//!
//! // Functions instrumented by tracing automatically report
//! // their span as transactions.
//! #[tracing::instrument]
//! async fn outer() {
//!     for i in 0..10 {
//!         inner(i).await;
//!     }
//! }
//!
//! // This creates spans inside the outer transaction, unless called directly.
//! #[tracing::instrument]
//! async fn inner(i: u32) {
//!     // Also works, since log events are ingested by the tracing system
//!     tracing::debug!(number = i, "Generates a breadcrumb");
//!
//!     tokio::time::sleep(Duration::from_millis(100)).await;
//! }
//! ```

#![doc(html_favicon_url = "https://sentry-brand.storage.googleapis.com/favicon.ico")]
#![doc(html_logo_url = "https://sentry-brand.storage.googleapis.com/sentry-glyph-black.png")]
#![warn(missing_docs)]

mod converters;
mod layer;

pub use converters::*;
pub use layer::*;
