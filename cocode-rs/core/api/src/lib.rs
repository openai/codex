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
pub mod model_hub;
pub mod provider_factory;
pub mod request_builder;
pub mod request_options_merge;
pub mod retry;
pub mod thinking_convert;
pub mod unified_stream;

// Re-export main types at crate root
pub use cache::CacheStats;
pub use cache::Cacheable;
pub use cache::PromptCacheConfig;
pub use client::ApiClient;
pub use client::ApiClientBuilder;
pub use client::ApiClientConfig;
pub use client::FallbackConfig;
pub use client::StreamOptions;
pub use error::ApiError;
pub use error::Result;
pub use model_hub::HubError;
pub use model_hub::ModelHub;
pub use model_hub::resolve_identity;
pub use provider_factory::create_model;
pub use provider_factory::create_provider;
pub use request_builder::RequestBuilder;
pub use request_builder::build_request;
pub use retry::RetryConfig;
pub use retry::RetryContext;
pub use retry::RetryDecision;
pub use thinking_convert::to_provider_options;
pub use unified_stream::CollectedResponse;
pub use unified_stream::QueryResultType;
pub use unified_stream::StreamingQueryResult;
pub use unified_stream::UnifiedStream;

// Re-export commonly used hyper-sdk types for convenience
pub use hyper_sdk::ContentBlock;
pub use hyper_sdk::FinishReason;
pub use hyper_sdk::GenerateRequest;
pub use hyper_sdk::GenerateResponse;
pub use hyper_sdk::Message;
pub use hyper_sdk::Role;
pub use hyper_sdk::StreamProcessor;
pub use hyper_sdk::StreamSnapshot;
pub use hyper_sdk::StreamUpdate;
pub use hyper_sdk::TokenUsage;
pub use hyper_sdk::ToolCall;
pub use hyper_sdk::ToolChoice;
pub use hyper_sdk::ToolDefinition;
pub use hyper_sdk::ToolResultContent;

/// Prelude module for convenient imports.
pub mod prelude {
    pub use crate::ContentBlock;
    pub use crate::FinishReason;
    pub use crate::GenerateRequest;
    pub use crate::GenerateResponse;
    pub use crate::Message;
    pub use crate::Role;
    pub use crate::ToolCall;
    pub use crate::ToolChoice;
    pub use crate::ToolDefinition;
    pub use crate::cache::PromptCacheConfig;
    pub use crate::client::ApiClient;
    pub use crate::client::StreamOptions;
    pub use crate::error::ApiError;
    pub use crate::error::Result;
    pub use crate::retry::RetryConfig;
    pub use crate::retry::RetryContext;
    pub use crate::unified_stream::StreamingQueryResult;
    pub use crate::unified_stream::UnifiedStream;
}
