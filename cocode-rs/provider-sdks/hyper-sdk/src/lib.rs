//! hyper-sdk - Unified multi-provider AI model SDK for Rust
//!
//! This crate provides a consistent API for interacting with multiple AI providers
//! (OpenAI, Anthropic, Google, etc.), supporting text generation, vision (VLM),
//! embeddings, and native streaming.
//!
//! # Features
//!
//! - **Provider Agnostic**: Single API across all providers
//! - **Capability Aware**: Support for LLM, VLM, and Embedding models
//! - **Native Streaming**: First-class async streaming support
//! - **Type Safe**: Strongly typed request/response types
//! - **Extensible**: Easy to add new providers
//!
//! # Quick Start
//!
//! ```no_run
//! use hyper_sdk::{OpenAIProvider, Provider, GenerateRequest, Message};
//!
//! # async fn example() -> hyper_sdk::Result<()> {
//! // Create a provider from environment variables
//! let provider = OpenAIProvider::from_env()?;
//!
//! // Get a model
//! let model = provider.model("gpt-4o")?;
//!
//! // Generate a response
//! let response = model.generate(
//!     GenerateRequest::new(vec![Message::user("Hello!")])
//! ).await?;
//!
//! println!("{}", response.text());
//! # Ok(())
//! # }
//! ```
//!
//! # Streaming
//!
//! ```no_run
//! use hyper_sdk::{OpenAIProvider, Provider, GenerateRequest, Message};
//!
//! # async fn example() -> hyper_sdk::Result<()> {
//! let provider = OpenAIProvider::from_env()?;
//! let model = provider.model("gpt-4o")?;
//!
//! let mut stream = model.stream(
//!     GenerateRequest::new(vec![Message::user("Hello!")])
//! ).await?;
//!
//! while let Some(event) = stream.next_event().await {
//!     if let hyper_sdk::StreamEvent::TextDelta { delta, .. } = event? {
//!         print!("{}", delta);
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Vision
//!
//! ```no_run
//! use hyper_sdk::{OpenAIProvider, Provider, GenerateRequest, Message, ImageSource};
//!
//! # async fn example() -> hyper_sdk::Result<()> {
//! let provider = OpenAIProvider::from_env()?;
//! let model = provider.model("gpt-4o")?;
//!
//! let response = model.generate(
//!     GenerateRequest::new(vec![
//!         Message::user_with_image(
//!             "What's in this image?",
//!             ImageSource::Url { url: "https://example.com/image.png".to_string() }
//!         )
//!     ])
//! ).await?;
//!
//! println!("{}", response.text());
//! # Ok(())
//! # }
//! ```
//!
//! # Tool Calling
//!
//! ```no_run
//! use hyper_sdk::{OpenAIProvider, Provider, GenerateRequest, Message, ToolDefinition};
//!
//! # async fn example() -> hyper_sdk::Result<()> {
//! let provider = OpenAIProvider::from_env()?;
//! let model = provider.model("gpt-4o")?;
//!
//! let response = model.generate(
//!     GenerateRequest::new(vec![Message::user("What's the weather in NYC?")])
//!         .tools(vec![
//!             ToolDefinition::full(
//!                 "get_weather",
//!                 "Get the current weather for a location",
//!                 serde_json::json!({
//!                     "type": "object",
//!                     "properties": {
//!                         "location": {"type": "string"}
//!                     },
//!                     "required": ["location"]
//!                 })
//!             )
//!         ])
//! ).await?;
//!
//! for tool_call in response.tool_calls() {
//!     println!("Tool: {} Args: {}", tool_call.name, tool_call.arguments);
//! }
//! # Ok(())
//! # }
//! ```

// Module declarations
pub mod call_id;
pub mod capability;
pub mod client;
pub mod compat;
pub mod conversation;
pub mod embedding;
pub mod error;
pub mod hooks;
pub mod messages;
pub mod model;
pub mod object;
pub mod options;
pub mod provider;
pub mod providers;
pub mod rate_limits;
pub mod registry;
pub mod request;
pub mod response;
pub mod retry;
pub mod session;
pub mod stream;
pub mod telemetry;
pub mod tools;

// Re-export main types at crate root

// Error types
pub use error::HyperError;
pub use error::Result;

// Capabilities
pub use capability::Capability;
pub use capability::ModelInfo;

// Messages
pub use messages::ContentBlock;
pub use messages::ImageDetail;
pub use messages::ImageSource;
pub use messages::Message;
pub use messages::ProviderMetadata;
pub use messages::Role;

// Tools
pub use tools::ToolCall;
pub use tools::ToolChoice;
pub use tools::ToolDefinition;
pub use tools::ToolResultContent;

// Request/Response
pub use request::GenerateRequest;
pub use response::FinishReason;
pub use response::GenerateResponse;
pub use response::TokenUsage;

