use std::collections::HashMap;
use std::time::Duration;

use reqwest::header::CONTENT_TYPE;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;

use crate::config::ClientConfig;
use crate::config::HttpRequest;
use crate::error::AnthropicError;
use crate::error::Result;
use crate::resources::Messages;
use crate::streaming::EventStream;
use crate::streaming::parse_sse_stream;
use crate::types::Message;
use crate::types::SdkHttpResponse;

/// API version header value.
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Environment variable for API key.
const API_KEY_ENV: &str = "ANTHROPIC_API_KEY";

/// The Anthropic API client.
#[derive(Debug, Clone)]
pub struct Client {
    http_client: reqwest::Client,
    config: ClientConfig,
}

impl Client {
    /// Create a new client with the given configuration.
    pub fn new(config: ClientConfig) -> Result<Self> {
        if config.api_key.is_empty() {
            return Err(AnthropicError::Configuration(
                "API key is required".to_string(),
            ));
        }

        let http_client = reqwest::Client::builder().timeout(config.timeout).build()?;

        Ok(Self {
            http_client,
            config,
        })
    }

    /// Create a new client using the ANTHROPIC_API_KEY environment variable.
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var(API_KEY_ENV).map_err(|_| {
            AnthropicError::Configuration(format!("Missing {API_KEY_ENV} environment variable"))
        })?;

        Self::new(ClientConfig::new(api_key))
    }

    /// Create a new client with the given API key.
    pub fn with_api_key(api_key: impl Into<String>) -> Result<Self> {
        Self::new(ClientConfig::new(api_key))
    }

    /// Get the messages resource.
    pub fn messages(&self) -> Messages<'_> {
        Messages::new(self)
    }

    /// Build the default headers for API requests.
    fn default_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            "x-api-key",
            HeaderValue::from_str(&self.config.api_key).expect("valid api key"),
        );
        headers.insert(
            "anthropic-version",
            HeaderValue::from_static(ANTHROPIC_VERSION),
        );
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
    pub(crate) async fn post<T: serde::de::DeserializeOwned>(
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
                let delay = Duration::from_millis(100 * 2_u64.pow(attempt - 1));
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
                        .get("request-id")
                        .and_then(|v| v.to_str().ok())
                        .map(String::from);

                    if status.is_success() {
                        return resp.json::<T>().await.map_err(AnthropicError::from);
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
                    let error = AnthropicError::Network(e);
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

    /// Send a POST request and capture HTTP metadata for Message responses.
    pub(crate) async fn post_message(
        &self,
        path: &str,
        body: serde_json::Value,
    ) -> Result<Message> {
        let base_url = format!("{}{}", self.config.base_url, path);
        let (url, headers, body) = self.apply_hook(base_url, self.default_headers(), body);
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                let delay = Duration::from_millis(100 * 2_u64.pow(attempt - 1));
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
                    let status_code = status.as_u16() as i32;
                    let request_id = resp
                        .headers()
                        .get("request-id")
                        .and_then(|v| v.to_str().ok())
                        .map(String::from);

                    // Capture headers
                    let response_headers: HashMap<String, String> = resp
                        .headers()
                        .iter()
                        .filter_map(|(k, v)| {
                            v.to_str().ok().map(|val| (k.to_string(), val.to_string()))
                        })
                        .collect();

                    let body_text = resp.text().await.map_err(AnthropicError::from)?;

                    if status.is_success() {
                        let mut message: Message = serde_json::from_str(&body_text)?;
                        message.sdk_http_response = Some(SdkHttpResponse::new(
                            status_code,
                            response_headers,
                            body_text,
                        ));
                        return Ok(message);
                    }

                    let error = parse_api_error(status.as_u16(), &body_text, request_id);

                    if error.is_retryable() && attempt < self.config.max_retries {
                        last_error = Some(error);
                        continue;
                    }

                    return Err(error);
                }
                Err(e) => {
                    let error = AnthropicError::Network(e);
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

    /// Send a streaming POST request to the API.
    ///
    /// Returns a stream of raw SSE events. The request body should include
    /// `"stream": true` to enable streaming.
    pub(crate) async fn post_stream(
        &self,
        path: &str,
        body: serde_json::Value,
    ) -> Result<EventStream> {
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
            // Read error body for non-success
            let request_id = response
                .headers()
                .get("request-id")
                .and_then(|v| v.to_str().ok())
                .map(String::from);
            let error_body = response.text().await.unwrap_or_default();
            return Err(parse_api_error(status.as_u16(), &error_body, request_id));
        }

        // Return SSE stream
        Ok(parse_sse_stream(response.bytes_stream()))
    }
}

/// Parse an API error response.
fn parse_api_error(status: u16, body: &str, request_id: Option<String>) -> AnthropicError {
    // Try to parse structured error
    if let Ok(error_response) = serde_json::from_str::<ApiErrorResponse>(body) {
        let message = error_response
            .error
            .message
            .unwrap_or_else(|| error_response.error.error_type.clone());

        match status {
            400 => AnthropicError::BadRequest(message),
            401 => AnthropicError::Authentication(message),
            403 => AnthropicError::PermissionDenied(message),
            404 => AnthropicError::NotFound(message),
            429 => AnthropicError::RateLimited { retry_after: None },
            500..=599 => AnthropicError::InternalServerError,
            _ => AnthropicError::Api {
                status,
                message,
                request_id,
            },
        }
    } else {
        AnthropicError::Api {
            status,
            message: body.to_string(),
            request_id,
        }
    }
}

/// API error response structure.
#[derive(Debug, serde::Deserialize)]
struct ApiErrorResponse {
    error: ApiError,
}

#[derive(Debug, serde::Deserialize)]
struct ApiError {
    #[serde(rename = "type")]
    error_type: String,
    message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_requires_api_key() {
        let result = Client::new(ClientConfig::default());
        assert!(matches!(result, Err(AnthropicError::Configuration(_))));
    }

    #[test]
    fn test_client_with_api_key() {
        let result = Client::with_api_key("test-key");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_api_error_structured() {
        let body = r#"{"error":{"type":"invalid_request_error","message":"Invalid model"}}"#;
        let error = parse_api_error(400, body, None);
        assert!(matches!(error, AnthropicError::BadRequest(_)));
    }

    #[test]
    fn test_parse_api_error_rate_limit() {
        let body = r#"{"error":{"type":"rate_limit_error","message":"Rate limited"}}"#;
        let error = parse_api_error(429, body, None);
        assert!(matches!(error, AnthropicError::RateLimited { .. }));
    }
}
