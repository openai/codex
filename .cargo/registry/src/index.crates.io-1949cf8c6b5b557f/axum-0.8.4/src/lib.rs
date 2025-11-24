//! axum is a web application framework that focuses on ergonomics and modularity.
//!
//! # High-level features
//!
//! - Route requests to handlers with a macro-free API.
//! - Declaratively parse requests using extractors.
//! - Simple and predictable error handling model.
//! - Generate responses with minimal boilerplate.
//! - Take full advantage of the [`tower`] and [`tower-http`] ecosystem of
//!   middleware, services, and utilities.
//!
//! In particular, the last point is what sets `axum` apart from other frameworks.
//! `axum` doesn't have its own middleware system but instead uses
//! [`tower::Service`]. This means `axum` gets timeouts, tracing, compression,
//! authorization, and more, for free. It also enables you to share middleware with
//! applications written using [`hyper`] or [`tonic`].
//!
//! # Compatibility
//!
//! axum is designed to work with [tokio] and [hyper]. Runtime and
//! transport layer independence is not a goal, at least for the time being.
//!
//! # Example
//!
//! The "Hello, World!" of axum is:
//!
//! ```rust,no_run
//! use axum::{
//!     routing::get,
//!     Router,
//! };
//!
//! #[tokio::main]
//! async fn main() {
//!     // build our application with a single route
//!     let app = Router::new().route("/", get(|| async { "Hello, World!" }));
//!
//!     // run our app with hyper, listening globally on port 3000
//!     let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
//!     axum::serve(listener, app).await.unwrap();
//! }
//! ```
//!
//! Note using `#[tokio::main]` requires you enable tokio's `macros` and `rt-multi-thread` features
//! or just `full` to enable all features (`cargo add tokio --features macros,rt-multi-thread`).
//!
//! # Routing
//!
//! [`Router`] is used to set up which paths go to which services:
//!
//! ```rust
//! use axum::{Router, routing::get};
//!
//! // our router
//! let app = Router::new()
//!     .route("/", get(root))
//!     .route("/foo", get(get_foo).post(post_foo))
//!     .route("/foo/bar", get(foo_bar));
//!
//! // which calls one of these handlers
//! async fn root() {}
//! async fn get_foo() {}
//! async fn post_foo() {}
//! async fn foo_bar() {}
//! # let _: Router = app;
//! ```
//!
//! See [`Router`] for more details on routing.
//!
//! # Handlers
//!
#![doc = include_str!("docs/handlers_intro.md")]
//!
//! See [`handler`](crate::handler) for more details on handlers.
//!
//! # Extractors
//!
//! An extractor is a type that implements [`FromRequest`] or [`FromRequestParts`]. Extractors are
//! how you pick apart the incoming request to get the parts your handler needs.
//!
//! ```rust
//! use axum::extract::{Path, Query, Json};
//! use std::collections::HashMap;
//!
//! // `Path` gives you the path parameters and deserializes them.
//! async fn path(Path(user_id): Path<u32>) {}
//!
//! // `Query` gives you the query parameters and deserializes them.
//! async fn query(Query(params): Query<HashMap<String, String>>) {}
//!
//! // Buffer the request body and deserialize it as JSON into a
//! // `serde_json::Value`. `Json` supports any type that implements
//! // `serde::Deserialize`.
//! async fn json(Json(payload): Json<serde_json::Value>) {}
//! ```
//!
//! See [`extract`](crate::extract) for more details on extractors.
//!
//! # Responses
//!
//! Anything that implements [`IntoResponse`] can be returned from handlers.
//!
//! ```rust,no_run
//! use axum::{
//!     body::Body,
//!     routing::get,
//!     response::Json,
//!     Router,
//! };
//! use serde_json::{Value, json};
//!
//! // `&'static str` becomes a `200 OK` with `content-type: text/plain; charset=utf-8`
//! async fn plain_text() -> &'static str {
//!     "foo"
//! }
//!
//! // `Json` gives a content-type of `application/json` and works with any type
//! // that implements `serde::Serialize`
//! async fn json() -> Json<Value> {
//!     Json(json!({ "data": 42 }))
//! }
//!
//! let app = Router::new()
//!     .route("/plain_text", get(plain_text))
//!     .route("/json", get(json));
//! # let _: Router = app;
//! ```
//!
//! See [`response`](crate::response) for more details on building responses.
//!
//! # Error handling
//!
//! axum aims to have a simple and predictable error handling model. That means
//! it is simple to convert errors into responses and you are guaranteed that
//! all errors are handled.
//!
//! See [`error_handling`](crate::error_handling) for more details on axum's
//! error handling model and how to handle errors gracefully.
//!
//! # Middleware
//!
//! There are several different ways to write middleware for axum. See
//! [`middleware`](crate::middleware) for more details.
//!
//! # Sharing state with handlers
//!
//! It is common to share some state between handlers. For example, a
//! pool of database connections or clients to other services may need to
//! be shared.
//!
//! The four most common ways of doing that are:
//!
//! - Using the [`State`] extractor
//! - Using request extensions
//! - Using closure captures
//! - Using task-local variables
//!
//! ## Using the [`State`] extractor
//!
//! ```rust,no_run
//! use axum::{
//!     extract::State,
//!     routing::get,
//!     Router,
//! };
//! use std::sync::Arc;
//!
//! struct AppState {
//!     // ...
//! }
//!
//! let shared_state = Arc::new(AppState { /* ... */ });
//!
//! let app = Router::new()
//!     .route("/", get(handler))
//!     .with_state(shared_state);
//!
//! async fn handler(
//!     State(state): State<Arc<AppState>>,
//! ) {
//!     // ...
//! }
//! # let _: Router = app;
//! ```
//!
//! You should prefer using [`State`] if possible since it's more type safe. The downside is that
//! it's less dynamic than task-local variables and request extensions.
//!
//! See [`State`] for more details about accessing state.
//!
//! ## Using request extensions
//!
//! Another way to share state with handlers is using [`Extension`](crate::extract::Extension) as
//! layer and extractor:
//!
//! ```rust,no_run
//! use axum::{
//!     extract::Extension,
//!     routing::get,
//!     Router,
//! };
//! use std::sync::Arc;
//!
//! struct AppState {
//!     // ...
//! }
//!
//! let shared_state = Arc::new(AppState { /* ... */ });
//!
//! let app = Router::new()
//!     .route("/", get(handler))
//!     .layer(Extension(shared_state));
//!
//! async fn handler(
//!     Extension(state): Extension<Arc<AppState>>,
//! ) {
//!     // ...
//! }
//! # let _: Router = app;
//! ```
//!
//! The downside to this approach is that you'll get runtime errors
//! (specifically a `500 Internal Server Error` response) if you try and extract
//! an extension that doesn't exist, perhaps because you forgot to add the
//! middleware or because you're extracting the wrong type.
//!
//! ## Using closure captures
//!
//! State can also be passed directly to handlers using closure captures:
//!
//! ```rust,no_run
//! use axum::{
//!     Json,
//!     extract::{Extension, Path},
//!     routing::{get, post},
//!     Router,
//! };
//! use std::sync::Arc;
//! use serde::Deserialize;
//!
//! struct AppState {
//!     // ...
//! }
//!
//! let shared_state = Arc::new(AppState { /* ... */ });
//!
//! let app = Router::new()
//!     .route(
//!         "/users",
//!         post({
//!             let shared_state = Arc::clone(&shared_state);
//!             move |body| create_user(body, shared_state)
//!         }),
//!     )
//!     .route(
//!         "/users/{id}",
//!         get({
//!             let shared_state = Arc::clone(&shared_state);
//!             move |path| get_user(path, shared_state)
//!         }),
//!     );
//!
//! async fn get_user(Path(user_id): Path<String>, state: Arc<AppState>) {
//!     // ...
//! }
//!
//! async fn create_user(Json(payload): Json<CreateUserPayload>, state: Arc<AppState>) {
//!     // ...
//! }
//!
//! #[derive(Deserialize)]
//! struct CreateUserPayload {
//!     // ...
//! }
//! # let _: Router = app;
//! ```
//!
//! The downside to this approach is that it's a the most verbose approach.
//!
//! ## Using task-local variables
//!
//! This also allows to share state with `IntoResponse` implementations:
//!
//! ```rust,no_run
//! use axum::{
//!     extract::Request,
//!     http::{header, StatusCode},
//!     middleware::{self, Next},
//!     response::{IntoResponse, Response},
//!     routing::get,
//!     Router,
//! };
//! use tokio::task_local;
//!
//! #[derive(Clone)]
//! struct CurrentUser {
//!     name: String,
//! }
//! task_local! {
//!     pub static USER: CurrentUser;
//! }
//!
//! async fn auth(req: Request, next: Next) -> Result<Response, StatusCode> {
//!     let auth_header = req
//!         .headers()
//!         .get(header::AUTHORIZATION)
//!         .and_then(|header| header.to_str().ok())
//!         .ok_or(StatusCode::UNAUTHORIZED)?;
//!     if let Some(current_user) = authorize_current_user(auth_header).await {
//!         // State is setup here in the middleware
//!         Ok(USER.scope(current_user, next.run(req)).await)
//!     } else {
//!         Err(StatusCode::UNAUTHORIZED)
//!     }
//! }
//! async fn authorize_current_user(auth_token: &str) -> Option<CurrentUser> {
//!     Some(CurrentUser {
//!         name: auth_token.to_string(),
//!     })
//! }
//!
//! struct UserResponse;
//!
//! impl IntoResponse for UserResponse {
//!     fn into_response(self) -> Response {
//!         // State is accessed here in the IntoResponse implementation
//!         let current_user = USER.with(|u| u.clone());
//!         (StatusCode::OK, current_user.name).into_response()
//!     }
//! }
//!
//! async fn handler() -> UserResponse {
//!     UserResponse
//! }
//!
//! let app: Router = Router::new()
//!     .route("/", get(handler))
//!     .route_layer(middleware::from_fn(auth));
//! ```
//!
//! The main downside to this approach is that it only works when the async executor being used
//! has the concept of task-local variables. The example above uses
//! [tokio's `task_local` macro](https://docs.rs/tokio/1/tokio/macro.task_local.html).
//! smol does not yet offer equivalent functionality at the time of writing (see
//! [this GitHub issue](https://github.com/smol-rs/async-executor/issues/139)).
//!
//! # Building integrations for axum
//!
//! Libraries authors that want to provide [`FromRequest`], [`FromRequestParts`], or
//! [`IntoResponse`] implementations should depend on the [`axum-core`] crate, instead of `axum` if
//! possible. [`axum-core`] contains core types and traits and is less likely to receive breaking
//! changes.
//!
//! # Required dependencies
//!
//! To use axum there are a few dependencies you have to pull in as well:
//!
//! ```toml
//! [dependencies]
//! axum = "<latest-version>"
//! tokio = { version = "<latest-version>", features = ["full"] }
//! tower = "<latest-version>"
//! ```
//!
//! The `"full"` feature for tokio isn't necessary but it's the easiest way to get started.
//!
//! Tower isn't strictly necessary either but helpful for testing. See the
//! testing example in the repo to learn more about testing axum apps.
//!
//! # Examples
//!
//! The axum repo contains [a number of examples][examples] that show how to put all the
//! pieces together.
//!
//! # Feature flags
//!
//! axum uses a set of [feature flags] to reduce the amount of compiled and
//! optional dependencies.
//!
//! The following optional features are available:
//!
//! Name | Description | Default?
//! ---|---|---
//! `http1` | Enables hyper's `http1` feature | Yes
//! `http2` | Enables hyper's `http2` feature | No
//! `json` | Enables the [`Json`] type and some similar convenience functionality | Yes
//! `macros` | Enables optional utility macros | No
//! `matched-path` | Enables capturing of every request's router path and the [`MatchedPath`] extractor | Yes
//! `multipart` | Enables parsing `multipart/form-data` requests with [`Multipart`] | No
//! `original-uri` | Enables capturing of every request's original URI and the [`OriginalUri`] extractor | Yes
//! `tokio` | Enables `tokio` as a dependency and `axum::serve`, `SSE` and `extract::connect_info` types. | Yes
//! `tower-log` | Enables `tower`'s `log` feature | Yes
//! `tracing` | Log rejections from built-in extractors | Yes
//! `ws` | Enables WebSockets support via [`extract::ws`] | No
//! `form` | Enables the `Form` extractor | Yes
//! `query` | Enables the `Query` extractor | Yes
//!
//! [`MatchedPath`]: crate::extract::MatchedPath
//! [`Multipart`]: crate::extract::Multipart
//! [`OriginalUri`]: crate::extract::OriginalUri
//! [`tower`]: https://crates.io/crates/tower
//! [`tower-http`]: https://crates.io/crates/tower-http
//! [`tokio`]: http://crates.io/crates/tokio
//! [`hyper`]: http://crates.io/crates/hyper
//! [`tonic`]: http://crates.io/crates/tonic
//! [feature flags]: https://doc.rust-lang.org/cargo/reference/features.html#the-features-section
//! [`IntoResponse`]: crate::response::IntoResponse
//! [`Timeout`]: tower::timeout::Timeout
//! [examples]: https://github.com/tokio-rs/axum/tree/main/examples
//! [`Router::merge`]: crate::routing::Router::merge
//! [`Service`]: tower::Service
//! [`Service::poll_ready`]: tower::Service::poll_ready
//! [`Service`'s]: tower::Service
//! [`tower::Service`]: tower::Service
//! [tower-guides]: https://github.com/tower-rs/tower/tree/master/guides
//! [`Uuid`]: https://docs.rs/uuid/latest/uuid/
//! [`FromRequest`]: crate::extract::FromRequest
//! [`FromRequestParts`]: crate::extract::FromRequestParts
//! [`HeaderMap`]: http::header::HeaderMap
//! [`Request`]: http::Request
//! [customize-extractor-error]: https://github.com/tokio-rs/axum/blob/main/examples/customize-extractor-error/src/main.rs
//! [axum-macros]: https://docs.rs/axum-macros
//! [`debug_handler`]: https://docs.rs/axum-macros/latest/axum_macros/attr.debug_handler.html
//! [`Handler`]: crate::handler::Handler
//! [`Infallible`]: std::convert::Infallible
//! [load shed]: tower::load_shed
//! [`axum-core`]: http://crates.io/crates/axum-core
//! [`State`]: crate::extract::State

