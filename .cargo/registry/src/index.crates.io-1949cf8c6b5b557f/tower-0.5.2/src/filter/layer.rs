use super::{AsyncFilter, Filter};
use tower_layer::Layer;

/// Conditionally dispatch requests to the inner service based on a synchronous
/// [predicate].
///
/// This [`Layer`] produces instances of the [`Filter`] service.
///
/// [predicate]: crate::filter::Predicate
/// [`Layer`]: crate::Layer
/// [`Filter`]: crate::filter::Filter
#[derive(Debug, Clone)]
pub struct FilterLayer<U> {
    predicate: U,
}

/// Conditionally dispatch requests to the inner service based on an asynchronous
/// [predicate].
///
/// This [`Layer`] produces instances of the [`AsyncFilter`] service.
///
/// [predicate]: crate::filter::AsyncPredicate
/// [`Layer`]: crate::Layer
/// [`Filter`]: crate::filter::AsyncFilter
#[derive(Debug, Clone)]
pub struct AsyncFilterLayer<U> {
    predicate: U,
}

// === impl FilterLayer ===

impl<U> FilterLayer<U> {
    /// Returns a new layer that produces [`Filter`] services with the given
    /// [`Predicate`].
    ///
    /// [`Predicate`]: crate::filter::Predicate
    /// [`Filter`]: crate::filter::Filter
    pub const fn new(predicate: U) -> Self {
        Self { predicate }
    }
}

impl<U: Clone, S> Layer<S> for FilterLayer<U> {
    type Service = Filter<S, U>;

    fn layer(&self, service: S) -> Self::Service {
        let predicate = self.predicate.clone();
        Filter::new(service, predicate)
    }
}

// === impl AsyncFilterLayer ===

impl<U> AsyncFilterLayer<U> {
    /// Returns a new layer that produces [`AsyncFilter`] services with the given
    /// [`AsyncPredicate`].
    ///
    /// [`AsyncPredicate`]: crate::filter::AsyncPredicate
    /// [`Filter`]: crate::filter::Filter
    pub const fn new(predicate: U) -> Self {
        Self { predicate }
    }
}

impl<U: Clone, S> Layer<S> for AsyncFilterLayer<U> {
    type Service = AsyncFilter<S, U>;

    fn layer(&self, service: S) -> Self::Service {
        let predicate = self.predicate.clone();
        AsyncFilter::new(service, predicate)
    }
}
