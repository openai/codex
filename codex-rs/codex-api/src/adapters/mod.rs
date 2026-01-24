//! Multi-LLM adapter support for codex-api.
//!
//! This module provides a trait-based abstraction for supporting multiple LLM providers
//! beyond the default OpenAI/Anthropic APIs. Adapters convert between codex-api's
//! canonical types (Prompt, ResponseEvent) and provider-specific wire formats.
//!
//! # Architecture
//!
//! The adapter system is used by the endpoint layer (ResponsesClient, ChatClient) to
//! support non-OpenAI providers. When `provider.name` is not "openai" (or empty),
//! the endpoint looks up an adapter from the registry and delegates to it.
//!
//! ```text
//! endpoint layer
//!   └── check provider.name
//!       ├── "openai" / empty → built-in OpenAI format
//!       └── other → get_adapter(name) → adapter.generate()
//! ```

pub mod anthropic;
pub mod genai;
pub mod openai;
pub mod volcengine_ark;
pub mod zai;

use crate::common::Prompt;
use crate::common::ResponseEvent;
use crate::error::ApiError;
use crate::interceptors::InterceptorContext;
use crate::interceptors::apply_interceptors;
use async_trait::async_trait;
use codex_client::Request as ClientRequest;
use codex_protocol::protocol::TokenUsage;
use http::HeaderValue;
use http::Method;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::RwLock;

// ============================================================================
// Request Hook System
// ============================================================================

/// HTTP request information that can be modified by a hook.
///
/// This struct is passed to `RequestHook::on_request()` before the actual
/// HTTP request is sent, allowing interceptors to modify headers, URL, or body.
#[derive(Debug, Clone)]
pub struct HttpRequest {
    /// Request URL.
    pub url: String,
    /// Request headers as key-value pairs.
    pub headers: HashMap<String, String>,
    /// Request body as JSON.
    pub body: serde_json::Value,
}

/// Trait for request hooks that can modify HTTP requests before they are sent.
///
/// This enables codex-api interceptors to be applied to adapter requests,
/// which use their own HTTP clients (provider SDKs).
///
/// # Example
///
/// ```ignore
/// struct MyHook;
///
/// impl RequestHook for MyHook {
///     fn on_request(&self, request: &mut HttpRequest) {
///         request.headers.insert("X-Custom".to_string(), "value".to_string());
///     }
/// }
/// ```
pub trait RequestHook: Send + Sync + Debug {
    /// Called before the HTTP request is sent.
    /// Implementations can modify the request's URL, headers, or body.
    fn on_request(&self, request: &mut HttpRequest);
}

// ============================================================================
// InterceptorHook - Bridges codex-api interceptors to provider SDKs
// ============================================================================

/// A RequestHook implementation that applies codex-api interceptors.
///
/// This bridges the interceptor system to provider SDKs by converting
/// between `HttpRequest` and `codex_client::Request` formats.
#[derive(Debug)]
pub struct InterceptorHook {
    ctx: InterceptorContext,
    interceptor_names: Vec<String>,
}

impl InterceptorHook {
    /// Create a new InterceptorHook.
    pub fn new(ctx: InterceptorContext, interceptor_names: Vec<String>) -> Self {
        Self {
            ctx,
            interceptor_names,
        }
    }
}

impl RequestHook for InterceptorHook {
    fn on_request(&self, request: &mut HttpRequest) {
        // Convert HttpRequest to codex_client::Request
        let mut client_req = ClientRequest::new(Method::POST, request.url.clone());

        // Convert HashMap<String, String> to HeaderMap
        for (k, v) in &request.headers {
            if let (Ok(name), Ok(value)) = (
                http::header::HeaderName::try_from(k.as_str()),
                HeaderValue::from_str(v),
            ) {
                client_req.headers.insert(name, value);
            }
        }
        client_req.body = Some(request.body.clone());

        // Apply interceptors
        apply_interceptors(&mut client_req, &self.ctx, &self.interceptor_names);

        // Convert back to HttpRequest
        request.url = client_req.url;
        request.headers = client_req
            .headers
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
            .collect();
        if let Some(body) = client_req.body {
            request.body = body;
        }
    }
}

