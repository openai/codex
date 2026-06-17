use std::env;

use aws_config::ecs::EcsCredentialsProvider;
use aws_config::meta::credentials::CredentialsProviderChain;
use aws_config::provider_config::ProviderConfig;
use aws_config::web_identity_token::WebIdentityTokenCredentialsProvider;
use aws_credential_types::provider::SharedCredentialsProvider;
use aws_types::region::Region;
use bytes::Bytes;
use codex_aws_auth::AwsAuthContext;
use codex_aws_auth::AwsRequestToSign;
use codex_aws_auth::AwsSignedRequest;
use codex_workload_identity::SubjectToken;
use codex_workload_identity::SubjectTokenError;
use codex_workload_identity::SubjectTokenProvider;
use http::HeaderMap;
use http::HeaderValue;
use http::Method;
use serde::Serialize;

pub const AWS_SUBJECT_TOKEN_TYPE: &str = "urn:ietf:params:aws:token-type:aws4_request";

const AWS_REGION_ENV: &str = "AWS_REGION";
const AWS_DEFAULT_REGION_ENV: &str = "AWS_DEFAULT_REGION";
const DEFAULT_SIGNING_REGION: &str = "us-east-1";
const OPENAI_AUDIENCE_HEADER: &str = "x-openai-workload-identity-audience";
const OPENAI_PROVIDER_HEADER: &str = "x-openai-workload-identity-provider";
const STS_ACTION_QUERY: &str = "Action=GetCallerIdentity&Version=2011-06-15";

/// Creates a SigV4-signed STS `GetCallerIdentity` proof from AWS workload credentials.
///
/// The credential chain intentionally excludes environment access keys, shared profiles, SSO,
/// credential processes, AWS login credentials, and EC2 instance metadata. It accepts only web
/// identity and ECS/EKS container credentials whose acquisition inputs can be removed from model
/// subprocesses.
#[derive(Clone, Debug)]
pub struct AwsSubjectTokenProvider {
    identity_provider_id: String,
    audience: String,
    region: Option<String>,
}

impl AwsSubjectTokenProvider {
    pub fn new(
        identity_provider_id: impl Into<String>,
        audience: impl Into<String>,
        region: Option<String>,
    ) -> Self {
        Self {
            identity_provider_id: identity_provider_id.into(),
            audience: audience.into(),
            region,
        }
    }

    async fn signing_context(&self) -> Result<(AwsAuthContext, String), SubjectTokenError> {
        let (region, endpoint) = resolve_sts_endpoint(self.region.as_deref())?;
        let provider_config =
            ProviderConfig::without_region().with_region(Some(Region::new(region.clone())));
        let credentials = CredentialsProviderChain::first_try(
            "WebIdentityToken",
            WebIdentityTokenCredentialsProvider::builder()
                .configure(&provider_config)
                .build(),
        )
        .or_else(
            "EcsContainer",
            EcsCredentialsProvider::builder()
                .configure(&provider_config)
                .build(),
        );
        let context = AwsAuthContext::from_provider(
            SharedCredentialsProvider::new(credentials),
            region,
            "sts",
        )
        .map_err(|_| invalid_configuration())?;
        Ok((context, endpoint))
    }

    async fn subject_token_with_context(
        &self,
        context: &AwsAuthContext,
        endpoint: String,
    ) -> Result<SubjectToken, SubjectTokenError> {
        let host = endpoint
            .strip_prefix("https://")
            .and_then(|value| value.split('/').next())
            .ok_or_else(invalid_configuration)?;
        let mut headers = HeaderMap::new();
        headers.insert(
            http::header::HOST,
            HeaderValue::from_str(host).map_err(|_| invalid_configuration())?,
        );
        headers.insert(
            OPENAI_PROVIDER_HEADER,
            HeaderValue::from_str(&self.identity_provider_id)
                .map_err(|_| invalid_configuration())?,
        );
        headers.insert(
            OPENAI_AUDIENCE_HEADER,
            HeaderValue::from_str(&self.audience).map_err(|_| invalid_configuration())?,
        );
        let signed = context
            .sign(AwsRequestToSign {
                method: Method::POST,
                url: endpoint,
                headers,
                body: Bytes::new(),
            })
            .await
            .map_err(|_| missing_workload_credentials())?;
        let value = serialize_proof(signed)?;
        SubjectToken::new(value, AWS_SUBJECT_TOKEN_TYPE, "aws")
    }
}

impl SubjectTokenProvider for AwsSubjectTokenProvider {
    async fn subject_token(&self) -> Result<SubjectToken, SubjectTokenError> {
        let (context, endpoint) = self.signing_context().await?;
        self.subject_token_with_context(&context, endpoint).await
    }
}

#[derive(Serialize)]
struct AwsProof {
    url: String,
    method: &'static str,
    headers: Vec<AwsProofHeader>,
}

#[derive(Serialize)]
struct AwsProofHeader {
    key: String,
    value: String,
}

fn serialize_proof(signed: AwsSignedRequest) -> Result<String, SubjectTokenError> {
    let mut headers = Vec::new();
    for name in [
        http::header::AUTHORIZATION.as_str(),
        http::header::HOST.as_str(),
        "x-amz-content-sha256",
        "x-amz-date",
        "x-amz-security-token",
        OPENAI_AUDIENCE_HEADER,
        OPENAI_PROVIDER_HEADER,
    ] {
        if let Some(value) = signed.headers.get(name) {
            headers.push(AwsProofHeader {
                key: name.to_string(),
                value: value
                    .to_str()
                    .map_err(|_| invalid_configuration())?
                    .to_string(),
            });
        }
    }
    if !signed.headers.contains_key("x-amz-security-token") {
        return Err(missing_workload_credentials());
    }
    serde_json::to_string(&AwsProof {
        url: signed.url,
        method: "POST",
        headers,
    })
    .map_err(|_| invalid_configuration())
}

fn resolve_sts_endpoint(
    configured_region: Option<&str>,
) -> Result<(String, String), SubjectTokenError> {
    let configured_region = configured_region
        .map(str::to_string)
        .or_else(|| env::var(AWS_REGION_ENV).ok())
        .or_else(|| env::var(AWS_DEFAULT_REGION_ENV).ok());
    let Some(region) = configured_region else {
        return Ok((
            DEFAULT_SIGNING_REGION.to_string(),
            format!("https://sts.amazonaws.com/?{STS_ACTION_QUERY}"),
        ));
    };
    let region = region.trim();
    if region.is_empty()
        || region.len() > 64
        || !region.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
    {
        return Err(invalid_configuration());
    }
    if region == "aws-global" {
        return Ok((
            DEFAULT_SIGNING_REGION.to_string(),
            format!("https://sts.amazonaws.com/?{STS_ACTION_QUERY}"),
        ));
    }
    let dns_suffix = if region.starts_with("cn-") {
        "amazonaws.com.cn"
    } else {
        "amazonaws.com"
    };
    Ok((
        region.to_string(),
        format!("https://sts.{region}.{dns_suffix}/?{STS_ACTION_QUERY}"),
    ))
}

fn invalid_configuration() -> SubjectTokenError {
    SubjectTokenError::InvalidConfiguration { provider: "aws" }
}

fn missing_workload_credentials() -> SubjectTokenError {
    SubjectTokenError::MissingPrerequisite {
        provider: "aws",
        prerequisite: "AWS web identity or container workload credentials".to_string(),
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
