Types and traits for generating responses.

# Building responses

Anything that implements [`IntoResponse`] can be returned from a handler. axum
provides implementations for common types:

```rust,no_run
use axum::{
    Json,
    response::{Html, IntoResponse},
    http::{StatusCode, Uri, header::{self, HeaderMap, HeaderName}},
};

// `()` gives an empty response
async fn empty() {}

// String will get a `text/plain; charset=utf-8` content-type
async fn plain_text(uri: Uri) -> String {
    format!("Hi from {}", uri.path())
}

// Bytes will get a `application/octet-stream` content-type
async fn bytes() -> Vec<u8> {
    vec![1, 2, 3, 4]
}

// `Json` will get a `application/json` content-type and work with anything that
// implements `serde::Serialize`
async fn json() -> Json<Vec<String>> {
    Json(vec!["foo".to_owned(), "bar".to_owned()])
}

// `Html` will get a `text/html` content-type
async fn html() -> Html<&'static str> {
    Html("<p>Hello, World!</p>")
}

// `StatusCode` gives an empty response with that status code
async fn status() -> StatusCode {
    StatusCode::NOT_FOUND
}

// `HeaderMap` gives an empty response with some headers
async fn headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::SERVER, "axum".parse().unwrap());
    headers
}

// An array of tuples also gives headers
async fn array_headers() -> [(HeaderName, &'static str); 2] {
    [
        (header::SERVER, "axum"),
        (header::CONTENT_TYPE, "text/plain")
    ]
}

// Use `impl IntoResponse` to avoid writing the whole type
async fn impl_trait() -> impl IntoResponse {
    [
        (header::SERVER, "axum"),
        (header::CONTENT_TYPE, "text/plain")
    ]
}
```

Additionally you can return tuples to build more complex responses from
individual parts.

```rust,no_run
use axum::{
    Json,
    response::IntoResponse,
    http::{StatusCode, HeaderMap, Uri, header},
    extract::Extension,
};

// `(StatusCode, impl IntoResponse)` will override the status code of the response
async fn with_status(uri: Uri) -> (StatusCode, String) {
    (StatusCode::NOT_FOUND, format!("Not Found: {}", uri.path()))
}

// Use `impl IntoResponse` to avoid having to type the whole type
async fn impl_trait(uri: Uri) -> impl IntoResponse {
    (StatusCode::NOT_FOUND, format!("Not Found: {}", uri.path()))
}

// `(HeaderMap, impl IntoResponse)` to add additional headers
async fn with_headers() -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "text/plain".parse().unwrap());
    (headers, "foo")
}

// Or an array of tuples to more easily build the headers
async fn with_array_headers() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/plain")], "foo")
}

// Use string keys for custom headers
async fn with_array_headers_custom() -> impl IntoResponse {
    ([("x-custom", "custom")], "foo")
}

// `(StatusCode, headers, impl IntoResponse)` to set status and add headers
// `headers` can be either a `HeaderMap` or an array of tuples
async fn with_status_and_array_headers() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        [(header::CONTENT_TYPE, "text/plain")],
        "foo",
    )
}

// `(Extension<_>, impl IntoResponse)` to set response extensions
async fn with_status_extensions() -> impl IntoResponse {
    (
        Extension(Foo("foo")),
        "foo",
    )
}

#[derive(Clone)]
struct Foo(&'static str);

// Or mix and match all the things
async fn all_the_things(uri: Uri) -> impl IntoResponse {
    let mut header_map = HeaderMap::new();
    if uri.path() == "/" {
        header_map.insert(header::SERVER, "axum".parse().unwrap());
    }

    (
        // set status code
        StatusCode::NOT_FOUND,
        // headers with an array
        [("x-custom", "custom")],
        // some extensions
        Extension(Foo("foo")),
        Extension(Foo("bar")),
        // more headers, built dynamically
        header_map,
        // and finally the body
        "foo",
    )
}
```

In general you can return tuples like:

