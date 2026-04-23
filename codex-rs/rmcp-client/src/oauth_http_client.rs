//! OAuth bootstrap helpers built on the shared `HttpClient` capability.
//!
//! Browser launch, callback handling, and token persistence remain
//! orchestrator-owned. This module covers the non-browser OAuth HTTP work
//! needed for discovery, registration, token exchange, and refresh so those
//! requests follow the same local-or-remote placement as the MCP transport.

use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use anyhow::Result;
use anyhow::anyhow;
use async_trait::async_trait;
use codex_exec_server::HttpClient;
use codex_exec_server::HttpHeader;
use codex_exec_server::HttpRequestParams;
use oauth2::HttpRequest;
use oauth2::HttpResponse;
use oauth2::http::HeaderName as OAuthHeaderName;
use oauth2::http::HeaderValue as OAuthHeaderValue;
use reqwest::Method;
use reqwest::Url;
use reqwest::header::HeaderMap;
use rmcp::transport::auth::AuthError;
use rmcp::transport::auth::AuthorizationManager;
use rmcp::transport::auth::OAuthHttpClient;
use rmcp::transport::auth::StoredCredentials;

const DEFAULT_TIMEOUT_MS: u64 = 30_000;

pub(crate) async fn create_oauth_authorization_manager(
    url: &str,
    default_headers: HeaderMap,
    http_client: Arc<dyn HttpClient>,
    initial_credentials: Option<StoredCredentials>,
) -> Result<AuthorizationManager> {
    let mut authorization_manager = AuthorizationManager::new(url)
        .await
        .map_err(|error| anyhow!(error))?;
    authorization_manager
        .with_oauth_http_client(HttpClientOAuthAdapter::new(http_client, default_headers))
        .map_err(|error| anyhow!(error))?;

    if let Some(initial_credentials) = initial_credentials {
        authorization_manager.set_credential_store(StaticCredentialStore::new(initial_credentials));
        authorization_manager
            .initialize_from_store()
            .await
            .map_err(|error| anyhow!(error))?;
    }

    Ok(authorization_manager)
}

#[derive(Clone)]
struct HttpClientOAuthAdapter {
    http_client: Arc<dyn HttpClient>,
    default_headers: HeaderMap,
}

impl HttpClientOAuthAdapter {
    fn new(http_client: Arc<dyn HttpClient>, default_headers: HeaderMap) -> Self {
        Self {
            http_client,
            default_headers,
        }
    }
}

#[async_trait]
impl OAuthHttpClient for HttpClientOAuthAdapter {
    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, AuthError> {
        let method = Method::from_bytes(request.method().as_str().as_bytes())
            .map_err(|error| AuthError::InternalError(error.to_string()))?;
        let url = Url::parse(&request.uri().to_string()).map_err(AuthError::UrlError)?;
        let mut headers = protocol_headers(&self.default_headers);
        for (name, value) in request.headers() {
            headers.push(HttpHeader {
                name: name.as_str().to_string(),
                value: value
                    .to_str()
                    .map_err(|error| AuthError::InternalError(error.to_string()))?
                    .to_string(),
            });
        }

        let response = self
            .http_client
            .http_request(HttpRequestParams {
                method: method.as_str().to_string(),
                url: url.to_string(),
                headers,
                body: Some(request.body().clone().into()),
                timeout_ms: Some(DEFAULT_TIMEOUT_MS),
                request_id: next_request_id("oauth-http"),
                stream_response: false,
            })
            .await
            .map_err(|error| AuthError::OAuthError(error.to_string()))?;

        let mut http_response = HttpResponse::new(response.body.into_inner());
        *http_response.status_mut() = reqwest::StatusCode::from_u16(response.status)
            .map_err(|error| AuthError::InternalError(error.to_string()))?;
        for header in response.headers {
            let header_name = header
                .name
                .parse::<OAuthHeaderName>()
                .map_err(|error| AuthError::InternalError(error.to_string()))?;
            let header_value = header
                .value
                .parse::<OAuthHeaderValue>()
                .map_err(|error| AuthError::InternalError(error.to_string()))?;
            http_response
                .headers_mut()
                .insert(header_name, header_value);
        }
        Ok(http_response)
    }
}

#[derive(Clone)]
struct StaticCredentialStore {
    stored: Arc<tokio::sync::Mutex<Option<StoredCredentials>>>,
}

