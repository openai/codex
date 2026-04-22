use std::sync::Arc;

use codex_api::AuthError;
use codex_api::AuthProvider;
use codex_api::SharedAuthProvider;
use codex_aws_auth::AwsAuthConfig;
use codex_aws_auth::AwsAuthContext;
use codex_aws_auth::AwsAuthError;
use codex_aws_auth::AwsRequestToSign;
use codex_client::Request;
use codex_client::RequestBody;
use codex_client::RequestCompression;
use codex_model_provider_info::ModelProviderAwsAuthInfo;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
use http::HeaderMap;
use tokio::sync::OnceCell;

use crate::BearerAuthProvider;

use super::mantle::aws_auth_config;
use super::mantle::region_from_config;

const AWS_BEARER_TOKEN_BEDROCK_ENV_VAR: &str = "AWS_BEARER_TOKEN_BEDROCK";
const LEGACY_SESSION_ID_HEADER: &str = "session_id";

enum BedrockAuthMethod {
    EnvBearerToken {
        token: String,
        region: String,
    },
    AwsSdkAuth {
        config: AwsAuthConfig,
        context: AwsAuthContext,
    },
}

async fn resolve_auth_method(aws: &ModelProviderAwsAuthInfo) -> Result<BedrockAuthMethod> {
    if let Some(token) = bearer_token_from_env() {
        let region = bearer_token_region_from_config(aws)?;
        return Ok(BedrockAuthMethod::EnvBearerToken { token, region });
    }

    let config = aws_auth_config(aws);
    let context = AwsAuthContext::load(config.clone())
        .await
        .map_err(aws_auth_error_to_codex_error)?;
    Ok(BedrockAuthMethod::AwsSdkAuth { config, context })
}

pub(super) async fn resolve_provider_auth(
    aws: &ModelProviderAwsAuthInfo,
) -> Result<SharedAuthProvider> {
    match resolve_auth_method(aws).await? {
        BedrockAuthMethod::EnvBearerToken { token, .. } => Ok(Arc::new(BearerAuthProvider {
            token: Some(token),
            account_id: None,
            is_fedramp_account: false,
        })),
        BedrockAuthMethod::AwsSdkAuth { config, context } => Ok(Arc::new(
            BedrockMantleSigV4AuthProvider::with_context(config, context),
        )),
    }
}

pub(super) async fn resolve_region(aws: &ModelProviderAwsAuthInfo) -> Result<String> {
    match resolve_auth_method(aws).await? {
        BedrockAuthMethod::EnvBearerToken { region, .. } => Ok(region),
        BedrockAuthMethod::AwsSdkAuth { context, .. } => Ok(context.region().to_string()),
    }
}

fn bearer_token_from_env() -> Option<String> {
    std::env::var(AWS_BEARER_TOKEN_BEDROCK_ENV_VAR)
        .ok()
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
}

fn bearer_token_region_from_config(aws: &ModelProviderAwsAuthInfo) -> Result<String> {
    region_from_config(aws).ok_or_else(|| {
        CodexErr::Fatal(
            "Amazon Bedrock bearer token auth requires \
`model_providers.amazon-bedrock.aws.region`"
                .to_string(),
        )
    })
}

fn aws_auth_error_to_codex_error(error: AwsAuthError) -> CodexErr {
    CodexErr::Fatal(format!("failed to resolve Amazon Bedrock auth: {error}"))
}

fn aws_auth_error_to_auth_error(error: AwsAuthError) -> AuthError {
    if error.is_retryable() {
        AuthError::Transient(error.to_string())
    } else {
        AuthError::Build(error.to_string())
    }
}

fn remove_headers_not_preserved_by_bedrock_mantle(headers: &mut HeaderMap) {
    // The Bedrock Mantle front door does not preserve this legacy OpenAI header
    // for SigV4 verification. Signing it makes the richer Codex agent request
    // fail even though raw Responses requests work.
    headers.remove(LEGACY_SESSION_ID_HEADER);
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

    async fn context(&self) -> std::result::Result<&AwsAuthContext, AuthError> {
        self.context
            .get_or_try_init(|| AwsAuthContext::load(self.config.clone()))
            .await
            .map_err(aws_auth_error_to_auth_error)
    }
}

#[async_trait::async_trait]
impl AuthProvider for BedrockMantleSigV4AuthProvider {
    fn add_auth_headers(&self, _headers: &mut HeaderMap) {}

    async fn apply_auth(&self, request: Request) -> std::result::Result<Request, AuthError> {
        let mut request = request;
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

#[cfg(test)]
mod tests {
    use codex_api::AuthProvider;
    use http::HeaderValue;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn bedrock_bearer_auth_uses_configured_region_and_header() {
        let token = "bedrock-api-key-test".to_string();
        let region = bearer_token_region_from_config(&ModelProviderAwsAuthInfo {
            profile: None,
            region: Some(" us-west-2 ".to_string()),
        })
        .expect("configured region should resolve");
        let provider = BearerAuthProvider {
            token: Some(token),
            account_id: None,
            is_fedramp_account: false,
        };
        let mut headers = http::HeaderMap::new();

        provider.add_auth_headers(&mut headers);

        assert_eq!(region, "us-west-2");
        assert!(
            headers
                .get(http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.starts_with("Bearer bedrock-api-key-"))
        );
    }

    #[test]
    fn bedrock_bearer_auth_rejects_missing_configured_region() {
        let err = bearer_token_region_from_config(&ModelProviderAwsAuthInfo {
            profile: None,
            region: None,
        })
        .expect_err("missing region should fail");

        assert_eq!(
            err.to_string(),
            "Fatal error: Amazon Bedrock bearer token auth requires \
`model_providers.amazon-bedrock.aws.region`"
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
