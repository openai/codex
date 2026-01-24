//! HTTP client for the OpenAI API.

use std::collections::HashMap;
use std::time::Duration;

use bytes::Bytes;
use futures::stream::Stream;
use reqwest::header::AUTHORIZATION;
use reqwest::header::CONTENT_TYPE;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use serde::de::DeserializeOwned;

use crate::config::ClientConfig;
use crate::config::HttpRequest;
use crate::error::OpenAIError;
use crate::error::Result;
use crate::resources::Embeddings;
use crate::resources::Responses;
use crate::types::Response;
use crate::types::SdkHttpResponse;

/// Environment variable for API key.
const API_KEY_ENV: &str = "OPENAI_API_KEY";

/// The OpenAI API client.
#[derive(Debug, Clone)]
pub struct Client {
    http_client: reqwest::Client,
    config: ClientConfig,
}

impl Client {
    /// Create a new client with the given configuration.
    pub fn new(config: ClientConfig) -> Result<Self> {
        if config.api_key.is_empty() {
            return Err(OpenAIError::Configuration(
                "API key is required".to_string(),
            ));
        }

        let http_client = reqwest::Client::builder().timeout(config.timeout).build()?;

        Ok(Self {
            http_client,
            config,
        })
    }

    /// Create a new client using the OPENAI_API_KEY environment variable.
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var(API_KEY_ENV).map_err(|_| {
            OpenAIError::Configuration(format!("Missing {API_KEY_ENV} environment variable"))
        })?;

