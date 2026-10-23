//! Exact substitution preserves chunk boundaries, authorization scope, and signed wire bytes.

use super::*;
use pretty_assertions::assert_eq;
use rama_core::bytes::Bytes;
use rama_core::futures::stream;
use rama_http::body::util::BodyExt;

const MARKER: &str = "codex-secret-0123456789abcdef";

fn config() -> BodySecretSubstitutionConfig {
    BodySecretSubstitutionConfig {
        replacements: vec![BodySecretReplacement {
            placeholder: MARKER.into(),
            secret: ProxySecretSource::EnvVar("SECRET".into()),
        }],
        port: 443,
        max_body_bytes: 1024,
    }
}

fn compile(config: BodySecretSubstitutionConfig, secret: &str) -> BodySecretSubstitution {
    config
        .compile(&|_| Some(secret.into()), &|_| {
            panic!("unexpected file read")
        })
        .unwrap()
}

#[tokio::test]
async fn substitutes_split_markers_and_updates_framing() {
    let body = Body::from_stream(stream::iter([
        Ok::<_, std::io::Error>(Bytes::from_static(b"before:codex-secret-0123")),
        Ok(Bytes::from_static(b"456789abcdef:after")),
    ]));
    let request = Request::builder()
        .method("PUT")
        .uri("https://bucket.s3.amazonaws.com/object")
        .header("transfer-encoding", "chunked")
        .header("content-encoding", "Identity, identity")
        .header("content-encoding", "IDENTITY")
        .body(body)
        .unwrap();
    let substituted = compile(config(), "actual-secret")
        .apply(request)
        .await
        .unwrap();
    assert_eq!(substituted.headers()["content-length"], "26");
    assert!(!substituted.headers().contains_key("transfer-encoding"));
    let payload = b"before:actual-secret:after";
    assert_eq!(
        substituted
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .as_ref(),
        payload
    );
}

