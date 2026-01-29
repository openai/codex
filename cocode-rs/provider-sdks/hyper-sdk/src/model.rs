//! Model trait for AI model instances.

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
///
/// hyper-sdk is a thin network layer - it just makes API calls.
/// Model selection, capability checking, and routing are handled
/// by the upper layer (core/api, config).
#[async_trait]
pub trait Model: Send + Sync + Debug {
    /// Get the model name (e.g., "gpt-4o", "claude-sonnet-4-20250514").
    fn model_name(&self) -> &str;

    /// Get the provider name.
    fn provider(&self) -> &str;

    /// Generate a response (non-streaming).
    #[must_use = "this returns a Result that must be handled"]
    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse, HyperError>;

    /// Generate a streaming response.
    ///
    /// The default implementation returns an error.
    /// Models that support streaming should override this.
    #[must_use = "this returns a Result that must be handled"]
    async fn stream(&self, _request: GenerateRequest) -> Result<StreamResponse, HyperError> {
        Err(HyperError::UnsupportedCapability("streaming".to_string()))
    }

    /// Generate embeddings.
    ///
    /// The default implementation returns an error.
    /// Models that support embeddings should override this.
    #[must_use = "this returns a Result that must be handled"]
    async fn embed(&self, _request: EmbedRequest) -> Result<EmbedResponse, HyperError> {
        Err(HyperError::UnsupportedCapability("embedding".to_string()))
    }

    /// Generate a structured object (non-streaming).
    ///
    /// The default implementation returns an error.
    /// Models that support structured output should override this.
    #[must_use = "this returns a Result that must be handled"]
    async fn generate_object(&self, _request: ObjectRequest) -> Result<ObjectResponse, HyperError> {
        Err(HyperError::UnsupportedCapability(
            "structured_output".to_string(),
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
            "structured_output".to_string(),
        ))
    }
}
