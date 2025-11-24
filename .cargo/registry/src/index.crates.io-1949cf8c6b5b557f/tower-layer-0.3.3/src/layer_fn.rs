use super::Layer;
use std::fmt;

/// Returns a new [`LayerFn`] that implements [`Layer`] by calling the
/// given function.
///
/// The [`Layer::layer`] method takes a type implementing [`Service`] and
/// returns a different type implementing [`Service`]. In many cases, this can
/// be implemented by a function or a closure. The [`LayerFn`] helper allows
/// writing simple [`Layer`] implementations without needing the boilerplate of
/// a new struct implementing [`Layer`].
///
/// # Example
/// ```rust
/// # use tower::Service;
/// # use std::task::{Poll, Context};
/// # use tower_layer::{Layer, layer_fn};
/// # use std::fmt;
/// # use std::convert::Infallible;
/// #
/// // A middleware that logs requests before forwarding them to another service
/// pub struct LogService<S> {
///     target: &'static str,
///     service: S,
/// }
///
/// impl<S, Request> Service<Request> for LogService<S>
/// where
///     S: Service<Request>,
///     Request: fmt::Debug,
/// {
///     type Response = S::Response;
///     type Error = S::Error;
///     type Future = S::Future;
///
///     fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
///         self.service.poll_ready(cx)
///     }
///
///     fn call(&mut self, request: Request) -> Self::Future {
///         // Log the request
///         println!("request = {:?}, target = {:?}", request, self.target);
///
///         self.service.call(request)
///     }
/// }
///
/// // A `Layer` that wraps services in `LogService`
/// let log_layer = layer_fn(|service| {
///     LogService {
///         service,
///         target: "tower-docs",
///     }
/// });
///
/// // An example service. This one uppercases strings
/// let uppercase_service = tower::service_fn(|request: String| async move {
///     Ok::<_, Infallible>(request.to_uppercase())
/// });
///
/// // Wrap our service in a `LogService` so requests are logged.
/// let wrapped_service = log_layer.layer(uppercase_service);
/// ```
///
/// [`Service`]: https://docs.rs/tower-service/latest/tower_service/trait.Service.html
/// [`Layer::layer`]: crate::Layer::layer
pub fn layer_fn<T>(f: T) -> LayerFn<T> {
    LayerFn { f }
}

/// A `Layer` implemented by a closure. See the docs for [`layer_fn`] for more details.
#[derive(Clone, Copy)]
pub struct LayerFn<F> {
    f: F,
}

impl<F, S, Out> Layer<S> for LayerFn<F>
where
    F: Fn(S) -> Out,
{
    type Service = Out;

    fn layer(&self, inner: S) -> Self::Service {
        (self.f)(inner)
    }
}

impl<F> fmt::Debug for LayerFn<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LayerFn")
            .field("f", &format_args!("{}", std::any::type_name::<F>()))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    #[test]
    fn layer_fn_has_useful_debug_impl() {
        struct WrappedService<S> {
            inner: S,
        }
        let layer = layer_fn(|svc| WrappedService { inner: svc });
        let _svc = layer.layer("foo");

        assert_eq!(
            "LayerFn { f: tower_layer::layer_fn::tests::layer_fn_has_useful_debug_impl::{{closure}} }".to_string(),
            format!("{:?}", layer),
        );
    }
}
