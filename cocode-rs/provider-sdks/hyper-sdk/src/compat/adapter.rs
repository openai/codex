//! HyperAdapter for codex-api compatibility.
//!
//! This adapter bridges hyper-sdk providers to the codex-api ProviderAdapter trait.

use crate::error::HyperError;
use crate::hooks::HookChain;
use crate::hooks::HookContext;
use crate::hooks::RequestHook;
use crate::hooks::ResponseHook;
use crate::hooks::StreamHook;
use crate::model::Model;
use crate::provider::Provider;
use crate::request::GenerateRequest;
use crate::response::GenerateResponse;
use crate::stream::StreamResponse;
use std::fmt::Debug;
use std::sync::Arc;

/// Adapter that bridges hyper-sdk providers to codex-api.
///
/// This allows using hyper-sdk providers with the existing codex-api
/// adapter infrastructure. It also supports hooks for request/response
/// interception.
///
/// # Example
///
/// ```no_run
/// use hyper_sdk::compat::HyperAdapter;
/// use hyper_sdk::hooks::{ResponseIdHook, LoggingHook};
/// use hyper_sdk::{OpenAIProvider, Provider};
/// use std::sync::Arc;
///
/// # fn example() -> hyper_sdk::Result<()> {
/// let provider = OpenAIProvider::from_env()?;
///
/// let mut adapter = HyperAdapter::new(Arc::new(provider))
///     .with_default_model("gpt-4o");
///
/// // Add hooks
/// adapter.add_request_hook(Arc::new(ResponseIdHook));
/// adapter.add_request_hook(Arc::new(LoggingHook::info()));
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct HyperAdapter {
    /// The underlying hyper-sdk provider.
    provider: Arc<dyn Provider>,
    /// Default model ID to use.
    default_model: Option<String>,
    /// Hook chain for intercepting requests/responses.
    hooks: HookChain,
}

impl HyperAdapter {
    /// Create a new HyperAdapter wrapping a provider.
    pub fn new(provider: Arc<dyn Provider>) -> Self {
        Self {
            provider,
            default_model: None,
            hooks: HookChain::new(),
        }
    }

    /// Create a HyperAdapter with pre-configured hooks.
    pub fn with_hooks(provider: Arc<dyn Provider>, hooks: HookChain) -> Self {
        Self {
            provider,
            default_model: None,
            hooks,
        }
    }

    /// Set the default model ID.
    pub fn with_default_model(mut self, model_id: impl Into<String>) -> Self {
        self.default_model = Some(model_id.into());
        self
    }

    /// Get the provider name.
    pub fn name(&self) -> &str {
        self.provider.name()
    }

    /// Get the underlying provider.
    pub fn provider(&self) -> &Arc<dyn Provider> {
        &self.provider
    }

    /// Get a model by ID.
    pub fn model(&self, model_id: &str) -> Result<Arc<dyn Model>, HyperError> {
        self.provider.model(model_id)
    }

    /// Get the default model.
    pub fn default_model(&self) -> Result<Arc<dyn Model>, HyperError> {
        let model_id = self
            .default_model
            .as_deref()
            .ok_or_else(|| HyperError::ConfigError("No default model configured".to_string()))?;
        self.model(model_id)
    }

    /// Get the default model ID if set.
    pub fn default_model_id(&self) -> Option<&str> {
        self.default_model.as_deref()
    }

    /// Add a request hook.
    pub fn add_request_hook(&mut self, hook: Arc<dyn RequestHook>) -> &mut Self {
        self.hooks.add_request_hook(hook);
        self
    }

    /// Add a response hook.
    pub fn add_response_hook(&mut self, hook: Arc<dyn ResponseHook>) -> &mut Self {
        self.hooks.add_response_hook(hook);
        self
    }

    /// Add a stream hook.
    pub fn add_stream_hook(&mut self, hook: Arc<dyn StreamHook>) -> &mut Self {
        self.hooks.add_stream_hook(hook);
        self
    }

    /// Get reference to the hook chain.
    pub fn hooks(&self) -> &HookChain {
        &self.hooks
    }

    /// Get mutable reference to the hook chain.
    pub fn hooks_mut(&mut self) -> &mut HookChain {
        &mut self.hooks
    }

    /// Check if the provider supports previous_response_id for conversation continuity.
    pub fn supports_previous_response_id(&self) -> bool {
        matches!(self.provider.name(), "openai" | "anthropic")
    }

    /// Generate a response with hook processing.
    ///
    /// This method:
    /// 1. Builds hook context
    /// 2. Runs request hooks
    /// 3. Calls the model
    /// 4. Runs response hooks
    pub async fn generate(
        &self,
        model_id: &str,
        mut request: GenerateRequest,
        previous_response_id: Option<&str>,
    ) -> Result<GenerateResponse, HyperError> {
        let model = self.provider.model(model_id)?;

        // Build hook context
        let mut hook_ctx = HookContext::with_provider(self.provider.name(), model_id);
        if let Some(prev_id) = previous_response_id {
            hook_ctx = hook_ctx.previous_response_id(prev_id);
        }

        // Run request hooks
        self.hooks
            .run_request_hooks(&mut request, &mut hook_ctx)
            .await?;

        // Generate
        let mut response = model.generate(request).await?;

        // Run response hooks
        self.hooks
            .run_response_hooks(&mut response, &hook_ctx)
            .await?;

        Ok(response)
    }

