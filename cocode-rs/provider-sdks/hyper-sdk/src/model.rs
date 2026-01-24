//! Model trait for AI model instances.

use crate::capability::Capability;
use crate::embedding::EmbedRequest;
use crate::embedding::EmbedResponse;
use crate::error::HyperError;
use crate::object::ObjectRequest;
use crate::object::ObjectResponse;
use crate::object::ObjectStreamResponse;
use crate::request::GenerateRequest;
use crate::response::GenerateResponse;
use crate::stream::StreamResponse;
use async_trait::async_trait;
use std::fmt::Debug;

/// A model instance that can generate text, embeddings, etc.
///
/// Models are created by providers and represent a specific AI model
/// (e.g., "gpt-4o", "claude-3-opus", "gemini-pro").
#[async_trait]
pub trait Model: Send + Sync + Debug {
    /// Get the model ID.
    fn model_id(&self) -> &str;

    /// Get the provider name.
    fn provider(&self) -> &str;

    /// Get the capabilities this model supports.
    fn capabilities(&self) -> &[Capability];

    /// Generate a response (non-streaming).
    #[must_use = "this returns a Result that must be handled"]
    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse, HyperError>;

    /// Generate a streaming response.
    ///
    /// The default implementation returns an error.
    /// Models that support streaming should override this.
    #[must_use = "this returns a Result that must be handled"]
    async fn stream(&self, _request: GenerateRequest) -> Result<StreamResponse, HyperError> {
        // Default: fall back to non-streaming
        if self.has_capability(Capability::Streaming) {
            Err(HyperError::Internal(
                "Streaming not implemented for this model".to_string(),
            ))
        } else {
            Err(HyperError::UnsupportedCapability(Capability::Streaming))
        }
    }

    /// Generate embeddings.
    ///
    /// The default implementation returns an error.
    /// Models that support embeddings should override this.
    #[must_use = "this returns a Result that must be handled"]
    async fn embed(&self, _request: EmbedRequest) -> Result<EmbedResponse, HyperError> {
        Err(HyperError::UnsupportedCapability(Capability::Embedding))
    }

    /// Generate a structured object (non-streaming).
    ///
    /// The default implementation returns an error.
    /// Models that support structured output should override this.
    #[must_use = "this returns a Result that must be handled"]
    async fn generate_object(&self, _request: ObjectRequest) -> Result<ObjectResponse, HyperError> {
        Err(HyperError::UnsupportedCapability(
            Capability::StructuredOutput,
        ))
    }

    /// Generate a structured object with streaming.
    ///
    /// The default implementation returns an error.
    /// Models that support structured output should override this.
    #[must_use = "this returns a Result that must be handled"]
    async fn stream_object(
        &self,
        _request: ObjectRequest,
    ) -> Result<ObjectStreamResponse, HyperError> {
        Err(HyperError::UnsupportedCapability(
            Capability::StructuredOutput,
        ))
    }

    /// Check if this model has a specific capability.
    fn has_capability(&self, capability: Capability) -> bool {
        self.capabilities().contains(&capability)
    }

    /// Check if this model supports streaming.
    fn supports_streaming(&self) -> bool {
        self.has_capability(Capability::Streaming)
    }

    /// Check if this model supports vision/images.
    fn supports_vision(&self) -> bool {
        self.has_capability(Capability::Vision)
    }

    /// Check if this model supports tool calling.
    fn supports_tools(&self) -> bool {
        self.has_capability(Capability::ToolCalling)
    }

    /// Check if this model supports extended thinking.
    fn supports_thinking(&self) -> bool {
        self.has_capability(Capability::ExtendedThinking)
    }

    /// Check if this model supports embeddings.
    fn supports_embedding(&self) -> bool {
        self.has_capability(Capability::Embedding)
    }

    /// Check if this model supports structured output.
    fn supports_structured_output(&self) -> bool {
        self.has_capability(Capability::StructuredOutput)
    }
}

/// A simple wrapper for model metadata without implementation.
///
/// Useful for testing or creating mock models.
#[derive(Debug, Clone)]
pub struct ModelMetadata {
    model_id: String,
    provider: String,
    capabilities: Vec<Capability>,
}

impl ModelMetadata {
    /// Create new model metadata.
    pub fn new(
        model_id: impl Into<String>,
        provider: impl Into<String>,
        capabilities: Vec<Capability>,
    ) -> Self {
        Self {
            model_id: model_id.into(),
            provider: provider.into(),
            capabilities,
        }
    }

    /// Get the model ID.
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    /// Get the provider name.
    pub fn provider(&self) -> &str {
        &self.provider
    }

    /// Get the capabilities.
    pub fn capabilities(&self) -> &[Capability] {
        &self.capabilities
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_metadata() {
        let meta = ModelMetadata::new(
            "gpt-4o",
            "openai",
            vec![
                Capability::TextGeneration,
                Capability::Streaming,
                Capability::Vision,
                Capability::ToolCalling,
            ],
        );

        assert_eq!(meta.model_id(), "gpt-4o");
        assert_eq!(meta.provider(), "openai");
        assert!(meta.capabilities().contains(&Capability::Vision));
    }
}
