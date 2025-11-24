Add another route to the router that calls a [`Service`].

# Example

```rust,no_run
use axum::{
    Router,
    body::Body,
    routing::{any_service, get_service},
    extract::Request,
    http::StatusCode,
    error_handling::HandleErrorLayer,
};
use tower_http::services::ServeFile;
use http::Response;
use std::{convert::Infallible, io};
use tower::service_fn;

let app = Router::new()
    .route(
        // Any request to `/` goes to a service
        "/",
        // Services whose response body is not `axum::body::BoxBody`
        // can be wrapped in `axum::routing::any_service` (or one of the other routing filters)
        // to have the response body mapped
        any_service(service_fn(|_: Request| async {
            let res = Response::new(Body::from("Hi from `GET /`"));
            Ok::<_, Infallible>(res)
        }))
    )
    .route_service(
        "/foo",
        // This service's response body is `axum::body::BoxBody` so
        // it can be routed to directly.
        service_fn(|req: Request| async move {
            let body = Body::from(format!("Hi from `{} /foo`", req.method()));
            let res = Response::new(body);
            Ok::<_, Infallible>(res)
        })
    )
    .route_service(
        // GET `/static/Cargo.toml` goes to a service from tower-http
        "/static/Cargo.toml",
        ServeFile::new("Cargo.toml"),
    );
# let _: Router = app;
```

Routing to arbitrary services in this way has complications for backpressure
([`Service::poll_ready`]). See the [Routing to services and backpressure] module
for more details.

# Panics

Panics for the same reasons as [`Router::route`] or if you attempt to route to a
`Router`:

```rust,should_panic
use axum::{routing::get, Router};

let app = Router::new().route_service(
    "/",
    Router::new().route("/foo", get(|| async {})),
);
# let _: Router = app;
```

Use [`Router::nest`] instead.

[Routing to services and backpressure]: middleware/index.html#routing-to-servicesmiddleware-and-backpressure
