use crate::auth::SharedAuthProvider;
use crate::endpoint::session::EndpointSession;
use crate::error::ApiError;
use crate::provider::Provider;
use crate::search::SearchRequest;
use crate::search::SearchResponse;
use codex_client::HttpTransport;
use codex_client::RequestTelemetry;
use http::HeaderMap;
use http::Method;
use serde_json::to_value;
use std::sync::Arc;

pub struct SearchClient<T: HttpTransport> {
    session: EndpointSession<T>,
}

impl<T: HttpTransport> SearchClient<T> {
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
        "alpha/search"
    }

    pub async fn search(
        &self,
        request: &SearchRequest,
        extra_headers: HeaderMap,
    ) -> Result<SearchResponse, ApiError> {
        let body = to_value(request)
            .map_err(|e| ApiError::Stream(format!("failed to encode search request: {e}")))?;
        let resp = self
            .session
            .execute(Method::POST, Self::path(), extra_headers, Some(body))
            .await?;
        serde_json::from_slice(&resp.body)
            .map_err(|e| ApiError::Stream(format!("failed to decode search response: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthProvider;
    use crate::provider::RetryConfig;
    use crate::search::AllowedCaller;
    use crate::search::ApproximateLocation;
    use crate::search::ExternalWebAccess;
    use crate::search::LocationType;
    use crate::search::OpenOperation;
    use crate::search::SearchCommands;
    use crate::search::SearchContextSize;
    use crate::search::SearchFilters;
    use crate::search::SearchImageSettings;
    use crate::search::SearchInput;
    use crate::search::SearchQuery;
    use crate::search::SearchSettings;
    use codex_client::Request;
    use codex_client::RequestBody;
    use codex_client::Response;
    use codex_client::StreamResponse;
    use codex_client::TransportError;
    use codex_protocol::ResponseItemId;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseItem;
    use http::StatusCode;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::sync::Mutex;
    use std::time::Duration;

    #[derive(Clone, Default)]
    struct DummyAuth;

    impl AuthProvider for DummyAuth {
        fn add_auth_headers(&self, _headers: &mut HeaderMap) {}
    }

    #[derive(Clone)]
    struct CapturingTransport {
        last_request: Arc<Mutex<Option<Request>>>,
        response_body: Arc<Vec<u8>>,
    }

    impl CapturingTransport {
        fn new(response_body: Vec<u8>) -> Self {
            Self {
                last_request: Arc::new(Mutex::new(None)),
                response_body: Arc::new(response_body),
            }
        }
    }

    impl HttpTransport for CapturingTransport {
        async fn execute(&self, req: Request) -> Result<Response, TransportError> {
            *self.last_request.lock().expect("lock request store") = Some(req);
            Ok(Response {
                status: StatusCode::OK,
                headers: HeaderMap::new(),
                body: self.response_body.as_ref().clone().into(),
            })
        }

        async fn stream(&self, _req: Request) -> Result<StreamResponse, TransportError> {
            Err(TransportError::Build("stream should not run".to_string()))
        }
    }

    fn provider() -> Provider {
        Provider {
            name: "test".to_string(),
            base_url: "https://example.com/v1".to_string(),
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

    #[tokio::test]
    async fn search_posts_typed_request_and_parses_output() {
        let transport = CapturingTransport::new(
            serde_json::to_vec(&json!({
                "encrypted_output": "ciphertext",
                "output": "search result",
                "results": [{
                    "type": "text_result",
                    "ref_id": "turn0search0",
                    "url": "https://example.com/result",
                    "future_field": {"preserved": true},
                }],
            }))
            .expect("serialize response"),
        );
        let client = SearchClient::new(transport.clone(), provider(), Arc::new(DummyAuth));

        let response = client
            .search(
                &SearchRequest {
                    id: "search-session".to_string(),
                    model: "gpt-test".to_string(),
                    reasoning: None,
                    input: Some(SearchInput::Items(vec![ResponseItem::Message {
                        id: Some(ResponseItemId::with_suffix("msg", "search")),
                        role: "user".to_string(),
                        content: vec![
                            ContentItem::InputText {
                                text: "find this".to_string(),
                            },
                            ContentItem::InputImage {
                                image_url: "https://example.com/image.png".to_string(),
                                detail: None,
                            },
                        ],
                        phase: None,
                        internal_chat_message_metadata_passthrough: None,
                    }])),
                    commands: Some(SearchCommands {
                        search_query: Some(vec![SearchQuery {
                            q: "OpenAI news".to_string(),
                            recency: Some(7),
                            domains: Some(vec!["openai.com".to_string()]),
                        }]),
                        open: Some(vec![OpenOperation {
                            ref_id: "https://openai.com".to_string(),
                            lineno: Some(12),
                        }]),
                        ..Default::default()
                    }),
                    settings: Some(SearchSettings {
                        user_location: Some(ApproximateLocation {
                            r#type: LocationType::Approximate,
                            country: Some("US".to_string()),
                            region: None,
                            city: Some("San Francisco".to_string()),
                            timezone: None,
                        }),
                        search_context_size: Some(SearchContextSize::Low),
                        filters: Some(SearchFilters {
                            allowed_domains: Some(vec!["openai.com".to_string()]),
                            blocked_domains: Some(vec!["example.com".to_string()]),
                        }),
                        image_settings: Some(SearchImageSettings {
                            max_results: Some(4),
                            caption: Some(true),
                        }),
                        allowed_callers: Some(vec![AllowedCaller::Direct]),
                        external_web_access: Some(ExternalWebAccess::Boolean(true)),
                    }),
                    max_output_tokens: Some(2500),
                },
                HeaderMap::new(),
            )
            .await
            .expect("search request should succeed");

        assert_eq!(
            response,
            SearchResponse {
                encrypted_output: Some("ciphertext".to_string()),
                output: "search result".to_string(),
                results: Some(vec![json!({
                    "type": "text_result",
                    "ref_id": "turn0search0",
                    "url": "https://example.com/result",
                    "future_field": {"preserved": true},
                })]),
            }
        );

        let request = transport
            .last_request
            .lock()
            .expect("lock request store")
            .clone()
            .expect("request should be captured");
        let body = request
            .body
            .as_ref()
            .and_then(RequestBody::json)
            .expect("request body should be JSON");
        assert_eq!(
            body,
            &json!({
                "id": "search-session",
                "model": "gpt-test",
                "input": [{
                    "type": "message",
                    "id": "msg_search",
                    "role": "user",
                    "content": [
                        {"type": "input_text", "text": "find this"},
                        {
                            "type": "input_image",
                            "image_url": "https://example.com/image.png"
                        }
                    ]
                }],
                "commands": {
                    "search_query": [{
                        "q": "OpenAI news",
                        "recency": 7,
                        "domains": ["openai.com"]
                    }],
                    "open": [{"ref_id": "https://openai.com", "lineno": 12}]
                },
                "settings": {
                    "user_location": {
                        "type": "approximate",
                        "country": "US",
                        "city": "San Francisco"
                    },
                    "search_context_size": "low",
                    "filters": {
                        "allowed_domains": ["openai.com"],
                        "blocked_domains": ["example.com"]
                    },
                    "image_settings": {"max_results": 4, "caption": true},
                    "allowed_callers": ["direct"],
                    "external_web_access": true
                },
                "max_output_tokens": 2500
            })
        );
    }
    #[test]
    fn search_response_defaults_missing_results_for_older_endpoints() {
        let response: SearchResponse = serde_json::from_value(json!({
            "encrypted_output": null,
            "output": "search result",
        }))
        .expect("response without results should deserialize");

        assert_eq!(
            response,
            SearchResponse {
                encrypted_output: None,
                output: "search result".to_string(),
                results: None,
            }
        );
    }

    #[test]
    fn search_response_preserves_supported_empty_results() {
        let response: SearchResponse = serde_json::from_value(json!({
            "encrypted_output": null,
            "output": "search result",
            "results": [],
        }))
        .expect("response with empty results should deserialize");

        assert_eq!(
            response,
            SearchResponse {
                encrypted_output: None,
                output: "search result".to_string(),
                results: Some(Vec::new()),
            }
        );
    }
}