        Self::new(ClientConfig::new(api_key))
    }

    /// Create a new client with the given API key.
    pub fn with_api_key(api_key: impl Into<String>) -> Result<Self> {
        Self::new(ClientConfig::new(api_key))
    }

    /// Get the responses resource.
    pub fn responses(&self) -> Responses<'_> {
        Responses::new(self)
    }

    /// Get the embeddings resource.
    pub fn embeddings(&self) -> Embeddings<'_> {
        Embeddings::new(self)
    }

    /// Build the default headers for API requests.
    fn default_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.config.api_key))
                .expect("valid api key"),
        );

        // Add optional organization header
        if let Some(org) = &self.config.organization {
            if let Ok(value) = HeaderValue::from_str(org) {
                headers.insert("OpenAI-Organization", value);
            }
        }

        // Add optional project header
        if let Some(project) = &self.config.project {
            if let Ok(value) = HeaderValue::from_str(project) {
                headers.insert("OpenAI-Project", value);
            }
        }

        headers
    }

    /// Apply request hook if configured.
    fn apply_hook(
        &self,
        url: String,
        headers: HeaderMap,
        body: serde_json::Value,
    ) -> (String, HeaderMap, serde_json::Value) {
        if let Some(hook) = &self.config.request_hook {
            // Convert HeaderMap to HashMap for hook
            let header_map: HashMap<String, String> = headers
                .iter()
                .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
                .collect();

            let mut http_request = HttpRequest {
                url,
                headers: header_map,
                body,
            };

            // Call the hook
            hook.on_request(&mut http_request);

            // Convert HashMap back to HeaderMap
            let mut new_headers = HeaderMap::new();
            for (k, v) in http_request.headers {
                if let (Ok(name), Ok(value)) = (
                    reqwest::header::HeaderName::try_from(k.as_str()),
                    HeaderValue::from_str(&v),
                ) {
                    new_headers.insert(name, value);
                }
            }

            (http_request.url, new_headers, http_request.body)
        } else {
            (url, headers, body)
        }
    }

    /// Send a POST request to the API.
    pub(crate) async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: serde_json::Value,
    ) -> Result<T> {
        let base_url = format!("{}{}", self.config.base_url, path);
        let (url, headers, body) = self.apply_hook(base_url, self.default_headers(), body);
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                // Exponential backoff
                let delay = Duration::from_millis(100 * 2_u64.pow(attempt as u32 - 1));
                tokio::time::sleep(delay).await;
            }

            let response = self
                .http_client
                .post(&url)
                .headers(headers.clone())
                .json(&body)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status();
                    let request_id = resp
                        .headers()
                        .get("x-request-id")
                        .and_then(|v| v.to_str().ok())
                        .map(String::from);

                    if status.is_success() {
                        return resp.json::<T>().await.map_err(OpenAIError::from);
                    }

                    // Try to parse error response
                    let error_body = resp.text().await.unwrap_or_default();
                    let error = parse_api_error(status.as_u16(), &error_body, request_id);

                    // Check if retryable
                    if error.is_retryable() && attempt < self.config.max_retries {
                        last_error = Some(error);
                        continue;
                    }

                    return Err(error);
                }
                Err(e) => {
                    let error = OpenAIError::Network(e);
                    if error.is_retryable() && attempt < self.config.max_retries {
                        last_error = Some(error);
                        continue;
                    }
                    return Err(error);
                }
            }
        }

        Err(last_error.expect("at least one error should have occurred"))
    }

    /// Send a GET request that returns Response with sdk_http_response populated.
    pub(crate) async fn get_response(&self, path: &str) -> Result<Response> {
        let base_url = format!("{}{}", self.config.base_url, path);
        let (url, headers, _) =
            self.apply_hook(base_url, self.default_headers(), serde_json::json!({}));
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                // Exponential backoff
                let delay = Duration::from_millis(100 * 2_u64.pow(attempt as u32 - 1));
                tokio::time::sleep(delay).await;
            }

            let response = self
                .http_client
                .get(&url)
                .headers(headers.clone())
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status();
                    let request_id = resp
                        .headers()
                        .get("x-request-id")
                        .and_then(|v| v.to_str().ok())
                        .map(String::from);

                    if status.is_success() {
                        // Capture raw body before deserializing
                        let body_text = resp.text().await.map_err(OpenAIError::from)?;
                        let mut result: Response =
                            serde_json::from_str(&body_text).map_err(|e| {
                                OpenAIError::Parse(format!(
                                    "Failed to parse response: {e}\nBody: {body_text}"
                                ))
                            })?;

                        // Store raw response body for round-trip preservation
                        result.sdk_http_response = Some(SdkHttpResponse::from_status_and_body(
                            status.as_u16() as i32,
                            body_text,
                        ));

                        return Ok(result);
                    }

                    // Try to parse error response
                    let error_body = resp.text().await.unwrap_or_default();
                    let error = parse_api_error(status.as_u16(), &error_body, request_id);

                    // Check if retryable
                    if error.is_retryable() && attempt < self.config.max_retries {
                        last_error = Some(error);
                        continue;
                    }

                    return Err(error);
                }
                Err(e) => {
                    let error = OpenAIError::Network(e);
                    if error.is_retryable() && attempt < self.config.max_retries {
                        last_error = Some(error);
                        continue;
                    }
                    return Err(error);
                }
            }
        }

        Err(last_error.expect("at least one error should have occurred"))
    }

    /// Send a POST request that returns Response with sdk_http_response populated.
    ///
    /// This is a specialized version of `post()` that captures the raw HTTP response
    /// body and stores it in `Response.sdk_http_response` for round-trip preservation.
    pub(crate) async fn post_response(
        &self,
        path: &str,
        body: serde_json::Value,
    ) -> Result<Response> {
        let base_url = format!("{}{}", self.config.base_url, path);
        let (url, headers, body) = self.apply_hook(base_url, self.default_headers(), body);
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                // Exponential backoff
                let delay = Duration::from_millis(100 * 2_u64.pow(attempt as u32 - 1));
                tokio::time::sleep(delay).await;
            }

            let response = self
                .http_client
                .post(&url)
                .headers(headers.clone())
                .json(&body)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status();
                    let request_id = resp
                        .headers()
                        .get("x-request-id")
                        .and_then(|v| v.to_str().ok())
                        .map(String::from);

                    if status.is_success() {
                        // Capture raw body before deserializing
                        let body_text = resp.text().await.map_err(OpenAIError::from)?;
                        let mut result: Response =
                            serde_json::from_str(&body_text).map_err(|e| {
                                OpenAIError::Parse(format!(
                                    "Failed to parse response: {e}\nBody: {body_text}"
                                ))
                            })?;

                        // Store raw response body for round-trip preservation
                        result.sdk_http_response = Some(SdkHttpResponse::from_status_and_body(
                            status.as_u16() as i32,
                            body_text,
                        ));

                        return Ok(result);
                    }

                    // Try to parse error response
                    let error_body = resp.text().await.unwrap_or_default();
                    let error = parse_api_error(status.as_u16(), &error_body, request_id);

                    // Check if retryable
                    if error.is_retryable() && attempt < self.config.max_retries {
                        last_error = Some(error);
                        continue;
                    }

                    return Err(error);
                }
                Err(e) => {
                    let error = OpenAIError::Network(e);
                    if error.is_retryable() && attempt < self.config.max_retries {
                        last_error = Some(error);
                        continue;
                    }
                    return Err(error);
                }
            }
        }

        Err(last_error.expect("at least one error should have occurred"))
    }

    /// Send a POST request that returns a byte stream for SSE processing.
    ///
    /// This method is used for streaming responses. Unlike `post()`, it does not
    /// deserialize the response but instead returns the raw byte stream that can
    /// be processed by the SSE decoder.
    ///
    /// Note: This method does not support retry logic since streaming responses
    /// cannot be easily retried.
    pub(crate) async fn post_stream(
        &self,
        path: &str,
        body: serde_json::Value,
    ) -> Result<impl Stream<Item = std::result::Result<Bytes, reqwest::Error>> + Send + 'static>
    {
        let base_url = format!("{}{}", self.config.base_url, path);
        let (url, headers, body) = self.apply_hook(base_url, self.default_headers(), body);

        let response = self
            .http_client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let request_id = response
                .headers()
                .get("x-request-id")
                .and_then(|v| v.to_str().ok())
                .map(String::from);
            let error_body = response.text().await.unwrap_or_default();
            return Err(parse_api_error(status.as_u16(), &error_body, request_id));
        }

        Ok(response.bytes_stream())
    }

    /// Send a GET request that returns a byte stream for SSE processing.
    ///
    /// This method is used for streaming responses from existing response IDs,
    /// such as when resuming an interrupted stream.
    ///
    /// Note: This method does not support retry logic since streaming responses
    /// cannot be easily retried.
    pub(crate) async fn get_stream(
        &self,
        path: &str,
    ) -> Result<impl Stream<Item = std::result::Result<Bytes, reqwest::Error>> + Send + 'static>
    {
        let base_url = format!("{}{}", self.config.base_url, path);
        let (url, headers, _) =
            self.apply_hook(base_url, self.default_headers(), serde_json::json!({}));

        let response = self.http_client.get(&url).headers(headers).send().await?;

        let status = response.status();
        if !status.is_success() {
            let request_id = response
                .headers()
                .get("x-request-id")
                .and_then(|v| v.to_str().ok())
                .map(String::from);
            let error_body = response.text().await.unwrap_or_default();
            return Err(parse_api_error(status.as_u16(), &error_body, request_id));
        }

        Ok(response.bytes_stream())
    }
}

