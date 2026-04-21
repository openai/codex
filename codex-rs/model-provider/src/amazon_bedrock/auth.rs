use std::sync::Arc;

use codex_api::AuthError;
use codex_api::AuthProvider;
use codex_api::SharedAuthProvider;
use codex_aws_auth::AwsAuthConfig;
use codex_aws_auth::AwsAuthContext;
use codex_aws_auth::AwsAuthError;
use codex_aws_auth::AwsRequestToSign;
use codex_aws_auth::region_from_bedrock_bearer_token;
use codex_client::Request;
use codex_client::RequestBody;
use codex_client::RequestCompression;
use codex_model_provider_info::ModelProviderAwsAuthInfo;
use codex_protocol::error::CodexErr;
use http::HeaderMap;
use http::HeaderValue;
use tokio::sync::OnceCell;

use super::mantle;

const AWS_BEARER_TOKEN_BEDROCK_ENV_VAR: &str = "AWS_BEARER_TOKEN_BEDROCK";
const LEGACY_SESSION_ID_HEADER: &str = "session_id";

pub(super) async fn resolve_provider_auth(
    aws: &ModelProviderAwsAuthInfo,
) -> codex_protocol::error::Result<SharedAuthProvider> {
    if let Some(token) = bearer_token_from_env() {
        return resolve_bearer_auth(token);
    }

    let config = mantle::aws_auth_config(aws);
    let context = AwsAuthContext::load(config.clone())
        .await
        .map_err(aws_auth_error_to_codex_error)?;
    Ok(Arc::new(BedrockMantleSigV4AuthProvider::with_context(
        config, context,
    )))
}

pub(super) async fn resolve_region(
    aws: &ModelProviderAwsAuthInfo,
) -> codex_protocol::error::Result<String> {
    if let Some(token) = bearer_token_from_env() {
        return region_from_bedrock_bearer_token(&token).map_err(aws_auth_error_to_codex_error);
    }

    let context = AwsAuthContext::load(mantle::aws_auth_config(aws))
        .await
        .map_err(aws_auth_error_to_codex_error)?;
    Ok(context.region().to_string())
}

fn resolve_bearer_auth(token: String) -> codex_protocol::error::Result<SharedAuthProvider> {
    let _region =
        region_from_bedrock_bearer_token(&token).map_err(aws_auth_error_to_codex_error)?;
    Ok(Arc::new(BedrockBearerAuthProvider::new(token)))
}

fn bearer_token_from_env() -> Option<String> {
    std::env::var(AWS_BEARER_TOKEN_BEDROCK_ENV_VAR)
        .ok()
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
}

fn aws_auth_error_to_codex_error(error: AwsAuthError) -> CodexErr {
    CodexErr::Fatal(format!("failed to resolve Amazon Bedrock auth: {error}"))
}

/// AWS SigV4 auth provider for Bedrock Mantle OpenAI-compatible requests.
#[derive(Debug)]
struct BedrockMantleSigV4AuthProvider {
    config: AwsAuthConfig,
    context: OnceCell<AwsAuthContext>,
}

impl BedrockMantleSigV4AuthProvider {
    fn with_context(config: AwsAuthConfig, context: AwsAuthContext) -> Self {
        let cell = OnceCell::new();
        let _ = cell.set(context);
        Self {
            config,
            context: cell,
        }
    }

    async fn context(&self) -> Result<&AwsAuthContext, AuthError> {
        self.context
            .get_or_try_init(|| AwsAuthContext::load(self.config.clone()))
            .await
            .map_err(aws_auth_error_to_auth_error)
    }
}

/// Amazon Bedrock bearer-token auth provider for OpenAI-compatible requests.
#[derive(Debug)]
struct BedrockBearerAuthProvider {
    token: String,
}

impl BedrockBearerAuthProvider {
    fn new(token: String) -> Self {
        Self { token }
    }
}

#[async_trait::async_trait]
impl AuthProvider for BedrockBearerAuthProvider {
    fn add_auth_headers(&self, headers: &mut HeaderMap) {
        let token = &self.token;
        if let Ok(header) = HeaderValue::from_str(&format!("Bearer {token}")) {
            let _ = headers.insert(http::header::AUTHORIZATION, header);
        }
    }
}

