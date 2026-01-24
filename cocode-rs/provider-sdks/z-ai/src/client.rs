//! HTTP client implementation for Z.AI SDK.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use reqwest::header::ACCEPT;
use reqwest::header::CONTENT_TYPE;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use serde::de::DeserializeOwned;

use crate::config::ClientConfig;
use crate::config::HttpRequest;
use crate::config::RequestHook;
use crate::error::Result;
use crate::error::ZaiError;
use crate::jwt::JwtTokenCache;
use crate::resources::Chat;
use crate::resources::Embeddings;
use crate::types::Completion;
use crate::types::SdkHttpResponse;

/// Environment variable for API key.
const API_KEY_ENV: &str = "ZAI_API_KEY";

/// Base client implementation with HTTP logic.
#[derive(Debug)]
pub(crate) struct BaseClient {
    http_client: reqwest::Client,
    config: ClientConfig,
    jwt_cache: Option<Arc<JwtTokenCache>>,
    request_hook: Option<Arc<dyn RequestHook>>,
}

impl BaseClient {
    fn new(config: ClientConfig) -> Result<Self> {
        if config.api_key.is_empty() {
            return Err(ZaiError::Configuration("API key is required".into()));
        }

        let jwt_cache = if !config.disable_token_cache {
            Some(Arc::new(JwtTokenCache::new(&config.api_key)?))
        } else {
            None
        };

        let http_client = reqwest::Client::builder().timeout(config.timeout).build()?;
        let request_hook = config.request_hook.clone();

        Ok(Self {
            http_client,
            config,
            jwt_cache,
            request_hook,
        })
    }

    async fn get_auth_header(&self) -> Result<String> {
        if let Some(ref cache) = self.jwt_cache {
            let token = cache.get_token().await?;
            Ok(format!("Bearer {token}"))
        } else {
            Ok(format!("Bearer {}", self.config.api_key))
        }
    }

    fn default_headers(&self, accept_language: Option<&str>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            "Zai-SDK-Ver",
            HeaderValue::from_static(env!("CARGO_PKG_VERSION")),
        );
        headers.insert("source_type", HeaderValue::from_static("z-ai-sdk-rust"));
        headers.insert("x-request-sdk", HeaderValue::from_static("z-ai-sdk-rust"));

        if let Some(ref channel) = self.config.source_channel {
            if let Ok(val) = HeaderValue::from_str(channel) {
                headers.insert("x-source-channel", val);
            }
        } else {
            headers.insert("x-source-channel", HeaderValue::from_static("rust-sdk"));
        }

