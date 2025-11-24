use pin_project_lite::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use super::Oneshot;

/// A tower [`Service`][tower-svc] converted into a hyper [`Service`][hyper-svc].
///
/// This wraps an inner tower service `S` in a [`hyper::service::Service`] implementation. See
/// the module-level documentation of [`service`][crate::service] for more information about using
/// [`tower`][tower] services and middleware with [`hyper`].
///
/// [hyper-svc]: hyper::service::Service
/// [tower]: https://docs.rs/tower/latest/tower/
/// [tower-svc]: https://docs.rs/tower/latest/tower/trait.Service.html
#[derive(Debug, Copy, Clone)]
pub struct TowerToHyperService<S> {
    service: S,
}

impl<S> TowerToHyperService<S> {
    /// Create a new [`TowerToHyperService`] from a tower service.
    pub fn new(tower_service: S) -> Self {
        Self {
            service: tower_service,
        }
    }
}

impl<S, R> hyper::service::Service<R> for TowerToHyperService<S>
where
    S: tower_service::Service<R> + Clone,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = TowerToHyperServiceFuture<S, R>;

    fn call(&self, req: R) -> Self::Future {
        TowerToHyperServiceFuture {
            future: Oneshot::new(self.service.clone(), req),
        }
    }
}

pin_project! {
    /// Response future for [`TowerToHyperService`].
    ///
    /// This future is acquired by [`call`][hyper::service::Service::call]ing a
    /// [`TowerToHyperService`].
    pub struct TowerToHyperServiceFuture<S, R>
    where
        S: tower_service::Service<R>,
    {
        #[pin]
        future: Oneshot<S, R>,
    }
}

impl<S, R> Future for TowerToHyperServiceFuture<S, R>
where
    S: tower_service::Service<R>,
{
    type Output = Result<S::Response, S::Error>;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().future.poll(cx)
    }
}
