Provide the state for the router. State passed to this method is global and will be used
for all requests this router receives. That means it is not suitable for holding state derived from a request, such as authorization data extracted in a middleware. Use [`Extension`] instead for such data.

```rust
use axum::{Router, routing::get, extract::State};

#[derive(Clone)]
struct AppState {}

let routes = Router::new()
    .route("/", get(|State(state): State<AppState>| async {
        // use state
    }))
    .with_state(AppState {});

# async {
let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
axum::serve(listener, routes).await.unwrap();
# };
```

# Returning routers with states from functions

When returning `Router`s from functions, it is generally recommended not to set the
state directly:

```rust
use axum::{Router, routing::get, extract::State};

#[derive(Clone)]
struct AppState {}

// Don't call `Router::with_state` here
fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(|_: State<AppState>| async {}))
}

// Instead do it before you run the server
let routes = routes().with_state(AppState {});

# async {
let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
axum::serve(listener, routes).await.unwrap();
# };
```

If you do need to provide the state, and you're _not_ nesting/merging the router
into another router, then return `Router` without any type parameters:

```rust
# use axum::{Router, routing::get, extract::State};
# #[derive(Clone)]
# struct AppState {}
#
// Don't return `Router<AppState>`
fn routes(state: AppState) -> Router {
    Router::new()
        .route("/", get(|_: State<AppState>| async {}))
        .with_state(state)
}

let routes = routes(AppState {});

# async {
let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
axum::serve(listener, routes).await.unwrap();
# };
```

This is because we can only call `Router::into_make_service` on `Router<()>`,
not `Router<AppState>`. See below for more details about why that is.

Note that the state defaults to `()` so `Router` and `Router<()>` is the same.

If you are nesting/merging the router it is recommended to use a generic state
type on the resulting router:

```rust
# use axum::{Router, routing::get, extract::State};
# #[derive(Clone)]
# struct AppState {}
#
fn routes<S>(state: AppState) -> Router<S> {
    Router::new()
        .route("/", get(|_: State<AppState>| async {}))
        .with_state(state)
}

let routes = Router::new().nest("/api", routes(AppState {}));

# async {
let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
axum::serve(listener, routes).await.unwrap();
# };
```

# What `S` in `Router<S>` means

`Router<S>` means a router that is _missing_ a state of type `S` to be able to
handle requests. It does _not_ mean a `Router` that _has_ a state of type `S`.

For example:

```rust
# use axum::{Router, routing::get, extract::State};
# #[derive(Clone)]
# struct AppState {}
# 
// A router that _needs_ an `AppState` to handle requests
let router: Router<AppState> = Router::new()
    .route("/", get(|_: State<AppState>| async {}));

// Once we call `Router::with_state` the router isn't missing
// the state anymore, because we just provided it
//
// Therefore the router type becomes `Router<()>`, i.e a router
// that is not missing any state
let router: Router<()> = router.with_state(AppState {});

// Only `Router<()>` has the `into_make_service` method.
//
// You cannot call `into_make_service` on a `Router<AppState>`
// because it is still missing an `AppState`.
# async {
let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
axum::serve(listener, router).await.unwrap();
# };
```

Perhaps a little counter intuitively, `Router::with_state` doesn't always return a
`Router<()>`. Instead you get to pick what the new missing state type is:

```rust
# use axum::{Router, routing::get, extract::State};
# #[derive(Clone)]
# struct AppState {}
# 
let router: Router<AppState> = Router::new()
    .route("/", get(|_: State<AppState>| async {}));

// When we call `with_state` we're able to pick what the next missing state type is.
// Here we pick `String`.
let string_router: Router<String> = router.with_state(AppState {});

// That allows us to add new routes that uses `String` as the state type
let string_router = string_router
    .route("/needs-string", get(|_: State<String>| async {}));

// Provide the `String` and choose `()` as the new missing state.
let final_router: Router<()> = string_router.with_state("foo".to_owned());

// Since we have a `Router<()>` we can run it.
# async {
let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
axum::serve(listener, final_router).await.unwrap();
# };
```

This why this returning `Router<AppState>` after calling `with_state` doesn't
work:

```rust,compile_fail
# use axum::{Router, routing::get, extract::State};
# #[derive(Clone)]
# struct AppState {}
# 
// This won't work because we're returning a `Router<AppState>`
// i.e. we're saying we're still missing an `AppState`
fn routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(|_: State<AppState>| async {}))
        .with_state(state)
}

let app = routes(AppState {});

// We can only call `Router::into_make_service` on a `Router<()>`
// but `app` is a `Router<AppState>`
# async {
let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
axum::serve(listener, app).await.unwrap();
# };
```

Instead return `Router<()>` since we have provided all the state needed:

```rust
# use axum::{Router, routing::get, extract::State};
# #[derive(Clone)]
# struct AppState {}
# 
// We've provided all the state necessary so return `Router<()>`
fn routes(state: AppState) -> Router<()> {
    Router::new()
        .route("/", get(|_: State<AppState>| async {}))
        .with_state(state)
}

let app = routes(AppState {});

// We can now call `Router::into_make_service`
# async {
let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
axum::serve(listener, app).await.unwrap();
# };
```

# A note about performance

If you need a `Router` that implements `Service` but you don't need any state (perhaps
you're making a library that uses axum internally) then it is recommended to call this
method before you start serving requests:

```rust
use axum::{Router, routing::get};

let app = Router::new()
    .route("/", get(|| async { /* ... */ }))
    // even though we don't need any state, call `with_state(())` anyway
    .with_state(());
# let _: Router = app;
```

This is not required but it gives axum a chance to update some internals in the router
which may impact performance and reduce allocations.

Note that [`Router::into_make_service`] and [`Router::into_make_service_with_connect_info`]
do this automatically.

[`Extension`]: crate::Extension
