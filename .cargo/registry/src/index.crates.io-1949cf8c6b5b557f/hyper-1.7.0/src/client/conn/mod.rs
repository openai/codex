//! Lower-level client connection API.
//!
//! The types in this module are to provide a lower-level API based around a
//! single connection. Connecting to a host, pooling connections, and the like
//! are not handled at this level. This module provides the building blocks to
//! customize those things externally.
//!
//! If you are looking for a convenient HTTP client, then you may wish to
//! consider [reqwest](https://github.com/seanmonstar/reqwest) for a high level
//! client or [`hyper-util`'s client](https://docs.rs/hyper-util/latest/hyper_util/client/index.html)
//! if you want to keep it more low level / basic.
//!
//! ## Example
//!
//! See the [client guide](https://hyper.rs/guides/1/client/basic/).

#[cfg(feature = "http1")]
pub mod http1;
#[cfg(feature = "http2")]
pub mod http2;

pub use super::dispatch::TrySendError;
