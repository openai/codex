//! Adapter support for ChatClient.
//!
//! This module extends `ChatClient` with adapter routing for non-OpenAI providers.
//! When `provider.adapter` is set, it looks up an adapter from the registry and
//! delegates to it.

use crate::adapters::AdapterConfig;
use crate::adapters::generate_result_to_stream;
use crate::adapters::get_adapter;
use crate::adapters::is_openai_provider;
use crate::auth::AuthProvider;
use crate::common::Prompt as ApiPrompt;
use crate::common::ResponseStream;
use crate::endpoint::chat::ChatClient;
use crate::error::ApiError;
use codex_client::HttpTransport;

impl<T: HttpTransport, A: AuthProvider> ChatClient<T, A> {
    /// Try to use an adapter for non-OpenAI providers.
    ///
    /// Returns `Ok(Some(stream))` if an adapter handled the request,
    /// `Ok(None)` if should fall through to built-in OpenAI handling.
    ///
    /// # Arguments
    ///
    /// * `model` - The model name to use
    /// * `prompt` - The prompt containing instructions, input, and tools
    pub(crate) async fn try_adapter(
        &self,
        model: &str,
        prompt: &ApiPrompt,
    ) -> Result<Option<ResponseStream>, ApiError> {
        let provider = self.streaming.provider();

        // Only use adapter if explicitly configured
        let adapter_name = match &provider.adapter {
            Some(name) if !name.is_empty() => name.as_str(),
            _ => return Ok(None), // No adapter configured, use built-in handling
        };

        // OpenAI adapter uses built-in handling
        if is_openai_provider(adapter_name) {
            return Ok(None);
        }

        // Try to find an adapter for this provider
        if let Some(adapter) = get_adapter(adapter_name) {
            let config = AdapterConfig {
                api_key: self.streaming.auth().bearer_token(),
                base_url: Some(provider.base_url.clone()),
                model: model.to_string(),
                extra: provider.model_parameters.clone(),
            };
            let result = adapter.generate(prompt, &config).await?;
            return Ok(Some(generate_result_to_stream(result)));
        }

        // Adapter not found in registry - fall through to built-in handling
        Ok(None)
    }
}
