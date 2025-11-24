use super::*;
use std::future::pending;
use tower::timeout::TimeoutLayer;

async fn unit() {}

async fn forever() {
    pending().await
}

fn timeout() -> TimeoutLayer {
    TimeoutLayer::new(Duration::from_millis(10))
}

#[crate::test]
async fn handler() {
    let app = Router::new().route(
        "/",
        get(forever.layer((
            HandleErrorLayer::new(|_: BoxError| async { StatusCode::REQUEST_TIMEOUT }),
            timeout(),
        ))),
    );

    let client = TestClient::new(app);

    let res = client.get("/").await;
    assert_eq!(res.status(), StatusCode::REQUEST_TIMEOUT);
}

#[crate::test]
async fn handler_multiple_methods_first() {
    let app = Router::new().route(
        "/",
        get(forever.layer((
            HandleErrorLayer::new(|_: BoxError| async { StatusCode::REQUEST_TIMEOUT }),
            timeout(),
        )))
        .post(unit),
    );

    let client = TestClient::new(app);

    let res = client.get("/").await;
    assert_eq!(res.status(), StatusCode::REQUEST_TIMEOUT);
}

#[crate::test]
async fn handler_multiple_methods_middle() {
    let app = Router::new().route(
        "/",
        delete(unit)
            .get(forever.layer((
                HandleErrorLayer::new(|_: BoxError| async { StatusCode::REQUEST_TIMEOUT }),
                timeout(),
            )))
            .post(unit),
    );

    let client = TestClient::new(app);

    let res = client.get("/").await;
    assert_eq!(res.status(), StatusCode::REQUEST_TIMEOUT);
}

#[crate::test]
async fn handler_multiple_methods_last() {
    let app = Router::new().route(
        "/",
        delete(unit).get(forever.layer((
            HandleErrorLayer::new(|_: BoxError| async { StatusCode::REQUEST_TIMEOUT }),
            timeout(),
        ))),
    );

    let client = TestClient::new(app);

    let res = client.get("/").await;
    assert_eq!(res.status(), StatusCode::REQUEST_TIMEOUT);
}

#[crate::test]
async fn handler_service_ext() {
    let fallible_service = tower::service_fn(|_| async { Err::<(), ()>(()) });
    let handle_error_service =
        fallible_service.handle_error(|_| async { StatusCode::INTERNAL_SERVER_ERROR });

    let app = Router::new().route("/", get_service(handle_error_service));

    let client = TestClient::new(app);

    let res = client.get("/").await;
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
}
