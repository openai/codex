use super::*;
use http::Method;
use tower::ServiceExt;

mod for_handlers {
    use super::*;

    #[crate::test]
    async fn get_handles_head() {
        let app = Router::new().route(
            "/",
            get(|| async {
                let mut headers = HeaderMap::new();
                headers.insert("x-some-header", "foobar".parse().unwrap());
                (headers, "you shouldn't see this")
            }),
        );

        // don't use reqwest because it always strips bodies from HEAD responses
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .method(Method::HEAD)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()["x-some-header"], "foobar");

        let body = BodyExt::collect(res.into_body()).await.unwrap().to_bytes();
        assert_eq!(body.len(), 0);
    }
}

mod for_services {
    use super::*;

    #[crate::test]
    async fn get_handles_head() {
        let app = Router::new().route(
            "/",
            get_service(service_fn(|_req: Request| async move {
                Ok::<_, Infallible>(
                    ([("x-some-header", "foobar")], "you shouldn't see this").into_response(),
                )
            })),
        );

        // don't use reqwest because it always strips bodies from HEAD responses
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .method(Method::HEAD)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()["x-some-header"], "foobar");

        let body = BodyExt::collect(res.into_body()).await.unwrap().to_bytes();
        assert_eq!(body.len(), 0);
    }
}
