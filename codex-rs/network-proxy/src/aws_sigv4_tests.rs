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

pub(crate) fn guest_authorization(service: &str, signed_headers: &str) -> String {
    format!(
        "AWS4-HMAC-SHA256 Credential=MASKEDACCESSKEY/20150830/us-east-1/{service}/aws4_request, SignedHeaders={signed_headers}, Signature={:064x}",
        1
    )
}

fn resign(signer: &AwsSigV4Signer, mut req: Request, headers: &str) -> Result<Request> {
    let auth = guest_authorization(&signer.config.service, headers);
    req.headers_mut()
        .insert("authorization", HeaderValue::from_str(&auth)?);
    req.headers_mut()
        .insert("x-amz-date", HeaderValue::from_static("20150830T123600Z"));
    let guest = signer.plan(&req)?.expect("selected guest credential");
    signer.sign(req, &guest)
}

#[test]
fn matches_aws_published_iam_signature_vector() {
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
    .unwrap();
    assert_eq!(
        signed.headers()["authorization"],
        "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/iam/aws4_request, SignedHeaders=content-type;host;x-amz-date, Signature=5d672d79c15b13162d9279b0855cfba6789a8edb4c82c400e06b5924a6f2b5d7"
    );
}

#[tokio::test]
async fn streams_explicitly_unsigned_s3_uploads() {
    let mut config = config("s3");
    config.session_token = Some(ProxySecretSource::EnvVar("SESSION_TOKEN".into()));
    let signer = signer(config);
    let upload = Request::builder()
        .uri("https://bucket.s3.amazonaws.com/upload")
        .header("x-amz-content-sha256", "UNSIGNED-PAYLOAD")
        .header("x-unsigned-metadata", "forward only")
        .body(Body::from(vec![b'x'; 1025]))
        .unwrap();
    let upload = resign(&signer, upload, "host;x-amz-date").unwrap();
    assert_eq!(upload.headers()["x-amz-content-sha256"], "UNSIGNED-PAYLOAD");
    let authorization = upload.headers()["authorization"].to_str().unwrap();
    assert!(authorization.contains("SignedHeaders=host;x-amz-date;x-amz-security-token,"));
    use rama_http::body::util::BodyExt as _;
    assert_eq!(
        upload.into_body().collect().await.unwrap().to_bytes().len(),
        1025
    );
}

#[test]
fn rejects_wrong_transport_scope_streaming_and_nonempty_bodies() {
    let signer = signer(config("s3"));
    let valid = "https://bucket.s3.amazonaws.com/key";
    for uri in [
        "http://bucket.s3.amazonaws.com/key",
        "https://bucket.s3.amazonaws.com:8443/key",
        "https://bucket.s3.amazonaws.com/key?X-Amz-Signature=guest",
    ] {
        let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
        assert!(resign(&signer, req, "host;x-amz-date").is_err(), "{uri}");
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
        assert!(resign(&signer, req, "host;x-amz-date").is_err());
    }
}