/// Parse an API error response.
fn parse_api_error(status: u16, body: &str, request_id: Option<String>) -> OpenAIError {
    // Try to parse structured error
    if let Ok(error_response) = serde_json::from_str::<ApiErrorResponse>(body) {
        let message = error_response.error.message;
        let code = error_response.error.code.as_deref().unwrap_or("");

        // Map specific error codes
        if code.contains("context_length_exceeded") {
            return OpenAIError::ContextWindowExceeded;
        }
        if code.contains("insufficient_quota") {
            return OpenAIError::QuotaExceeded;
        }
        if code.contains("previous_response_not_found") {
            return OpenAIError::PreviousResponseNotFound;
        }

        match status {
            400 => OpenAIError::BadRequest(message),
            401 => OpenAIError::Authentication(message),
            429 => OpenAIError::RateLimited { retry_after: None },
            500..=599 => OpenAIError::InternalServerError,
            _ => OpenAIError::Api {
                status,
                message,
                request_id,
            },
        }
    } else {
        OpenAIError::Api {
            status,
            message: body.to_string(),
            request_id,
        }
    }
}

/// API error response structure.
#[derive(Debug, serde::Deserialize)]
struct ApiErrorResponse {
    error: ApiErrorDetail,
}

#[derive(Debug, serde::Deserialize)]
struct ApiErrorDetail {
    #[serde(default)]
    code: Option<String>,
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_requires_api_key() {
        let result = Client::new(ClientConfig::default());
        assert!(matches!(result, Err(OpenAIError::Configuration(_))));
    }

