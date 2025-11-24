use std::sync::Arc;

use super::ConcurrencyLimit;
use tokio::sync::Semaphore;
use tower_layer::Layer;

/// Enforces a limit on the concurrent number of requests the underlying
/// service can handle.
#[derive(Debug, Clone)]
pub struct ConcurrencyLimitLayer {
    max: usize,
}

impl ConcurrencyLimitLayer {
    /// Create a new concurrency limit layer.
    pub const fn new(max: usize) -> Self {
        ConcurrencyLimitLayer { max }
    }
}

impl<S> Layer<S> for ConcurrencyLimitLayer {
    type Service = ConcurrencyLimit<S>;

    fn layer(&self, service: S) -> Self::Service {
        ConcurrencyLimit::new(service, self.max)
    }
}

/// Enforces a limit on the concurrent number of requests the underlying
/// service can handle.
///
/// Unlike [`ConcurrencyLimitLayer`], which enforces a per-service concurrency
/// limit, this layer accepts a owned semaphore (`Arc<Semaphore>`) which can be
/// shared across multiple services.
///
/// Cloning this layer will not create a new semaphore.
#[derive(Debug, Clone)]
pub struct GlobalConcurrencyLimitLayer {
    semaphore: Arc<Semaphore>,
}

impl GlobalConcurrencyLimitLayer {
    /// Create a new `GlobalConcurrencyLimitLayer`.
    pub fn new(max: usize) -> Self {
        Self::with_semaphore(Arc::new(Semaphore::new(max)))
    }

    /// Create a new `GlobalConcurrencyLimitLayer` from a `Arc<Semaphore>`
    pub fn with_semaphore(semaphore: Arc<Semaphore>) -> Self {
        GlobalConcurrencyLimitLayer { semaphore }
    }
}

impl<S> Layer<S> for GlobalConcurrencyLimitLayer {
    type Service = ConcurrencyLimit<S>;

    fn layer(&self, service: S) -> Self::Service {
        ConcurrencyLimit::with_semaphore(service, self.semaphore.clone())
    }
}
