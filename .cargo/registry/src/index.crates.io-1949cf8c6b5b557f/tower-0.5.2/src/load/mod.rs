//! Service load measurement
//!
//! This module provides the [`Load`] trait, which allows measuring how loaded a service is.
//! It also provides several wrapper types that measure load in different ways:
//!
//! - [`Constant`] — Always returns the same constant load value for a service.
//! - [`PendingRequests`] — Measures load by tracking the number of in-flight requests.
//! - [`PeakEwma`] — Measures load using a moving average of the peak latency for the service.
//!
//! In general, you will want to use one of these when using the types in [`tower::balance`] which
//! balance services depending on their load. Which load metric to use depends on your exact
//! use-case, but the ones above should get you quite far!
//!
//! When the `discover` feature is enabled, wrapper types for [`Discover`] that
//! wrap the discovered services with the given load estimator are also provided.
//!
//! # When does a request complete?
//!
//! For many applications, the request life-cycle is relatively simple: when a service responds to
//! a request, that request is done, and the system can forget about it. However, for some
//! applications, the service may respond to the initial request while other parts of the system
//! are still acting on that request. In such an application, the system load must take these
//! requests into account as well, or risk the system underestimating its own load.
//!
//! To support these use-cases, the load estimators in this module are parameterized by the
//! [`TrackCompletion`] trait, with [`CompleteOnResponse`] as the default type. The behavior of
//! [`CompleteOnResponse`] is what you would normally expect for a request-response cycle: when the
//! response is produced, the request is considered "finished", and load goes down. This can be
//! overridden by your own user-defined type to track more complex request completion semantics. See
//! the documentation for [`completion`] for more details.
//!
//! # Examples
//!
//! ```rust
//! # #[cfg(feature = "util")]
//! use tower::util::ServiceExt;
//! # #[cfg(feature = "util")]
//! use tower::{load::Load, Service};
//! # #[cfg(feature = "util")]
//! async fn simple_balance<S1, S2, R>(
//!     svc1: &mut S1,
//!     svc2: &mut S2,
//!     request: R
//! ) -> Result<S1::Response, S1::Error>
//! where
//!     S1: Load + Service<R>,
//!     S2: Load<Metric = S1::Metric> + Service<R, Response = S1::Response, Error = S1::Error>
//! {
//!     if svc1.load() < svc2.load() {
//!         svc1.ready().await?.call(request).await
//!     } else {
//!         svc2.ready().await?.call(request).await
//!     }
//! }
//! ```
//!
//! [`tower::balance`]: crate::balance
//! [`Discover`]: crate::discover::Discover
//! [`CompleteOnResponse`]: crate::load::completion::CompleteOnResponse
// TODO: a custom completion example would be good here

pub mod completion;
mod constant;
pub mod peak_ewma;
pub mod pending_requests;

pub use self::{
    completion::{CompleteOnResponse, TrackCompletion},
    constant::Constant,
    peak_ewma::PeakEwma,
    pending_requests::PendingRequests,
};

#[cfg(feature = "discover")]
pub use self::{peak_ewma::PeakEwmaDiscover, pending_requests::PendingRequestsDiscover};

/// Types that implement this trait can give an estimate of how loaded they are.
///
/// See the module documentation for more details.
pub trait Load {
    /// A comparable load metric.
    ///
    /// Lesser values indicate that the service is less loaded, and should be preferred for new
    /// requests over another service with a higher value.
    type Metric: PartialOrd;

    /// Estimate the service's current load.
    fn load(&self) -> Self::Metric;
}
