use crate::{body::Body, server::NamedService, Status};
use http::{Request, Response};
use std::{
    convert::Infallible,
    fmt,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{Service, ServiceExt};

/// A [`Service`] router.
#[derive(Debug, Clone)]
pub struct Routes {
    router: axum::Router,
}

#[derive(Debug, Default, Clone)]
/// Allows adding new services to routes by passing a mutable reference to this builder.
pub struct RoutesBuilder {
    routes: Option<Routes>,
}

impl RoutesBuilder {
    /// Add a new service.
    pub fn add_service<S>(&mut self, svc: S) -> &mut Self
    where
        S: Service<Request<Body>, Error = Infallible>
            + NamedService
            + Clone
            + Send
            + Sync
            + 'static,
        S::Response: axum::response::IntoResponse,
        S::Future: Send + 'static,
    {
        let routes = self.routes.take().unwrap_or_default();
        self.routes.replace(routes.add_service(svc));
        self
    }

    /// Returns the routes with added services or empty [`Routes`] if no service was added
    pub fn routes(self) -> Routes {
        self.routes.unwrap_or_default()
    }
}

impl Default for Routes {
    fn default() -> Self {
        Self {
            router: axum::Router::new().fallback(unimplemented),
        }
    }
}

impl Routes {
    /// Create a new routes with `svc` already added to it.
    pub fn new<S>(svc: S) -> Self
    where
        S: Service<Request<Body>, Error = Infallible>
            + NamedService
            + Clone
            + Send
            + Sync
            + 'static,
        S::Response: axum::response::IntoResponse,
        S::Future: Send + 'static,
    {
        Self::default().add_service(svc)
    }

    /// Create a new empty builder.
    pub fn builder() -> RoutesBuilder {
        RoutesBuilder::default()
    }

    /// Add a new service.
    pub fn add_service<S>(mut self, svc: S) -> Self
    where
        S: Service<Request<Body>, Error = Infallible>
            + NamedService
            + Clone
            + Send
            + Sync
            + 'static,
        S::Response: axum::response::IntoResponse,
        S::Future: Send + 'static,
    {
        self.router = self.router.route_service(
            &format!("/{}/{{*rest}}", S::NAME),
            svc.map_request(|req: Request<axum::body::Body>| req.map(Body::new)),
        );
        self
    }

    /// This makes axum perform update some internals of the router that improves perf.
    ///
    /// See <https://docs.rs/axum/latest/axum/routing/struct.Router.html#a-note-about-performance>
    pub fn prepare(self) -> Self {
        Self {
            router: self.router.with_state(()),
        }
    }

    /// Convert this `Routes` into an [`axum::Router`].
    pub fn into_axum_router(self) -> axum::Router {
        self.router
    }

    /// Get a mutable reference to the [`axum::Router`].
    pub fn axum_router_mut(&mut self) -> &mut axum::Router {
        &mut self.router
    }
}

impl From<Routes> for RoutesBuilder {
    fn from(routes: Routes) -> Self {
        Self {
            routes: Some(routes),
        }
    }
}

impl From<axum::Router> for RoutesBuilder {
    fn from(router: axum::Router) -> Self {
        Self {
            routes: Some(router.into()),
        }
    }
}

impl From<axum::Router> for Routes {
    fn from(router: axum::Router) -> Self {
        Self { router }
    }
}

async fn unimplemented() -> Response<Body> {
    let (parts, ()) = Status::unimplemented("").into_http::<()>().into_parts();
    Response::from_parts(parts, Body::empty())
}

impl<B> Service<Request<B>> for Routes
where
    B: http_body::Body<Data = bytes::Bytes> + Send + 'static,
    B::Error: Into<crate::BoxError>,
{
    type Response = Response<Body>;
    type Error = Infallible;
    type Future = RoutesFuture;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Service::<Request<B>>::poll_ready(&mut self.router, cx)
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        RoutesFuture(self.router.call(req))
    }
}

pub struct RoutesFuture(axum::routing::future::RouteFuture<Infallible>);

impl fmt::Debug for RoutesFuture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("RoutesFuture").finish()
    }
}

impl Future for RoutesFuture {
    type Output = Result<Response<Body>, Infallible>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.as_mut().0)
            .poll(cx)
            .map_ok(|res| res.map(Body::new))
    }
}
