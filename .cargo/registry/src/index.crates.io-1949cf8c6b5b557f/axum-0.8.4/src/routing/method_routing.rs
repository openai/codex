//! Route to services and handlers based on HTTP methods.

use super::{future::InfallibleRouteFuture, IntoMakeService};
#[cfg(feature = "tokio")]
use crate::extract::connect_info::IntoMakeServiceWithConnectInfo;
use crate::{
    body::{Body, Bytes, HttpBody},
    boxed::BoxedIntoRoute,
    error_handling::{HandleError, HandleErrorLayer},
    handler::Handler,
    http::{Method, StatusCode},
    response::Response,
    routing::{future::RouteFuture, Fallback, MethodFilter, Route},
};
use axum_core::{extract::Request, response::IntoResponse, BoxError};
use bytes::BytesMut;
use std::{
    borrow::Cow,
    convert::Infallible,
    fmt,
    task::{Context, Poll},
};
use tower::service_fn;
use tower_layer::Layer;
use tower_service::Service;

macro_rules! top_level_service_fn {
    (
        $name:ident, GET
    ) => {
        top_level_service_fn!(
            /// Route `GET` requests to the given service.
            ///
            /// # Example
            ///
            /// ```rust
            /// use axum::{
            ///     extract::Request,
            ///     Router,
            ///     routing::get_service,
            ///     body::Body,
            /// };
            /// use http::Response;
            /// use std::convert::Infallible;
            ///
            /// let service = tower::service_fn(|request: Request| async {
            ///     Ok::<_, Infallible>(Response::new(Body::empty()))
            /// });
            ///
            /// // Requests to `GET /` will go to `service`.
            /// let app = Router::new().route("/", get_service(service));
            /// # let _: Router = app;
            /// ```
            ///
            /// Note that `get` routes will also be called for `HEAD` requests but will have
            /// the response body removed. Make sure to add explicit `HEAD` routes
            /// afterwards.
            $name,
            GET
        );
    };

    (
        $name:ident, CONNECT
    ) => {
        top_level_service_fn!(
            /// Route `CONNECT` requests to the given service.
            ///
            /// See [`MethodFilter::CONNECT`] for when you'd want to use this,
            /// and [`get_service`] for an example.
            $name,
            CONNECT
        );
    };

    (
        $name:ident, $method:ident
    ) => {
        top_level_service_fn!(
            #[doc = concat!("Route `", stringify!($method) ,"` requests to the given service.")]
            ///
            /// See [`get_service`] for an example.
            $name,
            $method
        );
    };

    (
        $(#[$m:meta])+
        $name:ident, $method:ident
    ) => {
        $(#[$m])+
        pub fn $name<T, S>(svc: T) -> MethodRouter<S, T::Error>
        where
            T: Service<Request> + Clone + Send + Sync + 'static,
            T::Response: IntoResponse + 'static,
            T::Future: Send + 'static,
            S: Clone,
        {
            on_service(MethodFilter::$method, svc)
        }
    };
}

macro_rules! top_level_handler_fn {
    (
        $name:ident, GET
    ) => {
        top_level_handler_fn!(
            /// Route `GET` requests to the given handler.
            ///
            /// # Example
            ///
            /// ```rust
            /// use axum::{
            ///     routing::get,
            ///     Router,
            /// };
            ///
            /// async fn handler() {}
            ///
            /// // Requests to `GET /` will go to `handler`.
            /// let app = Router::new().route("/", get(handler));
            /// # let _: Router = app;
            /// ```
            ///
            /// Note that `get` routes will also be called for `HEAD` requests but will have
            /// the response body removed. Make sure to add explicit `HEAD` routes
            /// afterwards.
            $name,
            GET
        );
    };

    (
        $name:ident, CONNECT
    ) => {
        top_level_handler_fn!(
            /// Route `CONNECT` requests to the given handler.
            ///
            /// See [`MethodFilter::CONNECT`] for when you'd want to use this,
            /// and [`get`] for an example.
            $name,
            CONNECT
        );
    };

    (
        $name:ident, $method:ident
    ) => {
        top_level_handler_fn!(
            #[doc = concat!("Route `", stringify!($method) ,"` requests to the given handler.")]
            ///
            /// See [`get`] for an example.
            $name,
            $method
        );
    };

    (
        $(#[$m:meta])+
        $name:ident, $method:ident
    ) => {
        $(#[$m])+
        pub fn $name<H, T, S>(handler: H) -> MethodRouter<S, Infallible>
        where
            H: Handler<T, S>,
            T: 'static,
            S: Clone + Send + Sync + 'static,
        {
            on(MethodFilter::$method, handler)
        }
    };
}

macro_rules! chained_service_fn {
    (
        $name:ident, GET
    ) => {
        chained_service_fn!(
            /// Chain an additional service that will only accept `GET` requests.
            ///
            /// # Example
            ///
            /// ```rust
            /// use axum::{
            ///     extract::Request,
            ///     Router,
            ///     routing::post_service,
            ///     body::Body,
            /// };
            /// use http::Response;
            /// use std::convert::Infallible;
            ///
            /// let service = tower::service_fn(|request: Request| async {
            ///     Ok::<_, Infallible>(Response::new(Body::empty()))
            /// });
            ///
            /// let other_service = tower::service_fn(|request: Request| async {
            ///     Ok::<_, Infallible>(Response::new(Body::empty()))
            /// });
            ///
            /// // Requests to `POST /` will go to `service` and `GET /` will go to
            /// // `other_service`.
            /// let app = Router::new().route("/", post_service(service).get_service(other_service));
            /// # let _: Router = app;
            /// ```
            ///
            /// Note that `get` routes will also be called for `HEAD` requests but will have
            /// the response body removed. Make sure to add explicit `HEAD` routes
            /// afterwards.
            $name,
            GET
        );
    };

    (
        $name:ident, CONNECT
    ) => {
        chained_service_fn!(
            /// Chain an additional service that will only accept `CONNECT` requests.
            ///
            /// See [`MethodFilter::CONNECT`] for when you'd want to use this,
            /// and [`MethodRouter::get_service`] for an example.
            $name,
            CONNECT
        );
    };

    (
        $name:ident, $method:ident
    ) => {
        chained_service_fn!(
            #[doc = concat!("Chain an additional service that will only accept `", stringify!($method),"` requests.")]
            ///
            /// See [`MethodRouter::get_service`] for an example.
            $name,
            $method
        );
    };

    (
        $(#[$m:meta])+
        $name:ident, $method:ident
    ) => {
        $(#[$m])+
        #[track_caller]
        pub fn $name<T>(self, svc: T) -> Self
        where
            T: Service<Request, Error = E>
                + Clone
                + Send
                + Sync
                + 'static,
            T::Response: IntoResponse + 'static,
            T::Future: Send + 'static,
        {
            self.on_service(MethodFilter::$method, svc)
        }
    };
}

macro_rules! chained_handler_fn {
    (
        $name:ident, GET
    ) => {
        chained_handler_fn!(
            /// Chain an additional handler that will only accept `GET` requests.
            ///
            /// # Example
            ///
            /// ```rust
            /// use axum::{routing::post, Router};
            ///
            /// async fn handler() {}
            ///
            /// async fn other_handler() {}
            ///
            /// // Requests to `POST /` will go to `handler` and `GET /` will go to
            /// // `other_handler`.
            /// let app = Router::new().route("/", post(handler).get(other_handler));
            /// # let _: Router = app;
            /// ```
            ///
            /// Note that `get` routes will also be called for `HEAD` requests but will have
            /// the response body removed. Make sure to add explicit `HEAD` routes
            /// afterwards.
            $name,
            GET
        );
    };

    (
        $name:ident, CONNECT
    ) => {
        chained_handler_fn!(
            /// Chain an additional handler that will only accept `CONNECT` requests.
            ///
            /// See [`MethodFilter::CONNECT`] for when you'd want to use this,
            /// and [`MethodRouter::get`] for an example.
            $name,
            CONNECT
        );
    };

    (
        $name:ident, $method:ident
    ) => {
        chained_handler_fn!(
            #[doc = concat!("Chain an additional handler that will only accept `", stringify!($method),"` requests.")]
            ///
            /// See [`MethodRouter::get`] for an example.
            $name,
            $method
        );
    };

    (
        $(#[$m:meta])+
        $name:ident, $method:ident
    ) => {
        $(#[$m])+
        #[track_caller]
        pub fn $name<H, T>(self, handler: H) -> Self
        where
            H: Handler<T, S>,
            T: 'static,
            S: Send + Sync + 'static,
        {
            self.on(MethodFilter::$method, handler)
        }
    };
}

top_level_service_fn!(connect_service, CONNECT);
top_level_service_fn!(delete_service, DELETE);
top_level_service_fn!(get_service, GET);
top_level_service_fn!(head_service, HEAD);
top_level_service_fn!(options_service, OPTIONS);
top_level_service_fn!(patch_service, PATCH);
top_level_service_fn!(post_service, POST);
top_level_service_fn!(put_service, PUT);
top_level_service_fn!(trace_service, TRACE);

/// Route requests with the given method to the service.
///
/// # Example
///
/// ```rust
/// use axum::{
///     extract::Request,
///     routing::on,
///     Router,
///     body::Body,
///     routing::{MethodFilter, on_service},
/// };
/// use http::Response;
/// use std::convert::Infallible;
///
/// let service = tower::service_fn(|request: Request| async {
///     Ok::<_, Infallible>(Response::new(Body::empty()))
/// });
///
/// // Requests to `POST /` will go to `service`.
/// let app = Router::new().route("/", on_service(MethodFilter::POST, service));
/// # let _: Router = app;
/// ```
pub fn on_service<T, S>(filter: MethodFilter, svc: T) -> MethodRouter<S, T::Error>
where
    T: Service<Request> + Clone + Send + Sync + 'static,
    T::Response: IntoResponse + 'static,
    T::Future: Send + 'static,
    S: Clone,
{
    MethodRouter::new().on_service(filter, svc)
}

/// Route requests to the given service regardless of its method.
///
/// # Example
///
/// ```rust
/// use axum::{
///     extract::Request,
///     Router,
///     routing::any_service,
///     body::Body,
/// };
/// use http::Response;
/// use std::convert::Infallible;
///
/// let service = tower::service_fn(|request: Request| async {
///     Ok::<_, Infallible>(Response::new(Body::empty()))
/// });
///
/// // All requests to `/` will go to `service`.
/// let app = Router::new().route("/", any_service(service));
/// # let _: Router = app;
/// ```
///
/// Additional methods can still be chained:
///
/// ```rust
/// use axum::{
///     extract::Request,
///     Router,
///     routing::any_service,
///     body::Body,
/// };
/// use http::Response;
/// use std::convert::Infallible;
///
/// let service = tower::service_fn(|request: Request| async {
///     # Ok::<_, Infallible>(Response::new(Body::empty()))
///     // ...
/// });
///
/// let other_service = tower::service_fn(|request: Request| async {
///     # Ok::<_, Infallible>(Response::new(Body::empty()))
///     // ...
/// });
///
/// // `POST /` goes to `other_service`. All other requests go to `service`
/// let app = Router::new().route("/", any_service(service).post_service(other_service));
/// # let _: Router = app;
/// ```
pub fn any_service<T, S>(svc: T) -> MethodRouter<S, T::Error>
where
    T: Service<Request> + Clone + Send + Sync + 'static,
    T::Response: IntoResponse + 'static,
    T::Future: Send + 'static,
    S: Clone,
{
    MethodRouter::new()
        .fallback_service(svc)
        .skip_allow_header()
}

top_level_handler_fn!(connect, CONNECT);
top_level_handler_fn!(delete, DELETE);
top_level_handler_fn!(get, GET);
top_level_handler_fn!(head, HEAD);
top_level_handler_fn!(options, OPTIONS);
top_level_handler_fn!(patch, PATCH);
top_level_handler_fn!(post, POST);
top_level_handler_fn!(put, PUT);
top_level_handler_fn!(trace, TRACE);

/// Route requests with the given method to the handler.
///
/// # Example
///
/// ```rust
/// use axum::{
///     routing::on,
///     Router,
///     routing::MethodFilter,
/// };
///
/// async fn handler() {}
///
/// // Requests to `POST /` will go to `handler`.
/// let app = Router::new().route("/", on(MethodFilter::POST, handler));
/// # let _: Router = app;
/// ```
pub fn on<H, T, S>(filter: MethodFilter, handler: H) -> MethodRouter<S, Infallible>
where
    H: Handler<T, S>,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    MethodRouter::new().on(filter, handler)
}

/// Route requests with the given handler regardless of the method.
///
/// # Example
///
/// ```rust
/// use axum::{
///     routing::any,
///     Router,
/// };
///
/// async fn handler() {}
///
/// // All requests to `/` will go to `handler`.
/// let app = Router::new().route("/", any(handler));
/// # let _: Router = app;
/// ```
///
/// Additional methods can still be chained:
///
/// ```rust
/// use axum::{
///     routing::any,
///     Router,
/// };
///
/// async fn handler() {}
///
/// async fn other_handler() {}
///
/// // `POST /` goes to `other_handler`. All other requests go to `handler`
/// let app = Router::new().route("/", any(handler).post(other_handler));
/// # let _: Router = app;
/// ```
pub fn any<H, T, S>(handler: H) -> MethodRouter<S, Infallible>
where
    H: Handler<T, S>,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    MethodRouter::new().fallback(handler).skip_allow_header()
}

/// A [`Service`] that accepts requests based on a [`MethodFilter`] and
/// allows chaining additional handlers and services.
///
/// # When does `MethodRouter` implement [`Service`]?
///
/// Whether or not `MethodRouter` implements [`Service`] depends on the state type it requires.
///
/// ```
/// use tower::Service;
/// use axum::{routing::get, extract::{State, Request}, body::Body};
///
/// // this `MethodRouter` doesn't require any state, i.e. the state is `()`,
/// let method_router = get(|| async {});
/// // and thus it implements `Service`
/// assert_service(method_router);
///
/// // this requires a `String` and doesn't implement `Service`
/// let method_router = get(|_: State<String>| async {});
/// // until you provide the `String` with `.with_state(...)`
/// let method_router_with_state = method_router.with_state(String::new());
/// // and then it implements `Service`
/// assert_service(method_router_with_state);
///
/// // helper to check that a value implements `Service`
/// fn assert_service<S>(service: S)
/// where
///     S: Service<Request>,
/// {}
/// ```
#[must_use]
pub struct MethodRouter<S = (), E = Infallible> {
    get: MethodEndpoint<S, E>,
    head: MethodEndpoint<S, E>,
    delete: MethodEndpoint<S, E>,
    options: MethodEndpoint<S, E>,
    patch: MethodEndpoint<S, E>,
    post: MethodEndpoint<S, E>,
    put: MethodEndpoint<S, E>,
    trace: MethodEndpoint<S, E>,
    connect: MethodEndpoint<S, E>,
    fallback: Fallback<S, E>,
    allow_header: AllowHeader,
}

#[derive(Clone, Debug)]
enum AllowHeader {
    /// No `Allow` header value has been built-up yet. This is the default state
    None,
    /// Don't set an `Allow` header. This is used when `any` or `any_service` are called.
    Skip,
    /// The current value of the `Allow` header.
    Bytes(BytesMut),
}

impl AllowHeader {
    fn merge(self, other: Self) -> Self {
        match (self, other) {
            (AllowHeader::Skip, _) | (_, AllowHeader::Skip) => AllowHeader::Skip,
            (AllowHeader::None, AllowHeader::None) => AllowHeader::None,
            (AllowHeader::None, AllowHeader::Bytes(pick)) => AllowHeader::Bytes(pick),
            (AllowHeader::Bytes(pick), AllowHeader::None) => AllowHeader::Bytes(pick),
            (AllowHeader::Bytes(mut a), AllowHeader::Bytes(b)) => {
                a.extend_from_slice(b",");
                a.extend_from_slice(&b);
                AllowHeader::Bytes(a)
            }
        }
    }
}

impl<S, E> fmt::Debug for MethodRouter<S, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MethodRouter")
            .field("get", &self.get)
            .field("head", &self.head)
            .field("delete", &self.delete)
            .field("options", &self.options)
            .field("patch", &self.patch)
            .field("post", &self.post)
            .field("put", &self.put)
            .field("trace", &self.trace)
            .field("connect", &self.connect)
            .field("fallback", &self.fallback)
            .field("allow_header", &self.allow_header)
            .finish()
    }
}

impl<S> MethodRouter<S, Infallible>
where
    S: Clone,
{
    /// Chain an additional handler that will accept requests matching the given
    /// `MethodFilter`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use axum::{
    ///     routing::get,
    ///     Router,
    ///     routing::MethodFilter
    /// };
    ///
    /// async fn handler() {}
    ///
    /// async fn other_handler() {}
    ///
    /// // Requests to `GET /` will go to `handler` and `DELETE /` will go to
    /// // `other_handler`
    /// let app = Router::new().route("/", get(handler).on(MethodFilter::DELETE, other_handler));
    /// # let _: Router = app;
    /// ```
    #[track_caller]
    pub fn on<H, T>(self, filter: MethodFilter, handler: H) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
        S: Send + Sync + 'static,
    {
        self.on_endpoint(
            filter,
            MethodEndpoint::BoxedHandler(BoxedIntoRoute::from_handler(handler)),
        )
    }

    chained_handler_fn!(connect, CONNECT);
    chained_handler_fn!(delete, DELETE);
    chained_handler_fn!(get, GET);
    chained_handler_fn!(head, HEAD);
    chained_handler_fn!(options, OPTIONS);
    chained_handler_fn!(patch, PATCH);
    chained_handler_fn!(post, POST);
    chained_handler_fn!(put, PUT);
    chained_handler_fn!(trace, TRACE);

    /// Add a fallback [`Handler`] to the router.
    pub fn fallback<H, T>(mut self, handler: H) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
        S: Send + Sync + 'static,
    {
        self.fallback = Fallback::BoxedHandler(BoxedIntoRoute::from_handler(handler));
        self
    }

    /// Add a fallback [`Handler`] if no custom one has been provided.
    pub(crate) fn default_fallback<H, T>(self, handler: H) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
        S: Send + Sync + 'static,
    {
        match self.fallback {
            Fallback::Default(_) => self.fallback(handler),
            _ => self,
        }
    }
}

impl MethodRouter<(), Infallible> {
    /// Convert the router into a [`MakeService`].
    ///
    /// This allows you to serve a single `MethodRouter` if you don't need any
    /// routing based on the path:
    ///
    /// ```rust
    /// use axum::{
    ///     handler::Handler,
    ///     http::{Uri, Method},
    ///     response::IntoResponse,
    ///     routing::get,
    /// };
    /// use std::net::SocketAddr;
    ///
    /// async fn handler(method: Method, uri: Uri, body: String) -> String {
    ///     format!("received `{method} {uri}` with body `{body:?}`")
    /// }
    ///
    /// let router = get(handler).post(handler);
    ///
    /// # async {
    /// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    /// axum::serve(listener, router.into_make_service()).await.unwrap();
    /// # };
    /// ```
    ///
    /// [`MakeService`]: tower::make::MakeService
    pub fn into_make_service(self) -> IntoMakeService<Self> {
        IntoMakeService::new(self.with_state(()))
    }

    /// Convert the router into a [`MakeService`] which stores information
    /// about the incoming connection.
    ///
    /// See [`Router::into_make_service_with_connect_info`] for more details.
    ///
    /// ```rust
    /// use axum::{
    ///     handler::Handler,
    ///     response::IntoResponse,
    ///     extract::ConnectInfo,
    ///     routing::get,
    /// };
    /// use std::net::SocketAddr;
    ///
    /// async fn handler(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> String {
    ///     format!("Hello {addr}")
    /// }
    ///
    /// let router = get(handler).post(handler);
    ///
    /// # async {
    /// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    /// axum::serve(listener, router.into_make_service()).await.unwrap();
    /// # };
    /// ```
    ///
    /// [`MakeService`]: tower::make::MakeService
    /// [`Router::into_make_service_with_connect_info`]: crate::routing::Router::into_make_service_with_connect_info
    #[cfg(feature = "tokio")]
    pub fn into_make_service_with_connect_info<C>(self) -> IntoMakeServiceWithConnectInfo<Self, C> {
        IntoMakeServiceWithConnectInfo::new(self.with_state(()))
    }
}

impl<S, E> MethodRouter<S, E>
where
    S: Clone,
{
    /// Create a default `MethodRouter` that will respond with `405 Method Not Allowed` to all
    /// requests.
    pub fn new() -> Self {
        let fallback = Route::new(service_fn(|_: Request| async {
            Ok(StatusCode::METHOD_NOT_ALLOWED)
        }));

        Self {
            get: MethodEndpoint::None,
            head: MethodEndpoint::None,
            delete: MethodEndpoint::None,
            options: MethodEndpoint::None,
            patch: MethodEndpoint::None,
            post: MethodEndpoint::None,
            put: MethodEndpoint::None,
            trace: MethodEndpoint::None,
            connect: MethodEndpoint::None,
            allow_header: AllowHeader::None,
            fallback: Fallback::Default(fallback),
        }
    }

    /// Provide the state for the router.
    pub fn with_state<S2>(self, state: S) -> MethodRouter<S2, E> {
        MethodRouter {
            get: self.get.with_state(&state),
            head: self.head.with_state(&state),
            delete: self.delete.with_state(&state),
            options: self.options.with_state(&state),
            patch: self.patch.with_state(&state),
            post: self.post.with_state(&state),
            put: self.put.with_state(&state),
            trace: self.trace.with_state(&state),
            connect: self.connect.with_state(&state),
            allow_header: self.allow_header,
            fallback: self.fallback.with_state(state),
        }
    }

    /// Chain an additional service that will accept requests matching the given
    /// `MethodFilter`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use axum::{
    ///     extract::Request,
    ///     Router,
    ///     routing::{MethodFilter, on_service},
    ///     body::Body,
    /// };
    /// use http::Response;
    /// use std::convert::Infallible;
    ///
    /// let service = tower::service_fn(|request: Request| async {
    ///     Ok::<_, Infallible>(Response::new(Body::empty()))
    /// });
    ///
    /// // Requests to `DELETE /` will go to `service`
    /// let app = Router::new().route("/", on_service(MethodFilter::DELETE, service));
    /// # let _: Router = app;
    /// ```
    #[track_caller]
    pub fn on_service<T>(self, filter: MethodFilter, svc: T) -> Self
    where
        T: Service<Request, Error = E> + Clone + Send + Sync + 'static,
        T::Response: IntoResponse + 'static,
        T::Future: Send + 'static,
    {
        self.on_endpoint(filter, MethodEndpoint::Route(Route::new(svc)))
    }

    #[track_caller]
    fn on_endpoint(mut self, filter: MethodFilter, endpoint: MethodEndpoint<S, E>) -> Self {
        // written as a separate function to generate less IR
        #[track_caller]
        fn set_endpoint<S, E>(
            method_name: &str,
            out: &mut MethodEndpoint<S, E>,
            endpoint: &MethodEndpoint<S, E>,
            endpoint_filter: MethodFilter,
            filter: MethodFilter,
            allow_header: &mut AllowHeader,
            methods: &[&'static str],
        ) where
            MethodEndpoint<S, E>: Clone,
            S: Clone,
        {
            if endpoint_filter.contains(filter) {
                if out.is_some() {
                    panic!(
                        "Overlapping method route. Cannot add two method routes that both handle \
                         `{method_name}`",
                    )
                }
                *out = endpoint.clone();
                for method in methods {
                    append_allow_header(allow_header, method);
                }
            }
        }

        set_endpoint(
            "GET",
            &mut self.get,
            &endpoint,
            filter,
            MethodFilter::GET,
            &mut self.allow_header,
            &["GET", "HEAD"],
        );

        set_endpoint(
            "HEAD",
            &mut self.head,
            &endpoint,
            filter,
            MethodFilter::HEAD,
            &mut self.allow_header,
            &["HEAD"],
        );

        set_endpoint(
            "TRACE",
            &mut self.trace,
            &endpoint,
            filter,
            MethodFilter::TRACE,
            &mut self.allow_header,
            &["TRACE"],
        );

        set_endpoint(
            "PUT",
            &mut self.put,
            &endpoint,
            filter,
            MethodFilter::PUT,
            &mut self.allow_header,
            &["PUT"],
        );

        set_endpoint(
            "POST",
            &mut self.post,
            &endpoint,
            filter,
            MethodFilter::POST,
            &mut self.allow_header,
            &["POST"],
        );

        set_endpoint(
            "PATCH",
            &mut self.patch,
            &endpoint,
            filter,
            MethodFilter::PATCH,
            &mut self.allow_header,
            &["PATCH"],
        );

        set_endpoint(
            "OPTIONS",
            &mut self.options,
            &endpoint,
            filter,
            MethodFilter::OPTIONS,
            &mut self.allow_header,
            &["OPTIONS"],
        );

        set_endpoint(
            "DELETE",
            &mut self.delete,
            &endpoint,
            filter,
            MethodFilter::DELETE,
            &mut self.allow_header,
            &["DELETE"],
        );

        set_endpoint(
            "CONNECT",
            &mut self.options,
            &endpoint,
            filter,
            MethodFilter::CONNECT,
            &mut self.allow_header,
            &["CONNECT"],
        );

        self
    }

    chained_service_fn!(connect_service, CONNECT);
    chained_service_fn!(delete_service, DELETE);
    chained_service_fn!(get_service, GET);
    chained_service_fn!(head_service, HEAD);
    chained_service_fn!(options_service, OPTIONS);
    chained_service_fn!(patch_service, PATCH);
    chained_service_fn!(post_service, POST);
    chained_service_fn!(put_service, PUT);
    chained_service_fn!(trace_service, TRACE);

    #[doc = include_str!("../docs/method_routing/fallback.md")]
    pub fn fallback_service<T>(mut self, svc: T) -> Self
    where
        T: Service<Request, Error = E> + Clone + Send + Sync + 'static,
        T::Response: IntoResponse + 'static,
        T::Future: Send + 'static,
    {
        self.fallback = Fallback::Service(Route::new(svc));
        self
    }

    #[doc = include_str!("../docs/method_routing/layer.md")]
    pub fn layer<L, NewError>(self, layer: L) -> MethodRouter<S, NewError>
    where
        L: Layer<Route<E>> + Clone + Send + Sync + 'static,
        L::Service: Service<Request> + Clone + Send + Sync + 'static,
        <L::Service as Service<Request>>::Response: IntoResponse + 'static,
        <L::Service as Service<Request>>::Error: Into<NewError> + 'static,
        <L::Service as Service<Request>>::Future: Send + 'static,
        E: 'static,
        S: 'static,
        NewError: 'static,
    {
        let layer_fn = move |route: Route<E>| route.layer(layer.clone());

        MethodRouter {
            get: self.get.map(layer_fn.clone()),
            head: self.head.map(layer_fn.clone()),
            delete: self.delete.map(layer_fn.clone()),
            options: self.options.map(layer_fn.clone()),
            patch: self.patch.map(layer_fn.clone()),
            post: self.post.map(layer_fn.clone()),
            put: self.put.map(layer_fn.clone()),
            trace: self.trace.map(layer_fn.clone()),
            connect: self.connect.map(layer_fn.clone()),
            fallback: self.fallback.map(layer_fn),
            allow_header: self.allow_header,
        }
    }

    #[doc = include_str!("../docs/method_routing/route_layer.md")]
    #[track_caller]
    pub fn route_layer<L>(mut self, layer: L) -> MethodRouter<S, E>
    where
        L: Layer<Route<E>> + Clone + Send + Sync + 'static,
        L::Service: Service<Request, Error = E> + Clone + Send + Sync + 'static,
        <L::Service as Service<Request>>::Response: IntoResponse + 'static,
        <L::Service as Service<Request>>::Future: Send + 'static,
        E: 'static,
        S: 'static,
    {
        if self.get.is_none()
            && self.head.is_none()
            && self.delete.is_none()
            && self.options.is_none()
            && self.patch.is_none()
            && self.post.is_none()
            && self.put.is_none()
            && self.trace.is_none()
            && self.connect.is_none()
        {
            panic!(
                "Adding a route_layer before any routes is a no-op. \
                 Add the routes you want the layer to apply to first."
            );
        }

        let layer_fn = move |svc| Route::new(layer.layer(svc));

        self.get = self.get.map(layer_fn.clone());
        self.head = self.head.map(layer_fn.clone());
        self.delete = self.delete.map(layer_fn.clone());
        self.options = self.options.map(layer_fn.clone());
        self.patch = self.patch.map(layer_fn.clone());
        self.post = self.post.map(layer_fn.clone());
        self.put = self.put.map(layer_fn.clone());
        self.trace = self.trace.map(layer_fn.clone());
        self.connect = self.connect.map(layer_fn);

        self
    }

    pub(crate) fn merge_for_path(
        mut self,
        path: Option<&str>,
        other: MethodRouter<S, E>,
    ) -> Result<Self, Cow<'static, str>> {
        // written using inner functions to generate less IR
        fn merge_inner<S, E>(
            path: Option<&str>,
            name: &str,
            first: MethodEndpoint<S, E>,
            second: MethodEndpoint<S, E>,
        ) -> Result<MethodEndpoint<S, E>, Cow<'static, str>> {
            match (first, second) {
                (MethodEndpoint::None, MethodEndpoint::None) => Ok(MethodEndpoint::None),
                (pick, MethodEndpoint::None) | (MethodEndpoint::None, pick) => Ok(pick),
                _ => {
                    if let Some(path) = path {
                        Err(format!(
                            "Overlapping method route. Handler for `{name} {path}` already exists"
                        )
                        .into())
                    } else {
                        Err(format!(
                            "Overlapping method route. Cannot merge two method routes that both \
                             define `{name}`"
                        )
                        .into())
                    }
                }
            }
        }

        self.get = merge_inner(path, "GET", self.get, other.get)?;
        self.head = merge_inner(path, "HEAD", self.head, other.head)?;
        self.delete = merge_inner(path, "DELETE", self.delete, other.delete)?;
        self.options = merge_inner(path, "OPTIONS", self.options, other.options)?;
        self.patch = merge_inner(path, "PATCH", self.patch, other.patch)?;
        self.post = merge_inner(path, "POST", self.post, other.post)?;
        self.put = merge_inner(path, "PUT", self.put, other.put)?;
        self.trace = merge_inner(path, "TRACE", self.trace, other.trace)?;
        self.connect = merge_inner(path, "CONNECT", self.connect, other.connect)?;

        self.fallback = self
            .fallback
            .merge(other.fallback)
            .ok_or("Cannot merge two `MethodRouter`s that both have a fallback")?;

        self.allow_header = self.allow_header.merge(other.allow_header);

        Ok(self)
    }

    #[doc = include_str!("../docs/method_routing/merge.md")]
    #[track_caller]
    pub fn merge(self, other: MethodRouter<S, E>) -> Self {
        match self.merge_for_path(None, other) {
            Ok(t) => t,
            // not using unwrap or unwrap_or_else to get a clean panic message + the right location
            Err(e) => panic!("{e}"),
        }
    }

    /// Apply a [`HandleErrorLayer`].
    ///
    /// This is a convenience method for doing `self.layer(HandleErrorLayer::new(f))`.
    pub fn handle_error<F, T>(self, f: F) -> MethodRouter<S, Infallible>
    where
        F: Clone + Send + Sync + 'static,
        HandleError<Route<E>, F, T>: Service<Request, Error = Infallible>,
        <HandleError<Route<E>, F, T> as Service<Request>>::Future: Send,
        <HandleError<Route<E>, F, T> as Service<Request>>::Response: IntoResponse + Send,
        T: 'static,
        E: 'static,
        S: 'static,
    {
        self.layer(HandleErrorLayer::new(f))
    }

    fn skip_allow_header(mut self) -> Self {
        self.allow_header = AllowHeader::Skip;
        self
    }

    pub(crate) fn call_with_state(&self, req: Request, state: S) -> RouteFuture<E> {
        macro_rules! call {
            (
                $req:expr,
                $method_variant:ident,
                $svc:expr
            ) => {
                if *req.method() == Method::$method_variant {
                    match $svc {
                        MethodEndpoint::None => {}
                        MethodEndpoint::Route(route) => {
                            return route.clone().oneshot_inner_owned($req);
                        }
                        MethodEndpoint::BoxedHandler(handler) => {
                            let route = handler.clone().into_route(state);
                            return route.oneshot_inner_owned($req);
                        }
                    }
                }
            };
        }

        // written with a pattern match like this to ensure we call all routes
        let Self {
            get,
            head,
            delete,
            options,
            patch,
            post,
            put,
            trace,
            connect,
            fallback,
            allow_header,
        } = self;

        call!(req, HEAD, head);
        call!(req, HEAD, get);
        call!(req, GET, get);
        call!(req, POST, post);
        call!(req, OPTIONS, options);
        call!(req, PATCH, patch);
        call!(req, PUT, put);
        call!(req, DELETE, delete);
        call!(req, TRACE, trace);
        call!(req, CONNECT, connect);

        let future = fallback.clone().call_with_state(req, state);

        match allow_header {
            AllowHeader::None => future.allow_header(Bytes::new()),
            AllowHeader::Skip => future,
            AllowHeader::Bytes(allow_header) => future.allow_header(allow_header.clone().freeze()),
        }
    }
}

fn append_allow_header(allow_header: &mut AllowHeader, method: &'static str) {
    match allow_header {
        AllowHeader::None => {
            *allow_header = AllowHeader::Bytes(BytesMut::from(method));
        }
        AllowHeader::Skip => {}
        AllowHeader::Bytes(allow_header) => {
            if let Ok(s) = std::str::from_utf8(allow_header) {
                if !s.contains(method) {
                    allow_header.extend_from_slice(b",");
                    allow_header.extend_from_slice(method.as_bytes());
                }
            } else {
                #[cfg(debug_assertions)]
                panic!("`allow_header` contained invalid uft-8. This should never happen")
            }
        }
    }
}

impl<S, E> Clone for MethodRouter<S, E> {
    fn clone(&self) -> Self {
        Self {
            get: self.get.clone(),
            head: self.head.clone(),
            delete: self.delete.clone(),
            options: self.options.clone(),
            patch: self.patch.clone(),
            post: self.post.clone(),
            put: self.put.clone(),
            trace: self.trace.clone(),
            connect: self.connect.clone(),
            fallback: self.fallback.clone(),
            allow_header: self.allow_header.clone(),
        }
    }
}

impl<S, E> Default for MethodRouter<S, E>
where
    S: Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

enum MethodEndpoint<S, E> {
    None,
    Route(Route<E>),
    BoxedHandler(BoxedIntoRoute<S, E>),
}

impl<S, E> MethodEndpoint<S, E>
where
    S: Clone,
{
    fn is_some(&self) -> bool {
        matches!(self, Self::Route(_) | Self::BoxedHandler(_))
    }

    fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    fn map<F, E2>(self, f: F) -> MethodEndpoint<S, E2>
    where
        S: 'static,
        E: 'static,
        F: FnOnce(Route<E>) -> Route<E2> + Clone + Send + Sync + 'static,
        E2: 'static,
    {
        match self {
            Self::None => MethodEndpoint::None,
            Self::Route(route) => MethodEndpoint::Route(f(route)),
            Self::BoxedHandler(handler) => MethodEndpoint::BoxedHandler(handler.map(f)),
        }
    }

    fn with_state<S2>(self, state: &S) -> MethodEndpoint<S2, E> {
        match self {
            MethodEndpoint::None => MethodEndpoint::None,
            MethodEndpoint::Route(route) => MethodEndpoint::Route(route),
            MethodEndpoint::BoxedHandler(handler) => {
                MethodEndpoint::Route(handler.into_route(state.clone()))
            }
        }
    }
}

impl<S, E> Clone for MethodEndpoint<S, E> {
    fn clone(&self) -> Self {
        match self {
            Self::None => Self::None,
            Self::Route(inner) => Self::Route(inner.clone()),
            Self::BoxedHandler(inner) => Self::BoxedHandler(inner.clone()),
        }
    }
}

impl<S, E> fmt::Debug for MethodEndpoint<S, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => f.debug_tuple("None").finish(),
            Self::Route(inner) => inner.fmt(f),
            Self::BoxedHandler(_) => f.debug_tuple("BoxedHandler").finish(),
        }
    }
}

