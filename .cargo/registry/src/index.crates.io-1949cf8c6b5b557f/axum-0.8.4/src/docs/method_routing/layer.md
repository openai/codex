Apply a [`tower::Layer`] to all routes in the router.

This can be used to add additional processing to a request for a group
of routes.

Note that the middleware is only applied to existing routes. So you have to
first add your routes (and / or fallback) and then call `layer` afterwards. Additional
routes added after `layer` is called will not have the middleware added.

Works similarly to [`Router::layer`](super::Router::layer). See that method for
more details.

# Example

```rust
use axum::{routing::get, Router};
use tower::limit::ConcurrencyLimitLayer;

async fn handler() {}

let app = Router::new().route(
    "/",
    // All requests to `GET /` will be sent through `ConcurrencyLimitLayer`
    get(handler).layer(ConcurrencyLimitLayer::new(64)),
);
# let _: Router = app;
```
