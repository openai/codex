//! Rust implementations of the Wayland backends

mod client_impl;
mod server_impl;

mod map;
pub(crate) mod socket;
mod wire;

/// Client-side rust implementation of a Wayland protocol backend
///
/// The main entrypoint is the [`Backend::connect()`][client::Backend::connect()] method.
#[path = "../client_api.rs"]
pub mod client;

/// Server-side rust implementation of a Wayland protocol backend
///
/// The main entrypoint is the [`Backend::new()`][server::Backend::new()] method.
#[path = "../server_api.rs"]
pub mod server;