#[tokio::test]
async fn scope_overflow_encoding_trailers_and_ambiguous_matches_fail_closed() {
    let mut small = config();
    small.max_body_bytes = 40;
    let transform = compile(small, "long-replacement-value");
    for (uri, body) in [
        ("http://example.com/", Body::from(MARKER)),
        ("https://example.com:444/", Body::from(MARKER)),
        ("https://example.com/", Body::from(vec![b'x'; 41])),
        (
            "https://example.com/",
            Body::empty().with_trailer_headers(rama_http::HeaderMap::new()),
        ),
    ] {
        let req = Request::builder().uri(uri).body(body).unwrap();
        assert!(transform.apply(req).await.is_err());
    }
    // Cover expansion and contraction in both orders at the final byte limit.
    let expanding_marker = "0123456789abcdef";
    let shrinking_marker = "fedcba9876543210";
    let mut mixed = config();
    mixed.max_body_bytes = 32;
    mixed.replacements[0].placeholder = expanding_marker.into();
    mixed.replacements.push(BodySecretReplacement {
        placeholder: shrinking_marker.into(),
        secret: ProxySecretSource::EnvVar("SHRINKING".into()),
    });
    for expanding_secret in ["x".repeat(30), "x".repeat(32)] {
        let transform = mixed
            .compile(
                &|name| match name {
                    "SECRET" => Some(expanding_secret.clone()),
                    "SHRINKING" => Some("y".into()),
                    _ => None,
                },
                &|_| panic!("unexpected file read"),
            )
            .unwrap();
        for (body, expected) in [
            (
                format!("{expanding_marker}{shrinking_marker}"),
                format!("{expanding_secret}y"),
            ),
            (
                format!("{shrinking_marker}{expanding_marker}"),
                format!("y{expanding_secret}"),
            ),
        ] {
            let result = transform
                .apply(
                    Request::builder()
                        .uri("https://example.com/")
                        .header("content-length", "32")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await;
            if expected.len() > mixed.max_body_bytes {
                assert_eq!(
                    result.unwrap_err().to_string(),
                    "expanded body substitution exceeds limit"
                );
                continue;
            }
            let substituted = result.unwrap();
            assert_eq!(substituted.headers()["content-length"], "31");
            assert!(!substituted.headers().contains_key("transfer-encoding"));
            assert_eq!(
                substituted
                    .into_body()
                    .collect()
                    .await
                    .unwrap()
                    .to_bytes()
                    .as_ref(),
                expected.as_bytes()
            );
        }
    }

    for coding in [None, Some("chunked")] {
        let mut req = Request::builder()
            .uri("https://example.com/")
            .header("content-length", "1");
        if let Some(coding) = coding {
            req = req.header("transfer-encoding", coding);
        }
        assert!(
            transform
                .apply(req.body(Body::from(MARKER)).unwrap())
                .await
                .is_err()
        );
    }

    let mut overlaps = config();
    overlaps.replacements[0].placeholder = "0123456789abcdef0".into();
    let transform = compile(overlaps, "replacement");
    assert!(
        transform
            .apply(
                Request::builder()
                    .uri("https://example.com/")
                    .body(Body::from("0123456789abcdef0123456789abcdef0"))
                    .unwrap()
            )
            .await
            .is_err()
    );
}

#[test]
fn rejects_duplicate_bindings_recursive_values_and_redacts_secrets() {
    let mut duplicate = config();
    duplicate
        .replacements
        .push(duplicate.replacements[0].clone());
    assert!(duplicate.validate().is_err());
    assert!(
        config()
            .compile(&|_| Some(MARKER.into()), &|_| panic!("unexpected file"))
            .is_err()
    );
    let compiled = compile(config(), "do-not-log-this-secret");
    assert!(!format!("{compiled:?}").contains("do-not-log-this-secret"));
}

#[tokio::test]
async fn rejects_integrity_headers_and_preserves_original_validation() {
    let transform = compile(config(), "secret");
    for (name, value) in [
        (
            "content-type",
            "Application/Vnd.Amazon.Eventstream; version=1",
        ),
        ("content-encoding", "gzip"),
        ("transfer-encoding", "gzip, chunked"),
        ("trailer", "digest"),
        ("x-amz-trailer", "x-amz-checksum-sha256"),
        ("x-amz-decoded-content-length", "32"),
        ("content-md5", "placeholder-digest"),
        ("digest", "sha-256=placeholder-digest"),
        ("content-digest", "sha-256=:placeholder-digest:"),
        ("repr-digest", "sha-256=:placeholder-digest:"),
        ("x-amz-content-sha256", "placeholder-digest"),
        ("x-amz-checksum-sha256", "placeholder-digest"),
        ("x-amz-checksum-crc32", "placeholder-digest"),
    ] {
        for strip in [false, true] {
            let mut request = Request::builder()
                .uri("https://example.com/")
                .header(name, value)
                .body(Body::from(MARKER))
                .unwrap();
            preserve_body_substitution_headers(&mut request);
            if strip {
                request.headers_mut().remove(name);
                preserve_body_substitution_headers(&mut request);
            }
            assert!(
                transform.apply(request).await.is_err(),
                "{name}, strip={strip}"
            );
        }
    }
    let mut request = Request::builder()
        .uri("https://example.com/")
        .body(Body::from(MARKER))
        .unwrap();
    preserve_body_substitution_headers(&mut request);
    request
        .headers_mut()
        .insert("content-encoding", HeaderValue::from_static("gzip"));
    assert!(transform.apply(request).await.is_err());
}

#[tokio::test]
async fn rejects_excessive_search_work_before_scanning_shared_prefixes() {
    let mut bounded = config();
    bounded.max_body_bytes = 1024 * 1024;
    bounded.replacements = (0..32)
        .map(|index| BodySecretReplacement {
            placeholder: format!("{}{:02}", "x".repeat(254), index),
            secret: ProxySecretSource::EnvVar("SECRET".into()),
        })
        .collect();
    let request = Request::builder()
        .uri("https://example.com/")
        .body(Body::from(vec![b'x'; bounded.max_body_bytes]))
        .unwrap();
    assert_eq!(
        compile(bounded, "secret")
            .apply(request)
            .await
            .unwrap_err()
            .to_string(),
        "body substitution search exceeds work limit"
    );
}
