use aws_credential_types::Credentials;
use aws_credential_types::provider::SharedCredentialsProvider;
use codex_aws_auth::AwsAuthContext;
use pretty_assertions::assert_eq;
use serde_json::Value;

use super::*;

#[test]
fn resolves_global_and_partition_endpoints() {
    assert_eq!(
        resolve_sts_endpoint(Some("aws-global")).expect("global endpoint"),
        (
            "us-east-1".to_string(),
            "https://sts.amazonaws.com/?Action=GetCallerIdentity&Version=2011-06-15".to_string(),
        )
    );
    assert_eq!(
        resolve_sts_endpoint(Some("us-west-2")).expect("regional endpoint"),
        (
            "us-west-2".to_string(),
            "https://sts.us-west-2.amazonaws.com/?Action=GetCallerIdentity&Version=2011-06-15"
                .to_string(),
        )
    );
    assert_eq!(
        resolve_sts_endpoint(Some("cn-north-1")).expect("China endpoint"),
        (
            "cn-north-1".to_string(),
            "https://sts.cn-north-1.amazonaws.com.cn/?Action=GetCallerIdentity&Version=2011-06-15"
                .to_string(),
        )
    );
}

#[tokio::test]
async fn signs_provider_and_audience_with_temporary_credentials() {
    let context = AwsAuthContext::from_provider(
        SharedCredentialsProvider::new(Credentials::new(
            "ASIATESTKEY",
            "test-secret",
            Some("test-session-token".to_string()),
            /*expires_after*/ None,
            "unit-test",
        )),
        "us-east-1",
        "sts",
    )
    .expect("valid context");
    let endpoint =
        "https://sts.us-east-1.amazonaws.com/?Action=GetCallerIdentity&Version=2011-06-15"
            .to_string();
    let signed = context
        .sign(AwsRequestToSign {
            method: Method::POST,
            url: endpoint.clone(),
            headers: {
                let mut headers = HeaderMap::new();
                headers.insert(
                    http::header::HOST,
                    HeaderValue::from_static("sts.us-east-1.amazonaws.com"),
                );
                headers.insert(
                    OPENAI_PROVIDER_HEADER,
                    HeaderValue::from_static("idp_example"),
                );
                headers.insert(
                    OPENAI_AUDIENCE_HEADER,
                    HeaderValue::from_static("https://auth.openai.com/workload-identity"),
                );
                headers
            },
            body: Bytes::new(),
        })
        .await
        .expect("request should sign");
    let value: Value = serde_json::from_str(&serialize_proof(signed).expect("valid proof"))
        .expect("proof should be JSON");

    assert_eq!(value["url"], endpoint);
    assert_eq!(value["method"], "POST");
    let headers = value["headers"]
        .as_array()
        .expect("headers should be an array");
    let header = |name: &str| {
        headers.iter().find_map(|header| {
            (header["key"] == name).then(|| header["value"].as_str().unwrap_or_default())
        })
    };
    assert_eq!(header(OPENAI_PROVIDER_HEADER), Some("idp_example"));
    assert_eq!(
        header(OPENAI_AUDIENCE_HEADER),
        Some("https://auth.openai.com/workload-identity")
    );
    assert_eq!(header("x-amz-security-token"), Some("test-session-token"));
    let authorization = header("authorization").expect("authorization header");
    assert!(authorization.contains(OPENAI_PROVIDER_HEADER));
    assert!(authorization.contains(OPENAI_AUDIENCE_HEADER));
    assert!(authorization.contains("x-amz-security-token"));
}

#[tokio::test]
async fn rejects_static_credentials_without_a_session_token() {
    let context = AwsAuthContext::from_provider(
        SharedCredentialsProvider::new(Credentials::new(
            "AKIATESTKEY",
            "test-secret",
            None,
            /*expires_after*/ None,
            "unit-test",
        )),
        "us-east-1",
        "sts",
    )
    .expect("valid context");
    let provider = AwsSubjectTokenProvider::new(
        "idp_example",
        "https://auth.openai.com/workload-identity",
        Some("us-east-1".to_string()),
    );

    let error = provider
        .subject_token_with_context(
            &context,
            "https://sts.us-east-1.amazonaws.com/?Action=GetCallerIdentity&Version=2011-06-15"
                .to_string(),
        )
        .await
        .expect_err("static credentials must not produce a workload proof");

    match error {
        SubjectTokenError::MissingPrerequisite {
            provider,
            prerequisite,
        } => {
            assert_eq!(provider, "aws");
            assert_eq!(
                prerequisite,
                "AWS web identity, container, or IMDSv2 workload credentials"
            );
        }
        other => panic!("unexpected error: {other}"),
    }
}