    #[test]
    fn test_client_with_api_key() {
        let result = Client::with_api_key("test-key");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_api_error_structured() {
        let body = r#"{"error":{"code":"invalid_request_error","message":"Invalid model"}}"#;
        let error = parse_api_error(400, body, None);
        assert!(matches!(error, OpenAIError::BadRequest(_)));
    }

    #[test]
    fn test_parse_api_error_rate_limit() {
        let body = r#"{"error":{"code":"rate_limit_error","message":"Rate limited"}}"#;
        let error = parse_api_error(429, body, None);
        assert!(matches!(error, OpenAIError::RateLimited { .. }));
    }

    #[test]
    fn test_parse_api_error_context_exceeded() {
        let body = r#"{"error":{"code":"context_length_exceeded","message":"Context too long"}}"#;
        let error = parse_api_error(400, body, None);
        assert!(matches!(error, OpenAIError::ContextWindowExceeded));
    }

    #[test]
    fn test_parse_api_error_quota_exceeded() {
        let body = r#"{"error":{"code":"insufficient_quota","message":"You exceeded your quota"}}"#;
        let error = parse_api_error(429, body, None);
        assert!(matches!(error, OpenAIError::QuotaExceeded));
    }
}

// ============================================================================
// Integration tests with wiremock
// ============================================================================

#[cfg(test)]
mod integration_tests {
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::header;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    use crate::Client;
    use crate::config::ClientConfig;
    use crate::error::OpenAIError;
    use crate::types::EmbeddingCreateParams;
    use crate::types::InputMessage;
    use crate::types::ResponseCreateParams;
    use crate::types::ResponseStatus;
    use crate::types::Tool;

    fn make_client(base_url: &str) -> Client {
        let config = ClientConfig::new("test-api-key").base_url(base_url);
        Client::new(config).expect("client creation should succeed")
    }

    #[tokio::test]
    async fn test_responses_create_success() {
        let mock_server = MockServer::start().await;

        let response_json = serde_json::json!({
            "id": "resp-123",
            "status": "completed",
            "output": [
                {
                    "type": "message",
                    "id": "msg-1",
                    "role": "assistant",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "Hello! How can I help?"
                        }
                    ]
                }
            ],
            "usage": {
                "input_tokens": 10,
                "output_tokens": 8,
                "total_tokens": 18
            },
            "model": "gpt-4o"
        });

        Mock::given(method("POST"))
            .and(path("/responses"))
            .and(header("authorization", "Bearer test-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_json))
            .mount(&mock_server)
            .await;

        let client = make_client(&mock_server.uri());
        let params = ResponseCreateParams::new("gpt-4o", vec![InputMessage::user_text("Hello!")]);

        let response = client.responses().create(params).await.unwrap();

        assert_eq!(response.id, "resp-123");
        assert_eq!(response.status, ResponseStatus::Completed);
        assert_eq!(response.text(), "Hello! How can I help?");
        assert_eq!(response.usage.total_tokens, 18);
    }

    #[tokio::test]
    async fn test_responses_create_with_tools() {
        let mock_server = MockServer::start().await;

        let response_json = serde_json::json!({
            "id": "resp-tool-123",
            "status": "completed",
            "output": [
                {
                    "type": "function_call",
                    "id": "fc-1",
                    "call_id": "call-abc",
                    "name": "get_weather",
                    "arguments": "{\"city\":\"London\"}"
                }
            ],
            "usage": {
                "input_tokens": 50,
                "output_tokens": 20,
                "total_tokens": 70
            }
        });

        Mock::given(method("POST"))
            .and(path("/responses"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_json))
            .mount(&mock_server)
            .await;

        let client = make_client(&mock_server.uri());
        let tool = Tool::function(
            "get_weather",
            Some("Get the weather".to_string()),
            serde_json::json!({"type": "object", "properties": {"city": {"type": "string"}}}),
        )
        .unwrap();

        let params = ResponseCreateParams::new("gpt-4o", vec![InputMessage::user_text("Weather?")])
            .tools(vec![tool]);

        let response = client.responses().create(params).await.unwrap();

        assert!(response.has_function_calls());
        let calls = response.function_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, "get_weather");
    }

