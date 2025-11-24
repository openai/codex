Add a fallback [`Handler`] for the case where a route exists, but the method of the request is not supported.

Sets a fallback on all previously registered [`MethodRouter`]s,
to be called when no matching method handler is set.

```rust,no_run
use axum::{response::IntoResponse, routing::get, Router};

async fn hello_world() -> impl IntoResponse {
    "Hello, world!\n"
}

async fn default_fallback() -> impl IntoResponse {
    "Default fallback\n"
}

async fn handle_405() -> impl IntoResponse {
    "Method not allowed fallback"
}

#[tokio::main]
async fn main() {
    let router = Router::new()
        .route("/", get(hello_world))
        .fallback(default_fallback)
        .method_not_allowed_fallback(handle_405);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    axum::serve(listener, router).await.unwrap();
}
```

The fallback only applies if there is a `MethodRouter` registered for a given path, 
but the method used in the request is not specified. In the example, a `GET` on 
`http://localhost:3000` causes the `hello_world` handler to react, while issuing a 
`POST` triggers `handle_405`. Calling an entirely different route, like `http://localhost:3000/hello` 
causes `default_fallback` to run.