// Embedding
pub use embedding::EmbedRequest;
pub use embedding::EmbedResponse;
pub use embedding::Embedding;
pub use embedding::EncodingFormat;

// Object generation (structured output)
pub use object::ObjectRequest;
pub use object::ObjectResponse;
pub use object::ObjectStreamEvent;
pub use object::ObjectStreamResponse;

// Streaming
pub use stream::CollectTextCallbacks;
pub use stream::DEFAULT_IDLE_TIMEOUT;
pub use stream::EventStream;
pub use stream::PrintCallbacks;
pub use stream::StreamCallbacks;
pub use stream::StreamConfig;
pub use stream::StreamError;
pub use stream::StreamEvent;
pub use stream::StreamProcessor;
pub use stream::StreamResponse;
pub use stream::StreamSnapshot;
pub use stream::StreamUpdate;
pub use stream::ThinkingSnapshot;
pub use stream::ToolCallSnapshot;

// Rate limits
pub use rate_limits::RateLimitSnapshot;

// Retry
pub use retry::RetryConfig;
pub use retry::RetryExecutor;

// Telemetry
pub use telemetry::LoggingTelemetry;
pub use telemetry::NoopTelemetry;
pub use telemetry::RequestTelemetry;
pub use telemetry::StreamTelemetry;

// Provider and Model traits
pub use model::Model;
pub use provider::Provider;
pub use provider::ProviderConfig;

// Client (recommended API)
pub use client::HyperClient;

// Registry
pub use registry::ProviderRegistry;

// Provider implementations
pub use providers::AnthropicProvider;
pub use providers::GeminiProvider;
pub use providers::OpenAICompatProvider;
pub use providers::OpenAIProvider;
pub use providers::VolcengineProvider;
pub use providers::ZaiProvider;
pub use providers::any_from_env;

// Provider options
pub use options::AnthropicOptions;
pub use options::GeminiOptions;
pub use options::OpenAIOptions;
pub use options::ProviderMarker;
pub use options::ProviderOptions;
pub use options::ProviderOptionsData;
pub use options::ReasoningEffort;
pub use options::ThinkingConfig;
pub use options::TypedProviderOptions;
pub use options::VolcengineOptions;
pub use options::ZaiOptions;
pub use options::downcast_options;
pub use options::try_downcast_options;
pub use options::validate_options_for_provider;

// Compatibility layer
pub use compat::HyperAdapter;

// Hooks
pub use hooks::CrossProviderSanitizationHook;
pub use hooks::HookChain;
pub use hooks::HookContext;
pub use hooks::LoggingHook;
pub use hooks::RequestHook;
pub use hooks::ResponseHook;
pub use hooks::ResponseIdHook;
pub use hooks::StreamHook;
pub use hooks::UsageTrackingHook;

// Session and Conversation
pub use conversation::ConversationContext;
pub use conversation::ConversationContextBuilder;
pub use session::SessionConfig;

/// Prelude module for convenient imports.
pub mod prelude {
    pub use crate::capability::Capability;
    pub use crate::capability::ModelInfo;
    pub use crate::client::HyperClient;
    pub use crate::conversation::ConversationContext;
    pub use crate::embedding::EmbedRequest;
    pub use crate::embedding::EmbedResponse;
    pub use crate::error::HyperError;
    pub use crate::error::Result;
    pub use crate::hooks::HookChain;
    pub use crate::hooks::HookContext;
    pub use crate::hooks::RequestHook;
    pub use crate::hooks::ResponseHook;
    pub use crate::messages::ContentBlock;
    pub use crate::messages::ImageSource;
    pub use crate::messages::Message;
    pub use crate::messages::ProviderMetadata;
    pub use crate::messages::Role;
    pub use crate::model::Model;
    pub use crate::object::ObjectRequest;
    pub use crate::object::ObjectResponse;
    pub use crate::provider::Provider;
    pub use crate::providers::AnthropicProvider;
    pub use crate::providers::GeminiProvider;
    pub use crate::providers::OpenAIProvider;
    pub use crate::providers::VolcengineProvider;
    pub use crate::providers::ZaiProvider;
    pub use crate::rate_limits::RateLimitSnapshot;
    pub use crate::request::GenerateRequest;
    pub use crate::response::FinishReason;
    pub use crate::response::GenerateResponse;
    pub use crate::session::SessionConfig;
    pub use crate::stream::StreamCallbacks;
    pub use crate::stream::StreamConfig;
    pub use crate::stream::StreamEvent;
    pub use crate::stream::StreamProcessor;
    pub use crate::stream::StreamResponse;
    pub use crate::stream::StreamSnapshot;
    pub use crate::stream::StreamUpdate;
    pub use crate::telemetry::RequestTelemetry;
    pub use crate::telemetry::StreamTelemetry;
    pub use crate::tools::ToolCall;
    pub use crate::tools::ToolDefinition;
}
