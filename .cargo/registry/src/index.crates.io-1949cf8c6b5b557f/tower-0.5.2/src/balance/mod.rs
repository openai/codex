//! Middleware that allows balancing load among multiple services.
//!
//! In larger systems, multiple endpoints are often available for a given service. As load
//! increases, you want to ensure that that load is spread evenly across the available services.
//! Otherwise, clients could see spikes in latency if their request goes to a particularly loaded
//! service, even when spare capacity is available to handle that request elsewhere.
//!
//! This module provides the [`p2c`] middleware, which implements the "[Power of
//! Two Random Choices]" algorithm. This is a simple but robust  technique for
//! spreading load across services with only inexact load measurements. Use this
//! if the set of available services is not within your control, and you simply
//! want to spread load among that set of services.
//!
//! [Power of Two Random Choices]: http://www.eecs.harvard.edu/~michaelm/postscripts/handbook2001.pdf
//!
//! # Examples
//!
//! ```rust
//! # #[cfg(feature = "util")]
//! # #[cfg(feature = "load")]
//! # fn warnings_are_errors() {
//! use tower::balance::p2c::Balance;
//! use tower::load::Load;
//! use tower::{Service, ServiceExt};
//! use futures_util::pin_mut;
//! # use futures_core::Stream;
//! # use futures_util::StreamExt;
//!
//! async fn spread<Req, S: Service<Req> + Load>(svc1: S, svc2: S, reqs: impl Stream<Item = Req>)
//! where
//!     S::Error: Into<tower::BoxError>,
//! # // this bound is pretty unfortunate, and the compiler does _not_ help
//!     S::Metric: std::fmt::Debug,
//! {
//!     // Spread load evenly across the two services
//!     let p2c = Balance::new(tower::discover::ServiceList::new(vec![svc1, svc2]));
//!
//!     // Issue all the requests that come in.
//!     // Some will go to svc1, some will go to svc2.
//!     pin_mut!(reqs);
//!     let mut responses = p2c.call_all(reqs);
//!     while let Some(rsp) = responses.next().await {
//!         // ...
//!     }
//! }
//! # }
//! ```

pub mod error;
pub mod p2c;
