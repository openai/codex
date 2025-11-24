//! Service discovery
//!
//! This module provides the [`Change`] enum, which indicates the arrival or departure of a service
//! from a collection of similar services. Most implementations should use the [`Discover`] trait
//! in their bounds to indicate that they can handle services coming and going. [`Discover`] itself
//! is primarily a convenience wrapper around [`TryStream<Ok = Change>`][`TryStream`].
//!
//! Every discovered service is assigned an identifier that is distinct among the currently active
//! services. If that service later goes away, a [`Change::Remove`] is yielded with that service's
//! identifier. From that point forward, the identifier may be re-used.
//!
//! # Examples
//!
//! ```rust
//! use futures_util::{future::poll_fn, pin_mut};
//! use tower::discover::{Change, Discover};
//! async fn services_monitor<D: Discover>(services: D) {
//!     pin_mut!(services);
//!     while let Some(Ok(change)) = poll_fn(|cx| services.as_mut().poll_discover(cx)).await {
//!         match change {
//!             Change::Insert(key, svc) => {
//!                 // a new service with identifier `key` was discovered
//!                 # let _ = (key, svc);
//!             }
//!             Change::Remove(key) => {
//!                 // the service with identifier `key` has gone away
//!                 # let _ = (key);
//!             }
//!         }
//!     }
//! }
//! ```
//!
//! [`TryStream`]: https://docs.rs/futures/latest/futures/stream/trait.TryStream.html

mod list;

pub use self::list::ServiceList;

use crate::sealed::Sealed;
use futures_core::TryStream;
use std::{
    pin::Pin,
    task::{Context, Poll},
};

/// A dynamically changing set of related services.
///
/// As new services arrive and old services are retired,
/// [`Change`]s are returned which provide unique identifiers
/// for the services.
///
/// See the module documentation for more details.
pub trait Discover: Sealed<Change<(), ()>> {
    /// A unique identifier for each active service.
    ///
    /// An identifier can be re-used once a [`Change::Remove`] has been yielded for its service.
    type Key: Eq;

    /// The type of [`Service`] yielded by this [`Discover`].
    ///
    /// [`Service`]: crate::Service
    type Service;

    /// Error produced during discovery
    type Error;

    /// Yields the next discovery change set.
    fn poll_discover(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Change<Self::Key, Self::Service>, Self::Error>>>;
}

impl<K, S, E, D: ?Sized> Sealed<Change<(), ()>> for D
where
    D: TryStream<Ok = Change<K, S>, Error = E>,
    K: Eq,
{
}

impl<K, S, E, D: ?Sized> Discover for D
where
    D: TryStream<Ok = Change<K, S>, Error = E>,
    K: Eq,
{
    type Key = K;
    type Service = S;
    type Error = E;

    fn poll_discover(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<D::Ok, D::Error>>> {
        TryStream::try_poll_next(self, cx)
    }
}

/// A change in the service set.
#[derive(Debug, Clone)]
pub enum Change<K, V> {
    /// A new service identified by key `K` was identified.
    Insert(K, V),
    /// The service identified by key `K` disappeared.
    Remove(K),
}
