use super::*;
use crate::ClientRouteClass;
use crate::HttpClientBuilder;
use crate::HttpClientFactory;
use crate::OutboundProxyPolicy;
use pretty_assertions::assert_eq;

#[test]
fn request_draft_preserves_reqwest_url_auth_and_header_precedence() {
    let pool = RouteAwareClientPool::new(
        HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy),
        ClientRouteClass::Api,
    );
    let client = HttpClientBuilder::new()
        .build_direct()
        .expect("client should build");
    let HttpClientBackend::Direct(transport) = &client.backend else {
        panic!("client should use direct transport");
    };
    let reqwest = &transport.inner;
    for url in [
        "https://example.com/path",
        "https://user:p%40ss@example.com/path",
        "https://:%E2%98%83@example.com/path",
        "https://%FF:password@example.com/path",
    ] {
        let headers = HeaderMap::from_iter([
            (
                http::header::AUTHORIZATION,
                HeaderValue::from_static("Bearer replacement"),
            ),
            (
                CONTENT_TYPE,
                HeaderValue::from_static("application/custom+json"),
            ),
        ]);
        for (actual, expected) in [
            (pool.post(url), reqwest.post(url)),
            (
                pool.post(url)
                    .header(http::header::AUTHORIZATION, "Bearer appended"),
                reqwest
                    .post(url)
                    .header(http::header::AUTHORIZATION, "Bearer appended"),
            ),
            (
                pool.post(url)
                    .header(http::header::AUTHORIZATION, "Bearer first")
                    .headers(headers.clone())
                    .header(http::header::AUTHORIZATION, "Bearer last"),
                reqwest
                    .post(url)
                    .header(http::header::AUTHORIZATION, "Bearer first")
                    .headers(headers)
                    .header(http::header::AUTHORIZATION, "Bearer last"),
            ),
            (
                pool.post(url)
                    .header(CONTENT_TYPE, "application/custom+json")
                    .headers(HeaderMap::new()),
                reqwest
                    .post(url)
                    .header(CONTENT_TYPE, "application/custom+json")
                    .headers(HeaderMap::new()),
            ),
            (
                pool.post(url).json(&serde_json::json!({"previous": true})),
                reqwest
                    .post(url)
                    .json(&serde_json::json!({"previous": true})),
            ),
        ] {
            let body = serde_json::json!({"query": "test"});
            let timeout = Duration::from_secs(7);
            let actual = actual
                .json(&body)
                .timeout(timeout)
                .request
                .expect("request inputs should be valid")
                .build(transport)
                .expect("request should build");
            let expected = expected
                .json(&body)
                .timeout(timeout)
                .build()
                .expect("reqwest should build");
            assert_eq!(
                (
                    actual.url(),
                    actual.method(),
                    actual.headers(),
                    actual.body().and_then(reqwest::Body::as_bytes),
                    actual.timeout()
                ),
                (
                    expected.url(),
                    expected.method(),
                    expected.headers(),
                    expected.body().and_then(reqwest::Body::as_bytes),
                    expected.timeout()
                ),
            );
            assert_eq!(
                actual
                    .headers()
                    .values()
                    .map(HeaderValue::is_sensitive)
                    .collect::<Vec<_>>(),
                expected
                    .headers()
                    .values()
                    .map(HeaderValue::is_sensitive)
                    .collect::<Vec<_>>(),
            );
        }
    }
}