#![cfg_attr(docsrs, feature(doc_auto_cfg, doc_cfg))]
#![cfg_attr(test, allow(clippy::float_cmp))]
#![cfg_attr(not(test), warn(clippy::print_stdout, clippy::dbg_macro))]

#[macro_use]
pub(crate) mod macros;

mod boxed;
mod extension;
#[cfg(feature = "form")]
mod form;
#[cfg(feature = "json")]
mod json;
mod service_ext;
mod util;

pub mod body;
pub mod error_handling;
pub mod extract;
pub mod handler;
pub mod middleware;
pub mod response;
pub mod routing;
#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
pub mod serve;

#[cfg(any(test, feature = "__private"))]
#[allow(missing_docs, missing_debug_implementations, clippy::print_stdout)]
pub mod test_helpers;

#[doc(no_inline)]
pub use http;

#[doc(inline)]
pub use self::extension::Extension;
#[doc(inline)]
#[cfg(feature = "json")]
pub use self::json::Json;
#[doc(inline)]
pub use self::routing::Router;

#[doc(inline)]
#[cfg(feature = "form")]
pub use self::form::Form;

#[doc(inline)]
pub use axum_core::{BoxError, Error, RequestExt, RequestPartsExt};

#[cfg(feature = "macros")]
pub use axum_macros::{debug_handler, debug_middleware};

#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
#[doc(inline)]
pub use self::serve::serve;

pub use self::service_ext::ServiceExt;

#[cfg(test)]
use axum_macros::__private_axum_test as test;
