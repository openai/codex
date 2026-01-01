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

use crate::common::Prompt;
use crate::common::ResponseEvent;
use crate::error::ApiError;
use async_trait::async_trait;
use codex_protocol::protocol::TokenUsage;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::RwLock;

/// Configuration for an adapter instance.
#[derive(Debug, Clone, Default)]
pub struct AdapterConfig {
    /// API key for authentication.
    pub api_key: Option<String>,
    /// Base URL override (if not using default).
    pub base_url: Option<String>,
    /// Model name to use.
    pub model: String,
    /// Additional provider-specific configuration as JSON.
    pub extra: Option<serde_json::Value>,
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
    }

    #[test]
    fn test_list_adapters() {
        let adapters = list_adapters();
        assert!(adapters.contains(&"genai".to_string()));
        assert!(adapters.contains(&"anthropic".to_string()));
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
