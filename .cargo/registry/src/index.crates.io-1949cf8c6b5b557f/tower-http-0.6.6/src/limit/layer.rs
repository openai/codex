use super::RequestBodyLimit;
use tower_layer::Layer;

/// Layer that applies the [`RequestBodyLimit`] middleware that intercepts requests
/// with body lengths greater than the configured limit and converts them into
/// `413 Payload Too Large` responses.
///
/// See the [module docs](crate::limit) for an example.
///
/// [`RequestBodyLimit`]: super::RequestBodyLimit
#[derive(Clone, Copy, Debug)]
pub struct RequestBodyLimitLayer {
    limit: usize,
}

impl RequestBodyLimitLayer {
    /// Create a new `RequestBodyLimitLayer` with the given body length limit.
    pub fn new(limit: usize) -> Self {
        Self { limit }
    }
}

impl<S> Layer<S> for RequestBodyLimitLayer {
    type Service = RequestBodyLimit<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RequestBodyLimit {
            inner,
            limit: self.limit,
        }
    }
}
