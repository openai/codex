Apply a [`tower::Layer`] to all routes in the router.

This can be used to add additional processing to a request for a group
of routes.

Note that the middleware is only applied to existing routes. So you have to
first add your routes (and / or fallback) and then call `layer` afterwards. Additional
routes added after `layer` is called will not have the middleware added.

If you want to add middleware to a single handler you can either use
[`MethodRouter::layer`] or [`Handler::layer`].

# Example

Adding the [`tower_http::trace::TraceLayer`]:

```rust
use axum::{routing::get, Router};
use tower_http::trace::TraceLayer;

let app = Router::new()
    .route("/foo", get(|| async {}))
    .route("/bar", get(|| async {}))
    .layer(TraceLayer::new_for_http());
# let _: Router = app;
```

If you need to write your own middleware see ["Writing
middleware"](crate::middleware#writing-middleware) for the different options.

If you only want middleware on some routes you can use [`Router::merge`]:

```rust
use axum::{routing::get, Router};
use tower_http::{trace::TraceLayer, compression::CompressionLayer};

let with_tracing = Router::new()
    .route("/foo", get(|| async {}))
    .layer(TraceLayer::new_for_http());

let with_compression = Router::new()
    .route("/bar", get(|| async {}))
    .layer(CompressionLayer::new());

// Merge everything into one `Router`
let app = Router::new()
    .merge(with_tracing)
    .merge(with_compression);
# let _: Router = app;
```

# Multiple middleware

It's recommended to use [`tower::ServiceBuilder`] when applying multiple
middleware. See [`middleware`](crate::middleware) for more details.

# Runs after routing

Middleware added with this method will run _after_ routing and thus cannot be
used to rewrite the request URI. See ["Rewriting request URI in
middleware"](crate::middleware#rewriting-request-uri-in-middleware) for more
details and a workaround.

# Error handling

See [`middleware`](crate::middleware) for details on how error handling impacts
middleware.