impl StaticCredentialStore {
    fn new(credentials: StoredCredentials) -> Self {
        Self {
            stored: Arc::new(tokio::sync::Mutex::new(Some(credentials))),
        }
    }
}

#[async_trait]
impl rmcp::transport::auth::CredentialStore for StaticCredentialStore {
    async fn load(&self) -> Result<Option<StoredCredentials>, AuthError> {
        Ok(self.stored.lock().await.clone())
    }

    async fn save(&self, credentials: StoredCredentials) -> Result<(), AuthError> {
        *self.stored.lock().await = Some(credentials);
        Ok(())
    }

    async fn clear(&self) -> Result<(), AuthError> {
        *self.stored.lock().await = None;
        Ok(())
    }
}

fn protocol_headers(headers: &HeaderMap) -> Vec<HttpHeader> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            Some(HttpHeader {
                name: name.as_str().to_string(),
                value: value.to_str().ok()?.to_string(),
            })
        })
        .collect()
}

fn next_request_id(prefix: &str) -> String {
    static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);
    let id = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{id}")
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use anyhow::Result;
    use codex_exec_server::ExecServerError;
    use codex_exec_server::HttpRequestResponse;
    use futures::FutureExt;
    use futures::future::BoxFuture;
    use oauth2::http::Method as OAuthMethod;
    use oauth2::http::Uri;
    use oauth2::http::header::CONTENT_TYPE;
    use pretty_assertions::assert_eq;

    use super::*;

    #[derive(Clone, Default)]
    struct TestHttpClient {
        requests: Arc<Mutex<Vec<HttpRequestParams>>>,
        responses: Arc<Mutex<Vec<Result<HttpRequestResponse, ExecServerError>>>>,
    }

    impl TestHttpClient {
        fn push_response(&self, response: Result<HttpRequestResponse, ExecServerError>) {
            self.responses.lock().unwrap().push(response);
        }

        fn requests(&self) -> Vec<HttpRequestParams> {
            self.requests.lock().unwrap().clone()
        }
    }

    impl HttpClient for TestHttpClient {
        fn http_request(
            &self,
            params: HttpRequestParams,
        ) -> BoxFuture<'_, Result<HttpRequestResponse, ExecServerError>> {
            let requests = Arc::clone(&self.requests);
            let responses = Arc::clone(&self.responses);
            async move {
                requests.lock().unwrap().push(params);
                responses.lock().unwrap().remove(0)
            }
            .boxed()
        }

        fn http_request_stream(
            &self,
            _params: HttpRequestParams,
        ) -> BoxFuture<
            '_,
            Result<
                (
                    codex_exec_server::HttpRequestResponse,
                    codex_exec_server::HttpResponseBodyStream,
                ),
                ExecServerError,
            >,
        > {
            async move { Err(ExecServerError::Protocol("unused".to_string())) }.boxed()
        }
    }

    #[tokio::test]
    async fn oauth_http_adapter_uses_http_client() -> Result<()> {
        let http_client = TestHttpClient::default();
        http_client.push_response(Ok(HttpRequestResponse {
            status: reqwest::StatusCode::OK.as_u16(),
            headers: vec![HttpHeader {
                name: "content-type".to_string(),
                value: "application/json".to_string(),
            }],
            body: br#"{"ok":true}"#.to_vec().into(),
        }));

        let adapter = HttpClientOAuthAdapter::new(Arc::new(http_client.clone()), HeaderMap::new());
        let mut request = HttpRequest::new(b"grant_type=refresh_token".to_vec());
        *request.method_mut() = OAuthMethod::POST;
        *request.uri_mut() = "https://example.com/token"
            .parse::<Uri>()
            .map_err(|error| anyhow!(error))?;
        request.headers_mut().insert(
            CONTENT_TYPE,
            OAuthHeaderValue::from_static("application/x-www-form-urlencoded"),
        );

        let response = adapter.execute(request).await?;

        assert_eq!(response.status(), reqwest::StatusCode::OK);
        assert_eq!(response.body().as_slice(), br#"{"ok":true}"#);
        assert_eq!(http_client.requests().len(), 1);
        assert_eq!(http_client.requests()[0].url, "https://example.com/token");
        assert_eq!(
            String::from_utf8(http_client.requests()[0].body.clone().unwrap().into_inner())?,
            "grant_type=refresh_token"
        );
        Ok(())
    }
}
