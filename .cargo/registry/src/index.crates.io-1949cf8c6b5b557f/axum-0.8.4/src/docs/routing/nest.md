Nest a [`Router`] at some path.

This allows you to break your application into smaller pieces and compose
them together.

# Example

```rust
use axum::{
    routing::{get, post},
    Router,
};

let user_routes = Router::new().route("/{id}", get(|| async {}));

let team_routes = Router::new().route("/", post(|| async {}));

let api_routes = Router::new()
    .nest("/users", user_routes)
    .nest("/teams", team_routes);

let app = Router::new().nest("/api", api_routes);

// Our app now accepts
// - GET /api/users/{id}
// - POST /api/teams
# let _: Router = app;
```

# How the URI changes

Note that nested routes will not see the original request URI but instead
have the matched prefix stripped. This is necessary for services like static
file serving to work. Use [`OriginalUri`] if you need the original request
URI.

# Captures from outer routes

Take care when using `nest` together with dynamic routes as nesting also
captures from the outer routes:

```rust
use axum::{
    extract::Path,
    routing::get,
    Router,
};
use std::collections::HashMap;

async fn users_get(Path(params): Path<HashMap<String, String>>) {
    // Both `version` and `id` were captured even though `users_api` only
    // explicitly captures `id`.
    let version = params.get("version");
    let id = params.get("id");
}

let users_api = Router::new().route("/users/{id}", get(users_get));

let app = Router::new().nest("/{version}/api", users_api);
# let _: Router = app;
```

# Differences from wildcard routes

Nested routes are similar to wildcard routes. The difference is that
wildcard routes still see the whole URI whereas nested routes will have
the prefix stripped:

```rust
use axum::{routing::get, http::Uri, Router};

let nested_router = Router::new()
    .route("/", get(|uri: Uri| async {
        // `uri` will _not_ contain `/bar`
    }));

let app = Router::new()
    .route("/foo/{*rest}", get(|uri: Uri| async {
        // `uri` will contain `/foo`
    }))
    .nest("/bar", nested_router);
# let _: Router = app;
```

Additionally, while the wildcard route `/foo/*rest` will not match the
paths `/foo` or `/foo/`, a nested router at `/foo` will match the path `/foo`
(but not `/foo/`), and a nested router at `/foo/` will match the path `/foo/`
(but not `/foo`).

# Fallbacks

If a nested router doesn't have its own fallback then it will inherit the
fallback from the outer router:

```rust
use axum::{routing::get, http::StatusCode, handler::Handler, Router};

async fn fallback() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Not Found")
}

let api_routes = Router::new().route("/users", get(|| async {}));

let app = Router::new()
    .nest("/api", api_routes)
    .fallback(fallback);
# let _: Router = app;
```

Here requests like `GET /api/not-found` will go into `api_routes` but because
it doesn't have a matching route and doesn't have its own fallback it will call
the fallback from the outer router, i.e. the `fallback` function.

If the nested router has its own fallback then the outer fallback will not be
inherited:

```rust
use axum::{
    routing::get,
    http::StatusCode,
    handler::Handler,
    Json,
    Router,
};

async fn fallback() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Not Found")
}

async fn api_fallback() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "status": "Not Found" })),
    )
}

let api_routes = Router::new()
    .route("/users", get(|| async {}))
    .fallback(api_fallback);

let app = Router::new()
    .nest("/api", api_routes)
    .fallback(fallback);
# let _: Router = app;
```

Here requests like `GET /api/not-found` will go to `api_fallback`.

# Nesting routers with state

When combining [`Router`]s with this method, each [`Router`] must have the
same type of state. If your routers have different types you can use
[`Router::with_state`] to provide the state and make the types match:

```rust
use axum::{
    Router,
    routing::get,
    extract::State,
};

#[derive(Clone)]
struct InnerState {}

#[derive(Clone)]
struct OuterState {}

async fn inner_handler(state: State<InnerState>) {}

let inner_router = Router::new()
    .route("/bar", get(inner_handler))
    .with_state(InnerState {});

async fn outer_handler(state: State<OuterState>) {}

let app = Router::new()
    .route("/", get(outer_handler))
    .nest("/foo", inner_router)
    .with_state(OuterState {});
# let _: axum::Router = app;
```

Note that the inner router will still inherit the fallback from the outer
router.

# Panics

- If the route overlaps with another route. See [`Router::route`]
  for more details.
- If the route contains a wildcard (`*`).
- If `path` is empty.

[`OriginalUri`]: crate::extract::OriginalUri
[fallbacks]: Router::fallback
