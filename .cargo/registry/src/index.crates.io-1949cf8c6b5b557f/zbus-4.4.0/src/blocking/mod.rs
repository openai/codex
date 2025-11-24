//! The blocking API.
//!
//! This module hosts all our blocking API. All the types under this module are thin wrappers
//! around the corresponding asynchronous types. Most of the method calls are simply calling their
//! asynchronous counterparts on the underlying types and use [`async_io::block_on`] to turn them
//! into blocking calls.
//!
//! # Caveats
//!
//! Since methods provided by these types run their own little runtime (`block_on`), you must not
//! use them in async contexts because of the infamous [async sandwich footgun][asf]. This is
//! an especially important fact to keep in mind for [`crate::interface`]. While
//! `dbus_interface` allows non-async methods for convenience, these methods are called from an
//! async context. The [`blocking` crate] provides an easy way around this problem though.
//!
//! [asf]: https://rust-lang.github.io/wg-async/vision/shiny_future/users_manual.html#caveat-beware-the-async-sandwich
//! [`blocking` crate]: https://docs.rs/blocking/

pub mod connection;
pub use connection::Connection;

mod message_iterator;
pub use message_iterator::*;
pub mod object_server;
pub use object_server::ObjectServer;
pub mod proxy;
pub use proxy::Proxy;

#[deprecated(since = "4.0.0", note = "Use `proxy::Builder` instead")]
#[doc(hidden)]
pub use proxy::Builder as ProxyBuilder;
#[deprecated(since = "4.0.0", note = "Use `proxy::OwnerChangedIterator` instead")]
#[doc(hidden)]
pub use proxy::OwnerChangedIterator;
#[deprecated(since = "4.0.0", note = "Use `proxy::PropertyChanged` instead")]
#[doc(hidden)]
pub use proxy::PropertyChanged;
#[deprecated(since = "4.0.0", note = "Use `proxy::PropertyIterator` instead")]
#[doc(hidden)]
pub use proxy::PropertyIterator;
#[deprecated(since = "4.0.0", note = "Use `proxy::SignalIterator` instead")]
#[doc(hidden)]
pub use proxy::SignalIterator;

#[deprecated(since = "4.0.0", note = "Use `object_server::InterfaceRef` instead")]
#[doc(hidden)]
pub use object_server::InterfaceRef;

#[deprecated(since = "4.0.0", note = "Use `connection::Builder` instead")]
#[doc(hidden)]
pub use connection::Builder as ConnectionBuilder;

pub mod fdo;