/// Build an InterceptorHook if there are interceptors configured.
///
/// Returns `None` if no interceptors are configured.
pub fn build_interceptor_hook(
    ctx: InterceptorContext,
    interceptor_names: &[String],
) -> Option<Arc<dyn RequestHook>> {
    if interceptor_names.is_empty() {
        None
    } else {
        Some(Arc::new(InterceptorHook::new(
            ctx,
            interceptor_names.to_vec(),
        )))
    }
}

/// Configuration for an adapter instance.
#[derive(Default)]
pub struct AdapterConfig {
    /// API key for authentication.
    pub api_key: Option<String>,
    /// Base URL override (if not using default).
    pub base_url: Option<String>,
    /// Model name to use.
    pub model: String,
    /// Additional provider-specific configuration as JSON.
    pub extra: Option<serde_json::Value>,
    /// Optional request hook for interceptor support.
    pub request_hook: Option<Arc<dyn RequestHook>>,
    /// Ultrathink config when ultrathink is active (keyword or toggle).
    /// Adapters use effort (OpenAI/Gemini) or budget_tokens (Claude).
    pub ultrathink_config: Option<crate::common::UltrathinkConfig>,
}

impl Debug for AdapterConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdapterConfig")
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("extra", &self.extra)
            .field("request_hook", &self.request_hook.is_some())
            .finish()
    }
}

impl Clone for AdapterConfig {
    fn clone(&self) -> Self {
        Self {
            api_key: self.api_key.clone(),
            base_url: self.base_url.clone(),
            model: self.model.clone(),
            extra: self.extra.clone(),
            request_hook: self.request_hook.clone(),
            ultrathink_config: self.ultrathink_config.clone(),
        }
    }
}

/// Result of a non-streaming generate call.
#[derive(Debug)]
pub struct GenerateResult {
    /// Response events (OutputItemDone for each response item).
    pub events: Vec<ResponseEvent>,
    /// Token usage statistics.
    pub usage: Option<TokenUsage>,
    /// Response ID for conversation continuity (if supported).
    pub response_id: Option<String>,
}

/// Trait for LLM provider adapters.
///
/// Adapters are responsible for:
/// 1. Converting codex-api's Prompt to provider-specific request format
/// 2. Making the API call (non-streaming only for now)
/// 3. Converting provider response back to ResponseEvent stream
/// 4. Mapping provider errors to ApiError
///
/// # Usage
///
/// Adapters are registered in the global registry at startup:
/// ```ignore
/// register_adapter(Arc::new(GeminiAdapter::new()));
/// ```
///
/// And retrieved by name in the endpoint layer:
/// ```ignore
/// if let Some(adapter) = get_adapter(provider_name) {
///     adapter.generate(prompt, config).await
/// }
/// ```
#[async_trait]
pub trait ProviderAdapter: Send + Sync + std::fmt::Debug {
    /// Unique name identifying this adapter (e.g., "gemini", "claude").
    fn name(&self) -> &str;

    /// Generate a response (non-streaming).
    ///
    /// This is the main entry point for using an adapter. It:
    /// 1. Converts the Prompt to provider format
    /// 2. Makes the API call
    /// 3. Returns ResponseEvents for each output item
    async fn generate(
        &self,
        prompt: &Prompt,
        config: &AdapterConfig,
    ) -> Result<GenerateResult, ApiError>;

    /// Check if this adapter supports conversation continuity via response IDs.
    fn supports_response_id(&self) -> bool {
        false
    }
}

// Re-export as LlmAdapter for clarity (ProviderAdapter is the same thing)
pub use ProviderAdapter as LlmAdapter;

// ============================================================================
// Adapter Registry
// ============================================================================

/// Thread-safe registry for LLM adapters.
#[derive(Debug, Default)]
struct AdapterRegistry {
    adapters: RwLock<HashMap<String, Arc<dyn ProviderAdapter>>>,
}

impl AdapterRegistry {
    fn new() -> Self {
        Self {
            adapters: RwLock::new(HashMap::new()),
        }
    }

    fn register(&self, adapter: Arc<dyn ProviderAdapter>) {
        let name = adapter.name().to_string();
        let mut adapters = self.adapters.write().unwrap();
        adapters.insert(name, adapter);
    }

    fn get(&self, name: &str) -> Option<Arc<dyn ProviderAdapter>> {
        let adapters = self.adapters.read().unwrap();
        adapters.get(name).cloned()
    }

    fn list(&self) -> Vec<String> {
        let adapters = self.adapters.read().unwrap();
        adapters.keys().cloned().collect()
    }
}

