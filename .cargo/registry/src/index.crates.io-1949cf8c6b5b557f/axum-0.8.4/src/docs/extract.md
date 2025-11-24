Types and traits for extracting data from requests.

# Intro

A handler function is an async function that takes any number of
"extractors" as arguments. An extractor is a type that implements
[`FromRequest`] or [`FromRequestParts`].

For example, [`Json`] is an extractor that consumes the request body and
deserializes it as JSON into some target type:

```rust,no_run
use axum::{
    extract::Json,
    routing::post,
    handler::Handler,
    Router,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct CreateUser {
    email: String,
    password: String,
}

async fn create_user(Json(payload): Json<CreateUser>) {
    // ...
}

let app = Router::new().route("/users", post(create_user));
# let _: Router = app;
```

# Common extractors

Some commonly used extractors are:

```rust,no_run
use axum::{
    extract::{Request, Json, Path, Extension, Query},
    routing::post,
    http::header::HeaderMap,
    body::{Bytes, Body},
    Router,
};
use serde_json::Value;
use std::collections::HashMap;

// `Path` gives you the path parameters and deserializes them. See its docs for
// more details
async fn path(Path(user_id): Path<u32>) {}

// `Query` gives you the query parameters and deserializes them.
async fn query(Query(params): Query<HashMap<String, String>>) {}

// `HeaderMap` gives you all the headers
async fn headers(headers: HeaderMap) {}

// `String` consumes the request body and ensures it is valid utf-8
async fn string(body: String) {}

// `Bytes` gives you the raw request body
async fn bytes(body: Bytes) {}

// We've already seen `Json` for parsing the request body as json
async fn json(Json(payload): Json<Value>) {}

// `Request` gives you the whole request for maximum control
async fn request(request: Request) {}

// `Extension` extracts data from "request extensions"
// This is commonly used to share state with handlers
async fn extension(Extension(state): Extension<State>) {}

#[derive(Clone)]
struct State { /* ... */ }

let app = Router::new()
    .route("/path/{user_id}", post(path))
    .route("/query", post(query))
    .route("/string", post(string))
    .route("/bytes", post(bytes))
    .route("/json", post(json))
    .route("/request", post(request))
    .route("/extension", post(extension));
# let _: Router = app;
```

# Applying multiple extractors

You can also apply multiple extractors:

```rust,no_run
use axum::{
    extract::{Path, Query},
    routing::get,
    Router,
};
use uuid::Uuid;
use serde::Deserialize;

let app = Router::new().route("/users/{id}/things", get(get_user_things));

#[derive(Deserialize)]
struct Pagination {
    page: usize,
    per_page: usize,
}

async fn get_user_things(
    Path(user_id): Path<Uuid>,
    Query(pagination): Query<Pagination>,
) {
    // ...
}
# let _: Router = app;
```

# The order of extractors

Extractors always run in the order of the function parameters that is from
left to right.

The request body is an asynchronous stream that can only be consumed once.
Therefore you can only have one extractor that consumes the request body. axum
enforces this by requiring such extractors to be the _last_ argument your
handler takes.

For example

```rust
use axum::{extract::State, http::{Method, HeaderMap}};
#
# #[derive(Clone)]
# struct AppState {
# }

async fn handler(
    // `Method` and `HeaderMap` don't consume the request body so they can
    // put anywhere in the argument list (but before `body`)
    method: Method,
    headers: HeaderMap,
    // `State` is also an extractor so it needs to be before `body`
    State(state): State<AppState>,
    // `String` consumes the request body and thus must be the last extractor
    body: String,
) {
    // ...
}
#
# let _: axum::routing::MethodRouter<AppState> = axum::routing::get(handler);
```

We get a compile error if `String` isn't the last extractor:

```rust,compile_fail
use axum::http::Method;

async fn handler(
    // this doesn't work since `String` must be the last argument
    body: String,
    method: Method,
) {
    // ...
}
#
# let _: axum::routing::MethodRouter = axum::routing::get(handler);
```

This also means you cannot consume the request body twice:

```rust,compile_fail
use axum::Json;
use serde::Deserialize;

#[derive(Deserialize)]
struct Payload {}

async fn handler(
    // `String` and `Json` both consume the request body
    // so they cannot both be used
    string_body: String,
    json_body: Json<Payload>,
) {
    // ...
}
#
# let _: axum::routing::MethodRouter = axum::routing::get(handler);
```

