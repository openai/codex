use super::{Rate, RateLimit};
use std::time::Duration;
use tower_layer::Layer;

/// Enforces a rate limit on the number of requests the underlying
/// service can handle over a period of time.
#[derive(Debug, Clone)]
pub struct RateLimitLayer {
    rate: Rate,
}

impl RateLimitLayer {
    /// Create new rate limit layer.
    pub const fn new(num: u64, per: Duration) -> Self {
        let rate = Rate::new(num, per);
        RateLimitLayer { rate }
    }
}

impl<S> Layer<S> for RateLimitLayer {
    type Service = RateLimit<S>;

    fn layer(&self, service: S) -> Self::Service {
        RateLimit::new(service, self.rate)
    }
}
