use std::fmt;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower_service::Service;

/// Returns a new [`FutureService`] for the given future.
///
/// A [`FutureService`] allows you to treat a future that resolves to a service as a service. This
/// can be useful for services that are created asynchronously.
///
/// # Example
/// ```
/// use tower::{service_fn, Service, ServiceExt};
/// use tower::util::future_service;
/// use std::convert::Infallible;
///
/// # fn main() {
/// # async {
/// // A future which outputs a type implementing `Service`.
/// let future_of_a_service = async {
///     let svc = service_fn(|_req: ()| async { Ok::<_, Infallible>("ok") });
///     Ok::<_, Infallible>(svc)
/// };
///
/// // Wrap the future with a `FutureService`, allowing it to be used
/// // as a service without awaiting the future's completion:
/// let mut svc = future_service(Box::pin(future_of_a_service));
///
/// // Now, when we wait for the service to become ready, it will
/// // drive the future to completion internally.
/// let svc = svc.ready().await.unwrap();
/// let res = svc.call(()).await.unwrap();
/// # };
/// # }
/// ```
///
/// # Regarding the [`Unpin`] bound
///
/// The [`Unpin`] bound on `F` is necessary because the future will be polled in
/// [`Service::poll_ready`] which doesn't have a pinned receiver (it takes `&mut self` and not `self:
/// Pin<&mut Self>`). So we cannot put the future into a `Pin` without requiring `Unpin`.
///
/// This will most likely come up if you're calling `future_service` with an async block. In that
/// case you can use `Box::pin(async { ... })` as shown in the example.
pub fn future_service<F, S, R, E>(future: F) -> FutureService<F, S>
where
    F: Future<Output = Result<S, E>> + Unpin,
    S: Service<R, Error = E>,
{
    FutureService::new(future)
}

/// A type that implements [`Service`] for a [`Future`] that produces a [`Service`].
///
/// See [`future_service`] for more details.
#[derive(Clone)]
pub struct FutureService<F, S> {
    state: State<F, S>,
}

impl<F, S> FutureService<F, S> {
    /// Returns a new [`FutureService`] for the given future.
    ///
    /// A [`FutureService`] allows you to treat a future that resolves to a service as a service. This
    /// can be useful for services that are created asynchronously.
    ///
    /// # Example
    /// ```
    /// use tower::{service_fn, Service, ServiceExt};
    /// use tower::util::FutureService;
    /// use std::convert::Infallible;
    ///
    /// # fn main() {
    /// # async {
    /// // A future which outputs a type implementing `Service`.
    /// let future_of_a_service = async {
    ///     let svc = service_fn(|_req: ()| async { Ok::<_, Infallible>("ok") });
    ///     Ok::<_, Infallible>(svc)
    /// };
    ///
    /// // Wrap the future with a `FutureService`, allowing it to be used
    /// // as a service without awaiting the future's completion:
    /// let mut svc = FutureService::new(Box::pin(future_of_a_service));
    ///
    /// // Now, when we wait for the service to become ready, it will
    /// // drive the future to completion internally.
    /// let svc = svc.ready().await.unwrap();
    /// let res = svc.call(()).await.unwrap();
    /// # };
    /// # }
    /// ```
    ///
    /// # Regarding the [`Unpin`] bound
    ///
    /// The [`Unpin`] bound on `F` is necessary because the future will be polled in
    /// [`Service::poll_ready`] which doesn't have a pinned receiver (it takes `&mut self` and not `self:
    /// Pin<&mut Self>`). So we cannot put the future into a `Pin` without requiring `Unpin`.
    ///
    /// This will most likely come up if you're calling `future_service` with an async block. In that
    /// case you can use `Box::pin(async { ... })` as shown in the example.
    pub const fn new(future: F) -> Self {
        Self {
            state: State::Future(future),
        }
    }
}

impl<F, S> fmt::Debug for FutureService<F, S>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FutureService")
            .field("state", &format_args!("{:?}", self.state))
            .finish()
    }
}

#[derive(Clone)]
enum State<F, S> {
    Future(F),
    Service(S),
}

impl<F, S> fmt::Debug for State<F, S>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            State::Future(_) => f
                .debug_tuple("State::Future")
                .field(&format_args!("<{}>", std::any::type_name::<F>()))
                .finish(),
            State::Service(svc) => f.debug_tuple("State::Service").field(svc).finish(),
        }
    }
}

impl<F, S, R, E> Service<R> for FutureService<F, S>
where
    F: Future<Output = Result<S, E>> + Unpin,
    S: Service<R, Error = E>,
{
    type Response = S::Response;
    type Error = E;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        loop {
            self.state = match &mut self.state {
                State::Future(fut) => {
                    let fut = Pin::new(fut);
                    let svc = futures_core::ready!(fut.poll(cx)?);
                    State::Service(svc)
                }
                State::Service(svc) => return svc.poll_ready(cx),
            };
        }
    }

    fn call(&mut self, req: R) -> Self::Future {
        if let State::Service(svc) = &mut self.state {
            svc.call(req)
        } else {
            panic!("FutureService::call was called before FutureService::poll_ready")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::{future_service, ServiceExt};
    use crate::Service;
    use futures::future::{ready, Ready};
    use std::convert::Infallible;

    #[tokio::test]
    async fn pending_service_debug_impl() {
        let mut pending_svc = future_service(ready(Ok(DebugService)));

        assert_eq!(
            format!("{:?}", pending_svc),
            "FutureService { state: State::Future(<futures_util::future::ready::Ready<core::result::Result<tower::util::future_service::tests::DebugService, core::convert::Infallible>>>) }"
        );

        pending_svc.ready().await.unwrap();

        assert_eq!(
            format!("{:?}", pending_svc),
            "FutureService { state: State::Service(DebugService) }"
        );
    }

    #[derive(Debug)]
    struct DebugService;

    impl Service<()> for DebugService {
        type Response = ();
        type Error = Infallible;
        type Future = Ready<Result<Self::Response, Self::Error>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Ok(()).into()
        }

        fn call(&mut self, _req: ()) -> Self::Future {
            ready(Ok(()))
        }
    }
}