axum enforces this by requiring the last extractor implements [`FromRequest`]
and all others implement [`FromRequestParts`].

# Handling extractor rejections

If you want to handle the case of an extractor failing within a specific
handler, you can wrap it in `Result`, with the error being the rejection type
of the extractor:

```rust,no_run
use axum::{
    extract::{Json, rejection::JsonRejection},
    routing::post,
    Router,
};
use serde_json::Value;

async fn create_user(payload: Result<Json<Value>, JsonRejection>) {
    match payload {
        Ok(payload) => {
            // We got a valid JSON payload
        }
        Err(JsonRejection::MissingJsonContentType(_)) => {
            // Request didn't have `Content-Type: application/json`
            // header
        }
        Err(JsonRejection::JsonDataError(_)) => {
            // Couldn't deserialize the body into the target type
        }
        Err(JsonRejection::JsonSyntaxError(_)) => {
            // Syntax error in the body
        }
        Err(JsonRejection::BytesRejection(_)) => {
            // Failed to extract the request body
        }
        Err(_) => {
            // `JsonRejection` is marked `#[non_exhaustive]` so match must
            // include a catch-all case.
        }
    }
}

let app = Router::new().route("/users", post(create_user));
# let _: Router = app;
```

# Optional extractors

Some extractors implement [`OptionalFromRequestParts`] in addition to
[`FromRequestParts`], or [`OptionalFromRequest`] in addition to [`FromRequest`].

These extractors can be used inside of `Option`. It depends on the particular
`OptionalFromRequestParts` or `OptionalFromRequest` implementation what this
does: For example for `TypedHeader` from axum-extra, you get `None` if the
header you're trying to extract is not part of the request, but if the header
is present and fails to parse, the request is rejected.

```rust,no_run
use axum::{routing::post, Router};
use axum_extra::{headers::UserAgent, TypedHeader};
use serde_json::Value;

async fn foo(user_agent: Option<TypedHeader<UserAgent>>) {
    if let Some(TypedHeader(user_agent)) = user_agent {
        // The client sent a user agent
    } else {
        // No user agent header
    }
}

let app = Router::new().route("/foo", post(foo));
# let _: Router = app;
```

# Customizing extractor responses

If an extractor fails it will return a response with the error and your
handler will not be called. To customize the error response you have two 
options:

1. Use `Result<T, T::Rejection>` as your extractor like shown in ["Optional
   extractors"](#optional-extractors). This works well if you're only using
   the extractor in a single handler.
2. Create your own extractor that in its [`FromRequest`] implementation calls
   one of axum's built in extractors but returns a different response for
   rejections. See the [customize-extractor-error] example for more details.

# Accessing inner errors

axum's built-in extractors don't directly expose the inner error. This gives us
more flexibility and allows us to change internal implementations without
breaking the public API.

For example that means while [`Json`] is implemented using [`serde_json`] it
doesn't directly expose the [`serde_json::Error`] that's contained in
[`JsonRejection::JsonDataError`]. However it is still possible to access via
methods from [`std::error::Error`]:

```rust
use std::error::Error;
use axum::{
    extract::{Json, rejection::JsonRejection},
    response::IntoResponse,
    http::StatusCode,
};
use serde_json::{json, Value};

async fn handler(
    result: Result<Json<Value>, JsonRejection>,
) -> Result<Json<Value>, (StatusCode, String)> {
    match result {
        // if the client sent valid JSON then we're good
        Ok(Json(payload)) => Ok(Json(json!({ "payload": payload }))),

        Err(err) => match err {
            JsonRejection::JsonDataError(err) => {
                Err(serde_json_error_response(err))
            }
            JsonRejection::JsonSyntaxError(err) => {
                Err(serde_json_error_response(err))
            }
            // handle other rejections from the `Json` extractor
            JsonRejection::MissingJsonContentType(_) => Err((
                StatusCode::BAD_REQUEST,
                "Missing `Content-Type: application/json` header".to_string(),
            )),
            JsonRejection::BytesRejection(_) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to buffer request body".to_string(),
            )),
            // we must provide a catch-all case since `JsonRejection` is marked
            // `#[non_exhaustive]`
            _ => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Unknown error".to_string(),
            )),
        },
    }
}

