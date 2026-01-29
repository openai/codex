//! HyperClient - Main entry point for hyper-sdk.
//!
//! `HyperClient` provides a clean, dependency-injected API that avoids global state.
//! Each client holds its own provider registry, enabling proper test isolation and
//! multi-tenant scenarios.
//!
//! # Example
//!
//! ```no_run
//! use hyper_sdk::{HyperClient, OpenAIProvider, AnthropicProvider, GenerateRequest, Message};
//!
//! # async fn example() -> hyper_sdk::Result<()> {
//! // Method 1: Explicit configuration
//! let client = HyperClient::new()
//!     .with_provider(OpenAIProvider::builder().api_key("sk-xxx").build()?)
//!     .with_provider(AnthropicProvider::builder().api_key("sk-ant-xxx").build()?);
//!
//! let model = client.model("openai", "gpt-4o")?;
//! let response = model.generate(GenerateRequest::from_text("Hello!")).await?;
//!
//! // Method 2: Auto-configure from environment
//! let client = HyperClient::from_env()?;
//! let conversation = client.conversation("anthropic", "claude-sonnet-4-20250514")?;
//! # Ok(())
//! # }
//! ```

use crate::conversation::ConversationContext;
use crate::error::HyperError;
use crate::model::Model;
use crate::provider::Provider;
use crate::providers::AnthropicProvider;
use crate::providers::GeminiProvider;
use crate::providers::OpenAIProvider;
use crate::providers::VolcengineProvider;
use crate::providers::ZaiProvider;
use crate::registry::ProviderRegistry;
use std::sync::Arc;
use tracing::debug;

/// Main client for hyper-sdk, holding its own provider registry.
///
/// Unlike the global registry functions, `HyperClient` provides:
/// - **Test isolation**: Each client has its own registry
/// - **Multi-tenancy**: Different contexts can use different provider sets
/// - **Explicit dependencies**: No hidden global state
///
/// # Example
///
/// ```no_run
/// use hyper_sdk::{HyperClient, OpenAIProvider};
///
/// # fn example() -> hyper_sdk::Result<()> {
/// let client = HyperClient::new()
///     .with_provider(OpenAIProvider::from_env()?);
///
/// let model = client.model("openai", "gpt-4o")?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Default)]
pub struct HyperClient {
    registry: ProviderRegistry,
}

impl HyperClient {
    /// Create a new empty client.
    ///
    /// Use `with_provider()` to add providers, or use `from_env()` for
    /// automatic configuration.
    pub fn new() -> Self {
        Self {
            registry: ProviderRegistry::new(),
        }
    }