        if let Some(lang) = accept_language {
            if let Ok(val) = HeaderValue::from_str(lang) {
                headers.insert("Accept-Language", val);
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
        if let Some(hook) = &self.request_hook {
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

    pub(crate) async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: serde_json::Value,
        accept_language: Option<&str>,
    ) -> Result<T> {
        let base_url = format!("{}{path}", self.config.base_url);
        let auth_header = self.get_auth_header().await?;
        let mut base_headers = self.default_headers(accept_language);
        base_headers.insert(
            "Authorization",
            HeaderValue::from_str(&auth_header).expect("valid auth header"),
        );
        let (url, headers, body) = self.apply_hook(base_url, base_headers, body);
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
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
                        return resp.json::<T>().await.map_err(ZaiError::from);
                    }

                    let error_body = resp.text().await.unwrap_or_default();
                    let error = parse_api_error(status.as_u16() as i32, &error_body, request_id);

                    if error.is_retryable() && attempt < self.config.max_retries {
                        last_error = Some(error);
                        continue;
                    }

                    return Err(error);
                }
                Err(e) => {
                    let error = ZaiError::Network(e);
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

    /// Send a POST request and capture HTTP metadata for Completion responses.
    pub(crate) async fn post_completion(
        &self,
        path: &str,
        body: serde_json::Value,
        accept_language: Option<&str>,
    ) -> Result<Completion> {
        let base_url = format!("{}{path}", self.config.base_url);
        let auth_header = self.get_auth_header().await?;
        let mut base_headers = self.default_headers(accept_language);
        base_headers.insert(
            "Authorization",
            HeaderValue::from_str(&auth_header).expect("valid auth header"),
        );
        let (url, headers, body) = self.apply_hook(base_url, base_headers, body);
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
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
                    let status_code = status.as_u16() as i32;
                    let request_id = resp
                        .headers()
                        .get("x-request-id")
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

                    let body_text = resp.text().await.map_err(ZaiError::from)?;

                    if status.is_success() {
                        let mut completion: Completion = serde_json::from_str(&body_text)?;
                        completion.sdk_http_response = Some(SdkHttpResponse::new(
                            status_code,
                            response_headers,
                            body_text,
                        ));
                        return Ok(completion);
                    }

                    let error = parse_api_error(status_code, &body_text, request_id);

                    if error.is_retryable() && attempt < self.config.max_retries {
                        last_error = Some(error);
                        continue;
                    }

                    return Err(error);
                }
                Err(e) => {
                    let error = ZaiError::Network(e);
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

fn parse_api_error(status: i32, body: &str, request_id: Option<String>) -> ZaiError {
    // Try to parse structured error
    if let Ok(error_response) = serde_json::from_str::<ApiErrorResponse>(body) {
        let message = error_response.error.message.unwrap_or_default();

        match status {
            400 => ZaiError::BadRequest(message),
            401 => ZaiError::Authentication(message),
            429 => ZaiError::RateLimited { retry_after: None },
            500 => ZaiError::InternalServerError,
            503 => ZaiError::ServerFlowExceeded,
            _ => ZaiError::Api {
                status,
                message,
                request_id,
            },
        }
    } else {
        match status {
            400 => ZaiError::BadRequest(body.to_string()),
            401 => ZaiError::Authentication(body.to_string()),
            429 => ZaiError::RateLimited { retry_after: None },
            500 => ZaiError::InternalServerError,
            503 => ZaiError::ServerFlowExceeded,
            _ => ZaiError::Api {
                status,
                message: body.to_string(),
                request_id,
            },
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct ApiErrorResponse {
    error: ApiError,
}

#[derive(Debug, serde::Deserialize)]
struct ApiError {
    message: Option<String>,
}

/// Z.AI API client (api.z.ai).
///
/// This client adds Accept-Language: en-US,en header for Z.AI API.
#[derive(Debug, Clone)]
pub struct ZaiClient {
    inner: Arc<BaseClient>,
}

impl ZaiClient {
    /// Create a new Z.AI client with the given configuration.
    pub fn new(config: ClientConfig) -> Result<Self> {
        let mut config = config;
        // Ensure Z.AI base URL
        if config.base_url == ClientConfig::ZHIPUAI_BASE_URL {
            config.base_url = ClientConfig::ZAI_BASE_URL.to_string();
        }
        Ok(Self {
            inner: Arc::new(BaseClient::new(config)?),
        })
    }

    /// Create from environment variable ZAI_API_KEY.
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var(API_KEY_ENV).map_err(|_| {
            ZaiError::Configuration(format!("Missing {API_KEY_ENV} environment variable"))
        })?;
        Self::new(ClientConfig::zai(api_key))
    }

    /// Create with API key.
    pub fn with_api_key(api_key: impl Into<String>) -> Result<Self> {
        Self::new(ClientConfig::zai(api_key))
    }

    /// Get the chat resource.
    pub fn chat(&self) -> Chat<'_> {
        Chat::new(&self.inner, Some("en-US,en"))
    }

    /// Get the embeddings resource.
    pub fn embeddings(&self) -> Embeddings<'_> {
        Embeddings::new(&self.inner, Some("en-US,en"))
    }
}

/// ZhipuAI API client (open.bigmodel.cn).
#[derive(Debug, Clone)]
pub struct ZhipuAiClient {
    inner: Arc<BaseClient>,
}

impl ZhipuAiClient {
    /// Create a new ZhipuAI client with the given configuration.
    pub fn new(config: ClientConfig) -> Result<Self> {
        let mut config = config;
        // Ensure ZhipuAI base URL
        if config.base_url == ClientConfig::ZAI_BASE_URL {
            config.base_url = ClientConfig::ZHIPUAI_BASE_URL.to_string();
        }
        Ok(Self {
            inner: Arc::new(BaseClient::new(config)?),
        })
    }

    /// Create from environment variable ZAI_API_KEY.
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var(API_KEY_ENV).map_err(|_| {
            ZaiError::Configuration(format!("Missing {API_KEY_ENV} environment variable"))
        })?;
        Self::new(ClientConfig::zhipuai(api_key))
    }

    /// Create with API key.
    pub fn with_api_key(api_key: impl Into<String>) -> Result<Self> {
        Self::new(ClientConfig::zhipuai(api_key))
    }

    /// Get the chat resource.
    pub fn chat(&self) -> Chat<'_> {
        Chat::new(&self.inner, None)
    }

    /// Get the embeddings resource.
    pub fn embeddings(&self) -> Embeddings<'_> {
        Embeddings::new(&self.inner, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zai_client_requires_api_key() {
        let result = ZaiClient::new(ClientConfig::zai(""));
        assert!(result.is_err());
    }

    #[test]
    fn test_zhipuai_client_requires_api_key() {
        let result = ZhipuAiClient::new(ClientConfig::zhipuai(""));
        assert!(result.is_err());
    }

    #[test]
    fn test_zai_client_with_api_key() {
        let result = ZaiClient::with_api_key("test-key");
        assert!(result.is_ok());
    }

    #[test]
    fn test_zhipuai_client_with_api_key() {
        let result = ZhipuAiClient::with_api_key("test-key");
        assert!(result.is_ok());
    }
}