// attempt to extract the inner `serde_path_to_error::Error<serde_json::Error>`,
// if that succeeds we can provide a more specific error.
//
// `Json` uses `serde_path_to_error` so the error will be wrapped in `serde_path_to_error::Error`.
fn serde_json_error_response<E>(err: E) -> (StatusCode, String)
where
    E: Error + 'static,
{
    if let Some(err) = find_error_source::<serde_path_to_error::Error<serde_json::Error>>(&err) {
        let serde_json_err = err.inner();
        (
            StatusCode::BAD_REQUEST,
            format!(
                "Invalid JSON at line {} column {}",
                serde_json_err.line(),
                serde_json_err.column()
            ),
        )
    } else {
        (StatusCode::BAD_REQUEST, "Unknown error".to_string())
    }
}

// attempt to downcast `err` into a `T` and if that fails recursively try and
// downcast `err`'s source
fn find_error_source<'a, T>(err: &'a (dyn Error + 'static)) -> Option<&'a T>
where
    T: Error + 'static,
{
    if let Some(err) = err.downcast_ref::<T>() {
        Some(err)
    } else if let Some(source) = err.source() {
        find_error_source(source)
    } else {
        None
    }
}
# 
# #[tokio::main]
# async fn main() {
#     use axum::extract::FromRequest;
# 
#     let req = axum::http::Request::builder()
#         .header("content-type", "application/json")
#         .body(axum::body::Body::from("{"))
#         .unwrap();
# 
#     let err = match Json::<serde_json::Value>::from_request(req, &()).await.unwrap_err() {
#         JsonRejection::JsonSyntaxError(err) => err,
#         _ => panic!(),
#     };
# 
#     let (_, body) = serde_json_error_response(err);
#     assert_eq!(body, "Invalid JSON at line 1 column 1");
# }
```

Note that while this approach works it might break in the future if axum changes
its implementation to use a different error type internally. Such changes might
happen without major breaking versions.

# Defining custom extractors

You can also define your own extractors by implementing either
[`FromRequestParts`] or [`FromRequest`].

## Implementing `FromRequestParts`

Implement `FromRequestParts` if your extractor doesn't need access to the
request body:

```rust,no_run
use axum::{
    extract::FromRequestParts,
    routing::get,
    Router,
    http::{
        StatusCode,
        header::{HeaderValue, USER_AGENT},
        request::Parts,
    },
};

struct ExtractUserAgent(HeaderValue);

impl<S> FromRequestParts<S> for ExtractUserAgent
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        if let Some(user_agent) = parts.headers.get(USER_AGENT) {
            Ok(ExtractUserAgent(user_agent.clone()))
        } else {
            Err((StatusCode::BAD_REQUEST, "`User-Agent` header is missing"))
        }
    }
}

async fn handler(ExtractUserAgent(user_agent): ExtractUserAgent) {
    // ...
}

let app = Router::new().route("/foo", get(handler));
# let _: Router = app;
```

## Implementing `FromRequest`

If your extractor needs to consume the request body you must implement [`FromRequest`]

```rust,no_run
use axum::{
    extract::{Request, FromRequest},
    response::{Response, IntoResponse},
    body::{Bytes, Body},
    routing::get,
    Router,
    http::{
        StatusCode,
        header::{HeaderValue, USER_AGENT},
    },
};

struct ValidatedBody(Bytes);

impl<S> FromRequest<S> for ValidatedBody
where
    Bytes: FromRequest<S>,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let body = Bytes::from_request(req, state)
            .await
            .map_err(IntoResponse::into_response)?;

        // do validation...

        Ok(Self(body))
    }
}

async fn handler(ValidatedBody(body): ValidatedBody) {
    // ...
}

let app = Router::new().route("/foo", get(handler));
# let _: Router = app;
```

## Cannot implement both `FromRequest` and `FromRequestParts`

Note that you will make your extractor unusable by implementing both
`FromRequest` and `FromRequestParts` directly for the same type, unless it is
wrapping another extractor:

```rust,compile_fail
use axum::{
    Router,
    routing::get,
    extract::{FromRequest, Request, FromRequestParts},
    http::request::Parts,
    body::Body,
};
use std::convert::Infallible;

// Some extractor that doesn't wrap another extractor
struct MyExtractor;

// `MyExtractor` implements both `FromRequest`
impl<S> FromRequest<S> for MyExtractor
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        // ...
        # todo!()
    }
}

