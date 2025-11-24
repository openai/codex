use std::fmt;
use std::future::Future;
use std::task::{Context, Poll};
use tower_service::Service;

/// Returns a new [`ServiceFn`] with the given closure.
///
/// This lets you build a [`Service`] from an async function that returns a [`Result`].
///
/// # Example
///
/// ```
/// use tower::{service_fn, Service, ServiceExt, BoxError};
/// # struct Request;
/// # impl Request {
/// #     fn new() -> Self { Self }
/// # }
/// # struct Response(&'static str);
/// # impl Response {
/// #     fn new(body: &'static str) -> Self {
/// #         Self(body)
/// #     }
/// #     fn into_body(self) -> &'static str { self.0 }
/// # }
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), BoxError> {
/// async fn handle(request: Request) -> Result<Response, BoxError> {
///     let response = Response::new("Hello, World!");
///     Ok(response)
/// }
///
/// let mut service = service_fn(handle);
///
/// let response = service
///     .ready()
///     .await?
///     .call(Request::new())
///     .await?;
///
/// assert_eq!("Hello, World!", response.into_body());
/// #
/// # Ok(())
/// # }
/// ```
pub fn service_fn<T>(f: T) -> ServiceFn<T> {
    ServiceFn { f }
}

/// A [`Service`] implemented by a closure.
///
/// See [`service_fn`] for more details.
#[derive(Copy, Clone)]
pub struct ServiceFn<T> {
    f: T,
}

impl<T> fmt::Debug for ServiceFn<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceFn")
            .field("f", &format_args!("{}", std::any::type_name::<T>()))
            .finish()
    }
}

impl<T, F, Request, R, E> Service<Request> for ServiceFn<T>
where
    T: FnMut(Request) -> F,
    F: Future<Output = Result<R, E>>,
{
    type Response = R;
    type Error = E;
    type Future = F;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), E>> {
        Ok(()).into()
    }

    fn call(&mut self, req: Request) -> Self::Future {
        (self.f)(req)
    }
}
