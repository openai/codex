use codex_api::AuthError;
use codex_api::AuthProvider;
use codex_aws_auth::AwsAuthConfig;
use codex_aws_auth::AwsAuthContext;
use codex_aws_auth::AwsAuthError;
use codex_aws_auth::AwsRequestToSign;
use codex_client::Request;
use http::HeaderMap;
use tokio::sync::OnceCell;

/// AWS SigV4 auth provider for OpenAI-compatible model-provider requests.
#[derive(Debug)]
pub(crate) struct AwsSigV4AuthProvider {
    config: AwsAuthConfig,
    context: OnceCell<AwsAuthContext>,
}

impl AwsSigV4AuthProvider {
    pub(crate) fn new(config: AwsAuthConfig) -> Self {
        Self {
            config,
            context: OnceCell::new(),
        }
    }

    async fn context(&self) -> Result<&AwsAuthContext, AuthError> {
        self.context
            .get_or_try_init(|| AwsAuthContext::load(self.config.clone()))
            .await
            .map_err(aws_auth_error_to_auth_error)
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
impl AuthProvider for AwsSigV4AuthProvider {
    fn add_auth_headers(&self, _headers: &mut HeaderMap) {}

    /// AWS SigV4 requests are sent to OpenAI-compatible provider endpoints, not
    /// Codex's legacy Responses backend, so avoid signing and forwarding the
    /// Codex-private `session_id` compatibility header.
    fn should_send_legacy_conversation_header(&self) -> bool {
        false
    }

    async fn apply_auth(&self, mut request: Request) -> Result<Request, AuthError> {
        let body = request.prepare_body_for_send().map_err(AuthError::Build)?;
        let context = self.context().await?;
        let signed = context
            .sign(AwsRequestToSign {
                method: request.method.clone(),
                url: request.url.clone(),
                headers: request.headers.clone(),
                body,
            })
            .await
            .map_err(aws_auth_error_to_auth_error)?;

        request.url = signed.url;
        request.headers = signed.headers;
        Ok(request)
    }
}

#[cfg(test)]
mod tests {
    use codex_api::AuthProvider;

    use super::*;

    #[test]
    fn aws_sigv4_auth_disables_legacy_conversation_header() {
        let provider = AwsSigV4AuthProvider::new(AwsAuthConfig {
            region: Some("us-east-1".to_string()),
            profile: Some("codex-bedrock".to_string()),
            service: "bedrock-mantle".to_string(),
        });

        assert!(!provider.should_send_legacy_conversation_header());
    }
}
