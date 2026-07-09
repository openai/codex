use http::HeaderValue;
use http::header::CONTENT_LENGTH;
use http::header::CONTENT_TYPE;
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