    #[tokio::test]
    async fn test_responses_retrieve() {
        let mock_server = MockServer::start().await;

        let response_json = serde_json::json!({
            "id": "resp-retrieve-123",
            "status": "completed",
            "output": [
                {
                    "type": "message",
                    "id": "msg-1",
                    "role": "assistant",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "Retrieved response"
                        }
                    ]
                }
            ],
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5,
                "total_tokens": 15
            }
        });

        Mock::given(method("GET"))
            .and(path("/responses/resp-retrieve-123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_json))
            .mount(&mock_server)
            .await;

        let client = make_client(&mock_server.uri());
        let response = client
            .responses()
            .retrieve("resp-retrieve-123")
            .await
            .unwrap();

        assert_eq!(response.id, "resp-retrieve-123");
        assert_eq!(response.text(), "Retrieved response");
    }

    #[tokio::test]
    async fn test_responses_cancel() {
        let mock_server = MockServer::start().await;

        let response_json = serde_json::json!({
            "id": "resp-cancel-123",
            "status": "cancelled",
            "output": [],
            "usage": {
                "input_tokens": 10,
                "output_tokens": 0,
                "total_tokens": 10
            }
        });

        Mock::given(method("POST"))
            .and(path("/responses/resp-cancel-123/cancel"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_json))
            .mount(&mock_server)
            .await;

        let client = make_client(&mock_server.uri());
        let response = client.responses().cancel("resp-cancel-123").await.unwrap();

        assert_eq!(response.id, "resp-cancel-123");
        assert_eq!(response.status, ResponseStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_embeddings_create() {
        let mock_server = MockServer::start().await;

        let response_json = serde_json::json!({
            "object": "list",
            "model": "text-embedding-3-small",
            "data": [
                {
                    "object": "embedding",
                    "index": 0,
                    "embedding": [0.1, 0.2, 0.3, 0.4, 0.5]
                }
            ],
            "usage": {
                "prompt_tokens": 5,
                "total_tokens": 5
            }
        });

        Mock::given(method("POST"))
            .and(path("/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_json))
            .mount(&mock_server)
            .await;

        let client = make_client(&mock_server.uri());
        let params = EmbeddingCreateParams::new("text-embedding-3-small", "Hello, world!");

        let response = client.embeddings().create(params).await.unwrap();

        assert_eq!(response.model, "text-embedding-3-small");
        assert_eq!(response.data.len(), 1);
        assert_eq!(response.embedding().unwrap().len(), 5);
        assert_eq!(response.dimensions(), Some(5));
    }

    #[tokio::test]
    async fn test_embeddings_multiple_inputs() {
        let mock_server = MockServer::start().await;

        let response_json = serde_json::json!({
            "object": "list",
            "model": "text-embedding-3-small",
            "data": [
                {
                    "object": "embedding",
                    "index": 0,
                    "embedding": [0.1, 0.2, 0.3]
                },
                {
                    "object": "embedding",
                    "index": 1,
                    "embedding": [0.4, 0.5, 0.6]
                }
            ],
            "usage": {
                "prompt_tokens": 10,
                "total_tokens": 10
            }
        });

        Mock::given(method("POST"))
            .and(path("/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_json))
            .mount(&mock_server)
            .await;

        let client = make_client(&mock_server.uri());
        let params = EmbeddingCreateParams::new(
            "text-embedding-3-small",
            vec!["Hello".to_string(), "World".to_string()],
        );

        let response = client.embeddings().create(params).await.unwrap();

        assert_eq!(response.data.len(), 2);
        assert_eq!(response.embeddings().len(), 2);
    }

    // ========================================================================
    // Error handling tests
    // ========================================================================

    #[tokio::test]
    async fn test_rate_limit_error() {
        let mock_server = MockServer::start().await;

        let error_json = serde_json::json!({
            "error": {
                "code": "rate_limit_error",
                "message": "Rate limit exceeded"
            }
        });

        Mock::given(method("POST"))
            .and(path("/responses"))
            .respond_with(ResponseTemplate::new(429).set_body_json(&error_json))
            .mount(&mock_server)
            .await;

        let client = make_client(&mock_server.uri());
        let params = ResponseCreateParams::with_text("gpt-4o", "Hello");

        let result = client.responses().create(params).await;

        assert!(matches!(result, Err(OpenAIError::RateLimited { .. })));
    }

    #[tokio::test]
    async fn test_context_window_exceeded_error() {
        let mock_server = MockServer::start().await;

        let error_json = serde_json::json!({
            "error": {
                "code": "context_length_exceeded",
                "message": "Context length exceeded"
            }
        });

        Mock::given(method("POST"))
            .and(path("/responses"))
            .respond_with(ResponseTemplate::new(400).set_body_json(&error_json))
            .mount(&mock_server)
            .await;

        let client = make_client(&mock_server.uri());
        let params = ResponseCreateParams::with_text("gpt-4o", "Hello");

        let result = client.responses().create(params).await;

        assert!(matches!(result, Err(OpenAIError::ContextWindowExceeded)));
    }

    #[tokio::test]
    async fn test_authentication_error() {
        let mock_server = MockServer::start().await;

        let error_json = serde_json::json!({
            "error": {
                "code": "invalid_api_key",
                "message": "Invalid API key"
            }
        });

        Mock::given(method("POST"))
            .and(path("/responses"))
            .respond_with(ResponseTemplate::new(401).set_body_json(&error_json))
            .mount(&mock_server)
            .await;

        let client = make_client(&mock_server.uri());
        let params = ResponseCreateParams::with_text("gpt-4o", "Hello");

        let result = client.responses().create(params).await;

        assert!(matches!(result, Err(OpenAIError::Authentication(_))));
    }

    #[tokio::test]
    async fn test_internal_server_error() {
        let mock_server = MockServer::start().await;

        let error_json = serde_json::json!({
            "error": {
                "code": "server_error",
                "message": "Internal server error"
            }
        });

        // Server returns 500 on all 3 attempts (max_retries = 2 means 3 total attempts)
        Mock::given(method("POST"))
            .and(path("/responses"))
            .respond_with(ResponseTemplate::new(500).set_body_json(&error_json))
            .expect(3) // Expects 3 calls due to retry
            .mount(&mock_server)
            .await;

        let client = make_client(&mock_server.uri());
        let params = ResponseCreateParams::with_text("gpt-4o", "Hello");

        let result = client.responses().create(params).await;

        assert!(matches!(result, Err(OpenAIError::InternalServerError)));
    }

    #[tokio::test]
    async fn test_quota_exceeded_error() {
        let mock_server = MockServer::start().await;

        let error_json = serde_json::json!({
            "error": {
                "code": "insufficient_quota",
                "message": "You exceeded your quota"
            }
        });

        Mock::given(method("POST"))
            .and(path("/responses"))
            .respond_with(ResponseTemplate::new(429).set_body_json(&error_json))
            .mount(&mock_server)
            .await;

        let client = make_client(&mock_server.uri());
        let params = ResponseCreateParams::with_text("gpt-4o", "Hello");

        let result = client.responses().create(params).await;

        assert!(matches!(result, Err(OpenAIError::QuotaExceeded)));
    }
}
