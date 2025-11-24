//! HTTP client utilities

/// Legacy implementations of `connect` module and `Client`
#[cfg(feature = "client-legacy")]
pub mod legacy;

#[cfg(feature = "client-proxy")]
pub mod proxy;
