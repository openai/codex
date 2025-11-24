use crate::{
    body::{Body, Bytes},
    error_handling::HandleErrorLayer,
    extract::{self, DefaultBodyLimit, FromRef, Path, State},
    handler::{Handler, HandlerWithoutStateExt},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{
        delete, get, get_service, on, on_service, patch, patch_service,
        path_router::path_for_nested_route, post, MethodFilter,
    },
    test_helpers::{
        tracing_helpers::{capture_tracing, TracingEvent},
        *,
    },
    BoxError, Extension, Json, Router, ServiceExt,
};
use axum_core::extract::Request;
use counting_cloneable_state::CountingCloneableState;
use futures_util::stream::StreamExt;
use http::{
    header::{ALLOW, CONTENT_LENGTH, HOST},
    HeaderMap, Method, StatusCode, Uri,
};
use http_body_util::BodyExt;
use serde::Deserialize;
use serde_json::json;
use std::{
    convert::Infallible,
    future::{ready, IntoFuture, Ready},
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll},
    time::Duration,
};
use tower::{service_fn, util::MapResponseLayer, ServiceExt as TowerServiceExt};
use tower_http::{
    limit::RequestBodyLimitLayer, timeout::TimeoutLayer,
    validate_request::ValidateRequestHeaderLayer,
};
use tower_service::Service;

mod fallback;
mod get_to_head;
mod handle_error;
mod merge;
mod nest;

#[crate::test]
async fn hello_world() {
    async fn root(_: Request) -> &'static str {
        "Hello, World!"
    }

    async fn foo(_: Request) -> &'static str {
        "foo"
    }

    async fn users_create(_: Request) -> &'static str {
        "users#create"
    }

    let app = Router::new()
        .route("/", get(root).post(foo))
        .route("/users", post(users_create));

    let client = TestClient::new(app);

    let res = client.get("/").await;
    let body = res.text().await;
    assert_eq!(body, "Hello, World!");

    let res = client.post("/").await;
    let body = res.text().await;
    assert_eq!(body, "foo");

    let res = client.post("/users").await;
    let body = res.text().await;
    assert_eq!(body, "users#create");
}

#[crate::test]
async fn routing() {
    let app = Router::new()
        .route(
            "/users",
            get(|_: Request| async { "users#index" }).post(|_: Request| async { "users#create" }),
        )
        .route("/users/{id}", get(|_: Request| async { "users#show" }))
        .route(
            "/users/{id}/action",
            get(|_: Request| async { "users#action" }),
        );

    let client = TestClient::new(app);

    let res = client.get("/").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = client.get("/users").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "users#index");

    let res = client.post("/users").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "users#create");

    let res = client.get("/users/1").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "users#show");

    let res = client.get("/users/1/action").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "users#action");
}

#[crate::test]
async fn router_type_doesnt_change() {
    let app: Router = Router::new()
        .route(
            "/",
            on(MethodFilter::GET, |_: Request| async { "hi from GET" })
                .on(MethodFilter::POST, |_: Request| async { "hi from POST" }),
        )
        .layer(tower_http::trace::TraceLayer::new_for_http());

    let client = TestClient::new(app);

    let res = client.get("/").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "hi from GET");

    let res = client.post("/").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "hi from POST");
}

#[crate::test]
async fn routing_between_services() {
    use std::convert::Infallible;
    use tower::service_fn;

    async fn handle(_: Request) -> &'static str {
        "handler"
    }

    let app = Router::new()
        .route(
            "/one",
            get_service(service_fn(|_: Request| async {
                Ok::<_, Infallible>(Response::new(Body::from("one get")))
            }))
            .post_service(service_fn(|_: Request| async {
                Ok::<_, Infallible>(Response::new(Body::from("one post")))
            }))
            .on_service(
                MethodFilter::PUT,
                service_fn(|_: Request| async {
                    Ok::<_, Infallible>(Response::new(Body::from("one put")))
                }),
            ),
        )
        .route("/two", on_service(MethodFilter::GET, handle.into_service()));

    let client = TestClient::new(app);

    let res = client.get("/one").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "one get");

    let res = client.post("/one").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "one post");

    let res = client.put("/one").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "one put");

    let res = client.get("/two").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "handler");
}

