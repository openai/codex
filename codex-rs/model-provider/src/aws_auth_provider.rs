use codex_api::AuthError;
use codex_api::AuthProvider;
use codex_aws_auth::AwsAuthConfig;
use codex_aws_auth::AwsAuthContext;
use codex_aws_auth::AwsAuthError;
use codex_aws_auth::AwsRequestToSign;
use codex_client::Request;
use codex_client::RequestBody;
use codex_client::RequestCompression;
use http::HeaderMap;
use http::HeaderValue;
use tokio::sync::OnceCell;

const LEGACY_SESSION_ID_HEADER: &str = "session_id";

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
