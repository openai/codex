Turn off checks for compatibility with route matching syntax from 0.7.

This allows usage of paths starting with a colon `:` or an asterisk `*` which are otherwise prohibited.

# Example

```rust
use axum::{
    routing::get,
    Router,
};

let app = Router::<()>::new()
    .without_v07_checks()
    .route("/:colon", get(|| async {}))
    .route("/*asterisk", get(|| async {}));

// Our app now accepts
// - GET /:colon
// - GET /*asterisk
# let _: Router = app;
```

Adding such routes without calling this method first will panic.

```rust,should_panic
use axum::{
    routing::get,
    Router,
};

// This panics...
let app = Router::<()>::new()
    .route("/:colon", get(|| async {}));
```

# Merging

When two routers are merged, v0.7 checks are disabled for route registrations on the resulting router if both of the two routers had them also disabled.

# Nesting

Each router needs to have the checks explicitly disabled. Nesting a router with the checks either enabled or disabled has no effect on the outer router.