/// Global adapter registry with built-in adapters pre-registered.
static ADAPTER_REGISTRY: LazyLock<AdapterRegistry> = LazyLock::new(|| {
    let registry = AdapterRegistry::new();

    // Register built-in adapters
    registry.register(Arc::new(genai::GeminiAdapter::new()));
    registry.register(Arc::new(anthropic::AnthropicAdapter::new()));
    registry.register(Arc::new(volcengine_ark::VolcengineArkAdapter::new()));
    registry.register(Arc::new(zai::ZaiAdapter::new()));
    registry.register(Arc::new(openai::OpenAIAdapter::new()));

    registry
});

/// Get an adapter by name from the global registry.
///
/// Returns `None` if the adapter is not found.
///
/// # Example
/// ```ignore
/// if let Some(adapter) = get_adapter("gemini") {
///     let result = adapter.generate(&prompt, &config).await?;
/// }
/// ```
pub fn get_adapter(name: &str) -> Option<Arc<dyn ProviderAdapter>> {
    ADAPTER_REGISTRY.get(name)
}

/// Register a custom adapter in the global registry.
///
/// If an adapter with the same name already exists, it will be replaced.
pub fn register_adapter(adapter: Arc<dyn ProviderAdapter>) {
    ADAPTER_REGISTRY.register(adapter);
}

/// List all registered adapter names.
pub fn list_adapters() -> Vec<String> {
    ADAPTER_REGISTRY.list()
}

/// Check if a provider name represents the built-in OpenAI provider.
///
/// Returns `true` if the name is "openai", empty, or not specified.
/// The endpoint layer uses this to decide whether to use the built-in
/// OpenAI format or look up an adapter.
pub fn is_openai_provider(name: &str) -> bool {
    name.is_empty() || name.eq_ignore_ascii_case("openai")
}

/// Convert a GenerateResult (from adapter) to a ResponseStream.
///
/// This is used when a non-OpenAI adapter generates a complete response,
/// and we need to convert it to the streaming ResponseStream interface
/// expected by the rest of the codebase.
pub fn generate_result_to_stream(result: GenerateResult) -> crate::common::ResponseStream {
    use tokio::sync::mpsc;

    let (tx, rx) =
        mpsc::channel::<Result<ResponseEvent, crate::error::ApiError>>(result.events.len() + 1);

    tokio::spawn(async move {
        for event in result.events {
            if tx.send(Ok(event)).await.is_err() {
                break;
            }
        }
    });

    crate::common::ResponseStream { rx_event: rx }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_adapters_registered() {
        // GeminiAdapter should be pre-registered
        let adapter = get_adapter("genai");
        assert!(adapter.is_some());
        assert_eq!(adapter.unwrap().name(), "genai");

        // AnthropicAdapter should be pre-registered
        let adapter = get_adapter("anthropic");
        assert!(adapter.is_some());
        assert_eq!(adapter.unwrap().name(), "anthropic");

        // VolcengineArkAdapter should be pre-registered
        let adapter = get_adapter("volc_ark");
        assert!(adapter.is_some());
        assert_eq!(adapter.unwrap().name(), "volc_ark");

        // ZaiAdapter should be pre-registered
        let adapter = get_adapter("zai");
        assert!(adapter.is_some());
        assert_eq!(adapter.unwrap().name(), "zai");

        // OpenAIAdapter should be pre-registered
        let adapter = get_adapter("openai");
        assert!(adapter.is_some());
        assert_eq!(adapter.unwrap().name(), "openai");
    }

    #[test]
    fn test_list_adapters() {
        let adapters = list_adapters();
        assert!(adapters.contains(&"genai".to_string()));
        assert!(adapters.contains(&"anthropic".to_string()));
        assert!(adapters.contains(&"volc_ark".to_string()));
        assert!(adapters.contains(&"zai".to_string()));
        assert!(adapters.contains(&"openai".to_string()));
    }

    #[test]
    fn test_is_openai_provider() {
        assert!(is_openai_provider(""));
        assert!(is_openai_provider("openai"));
        assert!(is_openai_provider("OpenAI"));
        assert!(is_openai_provider("OPENAI"));
        assert!(!is_openai_provider("gemini"));
        assert!(!is_openai_provider("genai"));
    }

    #[test]
    fn test_get_unknown_adapter() {
        let adapter = get_adapter("unknown");
        assert!(adapter.is_none());
    }
}
