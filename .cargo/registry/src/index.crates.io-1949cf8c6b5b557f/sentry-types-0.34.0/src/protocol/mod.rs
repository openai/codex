//! This module exposes the types for the Sentry protocol in different versions.

// We would like to reserve the possibility to add floating point numbers to
// protocol types without breaking API (removing Eq) in the future.
#![allow(clippy::derive_partial_eq_without_eq)]

#[cfg(feature = "protocol")]
pub mod v7;

/// The latest version of the protocol.
pub const LATEST: u16 = 7;

#[cfg(feature = "protocol")]
pub use v7 as latest;

mod attachment;
mod envelope;
mod monitor;
mod session;
