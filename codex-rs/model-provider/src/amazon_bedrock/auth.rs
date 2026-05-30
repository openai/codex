use std::sync::Arc;

use codex_api::AuthError;
use codex_api::AuthProvider;
use codex_api::SharedAuthProvider;
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

use crate::BearerAuthProvider;

use super::mantle::aws_auth_config;
use super::mantle::region_from_config;
use super::provider_auth::AmazonBedrockAuth;

const AWS_BEARER_TOKEN_BEDROCK_ENV_VAR: &str = "AWS_BEARER_TOKEN_BEDROCK";
const AWS_REGION_ENV_VAR: &str = "AWS_REGION";
const AWS_DEFAULT_REGION_ENV_VAR: &str = "AWS_DEFAULT_REGION";

pub(super) enum BedrockAuthMethod {
    StoredBearerToken { token: String, region: String },
    EnvBearerToken { token: String, region: String },
    AwsSdkAuth { context: AwsAuthContext },
}

pub(super) async fn resolve_auth_method(
    stored_auth: Option<&AmazonBedrockAuth>,
    aws: &ModelProviderAwsAuthInfo,
) -> Result<BedrockAuthMethod> {
    if let Some(auth) = stored_auth {
        return Ok(BedrockAuthMethod::StoredBearerToken {
            token: auth.bearer_token.clone(),
            region: auth.region.clone(),
        });
    }

    if let Some(token) = non_empty_env_var_from(AWS_BEARER_TOKEN_BEDROCK_ENV_VAR, std::env::var) {
        let region = bearer_token_region(aws, std::env::var)?;
        return Ok(BedrockAuthMethod::EnvBearerToken { token, region });
    }

    let config = aws_auth_config(aws);
    let context = AwsAuthContext::load(config)
        .await
        .map_err(aws_auth_error_to_codex_error)?;
    Ok(BedrockAuthMethod::AwsSdkAuth { context })
}

pub(super) async fn resolve_provider_auth(
    stored_auth: Option<&AmazonBedrockAuth>,
    aws: &ModelProviderAwsAuthInfo,
) -> Result<SharedAuthProvider> {
    match resolve_auth_method(stored_auth, aws).await? {
        BedrockAuthMethod::StoredBearerToken { token, .. }
        | BedrockAuthMethod::EnvBearerToken { token, .. } => Ok(Arc::new(BearerAuthProvider {
            token: Some(token),
            account_id: None,
            is_fedramp_account: false,
        })),
        BedrockAuthMethod::AwsSdkAuth { context } => {
            Ok(Arc::new(BedrockMantleSigV4AuthProvider::new(context)))
        }
    }
}

