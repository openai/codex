//! Service utilities.
//!
//! [`hyper::service`] provides a [`Service`][hyper-svc] trait, representing an asynchronous
//! function from a `Request` to a `Response`. This provides an interface allowing middleware for
//! network application to be written in a modular and reusable way.
//!
//! This submodule provides an assortment of utilities for working with [`Service`][hyper-svc]s.
//! See the module-level documentation of [`hyper::service`] for more information.
//!
//! # Tower
//!
//! While [`hyper`] uses its own notion of a [`Service`][hyper-svc] internally, many other
//! libraries use a library such as [`tower`][tower] to provide the fundamental model of an
//! asynchronous function.
//!
//! The [`TowerToHyperService`] type provided by this submodule can be used to bridge these
//! ecosystems together. By wrapping a [`tower::Service`][tower-svc] in [`TowerToHyperService`],
//! it can be passed into [`hyper`] interfaces that expect a [`hyper::service::Service`].
//!
//! [hyper-svc]: hyper::service::Service
//! [tower]: https://docs.rs/tower/latest/tower/
//! [tower-svc]: https://docs.rs/tower/latest/tower/trait.Service.html

#[cfg(feature = "service")]
mod glue;
#[cfg(any(feature = "client-legacy", feature = "service"))]
mod oneshot;

#[cfg(feature = "service")]
pub use self::glue::{TowerToHyperService, TowerToHyperServiceFuture};
#[cfg(any(feature = "client-legacy", feature = "service"))]
pub(crate) use self::oneshot::Oneshot;
