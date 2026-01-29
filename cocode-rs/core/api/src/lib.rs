//! cocode-api - Provider abstraction layer for the agent system.
//!
//! This crate wraps hyper-sdk to provide:
//! - Unified streaming abstraction (stream vs non-stream)
//! - Retry logic with exponential backoff
//! - Prompt caching support
//! - Stall detection
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                         cocode-api                              │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  ApiClient         │  UnifiedStream      │  RetryContext       │
//! │  - retry           │  - Streaming mode   │  - backoff          │
//! │  - caching         │  - Non-stream mode  │                     │
//! │                    │  - Event emission   │                     │
//! ├────────────────────┴───────────────────────────────────────────┤
//! │                        hyper-sdk                                │
//! │  HyperClient, Message, StreamProcessor, GenerateRequest, ...   │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Quick Start
//!
//! ```ignore
//! use cocode_api::{ApiClient, StreamOptions};
//! use hyper_sdk::{OpenAIProvider, Provider, GenerateRequest, Message};
//!
//! let provider = OpenAIProvider::from_env()?;
//! let model = provider.model("gpt-4o")?;
//!
//! // Create the API client (model-agnostic)
//! let client = ApiClient::new();
//!
//! // Make a streaming request, passing the model per-call
//! let request = GenerateRequest::new(vec![Message::user("Hello!")]);
//! let mut stream = client.stream_request(&*model, request, StreamOptions::streaming()).await?;
//!
//! // Process results
//! while let Some(result) = stream.next().await {
//!     let result = result?;
//!     if result.has_content() {
//!         // Handle completed content blocks
//!     }
//! }
//! ```
//!
//! # Module Structure
//!
//! - [`error`] - Error types with status codes
//! - [`retry`] - Retry context with backoff
//! - [`unified_stream`] - Unified stream abstraction
//! - [`cache`] - Prompt caching helpers
//! - [`client`] - High-level API client
//! - [`provider_factory`] - Factory for creating providers from ProviderInfo

pub mod cache;
pub mod client;
pub mod error;
pub mod provider_factory;
pub mod retry;
pub mod unified_stream;

// Re-export main types at crate root
pub use cache::{CacheStats, Cacheable, PromptCacheConfig};
pub use client::{ApiClient, ApiClientBuilder, ApiClientConfig, FallbackConfig, StreamOptions};
pub use error::{ApiError, Result};
pub use provider_factory::{create_model, create_provider};
pub use retry::{RetryConfig, RetryContext, RetryDecision};
pub use unified_stream::{CollectedResponse, QueryResultType, StreamingQueryResult, UnifiedStream};

// Re-export commonly used hyper-sdk types for convenience
pub use hyper_sdk::{
    ContentBlock, FinishReason, GenerateRequest, GenerateResponse, Message, Role, StreamProcessor,
    StreamSnapshot, StreamUpdate, TokenUsage, ToolCall, ToolChoice, ToolDefinition,
    ToolResultContent,
};

/// Prelude module for convenient imports.
pub mod prelude {
    pub use crate::cache::PromptCacheConfig;
    pub use crate::client::{ApiClient, StreamOptions};
    pub use crate::error::{ApiError, Result};
    pub use crate::retry::{RetryConfig, RetryContext};
    pub use crate::unified_stream::{StreamingQueryResult, UnifiedStream};
    pub use crate::{
        ContentBlock, FinishReason, GenerateRequest, GenerateResponse, Message, Role, ToolCall,
        ToolChoice, ToolDefinition,
    };
}