- `(StatusCode, impl IntoResponse)`
- `(Parts, impl IntoResponse)`
- `(Response<()>, impl IntoResponse)`
- `(T1, .., Tn, impl IntoResponse)` where `T1` to `Tn` all implement [`IntoResponseParts`].
- `(StatusCode, T1, .., Tn, impl IntoResponse)` where `T1` to `Tn` all implement [`IntoResponseParts`].
- `(Parts, T1, .., Tn, impl IntoResponse)` where `T1` to `Tn` all implement [`IntoResponseParts`].
- `(Response<()>, T1, .., Tn, impl IntoResponse)` where `T1` to `Tn` all implement [`IntoResponseParts`].

This means you cannot accidentally override the status or body as [`IntoResponseParts`] only allows
setting headers and extensions.

Use [`Response`] for more low level control:

```rust,no_run
use axum::{
    Json,
    response::{IntoResponse, Response},
    body::Body,
    http::StatusCode,
};

async fn response() -> Response {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("x-foo", "custom header")
        .body(Body::from("not found"))
        .unwrap()
}
```

# Returning different response types

If you need to return multiple response types, and `Result<T, E>` isn't appropriate, you can call
`.into_response()` to turn things into `axum::response::Response`:

```rust
use axum::{
    response::{IntoResponse, Redirect, Response},
    http::StatusCode,
};

async fn handle() -> Response {
    if something() {
        "All good!".into_response()
    } else if something_else() {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Something went wrong...",
        ).into_response()
    } else {
        Redirect::to("/").into_response()
    }
}

fn something() -> bool {
    // ...
    # true
}

fn something_else() -> bool {
    // ...
    # true
}
```

# Regarding `impl IntoResponse`

You can use `impl IntoResponse` as the return type from handlers to avoid
typing large types. For example

```rust
use axum::http::StatusCode;

async fn handler() -> (StatusCode, [(&'static str, &'static str); 1], &'static str) {
    (StatusCode::OK, [("x-foo", "bar")], "Hello, World!")
}
```

Becomes easier using `impl IntoResponse`:

```rust
use axum::{http::StatusCode, response::IntoResponse};

async fn impl_into_response() -> impl IntoResponse {
    (StatusCode::OK, [("x-foo", "bar")], "Hello, World!")
}
```

However `impl IntoResponse` has a few limitations. Firstly it can only be used
to return a single type:

```rust,compile_fail
use axum::{http::StatusCode, response::IntoResponse};

async fn handler() -> impl IntoResponse {
    if check_something() {
        StatusCode::NOT_FOUND
    } else {
        "Hello, World!"
    }
}

fn check_something() -> bool {
    # false
    // ...
}
```

This function returns either a `StatusCode` or a `&'static str` which `impl
Trait` doesn't allow.

Secondly `impl IntoResponse` can lead to type inference issues when used with
`Result` and `?`:

```rust,compile_fail
use axum::{http::StatusCode, response::IntoResponse};

async fn handler() -> impl IntoResponse {
    create_thing()?;
    Ok(StatusCode::CREATED)
}

fn create_thing() -> Result<(), StatusCode> {
    # Ok(())
    // ...
}
```

This is because `?` supports using the [`From`] trait to convert to a different
error type but it doesn't know which type to convert to, because we only
specified `impl IntoResponse` as the return type.

`Result<impl IntoResponse, impl IntoResponse>` doesn't always work either:

```rust,compile_fail
use axum::{http::StatusCode, response::IntoResponse};

async fn handler() -> Result<impl IntoResponse, impl IntoResponse> {
    create_thing()?;
    Ok(StatusCode::CREATED)
}

fn create_thing() -> Result<(), StatusCode> {
    # Ok(())
    // ...
}
```

The solution is to use a concrete error type, such as `Result<impl IntoResponse, StatusCode>`:

```rust
use axum::{http::StatusCode, response::IntoResponse};

async fn handler() -> Result<impl IntoResponse, StatusCode> {
    create_thing()?;
    Ok(StatusCode::CREATED)
}

fn create_thing() -> Result<(), StatusCode> {
    # Ok(())
    // ...
}
```

Because of this it is generally not recommended to use `impl IntoResponse`
unless you're familiar with the details of how `impl Trait` works.

[`IntoResponse`]: crate::response::IntoResponse
[`IntoResponseParts`]: crate::response::IntoResponseParts
[`StatusCode`]: http::StatusCode