impl<B, E> Service<Request<B>> for MethodRouter<(), E>
where
    B: HttpBody<Data = Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
{
    type Response = Response;
    type Error = E;
    type Future = RouteFuture<E>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: Request<B>) -> Self::Future {
        let req = req.map(Body::new);
        self.call_with_state(req, ())
    }
}

impl<S> Handler<(), S> for MethodRouter<S>
where
    S: Clone + 'static,
{
    type Future = InfallibleRouteFuture;

    fn call(self, req: Request, state: S) -> Self::Future {
        InfallibleRouteFuture::new(self.call_with_state(req, state))
    }
}

// for `axum::serve(listener, router)`
#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
const _: () = {
    use crate::serve;

    impl<L> Service<serve::IncomingStream<'_, L>> for MethodRouter<()>
    where
        L: serve::Listener,
    {
        type Response = Self;
        type Error = Infallible;
        type Future = std::future::Ready<Result<Self::Response, Self::Error>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _req: serve::IncomingStream<'_, L>) -> Self::Future {
            std::future::ready(Ok(self.clone().with_state(())))
        }
    }
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{extract::State, handler::HandlerWithoutStateExt};
    use http::{header::ALLOW, HeaderMap};
    use http_body_util::BodyExt;
    use std::time::Duration;
    use tower::ServiceExt;
    use tower_http::{
        services::fs::ServeDir, timeout::TimeoutLayer, validate_request::ValidateRequestHeaderLayer,
    };

    #[crate::test]
    async fn method_not_allowed_by_default() {
        let mut svc = MethodRouter::new();
        let (status, _, body) = call(Method::GET, &mut svc).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert!(body.is_empty());
    }

    #[crate::test]
    async fn get_service_fn() {
        async fn handle(_req: Request) -> Result<Response<Body>, Infallible> {
            Ok(Response::new(Body::from("ok")))
        }

        let mut svc = get_service(service_fn(handle));

        let (status, _, body) = call(Method::GET, &mut svc).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "ok");
    }

    #[crate::test]
    async fn get_handler() {
        let mut svc = MethodRouter::new().get(ok);
        let (status, _, body) = call(Method::GET, &mut svc).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "ok");
    }

    #[crate::test]
    async fn get_accepts_head() {
        let mut svc = MethodRouter::new().get(ok);
        let (status, _, body) = call(Method::HEAD, &mut svc).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.is_empty());
    }

    #[crate::test]
    async fn head_takes_precedence_over_get() {
        let mut svc = MethodRouter::new().head(created).get(ok);
        let (status, _, body) = call(Method::HEAD, &mut svc).await;
        assert_eq!(status, StatusCode::CREATED);
        assert!(body.is_empty());
    }

    #[crate::test]
    async fn merge() {
        let mut svc = get(ok).merge(post(ok));

        let (status, _, _) = call(Method::GET, &mut svc).await;
        assert_eq!(status, StatusCode::OK);

        let (status, _, _) = call(Method::POST, &mut svc).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[crate::test]
    async fn layer() {
        let mut svc = MethodRouter::new()
            .get(|| async { std::future::pending::<()>().await })
            .layer(ValidateRequestHeaderLayer::bearer("password"));

        // method with route
        let (status, _, _) = call(Method::GET, &mut svc).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        // method without route
        let (status, _, _) = call(Method::DELETE, &mut svc).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[crate::test]
    async fn route_layer() {
        let mut svc = MethodRouter::new()
            .get(|| async { std::future::pending::<()>().await })
            .route_layer(ValidateRequestHeaderLayer::bearer("password"));

        // method with route
        let (status, _, _) = call(Method::GET, &mut svc).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        // method without route
        let (status, _, _) = call(Method::DELETE, &mut svc).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    }

    #[allow(dead_code)]
    async fn building_complex_router() {
        let app = crate::Router::new().route(
            "/",
            // use the all the things 💣️
            get(ok)
                .post(ok)
                .route_layer(ValidateRequestHeaderLayer::bearer("password"))
                .merge(delete_service(ServeDir::new(".")))
                .fallback(|| async { StatusCode::NOT_FOUND })
                .put(ok)
                .layer(TimeoutLayer::new(Duration::from_secs(10))),
        );

        let listener = tokio::net::TcpListener::bind("0.0.0.0:0").await.unwrap();
        crate::serve(listener, app).await.unwrap();
    }

    #[crate::test]
    async fn sets_allow_header() {
        let mut svc = MethodRouter::new().put(ok).patch(ok);
        let (status, headers, _) = call(Method::GET, &mut svc).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(headers[ALLOW], "PUT,PATCH");
    }

    #[crate::test]
    async fn sets_allow_header_get_head() {
        let mut svc = MethodRouter::new().get(ok).head(ok);
        let (status, headers, _) = call(Method::PUT, &mut svc).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(headers[ALLOW], "GET,HEAD");
    }

    #[crate::test]
    async fn empty_allow_header_by_default() {
        let mut svc = MethodRouter::new();
        let (status, headers, _) = call(Method::PATCH, &mut svc).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(headers[ALLOW], "");
    }

    #[crate::test]
    async fn allow_header_when_merging() {
        let a = put(ok).patch(ok);
        let b = get(ok).head(ok);
        let mut svc = a.merge(b);

        let (status, headers, _) = call(Method::DELETE, &mut svc).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(headers[ALLOW], "PUT,PATCH,GET,HEAD");
    }

    #[crate::test]
    async fn allow_header_any() {
        let mut svc = any(ok);

        let (status, headers, _) = call(Method::GET, &mut svc).await;
        assert_eq!(status, StatusCode::OK);
        assert!(!headers.contains_key(ALLOW));
    }

    #[crate::test]
    async fn allow_header_with_fallback() {
        let mut svc = MethodRouter::new()
            .get(ok)
            .fallback(|| async { (StatusCode::METHOD_NOT_ALLOWED, "Method not allowed") });

        let (status, headers, _) = call(Method::DELETE, &mut svc).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(headers[ALLOW], "GET,HEAD");
    }

    #[crate::test]
    async fn allow_header_with_fallback_that_sets_allow() {
        async fn fallback(method: Method) -> Response {
            if method == Method::POST {
                "OK".into_response()
            } else {
                (
                    StatusCode::METHOD_NOT_ALLOWED,
                    [(ALLOW, "GET,POST")],
                    "Method not allowed",
                )
                    .into_response()
            }
        }

        let mut svc = MethodRouter::new().get(ok).fallback(fallback);

        let (status, _, _) = call(Method::GET, &mut svc).await;
        assert_eq!(status, StatusCode::OK);

        let (status, _, _) = call(Method::POST, &mut svc).await;
        assert_eq!(status, StatusCode::OK);

        let (status, headers, _) = call(Method::DELETE, &mut svc).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(headers[ALLOW], "GET,POST");
    }

    #[crate::test]
    async fn allow_header_noop_middleware() {
        let mut svc = MethodRouter::new()
            .get(ok)
            .layer(tower::layer::util::Identity::new());

        let (status, headers, _) = call(Method::DELETE, &mut svc).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(headers[ALLOW], "GET,HEAD");
    }

    #[crate::test]
    #[should_panic(
        expected = "Overlapping method route. Cannot add two method routes that both handle `GET`"
    )]
    async fn handler_overlaps() {
        let _: MethodRouter<()> = get(ok).get(ok);
    }

    #[crate::test]
    #[should_panic(
        expected = "Overlapping method route. Cannot add two method routes that both handle `POST`"
    )]
    async fn service_overlaps() {
        let _: MethodRouter<()> = post_service(ok.into_service()).post_service(ok.into_service());
    }

    #[crate::test]
    async fn get_head_does_not_overlap() {
        let _: MethodRouter<()> = get(ok).head(ok);
    }

    #[crate::test]
    async fn head_get_does_not_overlap() {
        let _: MethodRouter<()> = head(ok).get(ok);
    }

    #[crate::test]
    async fn accessing_state() {
        let mut svc = MethodRouter::new()
            .get(|State(state): State<&'static str>| async move { state })
            .with_state("state");

        let (status, _, text) = call(Method::GET, &mut svc).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(text, "state");
    }

    #[crate::test]
    async fn fallback_accessing_state() {
        let mut svc = MethodRouter::new()
            .fallback(|State(state): State<&'static str>| async move { state })
            .with_state("state");

        let (status, _, text) = call(Method::GET, &mut svc).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(text, "state");
    }

    #[crate::test]
    async fn merge_accessing_state() {
        let one = get(|State(state): State<&'static str>| async move { state });
        let two = post(|State(state): State<&'static str>| async move { state });

        let mut svc = one.merge(two).with_state("state");

        let (status, _, text) = call(Method::GET, &mut svc).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(text, "state");

        let (status, _, _) = call(Method::POST, &mut svc).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(text, "state");
    }

    async fn call<S>(method: Method, svc: &mut S) -> (StatusCode, HeaderMap, String)
    where
        S: Service<Request, Error = Infallible>,
        S::Response: IntoResponse,
    {
        let request = Request::builder()
            .uri("/")
            .method(method)
            .body(Body::empty())
            .unwrap();
        let response = svc
            .ready()
            .await
            .unwrap()
            .call(request)
            .await
            .unwrap()
            .into_response();
        let (parts, body) = response.into_parts();
        let body =
            String::from_utf8(BodyExt::collect(body).await.unwrap().to_bytes().to_vec()).unwrap();
        (parts.status, parts.headers, body)
    }

    async fn ok() -> (StatusCode, &'static str) {
        (StatusCode::OK, "ok")
    }

    async fn created() -> (StatusCode, &'static str) {
        (StatusCode::CREATED, "created")
    }
}
