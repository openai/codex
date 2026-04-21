mod config;
mod signing;

use std::time::SystemTime;

use aws_credential_types::provider::ProvideCredentials;
use aws_credential_types::provider::SharedCredentialsProvider;
use base64::Engine;
use base64::engine::general_purpose;
use bytes::Bytes;
use http::HeaderMap;
use http::Method;
use thiserror::Error;
use url::Url;

/// AWS auth configuration used to resolve credentials and sign requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AwsAuthConfig {
    pub profile: Option<String>,
    pub service: String,
}

/// Generic HTTP request shape consumed by SigV4 signing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AwsRequestToSign {
    pub method: Method,
    pub url: String,
    pub headers: HeaderMap,
    pub body: Bytes,
}

/// Signed request parts returned to the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AwsSignedRequest {
    pub url: String,
    pub headers: HeaderMap,
}

/// Errors returned by credential loading or SigV4 signing.
#[derive(Debug, Error)]
pub enum AwsAuthError {
    #[error("AWS service name must not be empty")]
    EmptyService,
    #[error("AWS SDK config did not resolve a credentials provider")]
    MissingCredentialsProvider,
    #[error("AWS SDK config did not resolve a region")]
    MissingRegion,
    #[error("Amazon Bedrock bearer token is invalid: {0}")]
    InvalidBedrockBearerToken(String),
    #[error("failed to load AWS credentials: {0}")]
    Credentials(#[from] aws_credential_types::provider::error::CredentialsError),
    #[error("request URL is not a valid URI: {0}")]
    InvalidUri(#[source] http::uri::InvalidUri),
    #[error("failed to construct HTTP request for signing: {0}")]
    BuildHttpRequest(#[source] http::Error),
    #[error("request contains a non-UTF8 header value: {0}")]
    InvalidHeaderValue(#[source] http::header::ToStrError),
    #[error("failed to build signable request: {0}")]
    SigningRequest(#[source] aws_sigv4::http_request::SigningError),
    #[error("failed to build SigV4 signing params: {0}")]
    SigningParams(String),
    #[error("SigV4 signing failed: {0}")]
    SigningFailure(#[source] aws_sigv4::http_request::SigningError),
}

/// Loaded AWS auth context that can sign outbound HTTP requests.
#[derive(Clone)]
pub struct AwsAuthContext {
    credentials_provider: SharedCredentialsProvider,
    region: String,
    service: String,
}

impl std::fmt::Debug for AwsAuthContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AwsAuthContext")
            .field("region", &self.region)
            .field("service", &self.service)
            .finish_non_exhaustive()
    }
}

impl AwsAuthContext {
    pub async fn load(config: AwsAuthConfig) -> Result<Self, AwsAuthError> {
        let sdk_config = config::load_sdk_config(&config).await?;
        let credentials_provider = config::credentials_provider(&sdk_config)?;
        let region = config::resolved_region(&sdk_config)?;

        Ok(Self {
            credentials_provider,
            region,
            service: config.service.trim().to_string(),
        })
    }

    pub fn region(&self) -> &str {
        &self.region
    }

    pub fn service(&self) -> &str {
        &self.service
    }

    pub async fn sign(&self, request: AwsRequestToSign) -> Result<AwsSignedRequest, AwsAuthError> {
        self.sign_at(request, SystemTime::now()).await
    }

    async fn sign_at(
        &self,
        request: AwsRequestToSign,
        time: SystemTime,
    ) -> Result<AwsSignedRequest, AwsAuthError> {
        let credentials = self.credentials_provider.provide_credentials().await?;
        signing::sign_request(&credentials, &self.region, &self.service, request, time)
    }
}

/// Extracts the AWS region embedded in an Amazon Bedrock short-term bearer token.
pub fn region_from_bedrock_bearer_token(token: &str) -> Result<String, AwsAuthError> {
    const PREFIX: &str = "bedrock-api-key-";

    let token_body = token
        .trim()
        .strip_prefix(PREFIX)
        .ok_or_else(|| invalid_bedrock_bearer_token("missing bedrock-api-key prefix"))?;
    let encoded_token = token_body
        .split_once("&Version=")
        .map_or(token_body, |(encoded, _)| encoded);
    let decoded = general_purpose::STANDARD
        .decode(encoded_token)
        .map_err(|_| invalid_bedrock_bearer_token("base64 payload could not be decoded"))?;
    let decoded = String::from_utf8(decoded)
        .map_err(|_| invalid_bedrock_bearer_token("decoded payload is not UTF-8"))?;
    let decoded_url = if decoded.starts_with("http://") || decoded.starts_with("https://") {
        decoded
    } else {
        format!("https://{decoded}")
    };
    let url = Url::parse(&decoded_url)
        .map_err(|_| invalid_bedrock_bearer_token("decoded payload is not a URL"))?;
    let credential = url
        .query_pairs()
        .find_map(|(key, value)| (key == "X-Amz-Credential").then_some(value.into_owned()))
        .ok_or_else(|| invalid_bedrock_bearer_token("missing X-Amz-Credential"))?;
    let mut parts = credential.split('/');
    let _access_key = parts.next();
    let _date = parts.next();
    let region = parts
        .next()
        .filter(|region| !region.trim().is_empty())
        .ok_or_else(|| invalid_bedrock_bearer_token("credential scope is missing region"))?;

    Ok(region.to_string())
}

fn invalid_bedrock_bearer_token(message: &'static str) -> AwsAuthError {
    AwsAuthError::InvalidBedrockBearerToken(message.to_string())
}

impl AwsAuthError {
    /// Returns whether retrying the outbound request can reasonably recover from this auth error.
    pub fn is_retryable(&self) -> bool {
        match self {
            AwsAuthError::Credentials(error) => matches!(
                error,
                aws_credential_types::provider::error::CredentialsError::ProviderTimedOut(_)
                    | aws_credential_types::provider::error::CredentialsError::ProviderError(_)
            ),
            AwsAuthError::EmptyService
            | AwsAuthError::MissingCredentialsProvider
            | AwsAuthError::MissingRegion
            | AwsAuthError::InvalidBedrockBearerToken(_)
            | AwsAuthError::InvalidUri(_)
            | AwsAuthError::BuildHttpRequest(_)
            | AwsAuthError::InvalidHeaderValue(_)
            | AwsAuthError::SigningRequest(_)
            | AwsAuthError::SigningParams(_)
            | AwsAuthError::SigningFailure(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use std::time::UNIX_EPOCH;

    use aws_credential_types::Credentials;
    use aws_credential_types::provider::error::CredentialsError;
    use pretty_assertions::assert_eq;

    use super::*;

    fn test_context(session_token: Option<&str>) -> AwsAuthContext {
        AwsAuthContext {
            credentials_provider: SharedCredentialsProvider::new(Credentials::new(
                "AKIDEXAMPLE",
                "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
                session_token.map(str::to_string),
                /*expires_after*/ None,
                "unit-test",
            )),
            region: "us-east-1".to_string(),
            service: "bedrock".to_string(),
        }
    }

    fn test_request() -> AwsRequestToSign {
        let mut headers = HeaderMap::new();
        headers.insert(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("application/json"),
        );
        headers.insert("x-test-header", http::HeaderValue::from_static("present"));
        AwsRequestToSign {
            method: Method::POST,
            url: "https://bedrock-runtime.us-east-1.amazonaws.com/v1/responses".to_string(),
            headers,
            body: Bytes::from_static(br#"{"model":"openai.gpt-oss-120b-1:0"}"#),
        }
    }

    #[tokio::test]
    async fn sign_adds_sigv4_headers_and_preserves_existing_headers() {
        let signed = test_context(/*session_token*/ None)
            .sign_at(
                test_request(),
                UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            )
            .await
            .expect("request should sign");

        assert_eq!(
            signing::header_value(&signed.headers, http::header::CONTENT_TYPE.as_str()),
            Some("application/json".to_string())
        );
        assert_eq!(
            signing::header_value(&signed.headers, "x-test-header"),
            Some("present".to_string())
        );
        assert_eq!(
            signed.url,
            "https://bedrock-runtime.us-east-1.amazonaws.com/v1/responses"
        );
        assert!(
            signing::header_value(&signed.headers, http::header::AUTHORIZATION.as_str())
                .is_some_and(|value| value.starts_with("AWS4-HMAC-SHA256 "))
        );
        assert!(signing::header_value(&signed.headers, "x-amz-date").is_some());
    }

    #[test]
    fn credentials_provider_failures_are_retryable() {
        assert!(
            AwsAuthError::Credentials(CredentialsError::provider_error("temporarily unavailable"))
                .is_retryable()
        );
        assert!(
            AwsAuthError::Credentials(CredentialsError::provider_timed_out(Duration::from_secs(1)))
                .is_retryable()
        );
    }

    #[test]
    fn deterministic_aws_auth_errors_are_not_retryable() {
        assert!(!AwsAuthError::EmptyService.is_retryable());
        assert!(
            !AwsAuthError::Credentials(CredentialsError::not_loaded_no_source()).is_retryable()
        );
        assert!(
            !AwsAuthError::Credentials(CredentialsError::invalid_configuration("bad profile"))
                .is_retryable()
        );
        assert!(
            !AwsAuthError::Credentials(CredentialsError::unhandled("unexpected response"))
                .is_retryable()
        );
    }

    #[tokio::test]
    async fn sign_includes_session_token_when_credentials_have_one() {
        let signed = test_context(Some("session-token"))
            .sign_at(
                test_request(),
                UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            )
            .await
            .expect("request should sign");

        assert_eq!(
            signing::header_value(&signed.headers, "x-amz-security-token"),
            Some("session-token".to_string())
        );
    }

    #[tokio::test]
    async fn load_rejects_empty_service_name() {
        let err = AwsAuthContext::load(AwsAuthConfig {
            profile: None,
            service: "   ".to_string(),
        })
        .await
        .expect_err("empty service should be rejected");

        assert_eq!(err.to_string(), "AWS service name must not be empty");
    }

    #[test]
    fn region_from_bedrock_bearer_token_reads_sigv4_credential_scope() {
        let decoded = "bedrock.amazonaws.com/?Action=CallWithBearerToken&X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Credential=AKIDEXAMPLE%2F20260420%2Fus-west-2%2Fbedrock%2Faws4_request&Version=1";
        let encoded = general_purpose::STANDARD.encode(decoded);
        let token = format!("bedrock-api-key-{encoded}");

        assert_eq!(
            region_from_bedrock_bearer_token(&token).expect("token region should parse"),
            "us-west-2"
        );
    }

    #[test]
    fn region_from_bedrock_bearer_token_accepts_unencoded_version_suffix() {
        let decoded = "bedrock.amazonaws.com/?Action=CallWithBearerToken&X-Amz-Credential=AKIDEXAMPLE%2F20260420%2Feu-west-1%2Fbedrock%2Faws4_request";
        let encoded = general_purpose::STANDARD.encode(decoded);
        let token = format!("bedrock-api-key-{encoded}&Version=1");

        assert_eq!(
            region_from_bedrock_bearer_token(&token).expect("token region should parse"),
            "eu-west-1"
        );
    }

    #[test]
    fn region_from_bedrock_bearer_token_rejects_missing_credential_scope() {
        let decoded = "bedrock.amazonaws.com/?Action=CallWithBearerToken";
        let encoded = general_purpose::STANDARD.encode(decoded);
        let token = format!("bedrock-api-key-{encoded}");

        let err = region_from_bedrock_bearer_token(&token)
            .expect_err("missing credential scope should fail");

        assert_eq!(
            err.to_string(),
            "Amazon Bedrock bearer token is invalid: missing X-Amz-Credential"
        );
    }
}
