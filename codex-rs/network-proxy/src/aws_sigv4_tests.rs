//! SigV4 interoperability vectors and rejection boundaries for the actual signing action.

use super::*;
use pretty_assertions::assert_eq;
use rama_http::HeaderMap;

fn config(service: &str) -> AwsSigV4Config {
    AwsSigV4Config {
        placeholder_access_key_id: "MASKEDACCESSKEY".into(),
        region: "us-east-1".into(),
        service: service.into(),
        access_key_id: ProxySecretSource::EnvVar("ACCESS_KEY".into()),
        secret_access_key: ProxySecretSource::EnvVar("SECRET_KEY".into()),
        session_token: None,
        port: 443,
        max_body_bytes: 1024,
    }
}

fn signer(config: AwsSigV4Config) -> AwsSigV4Signer {
    config
        .compile(
            &|name| {
                Some(
                    match name {
                        "ACCESS_KEY" => "AKIDEXAMPLE",
                        "SECRET_KEY" => "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
                        "SESSION_TOKEN" => "secret-session-token",
                        _ => panic!("unexpected credential source"),
                    }
                    .into(),
                )
            },
            &|_| panic!("unexpected file lookup"),
        )
        .unwrap()
}

fn time() -> SystemTime {
    chrono::DateTime::parse_from_rfc3339("2015-08-30T12:36:00Z")
        .unwrap()
        .into()
}

pub(crate) fn guest_authorization(service: &str, signed_headers: &str) -> String {
    format!(
        "AWS4-HMAC-SHA256 Credential=MASKEDACCESSKEY/20150830/us-east-1/{service}/aws4_request, SignedHeaders={signed_headers}, Signature={:064x}",
        1
    )
}

async fn resign(signer: &AwsSigV4Signer, mut req: Request, headers: &str) -> Result<Request> {
    let auth = guest_authorization(&signer.config.service, headers);
    req.headers_mut()
        .insert("authorization", HeaderValue::from_str(&auth)?);
    req.headers_mut()
        .insert("x-amz-date", HeaderValue::from_static("20150830T123600Z"));
    let guest = signer.plan(&req)?.expect("selected guest credential");
    signer.sign(req, &guest).await
}

#[test]
fn matches_aws_published_iam_signature_vector() {
    use rama_core::futures::FutureExt as _;

    // AWS Signature Version 4 ListUsers example, 2015-08-30.
    let request = Request::builder()
        .method("GET")
        .uri("https://iam.amazonaws.com/?Action=ListUsers&Version=2010-05-08")
        .header("host", "iam.amazonaws.com")
        .header(
            "content-type",
            "application/x-www-form-urlencoded; charset=utf-8",
        )
        .body(Body::empty())
        .unwrap();
    let signed = resign(
        &signer(config("iam")),
        request,
        "content-type;host;x-amz-date",
    )
    .now_or_never()
    .expect("an empty request must sign without waiting for buffer capacity")
    .unwrap();
    assert_eq!(
        signed.headers()["authorization"],
        "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/iam/aws4_request, SignedHeaders=content-type;host;x-amz-date, Signature=5d672d79c15b13162d9279b0855cfba6789a8edb4c82c400e06b5924a6f2b5d7"
    );
}

