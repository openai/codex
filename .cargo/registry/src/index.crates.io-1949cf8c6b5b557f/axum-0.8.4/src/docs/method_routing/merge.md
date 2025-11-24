Merge two routers into one.

This is useful for breaking routers into smaller pieces and combining them
into one.

```rust
use axum::{
    routing::{get, post},
    Router,
};

let get = get(|| async {});
let post = post(|| async {});

let merged = get.merge(post);

let app = Router::new().route("/", merged);

// Our app now accepts
// - GET /
// - POST /
# let _: Router = app;
```
