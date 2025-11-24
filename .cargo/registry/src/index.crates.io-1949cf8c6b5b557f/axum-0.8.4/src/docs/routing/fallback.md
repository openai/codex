Add a fallback [`Handler`] to the router.

This service will be called if no routes matches the incoming request.

```rust
use axum::{
    Router,
    routing::get,
    handler::Handler,
    response::IntoResponse,
    http::{StatusCode, Uri},
};

let app = Router::new()
    .route("/foo", get(|| async { /* ... */ }))
    .fallback(fallback);

async fn fallback(uri: Uri) -> (StatusCode, String) {
    (StatusCode::NOT_FOUND, format!("No route for {uri}"))
}
# let _: Router = app;
```

Fallbacks only apply to routes that aren't matched by anything in the
router. If a handler is matched by a request but returns 404 the
fallback is not called. Note that this applies to [`MethodRouter`]s too: if the
request hits a valid path but the [`MethodRouter`] does not have an appropriate
method handler installed, the fallback is not called (use
[`MethodRouter::fallback`] for this purpose instead).


# Handling all requests without other routes

Using `Router::new().fallback(...)` to accept all request regardless of path or
method, if you don't have other routes, isn't optimal:

```rust
use axum::Router;

async fn handler() {}

let app = Router::new().fallback(handler);

# async {
let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
axum::serve(listener, app).await.unwrap();
# };
```

Running the handler directly is faster since it avoids the overhead of routing:

```rust
use axum::handler::HandlerWithoutStateExt;

async fn handler() {}

# async {
let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
axum::serve(listener, handler.into_make_service()).await.unwrap();
# };
```