#[tokio::test]
async fn signs_actual_payload_and_replaces_untrusted_signing_headers() {
    let mut config = config("s3");
    config.session_token = Some(ProxySecretSource::EnvVar("SESSION_TOKEN".into()));
    let signer = Arc::new(signer(config));
    let mut env = HashMap::from([
        (
            "AWS_ACCESS_KEY_ID".to_string(),
            signer.credentials.access_key_id().to_string(),
        ),
        (
            "SESSION_TOKEN".to_string(),
            "secret-session-token".to_string(),
        ),
    ]);
    mask_local_aws_env(
        std::slice::from_ref(&signer),
        &mut env,
        /*ca_path*/ None,
    );
    assert_eq!(env["AWS_ACCESS_KEY_ID"], "MASKEDACCESSKEY");
    assert_eq!(env["SESSION_TOKEN"], env["AWS_SESSION_TOKEN"]);
    assert_ne!(env["AWS_SESSION_TOKEN"], "secret-session-token");
    env.insert("AWS_ACCESS_KEY_ID".to_string(), "OTHERKEY".to_string());
    env.insert("AWS_SECRET_ACCESS_KEY".to_string(), "unrelated".to_string());
    mask_local_aws_env(
        std::slice::from_ref(&signer),
        &mut env,
        /*ca_path*/ None,
    );
    assert_eq!(env["AWS_SECRET_ACCESS_KEY"], "unrelated");
    let mut cookie = HeaderValue::from_static("b=2");
    cookie.set_sensitive(true);
    let request = Request::builder()
        .method("PUT")
        .version(rama_http::Version::HTTP_2)
        .uri("https://bucket.s3.amazonaws.com/a%20b?tag=b&tag=a&empty=")
        .header("host", "bucket.s3.amazonaws.com")
        .header("cookie", "a=1")
        .header("cookie", cookie)
        .header("authorization", "guest-signature")
        .header("x-amz-date", "19700101T000000Z")
        .header("x-amz-security-token", "guest-token")
        .header("x-amz-content-sha256", "untrusted-guest-hash")
        .body(Body::from("hello"))
        .unwrap();
    let signed = resign(
        &signer,
        request,
        "cookie;host;x-amz-content-sha256;x-amz-date",
    )
    .await
    .unwrap();
    assert_eq!(
        signed.headers()["x-amz-content-sha256"],
        "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );
    assert_eq!(signed.headers()["x-amz-date"], "20150830T123600Z");
    assert_eq!(
        signed.headers()["x-amz-security-token"],
        "secret-session-token"
    );
    assert!(signed.headers()["x-amz-security-token"].is_sensitive());
    assert!(signed.headers()["authorization"].is_sensitive());
    assert_eq!(
        signed
            .headers()
            .get_all("cookie")
            .iter()
            .collect::<Vec<_>>(),
        vec![&HeaderValue::from_static("a=1; b=2")]
    );
    assert!(signed.headers()["cookie"].is_sensitive());
    let forwarded = signed.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(forwarded.as_ref(), b"hello");
    assert!(!format!("{signer:?}").contains("EXAMPLEKEY"));
    assert!(!format!("{signer:?}").contains("AKIDEXAMPLE"));

    // An SDK's unsigned S3 upload streams through without consuming the literal-body budget.
    let upload = Request::builder()
        .uri("https://bucket.s3.amazonaws.com/upload")
        .header("x-amz-content-sha256", "UNSIGNED-PAYLOAD")
        .header("x-unsigned-metadata", "forward only")
        .body(Body::from(vec![b'x'; 1025]))
        .unwrap();
    let upload = resign(&signer, upload, "host;x-amz-date").await.unwrap();
    assert_eq!(upload.headers()["x-amz-content-sha256"], "UNSIGNED-PAYLOAD");
    let authorization = upload.headers()["authorization"].to_str().unwrap();
    assert!(authorization.contains("SignedHeaders=host;x-amz-date;x-amz-security-token,"));
    use rama_http::body::util::BodyExt as _;
    assert_eq!(
        upload.into_body().collect().await.unwrap().to_bytes().len(),
        1025
    );
}

#[tokio::test]
async fn canonicalizes_query_order_without_changing_encoded_s3_path() {
    use sha2::Digest;

    for service in ["s3", "s3-object-lambda", "s3-outposts"] {
        let signer = signer(config(service));
        let payload_hash = format!("{:x}", sha2::Sha256::digest(b""));
        let signed_headers = "host;x-amz-content-sha256;x-amz-date";
        // Build the canonical request independently of SDK request canonicalization.
        // The encoded space and repeated slash must remain part of the object key.
        let canonical_request = format!(
            "GET\n/a%20b//key\na=one&a=two&space=a%20b&z=last\nhost:bucket.s3.amazonaws.com\nx-amz-content-sha256:{payload_hash}\nx-amz-date:20150830T123600Z\n\n{signed_headers}\n{payload_hash}"
        );
        let scope = format!("20150830/us-east-1/{service}/aws4_request");
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n20150830T123600Z\n{scope}\n{:x}",
            sha2::Sha256::digest(canonical_request.as_bytes())
        );
        let key = aws_sigv4::sign::v4::generate_signing_key(
            "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
            time(),
            "us-east-1",
            service,
        );
        let signature = aws_sigv4::sign::v4::calculate_signature(key, string_to_sign.as_bytes());
        let authorization = format!(
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/{scope}, SignedHeaders={signed_headers}, Signature={signature}"
        );
        for query in [
            "z=last&a=two&a=one&space=a%20b",
            "a=one&space=a%20b&a=two&z=last",
        ] {
            let uri = format!("https://bucket.s3.amazonaws.com/a%20b//key?{query}");
            let request = Request::builder()
                .uri(&uri)
                .header("x-amz-content-sha256", payload_hash.as_str())
                .body(Body::empty())
                .unwrap();
            let signed = resign(&signer, request, signed_headers).await.unwrap();
            assert_eq!(signed.uri().to_string(), uri);
            assert_eq!(
                signed.headers()["authorization"],
                authorization,
                "{service}"
            );
        }
    }
}