    /// Stream a response with hook processing.
    ///
    /// Returns both the stream and the hook context for further processing.
    pub async fn stream(
        &self,
        model_id: &str,
        mut request: GenerateRequest,
        previous_response_id: Option<&str>,
    ) -> Result<(StreamResponse, HookContext), HyperError> {
        let model = self.provider.model(model_id)?;

        // Build hook context
        let mut hook_ctx = HookContext::with_provider(self.provider.name(), model_id);
        if let Some(prev_id) = previous_response_id {
            hook_ctx = hook_ctx.previous_response_id(prev_id);
        }

        // Run request hooks
        self.hooks
            .run_request_hooks(&mut request, &mut hook_ctx)
            .await?;

        // Stream
        let stream = model.stream(request).await?;

        Ok((stream, hook_ctx))
    }

    /// Process stream events through hooks.
    ///
    /// This should be called for each event in the stream if you want
    /// stream hooks to be executed.
    pub async fn process_stream_event(
        &self,
        event: &crate::stream::StreamEvent,
        context: &HookContext,
    ) -> Result<(), HyperError> {
        self.hooks.run_stream_hooks(event, context).await
    }

    /// Run response hooks on a final response.
    ///
    /// This should be called after collecting a streamed response.
    pub async fn process_response(
        &self,
        response: &mut GenerateResponse,
        context: &HookContext,
    ) -> Result<(), HyperError> {
        self.hooks.run_response_hooks(response, context).await
    }
}

impl Clone for HyperAdapter {
    fn clone(&self) -> Self {
        Self {
            provider: self.provider.clone(),
            default_model: self.default_model.clone(),
            hooks: self.hooks.clone(),
        }
    }
}

// Note: To implement codex_api::ProviderAdapter, we would need to add codex-api as a dependency.
// For now, this is a standalone adapter that can be integrated when needed.
//
// The integration would look like:
//
// #[async_trait]
// impl codex_api::ProviderAdapter for HyperAdapter {
//     fn name(&self) -> &str {
//         self.provider.name()
//     }
//
//     async fn generate(
//         &self,
//         prompt: &codex_api::Prompt,
//         config: &codex_api::AdapterConfig,
//         context: &codex_api::AdapterContext,
//     ) -> Result<codex_api::GenerateResult, codex_api::ApiError> {
//         let request = convert::prompt_to_request(prompt, config)?;
//         let response = self.generate(
//             &config.model,
//             request,
//             context.previous_response_id.as_deref(),
//         ).await?;
//         convert::response_to_generate_result(response)
//     }
//
//     async fn stream(
//         &self,
//         prompt: &codex_api::Prompt,
//         config: &codex_api::AdapterConfig,
//         context: &codex_api::AdapterContext,
//     ) -> Result<codex_api::ResponseStream, codex_api::ApiError> {
//         let request = convert::prompt_to_request(prompt, config)?;
//         let (stream, hook_ctx) = self.stream(
//             &config.model,
//             request,
//             context.previous_response_id.as_deref(),
//         ).await?;
//         Ok(convert::wrap_stream_with_hooks(stream, self.hooks.clone(), hook_ctx))
//     }
//
//     fn supports_previous_response_id(&self) -> bool {
//         self.supports_previous_response_id()
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::LoggingHook;
    use crate::hooks::ResponseIdHook;
    use crate::providers::OpenAIProvider;

    #[test]
    fn test_hyper_adapter_creation() {
        let provider = OpenAIProvider::builder()
            .api_key("test-key")
            .build()
            .unwrap();

        let adapter = HyperAdapter::new(Arc::new(provider)).with_default_model("gpt-4o");

        assert_eq!(adapter.name(), "openai");
        assert_eq!(adapter.default_model_id(), Some("gpt-4o"));
        assert!(adapter.supports_previous_response_id());
    }

    #[test]
    fn test_hyper_adapter_with_hooks() {
        let provider = OpenAIProvider::builder()
            .api_key("test-key")
            .build()
            .unwrap();

        let mut adapter = HyperAdapter::new(Arc::new(provider));
        adapter.add_request_hook(Arc::new(ResponseIdHook));
        adapter.add_request_hook(Arc::new(LoggingHook::info()));

        assert!(adapter.hooks().has_request_hooks());
        assert_eq!(adapter.hooks().request_hook_count(), 2);
    }

    #[test]
    fn test_hyper_adapter_clone() {
        let provider = OpenAIProvider::builder()
            .api_key("test-key")
            .build()
            .unwrap();

        let mut adapter = HyperAdapter::new(Arc::new(provider)).with_default_model("gpt-4o");
        adapter.add_request_hook(Arc::new(ResponseIdHook));

        let cloned = adapter.clone();
        assert_eq!(cloned.name(), "openai");
        assert_eq!(cloned.default_model_id(), Some("gpt-4o"));
        assert!(cloned.hooks().has_request_hooks());
    }

    #[test]
    fn test_supports_previous_response_id() {
        let openai = OpenAIProvider::builder()
            .api_key("test-key")
            .build()
            .unwrap();
        let adapter = HyperAdapter::new(Arc::new(openai));
        assert!(adapter.supports_previous_response_id());
    }
}
