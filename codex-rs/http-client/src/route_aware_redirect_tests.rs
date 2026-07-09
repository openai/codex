use http::HeaderValue;
use http::header::CONTENT_LENGTH;
use http::header::CONTENT_TYPE;
use http::header::COOKIE;
use http::header::REFERER;
use pretty_assertions::assert_eq;

use super::*;

#[test]
fn redirects_match_reqwest_method_and_body_rules() {
    let url = reqwest::Url::parse("https://example.com/next").expect("redirect URL should parse");
    let mut original = reqwest::Request::new(Method::POST, url.clone());
    original
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    original
        .headers_mut()
        .insert(CONTENT_LENGTH, HeaderValue::from_static("2"));
    *original.body_mut() = Some("{}".into());

    let found = redirect_request(
        StatusCode::FOUND,
        original.method().clone(),
        original.headers().clone(),
        original.version(),
        original.timeout().copied(),
        original.try_clone(),
        url.clone(),
    )
    .expect("POST redirect should be followed");
    assert_eq!(
        (
            found.method(),
            found.body().is_some(),
            found.headers().contains_key(CONTENT_TYPE),
            found.headers().contains_key(CONTENT_LENGTH),
        ),
        (&Method::GET, false, false, false)
    );

    let temporary = redirect_request(
        StatusCode::TEMPORARY_REDIRECT,
        original.method().clone(),
        original.headers().clone(),
        original.version(),
        original.timeout().copied(),
        original.try_clone(),
        url,
    )
    .expect("replayable temporary redirect should be followed");
    assert_eq!(
        (
            temporary.method(),
            temporary.body().is_some(),
            temporary.headers().get(CONTENT_TYPE),
            temporary.headers().get(CONTENT_LENGTH),
        ),
        (
            &Method::POST,
            true,
            Some(&HeaderValue::from_static("application/json")),
            Some(&HeaderValue::from_static("2")),
        )
    );
}

#[test]
fn redirect_referer_matches_reqwest_defaults() {
    let previous =
        reqwest::Url::parse("https://user:password@example.com/start#fragment").expect("valid URL");
    let next = reqwest::Url::parse("https://other.example/next").expect("valid URL");
    let mut headers = HeaderMap::new();

    insert_referer(&mut headers, &previous, &next);

    assert_eq!(
        headers.get(REFERER),
        Some(&HeaderValue::from_static("https://example.com/start"))
    );

    let mut downgrade_headers = HeaderMap::new();
    let downgrade = reqwest::Url::parse("http://other.example/next").expect("valid URL");
    insert_referer(&mut downgrade_headers, &previous, &downgrade);
    assert_eq!(downgrade_headers.get(REFERER), None);
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