#[tokio::test]
async fn rejects_wrong_transport_port_streaming_presigning_and_body_overflow() {
    let signer = signer(config("s3"));
    let valid = "https://bucket.s3.amazonaws.com/key";
    for uri in [
        "http://bucket.s3.amazonaws.com/key",
        "https://bucket.s3.amazonaws.com:8443/key",
        "https://bucket.s3.amazonaws.com/key?X-Amz-Signature=guest",
    ] {
        let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
        assert!(
            resign(&signer, req, "host;x-amz-date").await.is_err(),
            "{uri}"
        );
    }
    for (name, value) in [
        ("content-encoding", "aws-chunked"),
        ("x-amz-content-sha256", "STREAMING-AWS4-HMAC-SHA256-PAYLOAD"),
        ("content-length", "1"),
    ] {
        let req = Request::builder()
            .uri(valid)
            .header(name, value)
            .body(Body::from("abc"))
            .unwrap();
        assert!(
            resign(&signer, req, "host;x-amz-date").await.is_err(),
            "{name}"
        );
    }
    let auth = guest_authorization("s3", "host;x-amz-date");
    for value in [
        auth.replace("us-east-1", "eu-west-1"),
        auth.replace("/s3/", "/sts/"),
        auth.replace("20150830/", "20150831/"),
        auth.replace("AWS4-HMAC-SHA256", "AWS4-ECDSA-P256-SHA256"),
        auth.replace("x-amz-date,", "x-amz-date;x-missing,"),
    ] {
        let req = Request::builder()
            .uri(valid)
            .header("authorization", value)
            .header("x-amz-date", "20150830T123600Z")
            .body(Body::empty())
            .unwrap();
        assert!(signer.plan(&req).is_err());
    }
    let presigned = Request::builder()
        .uri("https://bucket.s3.amazonaws.com/key?X-Amz-Credential=MASKEDACCESSKEY%2F20150830&X-Amz-Signature=guest")
        .body(Body::empty()).unwrap();
    assert!(signer.plan(&presigned).is_err());
    for body in [
        Body::from(vec![b'x'; 1025]),
        Body::empty().with_trailer_headers(HeaderMap::new()),
    ] {
        let req = Request::builder().uri(valid).body(body).unwrap();
        assert!(resign(&signer, req, "host;x-amz-date").await.is_err());
    }
    let stalled = rama_core::futures::stream::pending::<std::io::Result<rama_core::bytes::Bytes>>();
    let mut req = Request::builder()
        .uri(valid)
        .body(Body::from_stream(stalled))
        .unwrap();
    // Other tests use the process-wide pool on real clocks.
    let _permit = crate::body_budget::reserve(&mut req).await.unwrap();
    tokio::time::pause();
    let start = tokio::time::Instant::now();
    let signing = resign(&signer, req, "host;x-amz-date");
    let result = tokio::time::timeout(Duration::from_secs(31), signing)
        .await
        .expect("signer must enforce its own deadline");
    let error = result.unwrap_err();
    assert_eq!(error.to_string(), "AWS signing body read timed out");
    assert!(start.elapsed() >= Duration::from_secs(30));
}

#[test]
fn credentials_are_required_and_configured_limits_are_bounded() {
    assert!(
        config("s3")
            .compile(&|_| None, &|_| panic!("unexpected file"))
            .is_err()
    );
    let first = signer(config("s3"));
    let mut other = config("sts");
    other.placeholder_access_key_id = format!("GUEST{}", first.credentials.access_key_id());
    let other = other
        .compile(&|key| Some(format!("other-{key}")), &|_| {
            panic!("unexpected file")
        })
        .unwrap();
    assert!(validate_guest_placeholders([&first, &other].into_iter()).is_err());

    let mut config = config("s3");
    config.max_body_bytes = 16 * 1024 * 1024 + 1;
    assert_eq!(
        config.validate().unwrap_err().to_string(),
        "AWS signing body limit exceeds 16 MiB"
    );
    config.port = 0;
    assert_eq!(
        config.validate().unwrap_err().to_string(),
        "invalid AWS signing port"
    );
}

#[tokio::test]
async fn sdk_trace_does_not_log_canonical_secret_headers() {
    use tracing::instrument::WithSubscriber;
    let capture = tempfile::NamedTempFile::new().unwrap();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(tracing::Level::TRACE)
        .with_writer(std::sync::Arc::new(capture.reopen().unwrap()))
        .finish();
    let request = Request::builder()
        .uri("https://example.com/")
        .header("x-api-key", "secret-header-value")
        .body(Body::empty())
        .unwrap();
    let mut config = config("execute-api");
    config.session_token = Some(ProxySecretSource::EnvVar("SESSION_TOKEN".into()));
    async {
        tracing::trace!("proxy audit remains enabled");
        resign(&signer(config), request, "host;x-amz-date;x-api-key")
            .await
            .unwrap();
    }
    .with_subscriber(subscriber)
    .await;
    let logs = std::fs::read_to_string(capture.path()).unwrap();
    assert!(logs.contains("proxy audit remains enabled"));
    assert!(!logs.contains("secret-header-value"));
    assert!(!logs.contains("secret-session-token"));
    assert!(!logs.contains("EXAMPLEKEY"));
}
