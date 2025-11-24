//!  Server connection API.
//!
//! The types in this module are to provide a lower-level API based around a
//! single connection. Accepting a connection and binding it with a service
//! are not handled at this level. This module provides the building blocks to
//! customize those things externally.
//!
//! This module is split by HTTP version, providing a connection builder for
//! each. They work similarly, but they each have specific options.
//!
//! If your server needs to support both versions, an auto-connection builder is
//! provided in the [`hyper-util`](https://github.com/hyperium/hyper-util/tree/master)
//! crate. This builder wraps the HTTP/1 and HTTP/2 connection builders from this
//! module, allowing you to set configuration for both. The builder will then check
//! the version of the incoming connection and serve it accordingly.

#[cfg(feature = "http1")]
pub mod http1;
#[cfg(feature = "http2")]
pub mod http2;