#[crate::test]
async fn middleware_on_single_route() {
    use tower_http::trace::TraceLayer;

    async fn handle(_: Request) -> &'static str {
        "Hello, World!"
    }

    let app = Router::new().route("/", get(handle.layer(TraceLayer::new_for_http())));

    let client = TestClient::new(app);

    let res = client.get("/").await;
    let body = res.text().await;

    assert_eq!(body, "Hello, World!");
}

#[crate::test]
async fn service_in_bottom() {
    async fn handler(_req: Request) -> Result<Response<Body>, Infallible> {
        Ok(Response::new(Body::empty()))
    }

    let app = Router::new().route("/", get_service(service_fn(handler)));

    TestClient::new(app);
}

#[crate::test]
async fn wrong_method_handler() {
    let app = Router::new()
        .route("/", get(|| async {}).post(|| async {}))
        .route("/foo", patch(|| async {}));

    let client = TestClient::new(app);

    let res = client.patch("/").await;
    assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(res.headers()[ALLOW], "GET,HEAD,POST");

    let res = client.patch("/foo").await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = client.post("/foo").await;
    assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(res.headers()[ALLOW], "PATCH");

    let res = client.get("/bar").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[crate::test]
async fn wrong_method_service() {
    #[derive(Clone)]
    struct Svc;

    impl<R> Service<R> for Svc {
        type Response = Response;
        type Error = Infallible;
        type Future = Ready<Result<Self::Response, Self::Error>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _req: R) -> Self::Future {
            ready(Ok(().into_response()))
        }
    }

    let app = Router::new()
        .route("/", get_service(Svc).post_service(Svc))
        .route("/foo", patch_service(Svc));

    let client = TestClient::new(app);

    let res = client.patch("/").await;
    assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(res.headers()[ALLOW], "GET,HEAD,POST");

    let res = client.patch("/foo").await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = client.post("/foo").await;
    assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(res.headers()[ALLOW], "PATCH");

    let res = client.get("/bar").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[crate::test]
async fn multiple_methods_for_one_handler() {
    async fn root(_: Request) -> &'static str {
        "Hello, World!"
    }

    let app = Router::new().route("/", on(MethodFilter::GET.or(MethodFilter::POST), root));

    let client = TestClient::new(app);

    let res = client.get("/").await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = client.post("/").await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[crate::test]
async fn wildcard_sees_whole_url() {
    let app = Router::new().route(
        "/api/{*rest}",
        get(|uri: Uri| async move { uri.to_string() }),
    );

    let client = TestClient::new(app);

    let res = client.get("/api/foo/bar").await;
    assert_eq!(res.text().await, "/api/foo/bar");
}

#[crate::test]
async fn middleware_applies_to_routes_above() {
    let app = Router::new()
        .route("/one", get(std::future::pending::<()>))
        .layer(TimeoutLayer::new(Duration::ZERO))
        .route("/two", get(|| async {}));

    let client = TestClient::new(app);

    let res = client.get("/one").await;
    assert_eq!(res.status(), StatusCode::REQUEST_TIMEOUT);

    let res = client.get("/two").await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[crate::test]
async fn not_found_for_extra_trailing_slash() {
    let app = Router::new().route("/foo", get(|| async {}));

    let client = TestClient::new(app);

    let res = client.get("/foo/").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = client.get("/foo").await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[crate::test]
async fn not_found_for_missing_trailing_slash() {
    let app = Router::new().route("/foo/", get(|| async {}));

    let client = TestClient::new(app);

    let res = client.get("/foo").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[crate::test]
async fn with_and_without_trailing_slash() {
    let app = Router::new()
        .route("/foo", get(|| async { "without tsr" }))
        .route("/foo/", get(|| async { "with tsr" }));

    let client = TestClient::new(app);

    let res = client.get("/foo/").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "with tsr");

    let res = client.get("/foo").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "without tsr");
}

// for https://github.com/tokio-rs/axum/issues/420
#[crate::test]
async fn wildcard_doesnt_match_just_trailing_slash() {
    let app = Router::new().route(
        "/x/{*path}",
        get(|Path(path): Path<String>| async move { path }),
    );

    let client = TestClient::new(app);

    let res = client.get("/x").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = client.get("/x/").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = client.get("/x/foo/bar").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await, "foo/bar");
}

#[crate::test]
async fn what_matches_wildcard() {
    let app = Router::new()
        .route("/{*key}", get(|| async { "root" }))
        .route("/x/{*key}", get(|| async { "x" }))
        .fallback(|| async { "fallback" });

    let client = TestClient::new(app);

    let get = |path| {
        let f = client.get(path);
        async move { f.await.text().await }
    };

    assert_eq!(get("/").await, "fallback");
    assert_eq!(get("/a").await, "root");
    assert_eq!(get("/a/").await, "root");
    assert_eq!(get("/a/b").await, "root");
    assert_eq!(get("/a/b/").await, "root");

    assert_eq!(get("/x").await, "root");
    assert_eq!(get("/x/").await, "root");
    assert_eq!(get("/x/a").await, "x");
    assert_eq!(get("/x/a/").await, "x");
    assert_eq!(get("/x/a/b").await, "x");
    assert_eq!(get("/x/a/b/").await, "x");
}

#[crate::test]
async fn static_and_dynamic_paths() {
    let app = Router::new()
        .route(
            "/{key}",
            get(|Path(key): Path<String>| async move { format!("dynamic: {key}") }),
        )
        .route("/foo", get(|| async { "static" }));

    let client = TestClient::new(app);

    let res = client.get("/bar").await;
    assert_eq!(res.text().await, "dynamic: bar");

    let res = client.get("/foo").await;
    assert_eq!(res.text().await, "static");
}

#[crate::test]
#[should_panic(expected = "Paths must start with a `/`. Use \"/\" for root routes")]
async fn empty_route() {
    let app = Router::new().route("", get(|| async {}));
    TestClient::new(app);
}

#[crate::test]
async fn middleware_still_run_for_unmatched_requests() {
    #[derive(Clone)]
    struct CountMiddleware<S>(S);

    static COUNT: AtomicUsize = AtomicUsize::new(0);

    impl<R, S> Service<R> for CountMiddleware<S>
    where
        S: Service<R>,
    {
        type Response = S::Response;
        type Error = S::Error;
        type Future = S::Future;

        fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            self.0.poll_ready(cx)
        }

        fn call(&mut self, req: R) -> Self::Future {
            COUNT.fetch_add(1, Ordering::SeqCst);
            self.0.call(req)
        }
    }

    let app = Router::new()
        .route("/", get(|| async {}))
        .layer(tower::layer::layer_fn(CountMiddleware));

    let client = TestClient::new(app);

    assert_eq!(COUNT.load(Ordering::SeqCst), 0);

    client.get("/").await;
    assert_eq!(COUNT.load(Ordering::SeqCst), 1);

    client.get("/not-found").await;
    assert_eq!(COUNT.load(Ordering::SeqCst), 2);
}

