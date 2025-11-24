use std::{convert::Infallible, fmt};

use crate::extract::Request;
use tower::Service;

use crate::{
    handler::Handler,
    routing::{future::RouteFuture, Route},
    Router,
};

pub(crate) struct BoxedIntoRoute<S, E>(Box<dyn ErasedIntoRoute<S, E>>);

impl<S> BoxedIntoRoute<S, Infallible>
where
    S: Clone + Send + Sync + 'static,
{
    pub(crate) fn from_handler<H, T>(handler: H) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        Self(Box::new(MakeErasedHandler {
            handler,
            into_route: |handler, state| Route::new(Handler::with_state(handler, state)),
        }))
    }
}

impl<S, E> BoxedIntoRoute<S, E> {
    pub(crate) fn map<F, E2>(self, f: F) -> BoxedIntoRoute<S, E2>
    where
        S: 'static,
        E: 'static,
        F: FnOnce(Route<E>) -> Route<E2> + Clone + Send + Sync + 'static,
        E2: 'static,
    {
        BoxedIntoRoute(Box::new(Map {
            inner: self.0,
            layer: Box::new(f),
        }))
    }

    pub(crate) fn into_route(self, state: S) -> Route<E> {
        self.0.into_route(state)
    }
}

impl<S, E> Clone for BoxedIntoRoute<S, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

impl<S, E> fmt::Debug for BoxedIntoRoute<S, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("BoxedIntoRoute").finish()
    }
}

pub(crate) trait ErasedIntoRoute<S, E>: Send + Sync {
    fn clone_box(&self) -> Box<dyn ErasedIntoRoute<S, E>>;

    fn into_route(self: Box<Self>, state: S) -> Route<E>;

    #[allow(dead_code)]
    fn call_with_state(self: Box<Self>, request: Request, state: S) -> RouteFuture<E>;
}

pub(crate) struct MakeErasedHandler<H, S> {
    pub(crate) handler: H,
    pub(crate) into_route: fn(H, S) -> Route,
}

impl<H, S> ErasedIntoRoute<S, Infallible> for MakeErasedHandler<H, S>
where
    H: Clone + Send + Sync + 'static,
    S: 'static,
{
    fn clone_box(&self) -> Box<dyn ErasedIntoRoute<S, Infallible>> {
        Box::new(self.clone())
    }

    fn into_route(self: Box<Self>, state: S) -> Route {
        (self.into_route)(self.handler, state)
    }

    fn call_with_state(self: Box<Self>, request: Request, state: S) -> RouteFuture<Infallible> {
        self.into_route(state).call(request)
    }
}

impl<H, S> Clone for MakeErasedHandler<H, S>
where
    H: Clone,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            into_route: self.into_route,
        }
    }
}

#[allow(dead_code)]
pub(crate) struct MakeErasedRouter<S> {
    pub(crate) router: Router<S>,
    pub(crate) into_route: fn(Router<S>, S) -> Route,
}

impl<S> ErasedIntoRoute<S, Infallible> for MakeErasedRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn clone_box(&self) -> Box<dyn ErasedIntoRoute<S, Infallible>> {
        Box::new(self.clone())
    }

    fn into_route(self: Box<Self>, state: S) -> Route {
        (self.into_route)(self.router, state)
    }

    fn call_with_state(self: Box<Self>, request: Request, state: S) -> RouteFuture<Infallible> {
        self.router.call_with_state(request, state)
    }
}

impl<S> Clone for MakeErasedRouter<S>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            router: self.router.clone(),
            into_route: self.into_route,
        }
    }
}

pub(crate) struct Map<S, E, E2> {
    pub(crate) inner: Box<dyn ErasedIntoRoute<S, E>>,
    pub(crate) layer: Box<dyn LayerFn<E, E2>>,
}

impl<S, E, E2> ErasedIntoRoute<S, E2> for Map<S, E, E2>
where
    S: 'static,
    E: 'static,
    E2: 'static,
{
    fn clone_box(&self) -> Box<dyn ErasedIntoRoute<S, E2>> {
        Box::new(Self {
            inner: self.inner.clone_box(),
            layer: self.layer.clone_box(),
        })
    }

    fn into_route(self: Box<Self>, state: S) -> Route<E2> {
        (self.layer)(self.inner.into_route(state))
    }

    fn call_with_state(self: Box<Self>, request: Request, state: S) -> RouteFuture<E2> {
        (self.layer)(self.inner.into_route(state)).call(request)
    }
}

pub(crate) trait LayerFn<E, E2>: FnOnce(Route<E>) -> Route<E2> + Send + Sync {
    fn clone_box(&self) -> Box<dyn LayerFn<E, E2>>;
}

impl<F, E, E2> LayerFn<E, E2> for F
where
    F: FnOnce(Route<E>) -> Route<E2> + Clone + Send + Sync + 'static,
{
    fn clone_box(&self) -> Box<dyn LayerFn<E, E2>> {
        Box::new(self.clone())
    }
}
