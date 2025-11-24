//! Limit the rate at which requests are processed.

mod layer;
#[allow(clippy::module_inception)]
mod rate;
mod service;

pub use self::{layer::RateLimitLayer, rate::Rate, service::RateLimit};
