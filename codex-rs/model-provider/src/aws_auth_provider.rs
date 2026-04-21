use codex_api::AuthError;
use codex_api::AuthProvider;
use codex_aws_auth::AwsAuthConfig;
use codex_aws_auth::AwsAuthContext;
use codex_aws_auth::AwsAuthError;
use codex_aws_auth::AwsRequestToSign;
use codex_client::Request;
use http::HeaderMap;
use http::HeaderValue;
use tokio::sync::OnceCell;

/// AWS SigV4 auth provider for OpenAI-compatible model-provider requests.
#[derive(Debug)]
pub(crate) struct AwsSigV4AuthProvider {
    config: AwsAuthConfig,
    context: OnceCell<AwsAuthContext>,
}

impl AwsSigV4AuthProvider {
    pub(crate) fn with_context(config: AwsAuthConfig, context: AwsAuthContext) -> Self {
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
pub(crate) struct AwsBedrockBearerAuthProvider {
    token: String,
}

impl AwsBedrockBearerAuthProvider {
    pub(crate) fn new(token: String) -> Self {
        Self { token }
    }
}

#[async_trait::async_trait]
impl AuthProvider for AwsBedrockBearerAuthProvider {
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
impl AuthProvider for AwsSigV4AuthProvider {
    fn add_auth_headers(&self, _headers: &mut HeaderMap) {}

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
    fn aws_bedrock_bearer_auth_adds_header() {
        let provider = AwsBedrockBearerAuthProvider::new("bedrock-token".to_string());
        let mut headers = HeaderMap::new();

        provider.add_auth_headers(&mut headers);

        assert_eq!(
            headers
                .get(http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok()),
            Some("Bearer bedrock-token")
        );
    }
}
