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

const AWS_BEARER_TOKEN_BEDROCK_ENV_VAR: &str = "AWS_BEARER_TOKEN_BEDROCK";
const AWS_REGION_ENV_VAR: &str = "AWS_REGION";
const AWS_DEFAULT_REGION_ENV_VAR: &str = "AWS_DEFAULT_REGION";

pub(super) enum BedrockAuthMethod {
    EnvBearerToken { token: String, region: String },
    AwsSdkAuth { context: AwsAuthContext },
}

pub(super) async fn resolve_auth_method(
    aws: &ModelProviderAwsAuthInfo,
) -> Result<BedrockAuthMethod> {
    if let Some(token) = non_empty_env_var(AWS_BEARER_TOKEN_BEDROCK_ENV_VAR) {
        let region = bearer_token_region(aws)?;
        return Ok(BedrockAuthMethod::EnvBearerToken { token, region });
    }

    let config = aws_auth_config(aws);
    let context = AwsAuthContext::load(config)
        .await
        .map_err(aws_auth_error_to_codex_error)?;
    Ok(BedrockAuthMethod::AwsSdkAuth { context })
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
        BedrockAuthMethod::AwsSdkAuth { context } => {
            Ok(Arc::new(BedrockMantleSigV4AuthProvider::new(context)))
        }
    }
}

fn non_empty_env_var(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn bearer_token_region(aws: &ModelProviderAwsAuthInfo) -> Result<String> {
    region_from_config(aws)
        .or_else(|| non_empty_env_var(AWS_REGION_ENV_VAR))
        .or_else(|| non_empty_env_var(AWS_DEFAULT_REGION_ENV_VAR))
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
    use std::sync::Mutex;
    use std::sync::MutexGuard;

    use codex_api::AuthProvider;
    use http::HeaderValue;
    use pretty_assertions::assert_eq;

    use super::*;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvVarGuard<'a> {
        _lock: MutexGuard<'a, ()>,
        saved: Vec<(&'static str, Option<String>)>,
    }

    impl<'a> EnvVarGuard<'a> {
        fn new(vars: Vec<(&'static str, Option<&str>)>) -> Self {
            let lock = ENV_LOCK.lock().expect("env lock should not be poisoned");
            let saved = vars
                .iter()
                .map(|(name, _)| (*name, std::env::var(name).ok()))
                .collect::<Vec<_>>();
            for (name, value) in vars {
                match value {
                    Some(value) => {
                        // SAFETY: Tests using this guard hold ENV_LOCK while
                        // mutating the process environment and restore the
                        // prior values on drop.
                        unsafe { std::env::set_var(name, value) };
                    }
                    None => {
                        // SAFETY: Tests using this guard hold ENV_LOCK while
                        // mutating the process environment and restore the
                        // prior values on drop.
                        unsafe { std::env::remove_var(name) };
                    }
                }
            }
            Self { _lock: lock, saved }
        }
    }

    impl Drop for EnvVarGuard<'_> {
        fn drop(&mut self) {
            for (name, value) in &self.saved {
                match value {
                    Some(value) => {
                        // SAFETY: Tests using this guard hold ENV_LOCK while
                        // mutating the process environment and restore the
                        // prior values on drop.
                        unsafe { std::env::set_var(name, value) };
                    }
                    None => {
                        // SAFETY: Tests using this guard hold ENV_LOCK while
                        // mutating the process environment and restore the
                        // prior values on drop.
                        unsafe { std::env::remove_var(name) };
                    }
                }
            }
        }
    }

    #[test]
    fn bedrock_bearer_auth_prefers_configured_region_and_uses_header() {
        let _env = EnvVarGuard::new(vec![
            (AWS_REGION_ENV_VAR, Some("eu-west-1")),
            (AWS_DEFAULT_REGION_ENV_VAR, /*value*/ None),
        ]);
        let token = "bedrock-api-key-test".to_string();
        let region = bearer_token_region(&ModelProviderAwsAuthInfo {
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
    fn bedrock_bearer_auth_uses_aws_region_env() {
        let _env = EnvVarGuard::new(vec![
            (AWS_REGION_ENV_VAR, Some(" eu-central-1 ")),
            (AWS_DEFAULT_REGION_ENV_VAR, /*value*/ None),
        ]);
        let region = bearer_token_region(&ModelProviderAwsAuthInfo {
            profile: None,
            region: None,
        })
        .expect("AWS_REGION should resolve");

        assert_eq!(region, "eu-central-1");
    }

    #[test]
    fn bedrock_bearer_auth_uses_aws_default_region_env() {
        let _env = EnvVarGuard::new(vec![
            (AWS_REGION_ENV_VAR, /*value*/ None),
            (AWS_DEFAULT_REGION_ENV_VAR, Some("ap-northeast-1")),
        ]);
        let region = bearer_token_region(&ModelProviderAwsAuthInfo {
            profile: None,
            region: None,
        })
        .expect("AWS_DEFAULT_REGION should resolve");

        assert_eq!(region, "ap-northeast-1");
    }

    #[test]
    fn bedrock_bearer_auth_rejects_missing_configured_region() {
        let _env = EnvVarGuard::new(vec![
            (AWS_REGION_ENV_VAR, /*value*/ None),
            (AWS_DEFAULT_REGION_ENV_VAR, /*value*/ None),
        ]);
        let err = bearer_token_region(&ModelProviderAwsAuthInfo {
            profile: None,
            region: None,
        })
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