    /// Add a provider to this client (builder pattern).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use hyper_sdk::{HyperClient, OpenAIProvider, AnthropicProvider};
    ///
    /// # fn example() -> hyper_sdk::Result<()> {
    /// let client = HyperClient::new()
    ///     .with_provider(OpenAIProvider::from_env()?)
    ///     .with_provider(AnthropicProvider::from_env()?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_provider<P: Provider + 'static>(self, provider: P) -> Self {
        self.registry.register(Arc::new(provider));
        self
    }

    /// Add an Arc-wrapped provider to this client.
    pub fn with_provider_arc(self, provider: Arc<dyn Provider>) -> Self {
        self.registry.register(provider);
        self
    }

    /// Register a provider (mutable method).
    ///
    /// This is useful when you need to add providers after initial construction.
    pub fn register<P: Provider + 'static>(&self, provider: P) {
        self.registry.register(Arc::new(provider));
    }

    /// Register an Arc-wrapped provider (mutable method).
    pub fn register_arc(&self, provider: Arc<dyn Provider>) {
        self.registry.register(provider);
    }

    /// Create a client with all available providers from environment variables.
    ///
    /// This attempts to configure each built-in provider using their standard
    /// environment variables:
    /// - `OPENAI_API_KEY` for OpenAI
    /// - `ANTHROPIC_API_KEY` for Anthropic
    /// - `GOOGLE_API_KEY` or `GEMINI_API_KEY` for Google Gemini
    /// - `ARK_API_KEY` for Volcengine Ark
    /// - `ZAI_API_KEY` or `ZHIPU_API_KEY` for Z.AI
    ///
    /// Providers with missing API keys are silently skipped.
    ///
    /// # Errors
    ///
    /// Returns an error if no providers could be configured.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use hyper_sdk::HyperClient;
    ///
    /// # fn example() -> hyper_sdk::Result<()> {
    /// let client = HyperClient::from_env()?;
    /// println!("Available providers: {:?}", client.list_providers());
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_env() -> Result<Self, HyperError> {
        let client = Self::new();

        // Try each provider, ignoring missing API keys
        if let Ok(p) = OpenAIProvider::from_env() {
            debug!(provider = "openai", "Configured provider from env");
            client.register(p);
        }
        if let Ok(p) = AnthropicProvider::from_env() {
            debug!(provider = "anthropic", "Configured provider from env");
            client.register(p);
        }
        if let Ok(p) = GeminiProvider::from_env() {
            debug!(provider = "gemini", "Configured provider from env");
            client.register(p);
        }
        if let Ok(p) = VolcengineProvider::from_env() {
            debug!(provider = "volcengine", "Configured provider from env");
            client.register(p);
        }
        if let Ok(p) = ZaiProvider::from_env() {
            debug!(provider = "zai", "Configured provider from env");
            client.register(p);
        }

        if client.registry.list().is_empty() {
            return Err(HyperError::ConfigError(
                "No provider could be initialized. Set OPENAI_API_KEY, ANTHROPIC_API_KEY, \
                GOOGLE_API_KEY, ARK_API_KEY, or ZAI_API_KEY."
                    .to_string(),
            ));
        }

        Ok(client)
    }

    /// Get a provider by name.
    ///
    /// Returns `None` if the provider is not registered.
    pub fn provider(&self, name: &str) -> Option<Arc<dyn Provider>> {
        self.registry.get(name)
    }

    /// Get a provider by name, returning an error if not found.
    #[must_use = "this returns a Result that must be handled"]
    pub fn require_provider(&self, name: &str) -> Result<Arc<dyn Provider>, HyperError> {
        self.provider(name)
            .ok_or_else(|| HyperError::ProviderNotFound(name.to_string()))
    }

    /// Get a model instance.
    ///
    /// This is a convenience method that looks up the provider and then
    /// gets the model from it.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use hyper_sdk::{HyperClient, GenerateRequest};
    ///
    /// # async fn example() -> hyper_sdk::Result<()> {
    /// let client = HyperClient::from_env()?;
    /// let model = client.model("openai", "gpt-4o")?;
    /// let response = model.generate(GenerateRequest::from_text("Hello!")).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use = "this returns a Result that must be handled"]
    pub fn model(&self, provider: &str, model_id: &str) -> Result<Arc<dyn Model>, HyperError> {
        self.require_provider(provider)?.model(model_id)
    }

    /// Create a ConversationContext bound to this client's registry.
    ///
    /// The conversation will use the specified provider and model for
    /// its initial configuration.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use hyper_sdk::{HyperClient, GenerateRequest, Message};
    ///
    /// # async fn example() -> hyper_sdk::Result<()> {
    /// let client = HyperClient::from_env()?;
    /// let mut conversation = client.conversation("openai", "gpt-4o")?;
    ///
    /// let model = client.model("openai", "gpt-4o")?;
    /// let response = conversation.generate(
    ///     model.as_ref(),
    ///     GenerateRequest::new(vec![Message::user("Hello!")])
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use = "this returns a Result that must be handled"]
    pub fn conversation(
        &self,
        provider: &str,
        model_id: &str,
    ) -> Result<ConversationContext, HyperError> {
        // Validate that the provider and model exist
        let _ = self.model(provider, model_id)?;

        Ok(ConversationContext::new().with_provider_info(provider, model_id))
    }

    /// List all registered provider names.
    pub fn list_providers(&self) -> Vec<String> {
        self.registry.list()
    }

    /// Check if a provider is registered.
    pub fn has_provider(&self, name: &str) -> bool {
        self.registry.has(name)
    }

    /// Remove a provider by name.
    pub fn remove_provider(&self, name: &str) -> Option<Arc<dyn Provider>> {
        self.registry.remove(name)
    }

    /// Clear all registered providers.
    pub fn clear(&self) {
        self.registry.clear();
    }

    /// Get a reference to the underlying registry.
    ///
    /// This is useful for advanced use cases where you need direct
    /// registry access.
    pub fn registry(&self) -> &ProviderRegistry {
        &self.registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct MockProvider {
        name: String,
    }

    #[async_trait::async_trait]
    impl Provider for MockProvider {
        fn name(&self) -> &str {
            &self.name
        }

        fn model(&self, model_id: &str) -> Result<Arc<dyn Model>, HyperError> {
            Err(HyperError::ModelNotFound(format!(
                "{}:{}",
                self.name, model_id
            )))
        }
    }

    #[test]
    fn test_client_new() {
        let client = HyperClient::new();
        assert!(client.list_providers().is_empty());
    }

    #[test]
    fn test_client_with_provider() {
        let client = HyperClient::new()
            .with_provider(MockProvider {
                name: "test1".to_string(),
            })
            .with_provider(MockProvider {
                name: "test2".to_string(),
            });

        assert!(client.has_provider("test1"));
        assert!(client.has_provider("test2"));
        assert!(!client.has_provider("test3"));
    }

    #[test]
    fn test_client_register() {
        let client = HyperClient::new();
        client.register(MockProvider {
            name: "test".to_string(),
        });

        assert!(client.has_provider("test"));
    }

    #[test]
    fn test_client_provider() {
        let client = HyperClient::new().with_provider(MockProvider {
            name: "test".to_string(),
        });

        assert!(client.provider("test").is_some());
        assert!(client.provider("nonexistent").is_none());
    }

    #[test]
    fn test_client_require_provider() {
        let client = HyperClient::new().with_provider(MockProvider {
            name: "test".to_string(),
        });

        assert!(client.require_provider("test").is_ok());
        assert!(matches!(
            client.require_provider("nonexistent"),
            Err(HyperError::ProviderNotFound(_))
        ));
    }

    #[test]
    fn test_client_model_not_found() {
        let client = HyperClient::new().with_provider(MockProvider {
            name: "test".to_string(),
        });

        // Provider exists but model doesn't
        let result = client.model("test", "gpt-4o");
        assert!(matches!(result, Err(HyperError::ModelNotFound(_))));

        // Provider doesn't exist
        let result = client.model("nonexistent", "gpt-4o");
        assert!(matches!(result, Err(HyperError::ProviderNotFound(_))));
    }

    #[test]
    fn test_client_list_providers() {
        let client = HyperClient::new()
            .with_provider(MockProvider {
                name: "alpha".to_string(),
            })
            .with_provider(MockProvider {
                name: "beta".to_string(),
            });

        let providers = client.list_providers();
        assert_eq!(providers.len(), 2);
        assert!(providers.contains(&"alpha".to_string()));
        assert!(providers.contains(&"beta".to_string()));
    }

    #[test]
    fn test_client_remove_provider() {
        let client = HyperClient::new().with_provider(MockProvider {
            name: "test".to_string(),
        });

        assert!(client.has_provider("test"));
        let removed = client.remove_provider("test");
        assert!(removed.is_some());
        assert!(!client.has_provider("test"));
    }

    #[test]
    fn test_client_clear() {
        let client = HyperClient::new()
            .with_provider(MockProvider {
                name: "test1".to_string(),
            })
            .with_provider(MockProvider {
                name: "test2".to_string(),
            });

        assert_eq!(client.list_providers().len(), 2);
        client.clear();
        assert!(client.list_providers().is_empty());
    }

    #[test]
    fn test_client_conversation() {
        // Can't create conversation for nonexistent provider
        let client = HyperClient::new();
        let result = client.conversation("openai", "gpt-4o");
        assert!(matches!(result, Err(HyperError::ProviderNotFound(_))));
    }
}
