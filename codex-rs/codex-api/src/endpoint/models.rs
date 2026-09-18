use crate::auth::SharedAuthProvider;
use crate::endpoint::session::EndpointSession;
use crate::error::ApiError;
use crate::provider::Provider;
use bytes::Bytes;
use codex_client::HttpTransport;
use codex_client::RequestTelemetry;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelsResponse;
use http::HeaderMap;
use http::Method;
use http::header::ETAG;
use std::sync::Arc;
use url::Url;

pub struct ModelsClient<T: HttpTransport> {
    session: EndpointSession<T>,
}

impl<T: HttpTransport> ModelsClient<T> {
    pub fn new(transport: T, provider: Provider, auth: SharedAuthProvider) -> Self {
        Self {
            session: EndpointSession::new(transport, provider, auth),
        }
    }

    pub fn with_telemetry(self, request: Option<Arc<dyn RequestTelemetry>>) -> Self {
        Self {
            session: self.session.with_request_telemetry(request),
        }
    }

    fn path() -> &'static str {
        "models"
    }

    fn append_client_version_query(req: &mut codex_client::Request, client_version: &str) {
        let separator = if req.url.contains('?') { '&' } else { '?' };
        req.url = format!("{}{}client_version={client_version}", req.url, separator);
    }

    pub fn request_url(provider: &Provider, client_version: &str) -> String {
        let mut request = provider.build_request(Method::GET, Self::path());
        Self::append_client_version_query(&mut request, client_version);
        request.url
    }

    /// Builds a full catalog URL, preserving provider routing parameters and client version.
    pub fn catalog_request_url(
        provider: &Provider,
        catalog_url: &str,
        client_version: &str,
    ) -> Result<String, ApiError> {
        let invalid = |message: &str| ApiError::InvalidRequest {
            message: message.to_string(),
        };
        let mut url = Url::parse(catalog_url)
            .map_err(|_| invalid("model_catalog_url must be an absolute URL"))?;
        {
            let mut query = url.query_pairs_mut();
            if let Some(params) = &provider.query_params {
                query.extend_pairs(params);
            }
            query.append_pair("client_version", client_version);
        }
        Ok(url.into())
    }

    /// Fetches and decodes a catalog, optionally bounding response bytes before decoding.
    pub async fn list_models(
        &self,
        request_url: String,
        extra_headers: HeaderMap,
        response_body_limit_bytes: Option<usize>,
    ) -> Result<(Vec<ModelInfo>, Option<String>), ApiError> {
        let (body, header_etag) = self
            .list_models_raw(request_url, extra_headers, response_body_limit_bytes)
            .await?;
        let ModelsResponse { models } =
            serde_json::from_slice::<ModelsResponse>(&body).map_err(|e| {
                ApiError::Stream(format!(
                    "failed to decode models response: {:?} at line {} column {} (body: {} bytes)",
                    e.classify(),
                    e.line(),
                    e.column(),
                    body.len()
                ))
            })?;

        Ok((models, header_etag))
    }

    /// Fetches a catalog using the provider's auth and retry policy without decoding it.
    ///
    /// Callers accepting provider-controlled catalogs must set a response-body limit
    /// and validate the native response without including raw values in diagnostics.
    pub async fn list_models_raw(
        &self,
        request_url: String,
        extra_headers: HeaderMap,
        response_body_limit_bytes: Option<usize>,
    ) -> Result<(Bytes, Option<String>), ApiError> {
        let resp = self
            .session
            .execute_with(
                Method::GET,
                Self::path(),
                extra_headers,
                /*body*/ None,
                move |req| {
                    req.url.clone_from(&request_url);
                    req.response_body_limit_bytes = response_body_limit_bytes;
                },
            )
            .await?;

        let header_etag = resp
            .headers
            .get(ETAG)
            .and_then(|value| value.to_str().ok())
            .map(ToString::to_string);

        Ok((resp.body, header_etag))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthError;
    use crate::auth::AuthProvider;
    use crate::auth::AuthProviderFuture;
    use crate::provider::RetryConfig;
    use codex_client::Request;
    use codex_client::Response;
    use codex_client::StreamResponse;
    use codex_client::TransportError;
    use http::HeaderMap;
    use http::StatusCode;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::time::Duration;

    #[derive(Clone)]
    struct CapturingTransport {
        last_request: Arc<Mutex<Option<Request>>>,
        body: Arc<ModelsResponse>,
        etag: Option<String>,
    }

    impl Default for CapturingTransport {
        fn default() -> Self {
            Self {
                last_request: Arc::new(Mutex::new(None)),
                body: Arc::new(ModelsResponse { models: Vec::new() }),
                etag: None,
            }
        }
    }

    impl HttpTransport for CapturingTransport {
        async fn execute(&self, req: Request) -> Result<Response, TransportError> {
            *self.last_request.lock().unwrap() = Some(req);
            let body = serde_json::to_vec(&*self.body).unwrap();
            let mut headers = HeaderMap::new();
            if let Some(etag) = &self.etag {
                headers.insert(ETAG, etag.parse().unwrap());
            }
            Ok(Response {
                status: StatusCode::OK,
                headers,
                body: body.into(),
            })
        }

        async fn stream(&self, _req: Request) -> Result<StreamResponse, TransportError> {
            Err(TransportError::Build("stream should not run".to_string()))
        }
    }

    #[derive(Clone, Default)]
    struct DummyAuth;

    impl AuthProvider for DummyAuth {
        fn add_auth_headers(&self, _headers: &mut HeaderMap) {}
    }

    #[derive(Default)]
    struct RetryAuth {
        request_limits: Mutex<Vec<Option<usize>>>,
    }

    impl AuthProvider for RetryAuth {
        fn add_auth_headers(&self, headers: &mut HeaderMap) {
            headers.insert(http::header::AUTHORIZATION, "Bearer test".parse().unwrap());
        }

        fn apply_auth(&self, mut request: Request) -> AuthProviderFuture<'_> {
            Box::pin(async move {
                let mut limits = self.request_limits.lock().unwrap();
                limits.push(request.response_body_limit_bytes);
                if limits.len() == 1 {
                    return Err(AuthError::Transient("retry authentication".to_string()));
                }
                self.add_auth_headers(&mut request.headers);
                Ok(request)
            })
        }
    }

    fn provider(base_url: &str) -> Provider {
        Provider {
            name: "test".to_string(),
            base_url: base_url.to_string(),
            query_params: None,
            headers: HeaderMap::new(),
            retry: RetryConfig {
                max_attempts: 1,
                base_delay: Duration::from_millis(1),
                retry_429: false,
                retry_5xx: true,
                retry_transport: true,
            },
            stream_idle_timeout: Duration::from_secs(1),
        }
    }

    #[test]
    fn catalog_request_url_preserves_routing_and_encodes_queries() {
        let mut provider = provider("https://gateway.example/v1");
        provider.query_params = Some(std::collections::HashMap::from([(
            "api-version".to_string(),
            "2026 09".to_string(),
        )]));
        let url = ModelsClient::<CapturingTransport>::catalog_request_url(
            &provider,
            "https://catalog.example/codex/models?deployment=one",
            "1.2.3",
        )
        .unwrap();
        assert_eq!(
            url,
            "https://catalog.example/codex/models?deployment=one&api-version=2026+09&client_version=1.2.3"
        );
    }

    #[test]
    fn catalog_request_url_rejects_relative_urls() {
        let provider = provider("https://gateway.example/v1");
        for catalog_url in ["/codex/models", "codex/models"] {
            let error = ModelsClient::<CapturingTransport>::catalog_request_url(
                &provider,
                catalog_url,
                "1.2.3",
            )
            .unwrap_err();
            assert!(matches!(error, ApiError::InvalidRequest { .. }));
        }
    }

    #[tokio::test]
    async fn response_body_limit_survives_auth_retry_without_limiting_other_models_requests() {
        let transport = CapturingTransport::default();
        let auth = Arc::new(RetryAuth::default());
        let mut provider = provider("https://example.com/api/codex");
        provider.retry.max_attempts = 2;
        let request_url = ModelsClient::<CapturingTransport>::request_url(&provider, "0.99.0");
        let client = ModelsClient::new(transport.clone(), provider, auth.clone());

        client
            .list_models(
                request_url.clone(),
                HeaderMap::new(),
                /*response_body_limit_bytes*/ Some(64),
            )
            .await
            .expect("limited request should succeed after auth retry");
        {
            let request = transport.last_request.lock().unwrap();
            let request = request.as_ref().unwrap();
            assert_eq!(request.response_body_limit_bytes, Some(64));
            assert_eq!(request.url, request_url);
            assert_eq!(request.headers[http::header::AUTHORIZATION], "Bearer test");
        }

        let (models, _) = client
            .list_models(
                request_url,
                HeaderMap::new(),
                /*response_body_limit_bytes*/ None,
            )
            .await
            .expect("ordinary request on the same client should remain unbounded");
        assert!(models.is_empty());
        assert_eq!(
            transport
                .last_request
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .response_body_limit_bytes,
            None
        );
        assert_eq!(
            *auth.request_limits.lock().unwrap(),
            vec![Some(64), Some(64), None]
        );
    }

    #[tokio::test]
    async fn appends_client_version_query() {
        let response = ModelsResponse { models: Vec::new() };

        let transport = CapturingTransport {
            last_request: Arc::new(Mutex::new(None)),
            body: Arc::new(response),
            etag: None,
        };

        let provider = provider("https://example.com/api/codex");
        let request_url = ModelsClient::<CapturingTransport>::request_url(&provider, "0.99.0");
        let client = ModelsClient::new(transport.clone(), provider, Arc::new(DummyAuth));

        let (models, _) = client
            .list_models(
                request_url,
                HeaderMap::new(),
                /*response_body_limit_bytes*/ None,
            )
            .await
            .expect("request should succeed");

        assert_eq!(models.len(), 0);

        let url = transport
            .last_request
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .url
            .clone();
        assert_eq!(
            url,
            "https://example.com/api/codex/models?client_version=0.99.0"
        );
    }

    #[tokio::test]
    async fn parses_models_response() {
        let response = ModelsResponse {
            models: vec![
                serde_json::from_value(json!({
                    "slug": "gpt-test",
                    "display_name": "gpt-test",
                    "description": "desc",
                    "default_reasoning_level": "medium",
                    "supported_reasoning_levels": [{"effort": "low", "description": "low"}, {"effort": "medium", "description": "medium"}, {"effort": "high", "description": "high"}],
                    "shell_type": "shell_command",
                    "visibility": "list",
                    "minimal_client_version": [0, 99, 0],
                    "supported_in_api": true,
                    "priority": 1,
                    "upgrade": null,
                    "support_verbosity": false,
                    "default_verbosity": null,
                    "apply_patch_tool_type": null,
                    "truncation_policy": {"mode": "bytes", "limit": 10_000},
                    "supports_image_detail_original": false,
                    "context_window": 272_000,
                    "experimental_supported_tools": [],
                }))
                .unwrap(),
            ],
        };

        let transport = CapturingTransport {
            last_request: Arc::new(Mutex::new(None)),
            body: Arc::new(response),
            etag: None,
        };

        let provider = provider("https://example.com/api/codex");
        let request_url = ModelsClient::<CapturingTransport>::request_url(&provider, "0.99.0");
        let client = ModelsClient::new(transport, provider, Arc::new(DummyAuth));

        let (models, _) = client
            .list_models(
                request_url,
                HeaderMap::new(),
                /*response_body_limit_bytes*/ None,
            )
            .await
            .expect("request should succeed");

        assert_eq!(models.len(), 1);
        assert_eq!(models[0].slug, "gpt-test");
        assert_eq!(models[0].supported_in_api, true);
        assert_eq!(models[0].priority, 1);
    }

    #[tokio::test]
    async fn list_models_includes_etag() {
        let response = ModelsResponse { models: Vec::new() };

        let transport = CapturingTransport {
            last_request: Arc::new(Mutex::new(None)),
            body: Arc::new(response),
            etag: Some("\"abc\"".to_string()),
        };

        let provider = provider("https://example.com/api/codex");
        let request_url = ModelsClient::<CapturingTransport>::request_url(&provider, "0.1.0");
        let client = ModelsClient::new(transport, provider, Arc::new(DummyAuth));

        let (models, etag) = client
            .list_models(
                request_url,
                HeaderMap::new(),
                /*response_body_limit_bytes*/ None,
            )
            .await
            .expect("request should succeed");

        assert_eq!(models.len(), 0);
        assert_eq!(etag, Some("\"abc\"".to_string()));
    }
}
