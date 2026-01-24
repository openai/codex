//! Provider implementations.

pub mod anthropic;
pub mod gemini;
pub mod openai;
pub mod openai_compat;
pub mod volcengine;
pub mod zai;

pub use anthropic::AnthropicProvider;
pub use gemini::GeminiProvider;
pub use openai::OpenAIProvider;
pub use openai_compat::OpenAICompatProvider;
pub use volcengine::VolcengineProvider;
pub use zai::ZaiProvider;

use crate::error::HyperError;
use std::sync::Arc;

/// Try to create a provider from environment variables.
///
/// Returns the first provider that can be created, or an error if none can be created.
pub fn any_from_env() -> Result<Arc<dyn crate::provider::Provider>, HyperError> {
    // Try providers in order of preference
    if let Ok(provider) = OpenAIProvider::from_env() {
        return Ok(Arc::new(provider));
    }

    if let Ok(provider) = AnthropicProvider::from_env() {
        return Ok(Arc::new(provider));
    }

    if let Ok(provider) = GeminiProvider::from_env() {
        return Ok(Arc::new(provider));
    }

    if let Ok(provider) = VolcengineProvider::from_env() {
        return Ok(Arc::new(provider));
    }

    if let Ok(provider) = ZaiProvider::from_env() {
        return Ok(Arc::new(provider));
    }

    Err(HyperError::ConfigError(
        "No provider could be initialized. Set OPENAI_API_KEY, ANTHROPIC_API_KEY, GOOGLE_API_KEY, ARK_API_KEY, or ZAI_API_KEY."
            .to_string(),
    ))
}
