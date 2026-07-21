use http::HeaderValue;
use http::header::CONTENT_ENCODING;
use http::header::CONTENT_LENGTH;
use http::header::CONTENT_TYPE;
use http::header::COOKIE;
use http::header::REFERER;
use http::header::TRANSFER_ENCODING;
use pretty_assertions::assert_eq;

use super::*;

#[derive(Clone, Copy, Debug)]
enum RedirectBodyBehavior {
    Drop,
    Preserve,
}

#[test]
fn redirects_match_reqwest_method_and_body_rules() {
    let original_url =
        reqwest::Url::parse("https://example.com/original").expect("original URL should parse");
    let redirected_url =
        reqwest::Url::parse("https://example.com/next").expect("redirect URL should parse");

    for (status, method, expected_method, body_behavior) in [
        (
            StatusCode::MOVED_PERMANENTLY,
            Method::POST,
            Method::GET,
            RedirectBodyBehavior::Drop,
        ),
        (
            StatusCode::FOUND,
            Method::POST,
            Method::GET,
            RedirectBodyBehavior::Drop,
        ),
        (
            StatusCode::SEE_OTHER,
            Method::POST,
            Method::GET,
            RedirectBodyBehavior::Drop,
        ),
        (
            StatusCode::SEE_OTHER,
            Method::HEAD,
            Method::HEAD,
            RedirectBodyBehavior::Drop,
        ),
        (
            StatusCode::MOVED_PERMANENTLY,
            Method::PUT,
            Method::PUT,
            RedirectBodyBehavior::Preserve,
        ),
        (
            StatusCode::FOUND,
            Method::PUT,
            Method::PUT,
            RedirectBodyBehavior::Preserve,
        ),
        (
            StatusCode::TEMPORARY_REDIRECT,
            Method::POST,
            Method::POST,
            RedirectBodyBehavior::Preserve,
        ),
        (
            StatusCode::PERMANENT_REDIRECT,
            Method::POST,
            Method::POST,
            RedirectBodyBehavior::Preserve,
        ),
    ] {
        let mut original = reqwest::Request::new(method.clone(), original_url.clone());
        *original.headers_mut() = HeaderMap::from_iter([
            (CONTENT_TYPE, HeaderValue::from_static("application/json")),
            (CONTENT_LENGTH, HeaderValue::from_static("2")),
            (CONTENT_ENCODING, HeaderValue::from_static("identity")),
            (TRANSFER_ENCODING, HeaderValue::from_static("chunked")),
            (
                http::header::ACCEPT,
                HeaderValue::from_static("application/json"),
            ),
        ]);
        *original.version_mut() = http::Version::HTTP_2;
        *original.timeout_mut() = Some(Duration::from_secs(7));
        *original.body_mut() = Some("{}".into());
        let mut expected_headers = original.headers().clone();
        let expected_body = match body_behavior {
            RedirectBodyBehavior::Drop => {
                for header in [
                    CONTENT_TYPE,
                    CONTENT_LENGTH,
                    CONTENT_ENCODING,
                    TRANSFER_ENCODING,
                ] {
                    expected_headers.remove(header);
                }
                None
            }
            RedirectBodyBehavior::Preserve => Some(b"{}".to_vec()),
        };

        let redirected = redirect_request(
            status,
            original.method().clone(),
            original.headers().clone(),
            original.version(),
            original.timeout().copied(),
            original.try_clone(),
            redirected_url.clone(),
        )
        .expect("supported redirect should be followed");

        assert_eq!(
            (
                redirected.method().clone(),
                redirected.url().clone(),
                redirected.headers().clone(),
                redirected.version(),
                redirected.timeout().copied(),
                redirected
                    .body()
                    .and_then(reqwest::Body::as_bytes)
                    .map(<[u8]>::to_vec),
            ),
            (
                expected_method,
                redirected_url.clone(),
                expected_headers,
                http::Version::HTTP_2,
                Some(Duration::from_secs(7)),
                expected_body,
            ),
            "redirect behavior for {status} {method}",
        );
    }
}

#[test]
fn redirect_referer_follows_strict_origin_when_cross_origin() {
    let previous =
        reqwest::Url::parse("https://user:password@example.com/start?access_token=secret#fragment")
            .expect("valid URL");

    let mut same_origin_headers = HeaderMap::new();
    let same_origin = reqwest::Url::parse("https://example.com/next").expect("valid URL");
    insert_referer(&mut same_origin_headers, &previous, &same_origin);
    assert_eq!(
        same_origin_headers.get(REFERER),
        Some(&HeaderValue::from_static(
            "https://example.com/start?access_token=secret"
        ))
    );

    let mut cross_origin_headers = HeaderMap::new();
    let cross_origin = reqwest::Url::parse("https://other.example/next").expect("valid URL");
    insert_referer(&mut cross_origin_headers, &previous, &cross_origin);
    assert_eq!(
        cross_origin_headers.get(REFERER),
        Some(&HeaderValue::from_static("https://example.com/"))
    );

    let mut downgrade_headers = HeaderMap::from_iter([(
        REFERER,
        HeaderValue::from_static("https://stale.example/path?secret=value"),
    )]);
    let downgrade = reqwest::Url::parse("http://other.example/next").expect("valid URL");
    insert_referer(&mut downgrade_headers, &previous, &downgrade);
    assert_eq!(downgrade_headers.get(REFERER), None);
}

#[test]
fn non_replayable_redirect_is_not_followed_regardless_of_target_scheme() {
    let next_url = reqwest::Url::parse("ftp://example.com/next").expect("valid URL");

    let redirected = redirect_request(
        StatusCode::TEMPORARY_REDIRECT,
        Method::POST,
        HeaderMap::new(),
        http::Version::HTTP_11,
        /*timeout*/ None,
        /*replay*/ None,
        next_url,
    );

    assert!(redirected.is_none());
}

#[test]
fn redirect_credentials_are_retained_only_for_the_same_origin() {
    for (previous, next, retain_credentials) in [
        (
            "https://example.com:8080/start",
            "https://example.com:8080/next",
            true,
        ),
        (
            "https://example.com:8080/start",
            "http://example.com:8080/next",
            false,
        ),
        (
            "https://example.com:8080/start",
            "https://other.example:8080/next",
            false,
        ),
        (
            "https://example.com:8080/start",
            "https://example.com:8081/next",
            false,
        ),
    ] {
        let previous = reqwest::Url::parse(previous).expect("previous URL should parse");
        let next = reqwest::Url::parse(next).expect("next URL should parse");
        let mut headers = HeaderMap::from_iter([
            (AUTHORIZATION, HeaderValue::from_static("Bearer secret")),
            (COOKIE, HeaderValue::from_static("session=secret")),
        ]);

        remove_sensitive_headers(&mut headers, &previous, &next);

        assert_eq!(
            (
                headers.contains_key(AUTHORIZATION),
                headers.contains_key(COOKIE),
            ),
            (retain_credentials, retain_credentials),
            "credential handling for {previous} -> {next}"
        );
    }
}
