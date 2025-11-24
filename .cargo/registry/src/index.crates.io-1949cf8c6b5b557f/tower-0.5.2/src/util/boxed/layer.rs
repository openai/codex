use crate::util::BoxService;
use std::{fmt, sync::Arc};
use tower_layer::{layer_fn, Layer};
use tower_service::Service;

/// A boxed [`Layer`] trait object.
///
/// [`BoxLayer`] turns a layer into a trait object, allowing both the [`Layer`] itself
/// and the output [`Service`] to be dynamic, while having consistent types.
///
/// This [`Layer`] produces [`BoxService`] instances erasing the type of the
/// [`Service`] produced by the wrapped [`Layer`].
///
/// # Example
///
/// `BoxLayer` can, for example, be useful to create layers dynamically that otherwise wouldn't have
/// the same types. In this example, we include a [`Timeout`] layer
/// only if an environment variable is set. We can use `BoxLayer`
/// to return a consistent type regardless of runtime configuration:
///
/// ```
/// use std::time::Duration;
/// use tower::{Service, ServiceBuilder, BoxError, util::BoxLayer};
///
/// fn common_layer<S, T>() -> BoxLayer<S, T, S::Response, BoxError>
/// where
///     S: Service<T> + Send + 'static,
///     S::Future: Send + 'static,
///     S::Error: Into<BoxError> + 'static,
/// {
///     let builder = ServiceBuilder::new()
///         .concurrency_limit(100);
///
///     if std::env::var("SET_TIMEOUT").is_ok() {
///         let layer = builder
///             .timeout(Duration::from_secs(30))
///             .into_inner();
///
///         BoxLayer::new(layer)
///     } else {
///         let layer = builder
///             .map_err(Into::into)
///             .into_inner();
///
///         BoxLayer::new(layer)
///     }
/// }
/// ```
///
/// [`Layer`]: tower_layer::Layer
/// [`Service`]: tower_service::Service
/// [`BoxService`]: super::BoxService
/// [`Timeout`]: crate::timeout
pub struct BoxLayer<In, T, U, E> {
    boxed: Arc<dyn Layer<In, Service = BoxService<T, U, E>> + Send + Sync + 'static>,
}

impl<In, T, U, E> BoxLayer<In, T, U, E> {
    /// Create a new [`BoxLayer`].
    pub fn new<L>(inner_layer: L) -> Self
    where
        L: Layer<In> + Send + Sync + 'static,
        L::Service: Service<T, Response = U, Error = E> + Send + 'static,
        <L::Service as Service<T>>::Future: Send + 'static,
    {
        let layer = layer_fn(move |inner: In| {
            let out = inner_layer.layer(inner);
            BoxService::new(out)
        });

        Self {
            boxed: Arc::new(layer),
        }
    }
}

impl<In, T, U, E> Layer<In> for BoxLayer<In, T, U, E> {
    type Service = BoxService<T, U, E>;

    fn layer(&self, inner: In) -> Self::Service {
        self.boxed.layer(inner)
    }
}

impl<In, T, U, E> Clone for BoxLayer<In, T, U, E> {
    fn clone(&self) -> Self {
        Self {
            boxed: Arc::clone(&self.boxed),
        }
    }
}

impl<In, T, U, E> fmt::Debug for BoxLayer<In, T, U, E> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("BoxLayer").finish()
    }
}
