Merge the paths and fallbacks of two routers into a single [`Router`].

This is useful for breaking apps into smaller pieces and combining them
into one.

```rust
use axum::{
    routing::get,
    Router,
};
#
# async fn users_list() {}
# async fn users_show() {}
# async fn teams_list() {}

// define some routes separately
let user_routes = Router::new()
    .route("/users", get(users_list))
    .route("/users/{id}", get(users_show));

let team_routes = Router::new()
    .route("/teams", get(teams_list));

// combine them into one
let app = Router::new()
    .merge(user_routes)
    .merge(team_routes);

// could also do `user_routes.merge(team_routes)`

// Our app now accepts
// - GET /users
// - GET /users/{id}
// - GET /teams
# let _: Router = app;
```

# Merging routers with state

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
    .merge(inner_router)
    .with_state(OuterState {});
# let _: axum::Router = app;
```

# Merging routers with fallbacks

When combining [`Router`]s with this method, the [fallback](Router::fallback) is also merged.
However only one of the routers can have a fallback.

# Panics

- If two routers that each have a [fallback](Router::fallback) are merged. This
  is because `Router` only allows a single fallback.