fn non_empty_env_var_from(
    name: &'static str,
    env_var: impl Fn(&'static str) -> std::result::Result<String, std::env::VarError>,
) -> Option<String> {
    env_var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn bearer_token_region(
    aws: &ModelProviderAwsAuthInfo,
    env_var: impl Fn(&'static str) -> std::result::Result<String, std::env::VarError> + Copy,
) -> Result<String> {
    region_from_config(aws)
        .or_else(|| non_empty_env_var_from(AWS_REGION_ENV_VAR, env_var))
        .or_else(|| non_empty_env_var_from(AWS_DEFAULT_REGION_ENV_VAR, env_var))
        .ok_or_else(|| {
            CodexErr::Fatal(
                "Amazon Bedrock bearer token auth requires \
`model_providers.amazon-bedrock.aws.region`, `AWS_REGION`, or `AWS_DEFAULT_REGION`"
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
    // The Bedrock Mantle front door does not preserve legacy OpenAI
    // compatibility headers that use snake_case, such as `session_id` and
    // `thread_id`, before SigV4 verification. Signing that header class makes
    // richer Codex agent requests fail even though raw Responses requests work.
    let headers_to_remove = headers
        .keys()
        .filter(|name| name.as_str().contains('_'))
        .cloned()
        .collect::<Vec<_>>();
    for name in headers_to_remove {
        headers.remove(name);
    }
}

/// AWS SigV4 auth provider for Bedrock Mantle OpenAI-compatible requests.
#[derive(Debug)]
struct BedrockMantleSigV4AuthProvider {
    context: AwsAuthContext,
}

impl BedrockMantleSigV4AuthProvider {
    fn new(context: AwsAuthContext) -> Self {
        Self { context }
    }
}

#[async_trait::async_trait]
impl AuthProvider for BedrockMantleSigV4AuthProvider {
    fn add_auth_headers(&self, _headers: &mut HeaderMap) {}

    async fn apply_auth(&self, request: Request) -> std::result::Result<Request, AuthError> {
        let mut request = request;
        remove_headers_not_preserved_by_bedrock_mantle(&mut request.headers);
        let prepared = request.prepare_body_for_send().map_err(AuthError::Build)?;
        let signed = self
            .context
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

    fn missing_env_var(_: &'static str) -> std::result::Result<String, std::env::VarError> {
        Err(std::env::VarError::NotPresent)
    }

    #[test]
    fn bedrock_bearer_auth_prefers_configured_region_and_uses_header() {
        let token = "bedrock-api-key-test".to_string();
        let region = bearer_token_region(
            &ModelProviderAwsAuthInfo {
                profile: None,
                region: Some(" us-west-2 ".to_string()),
            },
            |name| match name {
                AWS_REGION_ENV_VAR => Ok("eu-west-1".to_string()),
                _ => Err(std::env::VarError::NotPresent),
            },
        )
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

    #[tokio::test]
    async fn stored_bedrock_bearer_auth_takes_precedence() {
        let auth = AmazonBedrockAuth {
            bearer_token: "stored-bedrock-key".to_string(),
            region: "eu-west-1".to_string(),
        };

        let method = resolve_auth_method(
            Some(&auth),
            &ModelProviderAwsAuthInfo {
                profile: None,
                region: Some("us-east-1".to_string()),
            },
        )
        .await
        .expect("stored auth should resolve");

        match method {
            BedrockAuthMethod::StoredBearerToken { token, region } => {
                assert_eq!(
                    (token, region),
                    ("stored-bedrock-key".to_string(), "eu-west-1".to_string())
                );
            }
            BedrockAuthMethod::EnvBearerToken { .. } | BedrockAuthMethod::AwsSdkAuth { .. } => {
                panic!("stored auth should take precedence")
            }
        }
    }

    #[test]
    fn bedrock_bearer_auth_uses_aws_region_env() {
        let region = bearer_token_region(
            &ModelProviderAwsAuthInfo {
                profile: None,
                region: None,
            },
            |name| match name {
                AWS_REGION_ENV_VAR => Ok(" eu-central-1 ".to_string()),
                _ => Err(std::env::VarError::NotPresent),
            },
        )
        .expect("AWS_REGION should resolve");

        assert_eq!(region, "eu-central-1");
    }

    #[test]
    fn bedrock_bearer_auth_uses_aws_default_region_env() {
        let region = bearer_token_region(
            &ModelProviderAwsAuthInfo {
                profile: None,
                region: None,
            },
            |name| match name {
                AWS_DEFAULT_REGION_ENV_VAR => Ok("ap-northeast-1".to_string()),
                _ => Err(std::env::VarError::NotPresent),
            },
        )
        .expect("AWS_DEFAULT_REGION should resolve");

        assert_eq!(region, "ap-northeast-1");
    }

    #[test]
    fn bedrock_bearer_auth_rejects_missing_configured_region() {
        let err = bearer_token_region(
            &ModelProviderAwsAuthInfo {
                profile: None,
                region: None,
            },
            missing_env_var,
        )
        .expect_err("missing region should fail");

        assert_eq!(
            err.to_string(),
            "Fatal error: Amazon Bedrock bearer token auth requires \
`model_providers.amazon-bedrock.aws.region`, `AWS_REGION`, or `AWS_DEFAULT_REGION`"
        );
    }

    #[test]
    fn bedrock_mantle_sigv4_strips_headers_not_preserved_by_mantle() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "session_id",
            HeaderValue::from_static("019dae79-15c3-70c3-8736-3219b8602b37"),
        );
        headers.insert(
            "thread_id",
            HeaderValue::from_static("019dae79-15c3-70c3-8736-3219b8602b37"),
        );
        headers.insert(
            "future_identity_header",
            HeaderValue::from_static("019dae79-15c3-70c3-8736-3219b8602b37"),
        );
        headers.insert(
            "x-client-request-id",
            HeaderValue::from_static("request-id"),
        );

        remove_headers_not_preserved_by_bedrock_mantle(&mut headers);

        assert!(!headers.contains_key("session_id"));
        assert!(!headers.contains_key("thread_id"));
        assert!(!headers.contains_key("future_identity_header"));
        assert_eq!(
            headers
                .get("x-client-request-id")
                .and_then(|value| value.to_str().ok()),
            Some("request-id")
        );
    }
}