// and `FromRequestParts`
impl<S> FromRequestParts<S> for MyExtractor
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // ...
        # todo!()
    }
}

let app = Router::new().route(
    "/",
    // This fails when we go to actually use `MyExtractor` in a handler function.
    // This is due to a limit in Rust's type system.
    //
    // The workaround is to implement either `FromRequest` or `FromRequestParts`
    // but not both, if your extractor doesn't wrap another extractor.
    //
    // See "Wrapping extractors" for how to wrap other extractors.
    get(|_: MyExtractor| async {}),
);
# let _: Router = app;
```

# Accessing other extractors in `FromRequest` or `FromRequestParts` implementations

When defining custom extractors you often need to access another extractor
in your implementation.

```rust
use axum::{
    extract::{Extension, FromRequestParts},
    http::{StatusCode, HeaderMap, request::Parts},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};

#[derive(Clone)]
struct State {
    // ...
}

struct AuthenticatedUser {
    // ...
}

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // You can either call them directly...
        let headers = HeaderMap::from_request_parts(parts, state)
            .await
            .map_err(|err| match err {})?;

        // ... or use `extract` / `extract_with_state` from `RequestExt` / `RequestPartsExt`
        use axum::RequestPartsExt;
        let Extension(state) = parts.extract::<Extension<State>>()
            .await
            .map_err(|err| err.into_response())?;

        unimplemented!("actually perform the authorization")
    }
}

async fn handler(user: AuthenticatedUser) {
    // ...
}

let state = State { /* ... */ };

let app = Router::new().route("/", get(handler)).layer(Extension(state));
# let _: Router = app;
```

# Request body limits

For security reasons, [`Bytes`] will, by default, not accept bodies larger than
2MB. This also applies to extractors that uses [`Bytes`] internally such as
`String`, [`Json`], and [`Form`].

For more details, including how to disable this limit, see [`DefaultBodyLimit`].

# Wrapping extractors

If you want write an extractor that generically wraps another extractor (that
may or may not consume the request body) you should implement both
[`FromRequest`] and [`FromRequestParts`]:

```rust
use axum::{
    Router,
    body::Body,
    routing::get,
    extract::{Request, FromRequest, FromRequestParts},
    http::{HeaderMap, request::Parts},
};
use std::time::{Instant, Duration};

// an extractor that wraps another and measures how long time it takes to run
struct Timing<E> {
    extractor: E,
    duration: Duration,
}

// we must implement both `FromRequestParts`
impl<S, T> FromRequestParts<S> for Timing<T>
where
    S: Send + Sync,
    T: FromRequestParts<S>,
{
    type Rejection = T::Rejection;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let start = Instant::now();
        let extractor = T::from_request_parts(parts, state).await?;
        let duration = start.elapsed();
        Ok(Timing {
            extractor,
            duration,
        })
    }
}

// and `FromRequest`
impl<S, T> FromRequest<S> for Timing<T>
where
    S: Send + Sync,
    T: FromRequest<S>,
{
    type Rejection = T::Rejection;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let start = Instant::now();
        let extractor = T::from_request(req, state).await?;
        let duration = start.elapsed();
        Ok(Timing {
            extractor,
            duration,
        })
    }
}

async fn handler(
    // this uses the `FromRequestParts` impl
    _: Timing<HeaderMap>,
    // this uses the `FromRequest` impl
    _: Timing<String>,
) {}
# let _: axum::routing::MethodRouter = axum::routing::get(handler);
```

# Logging rejections

All built-in extractors will log rejections for easier debugging. To see the
logs, enable the `tracing` feature for axum (enabled by default) and the
`axum::rejection=trace` tracing target, for example with
`RUST_LOG=info,axum::rejection=trace cargo run`.

[axum-extra]: https://docs.rs/axum-extra/latest/axum_extra/extract/index.html
[`body::Body`]: crate::body::Body
[`Bytes`]: crate::body::Bytes
[customize-extractor-error]: https://github.com/tokio-rs/axum/blob/main/examples/customize-extractor-error/src/main.rs
[`HeaderMap`]: https://docs.rs/http/latest/http/header/struct.HeaderMap.html
[`Request`]: https://docs.rs/http/latest/http/struct.Request.html
[`RequestParts::body_mut`]: crate::extract::RequestParts::body_mut
[`JsonRejection::JsonDataError`]: rejection::JsonRejection::JsonDataError
