use super::*;
use crate::extract::OriginalUri;
use serde_json::{json, Value};
use tower::limit::ConcurrencyLimitLayer;

#[crate::test]
async fn basic() {
    let one = Router::new()
        .route("/foo", get(|| async {}))
        .route("/bar", get(|| async {}));
    let two = Router::new().route("/baz", get(|| async {}));
    let app = one.merge(two);

    let client = TestClient::new(app);

    let res = client.get("/foo").await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = client.get("/bar").await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = client.get("/baz").await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = client.get("/qux").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[crate::test]
async fn multiple_ors_balanced_differently() {
    let one = Router::new().route("/one", get(|| async { "one" }));
    let two = Router::new().route("/two", get(|| async { "two" }));
    let three = Router::new().route("/three", get(|| async { "three" }));
    let four = Router::new().route("/four", get(|| async { "four" }));

    test(
        "one",
        one.clone()
            .merge(two.clone())
            .merge(three.clone())
            .merge(four.clone()),
    )
    .await;

    test(
        "two",
        one.clone()
            .merge(two.clone())
            .merge(three.clone().merge(four.clone())),
    )
    .await;

    test(
        "three",
        one.clone()
            .merge(two.clone().merge(three.clone()).merge(four.clone())),
    )
    .await;

    test("four", one.merge(two.merge(three.merge(four)))).await;

    async fn test(name: &str, app: Router) {
        let client = TestClient::new(app);

        for n in ["one", "two", "three", "four"].iter() {
            println!("running: {name} / {n}");
            let res = client.get(&format!("/{n}")).await;
            assert_eq!(res.status(), StatusCode::OK);
            assert_eq!(res.text().await, *n);
        }
    }
}

#[crate::test]
async fn nested_or() {
    let bar = Router::new().route("/bar", get(|| async { "bar" }));
    let baz = Router::new().route("/baz", get(|| async { "baz" }));

    let bar_or_baz = bar.merge(baz);

    let client = TestClient::new(bar_or_baz.clone());
    assert_eq!(client.get("/bar").await.text().await, "bar");
    assert_eq!(client.get("/baz").await.text().await, "baz");

    let client = TestClient::new(Router::new().nest("/foo", bar_or_baz));
    assert_eq!(client.get("/foo/bar").await.text().await, "bar");
    assert_eq!(client.get("/foo/baz").await.text().await, "baz");
}

#[crate::test]
async fn or_with_route_following() {
    let one = Router::new().route("/one", get(|| async { "one" }));
    let two = Router::new().route("/two", get(|| async { "two" }));
    let app = one.merge(two).route("/three", get(|| async { "three" }));

    let client = TestClient::new(app);

    let res = client.get("/one").await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = client.get("/two").await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = client.get("/three").await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[crate::test]
async fn layer() {
    let one = Router::new().route("/foo", get(|| async {}));
    let two = Router::new()
        .route("/bar", get(|| async {}))
        .layer(ConcurrencyLimitLayer::new(10));
    let app = one.merge(two);

    let client = TestClient::new(app);

    let res = client.get("/foo").await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = client.get("/bar").await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[crate::test]
async fn layer_and_handle_error() {
    let one = Router::new().route("/foo", get(|| async {}));
    let two = Router::new()
        .route("/timeout", get(std::future::pending::<()>))
        .layer(TimeoutLayer::new(Duration::from_millis(10)));
    let app = one.merge(two);

    let client = TestClient::new(app);

    let res = client.get("/timeout").await;
    assert_eq!(res.status(), StatusCode::REQUEST_TIMEOUT);
    let res = client.get("/foo").await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[crate::test]
async fn nesting() {
    let one = Router::new().route("/foo", get(|| async {}));
    let two = Router::new().nest("/bar", Router::new().route("/baz", get(|| async {})));
    let app = one.merge(two);

    let client = TestClient::new(app);

    let res = client.get("/bar/baz").await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[crate::test]
async fn boxed() {
    let one = Router::new().route("/foo", get(|| async {}));
    let two = Router::new().route("/bar", get(|| async {}));
    let app = one.merge(two);

    let client = TestClient::new(app);

    let res = client.get("/bar").await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[crate::test]
async fn many_ors() {
    let app = Router::new()
        .route("/r1", get(|| async {}))
        .merge(Router::new().route("/r2", get(|| async {})))
        .merge(Router::new().route("/r3", get(|| async {})))
        .merge(Router::new().route("/r4", get(|| async {})))
        .merge(Router::new().route("/r5", get(|| async {})))
        .merge(Router::new().route("/r6", get(|| async {})))
        .merge(Router::new().route("/r7", get(|| async {})));

    let client = TestClient::new(app);

    for n in 1..=7 {
        let res = client.get(&format!("/r{n}")).await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    let res = client.get("/r8").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[crate::test]
async fn services() {
    use crate::routing::get_service;

    let app = Router::new()
        .route(
            "/foo",
            get_service(service_fn(|_: Request| async {
                Ok::<_, Infallible>(Response::new(Body::empty()))
            })),
        )
        .merge(Router::new().route(
            "/bar",
            get_service(service_fn(|_: Request| async {
                Ok::<_, Infallible>(Response::new(Body::empty()))
            })),
        ));

    let client = TestClient::new(app);

    let res = client.get("/foo").await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = client.get("/bar").await;
    assert_eq!(res.status(), StatusCode::OK);
}

async fn all_the_uris(
    uri: Uri,
    OriginalUri(original_uri): OriginalUri,
    req: Request,
) -> impl IntoResponse {
    Json(json!({
        "uri": uri.to_string(),
        "request_uri": req.uri().to_string(),
        "original_uri": original_uri.to_string(),
    }))
}

#[crate::test]
async fn nesting_and_seeing_the_right_uri() {
    let one = Router::new().nest("/foo/", Router::new().route("/bar", get(all_the_uris)));
    let two = Router::new().route("/foo", get(all_the_uris));

    let client = TestClient::new(one.merge(two));

    let res = client.get("/foo/bar").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.json::<Value>().await,
        json!({
            "uri": "/bar",
            "request_uri": "/bar",
            "original_uri": "/foo/bar",
        })
    );

    let res = client.get("/foo").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.json::<Value>().await,
        json!({
            "uri": "/foo",
            "request_uri": "/foo",
            "original_uri": "/foo",
        })
    );
}

#[crate::test]
async fn nesting_and_seeing_the_right_uri_at_more_levels_of_nesting() {
    let one = Router::new().nest(
        "/foo/",
        Router::new().nest("/bar", Router::new().route("/baz", get(all_the_uris))),
    );
    let two = Router::new().route("/foo", get(all_the_uris));

    let client = TestClient::new(one.merge(two));

    let res = client.get("/foo/bar/baz").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.json::<Value>().await,
        json!({
            "uri": "/baz",
            "request_uri": "/baz",
            "original_uri": "/foo/bar/baz",
        })
    );

    let res = client.get("/foo").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.json::<Value>().await,
        json!({
            "uri": "/foo",
            "request_uri": "/foo",
            "original_uri": "/foo",
        })
    );
}

#[crate::test]
async fn nesting_and_seeing_the_right_uri_ors_with_nesting() {
    let one = Router::new().nest(
        "/one",
        Router::new().nest("/bar", Router::new().route("/baz", get(all_the_uris))),
    );
    let two = Router::new().nest("/two", Router::new().route("/qux", get(all_the_uris)));
    let three = Router::new().route("/three", get(all_the_uris));

    let client = TestClient::new(one.merge(two).merge(three));

    let res = client.get("/one/bar/baz").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.json::<Value>().await,
        json!({
            "uri": "/baz",
            "request_uri": "/baz",
            "original_uri": "/one/bar/baz",
        })
    );

    let res = client.get("/two/qux").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.json::<Value>().await,
        json!({
            "uri": "/qux",
            "request_uri": "/qux",
            "original_uri": "/two/qux",
        })
    );

    let res = client.get("/three").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.json::<Value>().await,
        json!({
            "uri": "/three",
            "request_uri": "/three",
            "original_uri": "/three",
        })
    );
}

#[crate::test]
async fn nesting_and_seeing_the_right_uri_ors_with_multi_segment_uris() {
    let one = Router::new().nest(
        "/one",
        Router::new().nest("/foo", Router::new().route("/bar", get(all_the_uris))),
    );
    let two = Router::new().route("/two/foo", get(all_the_uris));

    let client = TestClient::new(one.merge(two));

    let res = client.get("/one/foo/bar").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.json::<Value>().await,
        json!({
            "uri": "/bar",
            "request_uri": "/bar",
            "original_uri": "/one/foo/bar",
        })
    );

    let res = client.get("/two/foo").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.json::<Value>().await,
        json!({
            "uri": "/two/foo",
            "request_uri": "/two/foo",
            "original_uri": "/two/foo",
        })
    );
}

#[crate::test]
async fn middleware_that_return_early() {
    let private = Router::new()
        .route("/", get(|| async {}))
        .layer(ValidateRequestHeaderLayer::bearer("password"));

    let public = Router::new().route("/public", get(|| async {}));

    let client = TestClient::new(private.merge(public));

    assert_eq!(client.get("/").await.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        client
            .get("/")
            .header("authorization", "Bearer password")
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        client.get("/doesnt-exist").await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(client.get("/public").await.status(), StatusCode::OK);
}
