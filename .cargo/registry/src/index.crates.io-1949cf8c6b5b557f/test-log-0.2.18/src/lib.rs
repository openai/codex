// Copyright (C) 2019-2025 Daniel Mueller <deso@posteo.net>
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

#![deny(missing_docs)]
#![allow(clippy::test_attr_in_doctest)]

//! A crate providing a replacement #[[macro@test]] attribute that
//! initializes logging and/or tracing infrastructure before running
//! tests.

/// A procedural macro for the `test` attribute.
///
/// The attribute can be used to define a test that has the `env_logger`
/// and/or `tracing` crates initialized (depending on the features used).
///
/// # Example
///
/// Specify the attribute on a per-test basis:
/// ```rust
/// # use logging::info;
/// # // Note that no test would actually run, regardless of `no_run`,
/// # // because we do not invoke the function.
/// #[test_log::test]
/// fn it_works() {
///   info!("Checking whether it still works...");
///   assert_eq!(2 + 2, 4);
///   info!("Looks good!");
/// }
/// ```
///
/// It can be very convenient to convert over all tests by overriding
/// the `#[test]` attribute on a per-module basis:
/// ```rust
/// use test_log::test;
///
/// #[test]
/// fn it_still_works() {
///   // ...
/// }
/// ```
///
/// The crate also supports stacking with other `#[test]` attributes.
/// For example, you can stack
/// [`#[tokio::test]`](https://docs.rs/tokio/1.45.1/tokio/attr.test.html)
/// on top of this crate's `#[test]` attribute and test `async`
/// functionality this way:
///
/// ```rust
/// #[tokio::test]
/// #[test_log::test]
/// async fn it_still_works() {
///   // ...
/// }
/// ```
///
/// Note that stacking `#[test]` attributes this way requires some minimal
/// level of cooperation from the other crate to work properly (see
/// [#46](https://github.com/d-e-s-o/test-log/pull/46) for details), but as
/// a fallback a wrapping style can be used as well:
/// ```rust
/// use test_log::test;
///
/// #[test(tokio::test)]
/// async fn it_also_works() {
///   // ...
/// }
/// ```
pub use test_log_macros::test;

#[cfg(feature = "trace")]
#[doc(hidden)]
pub use tracing_subscriber;

#[cfg(feature = "log")]
#[doc(hidden)]
pub use env_logger;
