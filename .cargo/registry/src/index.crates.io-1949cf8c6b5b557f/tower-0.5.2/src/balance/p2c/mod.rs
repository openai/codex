//! This module implements the "[Power of Two Random Choices]" load balancing algorithm.
//!
//! It is a simple but robust technique for spreading load across services with only inexact load
//! measurements. As its name implies, whenever a request comes in, it samples two ready services
//! at random, and issues the request to whichever service is less loaded. How loaded a service is
//! is determined by the return value of [`Load`](crate::load::Load).
//!
//! As described in the [Finagle Guide][finagle]:
//!
//! > The algorithm randomly picks two services from the set of ready endpoints and
//! > selects the least loaded of the two. By repeatedly using this strategy, we can
//! > expect a manageable upper bound on the maximum load of any server.
//! >
//! > The maximum load variance between any two servers is bound by `ln(ln(n))` where
//! > `n` is the number of servers in the cluster.
//!
//! The balance service and layer implementations rely on _service discovery_ to provide the
//! underlying set of services to balance requests across. This happens through the
//! [`Discover`](crate::discover::Discover) trait, which is essentially a [`Stream`] that indicates
//! when services become available or go away. If you have a fixed set of services, consider using
//! [`ServiceList`](crate::discover::ServiceList).
//!
//! Since the load balancer needs to perform _random_ choices, the constructors in this module
//! usually come in two forms: one that uses randomness provided by the operating system, and one
//! that lets you specify the random seed to use. Usually the former is what you'll want, though
//! the latter may come in handy for reproducibility or to reduce reliance on the operating system.
//!
//! [Power of Two Random Choices]: http://www.eecs.harvard.edu/~michaelm/postscripts/handbook2001.pdf
//! [finagle]: https://twitter.github.io/finagle/guide/Clients.html#power-of-two-choices-p2c-least-loaded
//! [`Stream`]: https://docs.rs/futures/0.3/futures/stream/trait.Stream.html

mod layer;
mod make;
mod service;

#[cfg(test)]
mod test;

pub use layer::MakeBalanceLayer;
pub use make::{MakeBalance, MakeFuture};
pub use service::Balance;
