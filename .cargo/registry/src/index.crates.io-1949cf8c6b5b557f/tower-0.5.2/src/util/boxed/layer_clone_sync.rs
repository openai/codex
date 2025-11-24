use std::{fmt, sync::Arc};
use tower_layer::{layer_fn, Layer};
use tower_service::Service;

use crate::util::BoxCloneSyncService;

/// A [`Clone`] + [`Send`] + [`Sync`] boxed [`Layer`].
///
/// [`BoxCloneSyncServiceLayer`] turns a layer into a trait object, allowing both the [`Layer`] itself
/// and the output [`Service`] to be dynamic, while having consistent types.
///
/// This [`Layer`] produces [`BoxCloneSyncService`] instances erasing the type of the
/// [`Service`] produced by the wrapped [`Layer`].
///
/// This is similar to [`BoxCloneServiceLayer`](super::BoxCloneServiceLayer) except the layer and resulting
/// service implements [`Sync`].
///
/// # Example
///
/// `BoxCloneSyncServiceLayer` can, for example, be useful to create layers dynamically that otherwise wouldn't have
/// the same types, when the underlying service must be clone and sync (for example, when building a Hyper connector).
/// In this example, we include a [`Timeout`] layer only if an environment variable is set. We can use
/// `BoxCloneSyncServiceLayer` to return a consistent type regardless of runtime configuration:
///
/// ```
/// use std::time::Duration;
/// use tower::{Service, ServiceBuilder, BoxError};
/// use tower::util::{BoxCloneSyncServiceLayer, BoxCloneSyncService};
///
/// #
/// # struct Request;
/// # struct Response;
/// # impl Response {
/// #     fn new() -> Self { Self }
/// # }
///
/// fn common_layer<S, T>() -> BoxCloneSyncServiceLayer<S, T, S::Response, BoxError>
/// where
///     S: Service<T> + Clone + Send + Sync + 'static,
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
///         BoxCloneSyncServiceLayer::new(layer)
///     } else {
///         let layer = builder
///             .map_err(Into::into)
///             .into_inner();
///
///         BoxCloneSyncServiceLayer::new(layer)
///     }
/// }
///
/// // We can clone the layer (this is true of BoxLayer as well)
/// let boxed_clone_sync_layer = common_layer();
///
/// let cloned_sync_layer = boxed_clone_sync_layer.clone();
///
/// // Using the `BoxCloneSyncServiceLayer` we can create a `BoxCloneSyncService`
/// let service: BoxCloneSyncService<Request, Response, BoxError> = ServiceBuilder::new().layer(cloned_sync_layer)
///      .service_fn(|req: Request| async {
///         Ok::<_, BoxError>(Response::new())
///     });
///
/// # let service = assert_service(service);
///
/// // And we can still clone the service
/// let cloned_service = service.clone();
/// #
/// # fn assert_service<S, R>(svc: S) -> S
/// # where S: Service<R> { svc }
///
/// ```
///
/// [`Layer`]: tower_layer::Layer
/// [`Service`]: tower_service::Service
/// [`BoxService`]: super::BoxService
/// [`Timeout`]: crate::timeout
pub struct BoxCloneSyncServiceLayer<In, T, U, E> {
    boxed: Arc<dyn Layer<In, Service = BoxCloneSyncService<T, U, E>> + Send + Sync + 'static>,
}

impl<In, T, U, E> BoxCloneSyncServiceLayer<In, T, U, E> {
    /// Create a new [`BoxCloneSyncServiceLayer`].
    pub fn new<L>(inner_layer: L) -> Self
    where
        L: Layer<In> + Send + Sync + 'static,
        L::Service: Service<T, Response = U, Error = E> + Send + Sync + Clone + 'static,
        <L::Service as Service<T>>::Future: Send + 'static,
    {
        let layer = layer_fn(move |inner: In| {
            let out = inner_layer.layer(inner);
            BoxCloneSyncService::new(out)
        });

        Self {
            boxed: Arc::new(layer),
        }
    }
}

impl<In, T, U, E> Layer<In> for BoxCloneSyncServiceLayer<In, T, U, E> {
    type Service = BoxCloneSyncService<T, U, E>;

    fn layer(&self, inner: In) -> Self::Service {
        self.boxed.layer(inner)
    }
}

impl<In, T, U, E> Clone for BoxCloneSyncServiceLayer<In, T, U, E> {
    fn clone(&self) -> Self {
        Self {
            boxed: Arc::clone(&self.boxed),
        }
    }
}

impl<In, T, U, E> fmt::Debug for BoxCloneSyncServiceLayer<In, T, U, E> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("BoxCloneSyncServiceLayer").finish()
    }
}
