//! HTTP client for the Volcengine Ark API.

use std::collections::HashMap;
use std::time::Duration;

use reqwest::header::AUTHORIZATION;
use reqwest::header::CONTENT_TYPE;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use serde::de::DeserializeOwned;

use crate::config::ClientConfig;
use crate::config::HttpRequest;
use crate::error::ArkError;
use crate::error::Result;
use crate::resources::Embeddings;
use crate::resources::Responses;
use crate::types::Response;
use crate::types::SdkHttpResponse;

/// Environment variable for API key.
const API_KEY_ENV: &str = "ARK_API_KEY";

/// The Volcengine Ark API client.
#[derive(Debug, Clone)]
pub struct Client {
    http_client: reqwest::Client,
    config: ClientConfig,
}

impl Client {
    /// Create a new client with the given configuration.
    pub fn new(config: ClientConfig) -> Result<Self> {
        if config.api_key.is_empty() {
            return Err(ArkError::Configuration("API key is required".to_string()));
        }

        let http_client = reqwest::Client::builder().timeout(config.timeout).build()?;

        Ok(Self {
            http_client,
            config,
        })
    }

    /// Create a new client using the ARK_API_KEY environment variable.
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var(API_KEY_ENV).map_err(|_| {
            ArkError::Configuration(format!("Missing {API_KEY_ENV} environment variable"))
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
                        return resp.json::<T>().await.map_err(ArkError::from);
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
                    let error = ArkError::Network(e);
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
                        let body_text = resp.text().await.map_err(ArkError::from)?;
                        let mut result: Response =
                            serde_json::from_str(&body_text).map_err(|e| {
                                ArkError::Parse(format!(
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
                    let error = ArkError::Network(e);
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
}

/// Parse an API error response.
fn parse_api_error(status: u16, body: &str, request_id: Option<String>) -> ArkError {
    // Try to parse structured error
    if let Ok(error_response) = serde_json::from_str::<ApiErrorResponse>(body) {
        let message = error_response.error.message;
        let code = error_response.error.code.as_deref().unwrap_or("");

        // Map specific error codes
        if code.contains("context_length_exceeded") {
            return ArkError::ContextWindowExceeded;
        }
        if code.contains("insufficient_quota") {
            return ArkError::QuotaExceeded;
        }
        if code.contains("previous_response_not_found") {
            return ArkError::PreviousResponseNotFound;
        }

        match status {
            400 => ArkError::BadRequest(message),
            401 => ArkError::Authentication(message),
            429 => ArkError::RateLimited { retry_after: None },
            500..=599 => ArkError::InternalServerError,
            _ => ArkError::Api {
                status,
                message,
                request_id,
            },
        }
    } else {
        ArkError::Api {
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
        assert!(matches!(result, Err(ArkError::Configuration(_))));
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
        assert!(matches!(error, ArkError::BadRequest(_)));
    }

    #[test]
    fn test_parse_api_error_rate_limit() {
        let body = r#"{"error":{"code":"rate_limit_error","message":"Rate limited"}}"#;
        let error = parse_api_error(429, body, None);
        assert!(matches!(error, ArkError::RateLimited { .. }));
    }

    #[test]
    fn test_parse_api_error_context_exceeded() {
        let body = r#"{"error":{"code":"context_length_exceeded","message":"Context too long"}}"#;
        let error = parse_api_error(400, body, None);
        assert!(matches!(error, ArkError::ContextWindowExceeded));
    }
}