fn aws_auth_error_to_auth_error(error: AwsAuthError) -> AuthError {
    if error.is_retryable() {
        AuthError::Transient(error.to_string())
    } else {
        AuthError::Build(error.to_string())
    }
}

#[async_trait::async_trait]
impl AuthProvider for BedrockMantleSigV4AuthProvider {
    fn add_auth_headers(&self, _headers: &mut HeaderMap) {}

    async fn apply_auth(&self, mut request: Request) -> Result<Request, AuthError> {
        remove_headers_not_preserved_by_bedrock_mantle(&mut request.headers);
        let prepared = request.prepare_body_for_send().map_err(AuthError::Build)?;
        let context = self.context().await?;
        let signed = context
            .sign(AwsRequestToSign {
                method: request.method.clone(),
                url: request.url.clone(),
                headers: prepared.headers.clone(),
                body: prepared.body_bytes(),
            })
            .await
            .map_err(aws_auth_error_to_auth_error)?;

        request.url = signed.url;
        request.headers = signed.headers;
        request.body = prepared.body.map(RequestBody::Raw);
        request.compression = RequestCompression::None;
        Ok(request)
    }
}

fn remove_headers_not_preserved_by_bedrock_mantle(headers: &mut HeaderMap) {
    // The Bedrock Mantle front door does not preserve this legacy OpenAI header
    // for SigV4 verification. Signing it makes the richer Codex agent request
    // fail even though raw Responses requests work.
    headers.remove(LEGACY_SESSION_ID_HEADER);
}

#[cfg(test)]
mod tests {
    use codex_api::AuthProvider;
    use pretty_assertions::assert_eq;

    use super::*;

    fn bedrock_token_for_region(region: &str) -> String {
        let encoded = match region {
            "us-west-2" => {
                "YmVkcm9jay5hbWF6b25hd3MuY29tLz9BY3Rpb249Q2FsbFdpdGhCZWFyZXJUb2tlbiZYLUFtei1DcmVkZW50aWFsPUFLSURFWEFNUExFJTJGMjAyNjA0MjAlMkZ1cy13ZXN0LTIlMkZiZWRyb2NrJTJGYXdzNF9yZXF1ZXN0JlZlcnNpb249MQ=="
            }
            _ => panic!("test token fixture missing for {region}"),
        };
        format!("bedrock-api-key-{encoded}")
    }

    #[test]
    fn bedrock_bearer_auth_adds_header() {
        let provider = BedrockBearerAuthProvider::new("bedrock-token".to_string());
        let mut headers = HeaderMap::new();

        provider.add_auth_headers(&mut headers);

        assert_eq!(
            headers
                .get(http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok()),
            Some("Bearer bedrock-token")
        );
    }

    #[test]
    fn resolve_bedrock_bearer_auth_uses_token_region_and_header() {
        let token = bedrock_token_for_region("us-west-2");
        let region = region_from_bedrock_bearer_token(&token).expect("bearer token should resolve");
        let resolved = resolve_bearer_auth(token).expect("bearer auth should resolve");
        let mut headers = http::HeaderMap::new();

        resolved.add_auth_headers(&mut headers);

        assert_eq!(region, "us-west-2");
        assert!(
            headers
                .get(http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.starts_with("Bearer bedrock-api-key-"))
        );
    }

    #[test]
    fn bedrock_mantle_sigv4_strips_legacy_session_id_header() {
        let mut headers = HeaderMap::new();
        headers.insert(
            LEGACY_SESSION_ID_HEADER,
            HeaderValue::from_static("019dae79-15c3-70c3-8736-3219b8602b37"),
        );
        headers.insert(
            "x-client-request-id",
            HeaderValue::from_static("request-id"),
        );

        remove_headers_not_preserved_by_bedrock_mantle(&mut headers);

        assert!(!headers.contains_key(LEGACY_SESSION_ID_HEADER));
        assert_eq!(
            headers
                .get("x-client-request-id")
                .and_then(|value| value.to_str().ok()),
            Some("request-id")
        );
    }
}