#[crate::test]
#[should_panic(expected = "\
    Invalid route: `Router::route_service` cannot be used with `Router`s. \
    Use `Router::nest` instead\
")]
async fn routing_to_router_panics() {
    TestClient::new(Router::new().route_service("/", Router::new()));
}

#[crate::test]
async fn route_layer() {
    let app = Router::new()
        .route("/foo", get(|| async {}))
        .route_layer(ValidateRequestHeaderLayer::bearer("password"));

    let client = TestClient::new(app);

    let res = client
        .get("/foo")
        .header("authorization", "Bearer password")
        .await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = client.get("/foo").await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let res = client.get("/not-found").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // it would be nice if this would return `405 Method Not Allowed`
    // but that requires knowing more about which method route we're calling, which we
    // don't know currently since it's just a generic `Service`
    let res = client.post("/foo").await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[crate::test]
async fn different_methods_added_in_different_routes() {
    let app = Router::new()
        .route("/", get(|| async { "GET" }))
        .route("/", post(|| async { "POST" }));

    let client = TestClient::new(app);

    let res = client.get("/").await;
    let body = res.text().await;
    assert_eq!(body, "GET");

    let res = client.post("/").await;
    let body = res.text().await;
    assert_eq!(body, "POST");
}

#[crate::test]
#[should_panic(expected = "Cannot merge two `Router`s that both have a fallback")]
async fn merging_routers_with_fallbacks_panics() {
    async fn fallback() {}
    let one = Router::new().fallback(fallback);
    let two = Router::new().fallback(fallback);
    TestClient::new(one.merge(two));
}

#[test]
#[should_panic(expected = "Overlapping method route. Handler for `GET /foo/bar` already exists")]
fn routes_with_overlapping_method_routes() {
    async fn handler() {}
    let _: Router = Router::new()
        .route("/foo/bar", get(handler))
        .route("/foo/bar", get(handler));
}

#[test]
#[should_panic(expected = "Overlapping method route. Handler for `GET /foo/bar` already exists")]
fn merging_with_overlapping_method_routes() {
    async fn handler() {}
    let app: Router = Router::new().route("/foo/bar", get(handler));
    _ = app.clone().merge(app);
}

#[crate::test]
async fn merging_routers_with_same_paths_but_different_methods() {
    let one = Router::new().route("/", get(|| async { "GET" }));
    let two = Router::new().route("/", post(|| async { "POST" }));

    let client = TestClient::new(one.merge(two));

    let res = client.get("/").await;
    let body = res.text().await;
    assert_eq!(body, "GET");

    let res = client.post("/").await;
    let body = res.text().await;
    assert_eq!(body, "POST");
}

#[crate::test]
async fn head_content_length_through_hyper_server() {
    let app = Router::new()
        .route("/", get(|| async { "foo" }))
        .route("/json", get(|| async { Json(json!({ "foo": 1 })) }));

    let client = TestClient::new(app);

    let res = client.head("/").await;
    assert_eq!(res.headers()["content-length"], "3");
    assert!(res.text().await.is_empty());

    let res = client.head("/json").await;
    assert_eq!(res.headers()["content-length"], "9");
    assert!(res.text().await.is_empty());
}

#[crate::test]
async fn head_content_length_through_hyper_server_that_hits_fallback() {
    let app = Router::new().fallback(|| async { "foo" });

    let client = TestClient::new(app);

    let res = client.head("/").await;
    assert_eq!(res.headers()["content-length"], "3");
}

#[crate::test]
async fn head_with_middleware_applied() {
    use tower_http::compression::{predicate::SizeAbove, CompressionLayer};

    let app = Router::new()
        .nest(
            "/foo",
            Router::new().route("/", get(|| async { "Hello, World!" })),
        )
        .layer(CompressionLayer::new().compress_when(SizeAbove::new(0)));

    let client = TestClient::new(app);

    // send GET request
    let res = client.get("/foo").header("accept-encoding", "gzip").await;
    assert_eq!(res.headers()["transfer-encoding"], "chunked");
    // cannot have `transfer-encoding: chunked` and `content-length`
    assert!(!res.headers().contains_key("content-length"));

    // send HEAD request
    let res = client.head("/foo").header("accept-encoding", "gzip").await;
    // no response body so no `transfer-encoding`
    assert!(!res.headers().contains_key("transfer-encoding"));
    // no content-length since we cannot know it since the response
    // is compressed
    assert!(!res.headers().contains_key("content-length"));
}

#[crate::test]
#[should_panic(expected = "Paths must start with a `/`")]
async fn routes_must_start_with_slash() {
    let app = Router::new().route(":foo", get(|| async {}));
    TestClient::new(app);
}

#[crate::test]
async fn body_limited_by_default() {
    let app = Router::new()
        .route("/bytes", post(|_: Bytes| async {}))
        .route("/string", post(|_: String| async {}))
        .route("/json", post(|_: Json<serde_json::Value>| async {}));

    let client = TestClient::new(app);

    for uri in ["/bytes", "/string", "/json"] {
        println!("calling {uri}");

        let stream = futures_util::stream::repeat("a".repeat(1000)).map(Ok::<_, hyper::Error>);
        let body = reqwest::Body::wrap_stream(stream);

        let res_future = client
            .post(uri)
            .header("content-type", "application/json")
            .body(body)
            .into_future();
        let res = tokio::time::timeout(Duration::from_secs(3), res_future)
            .await
            .expect("never got response");

        assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }
}

#[crate::test]
async fn disabling_the_default_limit() {
    let app = Router::new()
        .route("/", post(|_: Bytes| async {}))
        .layer(DefaultBodyLimit::disable());

    let client = TestClient::new(app);

    // `DEFAULT_LIMIT` is 2mb so make a body larger than that
    let body = reqwest::Body::from("a".repeat(3_000_000));

    let res = client.post("/").body(body).await;

    assert_eq!(res.status(), StatusCode::OK);
}

#[crate::test]
async fn limited_body_with_content_length() {
    const LIMIT: usize = 3;

    let app = Router::new()
        .route(
            "/",
            post(|headers: HeaderMap, _body: Bytes| async move {
                assert!(headers.get(CONTENT_LENGTH).is_some());
            }),
        )
        .layer(RequestBodyLimitLayer::new(LIMIT));

    let client = TestClient::new(app);

    let res = client.post("/").body("a".repeat(LIMIT)).await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = client.post("/").body("a".repeat(LIMIT * 2)).await;
    assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[crate::test]
async fn changing_the_default_limit() {
    let new_limit = 2;

    let app = Router::new()
        .route("/", post(|_: Bytes| async {}))
        .layer(DefaultBodyLimit::max(new_limit));

    let client = TestClient::new(app);

    let res = client
        .post("/")
        .body(reqwest::Body::from("a".repeat(new_limit)))
        .await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = client
        .post("/")
        .body(reqwest::Body::from("a".repeat(new_limit + 1)))
        .await;
    assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[crate::test]
async fn changing_the_default_limit_differently_on_different_routes() {
    let limit1 = 2;
    let limit2 = 10;

    let app = Router::new()
        .route(
            "/limit1",
            post(|_: Bytes| async {}).layer(DefaultBodyLimit::max(limit1)),
        )
        .route(
            "/limit2",
            post(|_: Bytes| async {}).layer(DefaultBodyLimit::max(limit2)),
        )
        .route("/default", post(|_: Bytes| async {}));

    let client = TestClient::new(app);

    let res = client
        .post("/limit1")
        .body(reqwest::Body::from("a".repeat(limit1)))
        .await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = client
        .post("/limit1")
        .body(reqwest::Body::from("a".repeat(limit2)))
        .await;
    assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);

    let res = client
        .post("/limit2")
        .body(reqwest::Body::from("a".repeat(limit1)))
        .await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = client
        .post("/limit2")
        .body(reqwest::Body::from("a".repeat(limit2)))
        .await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = client
        .post("/limit2")
        .body(reqwest::Body::from("a".repeat(limit1 + limit2)))
        .await;
    assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);

    let res = client
        .post("/default")
        .body(reqwest::Body::from("a".repeat(limit1 + limit2)))
        .await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = client
        .post("/default")
        // `DEFAULT_LIMIT` is 2mb so make a body larger than that
        .body(reqwest::Body::from("a".repeat(3_000_000)))
        .await;
    assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[crate::test]
async fn limited_body_with_streaming_body() {
    const LIMIT: usize = 3;

    let app = Router::new()
        .route(
            "/",
            post(|headers: HeaderMap, _body: Bytes| async move {
                assert!(headers.get(CONTENT_LENGTH).is_none());
            }),
        )
        .layer(RequestBodyLimitLayer::new(LIMIT));

    let client = TestClient::new(app);

    let stream = futures_util::stream::iter(vec![Ok::<_, hyper::Error>("a".repeat(LIMIT))]);
    let res = client
        .post("/")
        .body(reqwest::Body::wrap_stream(stream))
        .await;
    assert_eq!(res.status(), StatusCode::OK);

    let stream = futures_util::stream::iter(vec![Ok::<_, hyper::Error>("a".repeat(LIMIT * 2))]);
    let res = client
        .post("/")
        .body(reqwest::Body::wrap_stream(stream))
        .await;
    assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[crate::test]
async fn extract_state() {
    #[derive(Clone)]
    struct AppState {
        value: i32,
        inner: InnerState,
    }

    #[derive(Clone)]
    struct InnerState {
        value: i32,
    }

    impl FromRef<AppState> for InnerState {
        fn from_ref(state: &AppState) -> Self {
            state.inner.clone()
        }
    }

    async fn handler(State(outer): State<AppState>, State(inner): State<InnerState>) {
        assert_eq!(outer.value, 1);
        assert_eq!(inner.value, 2);
    }

    let state = AppState {
        value: 1,
        inner: InnerState { value: 2 },
    };

    let app = Router::new().route("/", get(handler)).with_state(state);
    let client = TestClient::new(app);

    let res = client.get("/").await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[crate::test]
async fn explicitly_set_state() {
    let app = Router::new()
        .route_service(
            "/",
            get(|State(state): State<&'static str>| async move { state }).with_state("foo"),
        )
        .with_state("...");

    let client = TestClient::new(app);
    let res = client.get("/").await;
    assert_eq!(res.text().await, "foo");
}

#[crate::test]
async fn layer_response_into_response() {
    fn map_response<B>(_res: Response<B>) -> Result<Response<B>, impl IntoResponse> {
        let headers = [("x-foo", "bar")];
        let status = StatusCode::IM_A_TEAPOT;
        Err((headers, status))
    }

    let app = Router::new()
        .route("/", get(|| async {}))
        .layer(MapResponseLayer::new(map_response));

    let client = TestClient::new(app);

    let res = client.get("/").await;
    assert_eq!(res.headers()["x-foo"], "bar");
    assert_eq!(res.status(), StatusCode::IM_A_TEAPOT);
}

#[allow(dead_code)]
fn method_router_fallback_with_state() {
    async fn fallback(_: State<&'static str>) {}

    async fn not_found(_: State<&'static str>) {}

    let state = "foo";

    let _: Router = Router::new()
        .fallback(get(fallback).fallback(not_found))
        .with_state(state);
}

#[test]
fn test_path_for_nested_route() {
    assert_eq!(path_for_nested_route("/", "/"), "/");

    assert_eq!(path_for_nested_route("/a", "/"), "/a");
    assert_eq!(path_for_nested_route("/", "/b"), "/b");
    assert_eq!(path_for_nested_route("/a/", "/"), "/a/");
    assert_eq!(path_for_nested_route("/", "/b/"), "/b/");

    assert_eq!(path_for_nested_route("/a", "/b"), "/a/b");
    assert_eq!(path_for_nested_route("/a/", "/b"), "/a/b");
    assert_eq!(path_for_nested_route("/a", "/b/"), "/a/b/");
    assert_eq!(path_for_nested_route("/a/", "/b/"), "/a/b/");
}

#[crate::test]
async fn state_isnt_cloned_too_much() {
    let state = CountingCloneableState::new();

    let app = Router::new()
        .route("/", get(|_: State<CountingCloneableState>| async {}))
        .with_state(state.clone());

    let client = TestClient::new(app);

    // ignore clones made during setup
    state.setup_done();

    client.get("/").await;

    assert_eq!(state.count(), 3);
}

#[crate::test]
async fn state_isnt_cloned_too_much_in_layer() {
    async fn layer(State(_): State<CountingCloneableState>, req: Request, next: Next) -> Response {
        next.run(req).await
    }

    let state = CountingCloneableState::new();

    let app = Router::new().layer(middleware::from_fn_with_state(state.clone(), layer));

    let client = TestClient::new(app);

    // ignore clones made during setup
    state.setup_done();

    client.get("/").await;

    assert_eq!(state.count(), 3);
}

#[crate::test]
async fn logging_rejections() {
    #[derive(Deserialize, Eq, PartialEq, Debug)]
    #[serde(deny_unknown_fields)]
    struct RejectionEvent {
        message: String,
        status: u16,
        body: String,
        rejection_type: String,
    }

    let events = capture_tracing::<RejectionEvent, _>(|| async {
        let app = Router::new()
            .route("/extension", get(|_: Extension<Infallible>| async {}))
            .route("/string", post(|_: String| async {}));

        let client = TestClient::new(app);

        assert_eq!(
            client.get("/extension").await.status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );

        assert_eq!(
            client
                .post("/string")
                .body(Vec::from([0, 159, 146, 150]))
                .await
                .status(),
            StatusCode::BAD_REQUEST,
        );
    })
    .with_filter("axum::rejection=trace")
    .await;

    assert_eq!(
        events,
        Vec::from([
            TracingEvent {
                fields: RejectionEvent {
                    message: "rejecting request".to_owned(),
                    status: 500,
                    body: "Missing request extension: Extension of \
                        type `core::convert::Infallible` was not found. \
                        Perhaps you forgot to add it? See `axum::Extension`."
                        .to_owned(),
                    rejection_type: "axum::extract::rejection::MissingExtension".to_owned(),
                },
                target: "axum::rejection".to_owned(),
                level: "TRACE".to_owned(),
            },
            TracingEvent {
                fields: RejectionEvent {
                    message: "rejecting request".to_owned(),
                    status: 400,
                    body: "Request body didn't contain valid UTF-8: \
                        invalid utf-8 sequence of 1 bytes from index 1"
                        .to_owned(),
                    rejection_type: "axum_core::extract::rejection::InvalidUtf8".to_owned(),
                },
                target: "axum::rejection".to_owned(),
                level: "TRACE".to_owned(),
            },
        ])
    )
}

// https://github.com/tokio-rs/axum/issues/1955
#[crate::test]
async fn connect_going_to_custom_fallback() {
    let app = Router::new().fallback(|| async { (StatusCode::NOT_FOUND, "custom fallback") });

    let req = Request::builder()
        .uri("example.com:443")
        .method(Method::CONNECT)
        .header(HOST, "example.com:443")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let text = String::from_utf8(res.collect().await.unwrap().to_bytes().to_vec()).unwrap();
    assert_eq!(text, "custom fallback");
}

// https://github.com/tokio-rs/axum/issues/1955
#[crate::test]
async fn connect_going_to_default_fallback() {
    let app = Router::new();

    let req = Request::builder()
        .uri("example.com:443")
        .method(Method::CONNECT)
        .header(HOST, "example.com:443")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let body = res.collect().await.unwrap().to_bytes();
    assert!(body.is_empty());
}

#[crate::test]
async fn impl_handler_for_into_response() {
    let app = Router::new().route("/things", post((StatusCode::CREATED, "thing created")));

    let client = TestClient::new(app);

    let res = client.post("/things").await;
    assert_eq!(res.status(), StatusCode::CREATED);
    assert_eq!(res.text().await, "thing created");
}

#[crate::test]
#[should_panic(
    expected = "Path segments must not start with `:`. For capture groups, use `{capture}`. If you meant to literally match a segment starting with a colon, call `without_v07_checks` on the router."
)]
async fn colon_in_route() {
    _ = Router::<()>::new().route("/:foo", get(|| async move {}));
}

#[crate::test]
#[should_panic(
    expected = "Path segments must not start with `*`. For wildcard capture, use `{*wildcard}`. If you meant to literally match a segment starting with an asterisk, call `without_v07_checks` on the router."
)]
async fn asterisk_in_route() {
    _ = Router::<()>::new().route("/*foo", get(|| async move {}));
}

#[crate::test]
async fn middleware_adding_body() {
    let app = Router::new()
        .route("/", get(()))
        .layer(MapResponseLayer::new(|mut res: Response| -> Response {
            // If there is a content-length header, its value will be zero and axum will avoid
            // overwriting it. But this means our content-length doesn’t match the length of the
            // body, which leads to panics in Hyper. Thus we have to ensure that axum doesn’t add
            // on content-length headers until after middleware has been run.
            assert!(!res.headers().contains_key("content-length"));
            *res.body_mut() = "…".into();
            res
        }));

    let client = TestClient::new(app);
    let res = client.get("/").await;

    let headers = res.headers();
    let header = headers.get("content-length");
    assert!(header.is_some());
    assert_eq!(header.unwrap().to_str().unwrap(), "3");

    assert_eq!(res.text().await, "…");
}
