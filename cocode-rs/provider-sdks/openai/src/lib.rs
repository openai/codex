//! Rust SDK for OpenAI Responses API.
//!
//! This crate provides a client for the OpenAI API with both streaming and
//! non-streaming support.
//!
//! # Features
//!
//! - Response API (non-streaming)
//! - Embeddings API
//! - Chat/text conversations
//! - Image input (base64, URL, and file ID)
//! - Tool/function calling
//! - Extended thinking mode
//! - Prompt caching
//!
//! # Example
//!
//! ```ignore
//! use openai_sdk::{Client, ResponseCreateParams, InputMessage, ThinkingConfig};
//!
//! #[tokio::main]
//! async fn main() -> openai_sdk::Result<()> {
//!     // Create client with API key
//!     let client = Client::from_env()?;
//!
//!     // Build request parameters
//!     let params = ResponseCreateParams::new("gpt-4o", vec![
//!         InputMessage::user_text("Hello, what is 2 + 2?")
//!     ])
//!     .max_output_tokens(1024)
//!     .thinking(ThinkingConfig::enabled(2048));
//!
//!     // Make API call
//!     let response = client.responses().create(params).await?;
//!
//!     // Extract text response
//!     println!("Response: {}", response.text());
//!
//!     // Check for function calls
//!     if response.has_function_calls() {
//!         for (call_id, name, args) in response.function_calls() {
//!             println!("Function call: {} -> {}", name, args);
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! # Embeddings Example
//!
//! ```ignore
//! use openai_sdk::{Client, EmbeddingCreateParams};
//!
//! let client = Client::from_env()?;
//! let response = client.embeddings().create(
//!     EmbeddingCreateParams::new("text-embedding-3-small", "Hello, world!")
//!         .dimensions(256)
//! ).await?;
//!
//! println!("Embedding: {:?}", response.embedding());
//! ```

mod client;
mod config;
pub mod error;
pub mod resources;
pub mod streaming;
pub mod types;

// Re-export main types at crate root for convenience
pub use client::Client;
pub use config::ClientConfig;
pub use config::HttpRequest;
pub use config::RequestHook;
pub use error::OpenAIError;
pub use error::Result;

// Common types
pub use types::CustomToolInputFormat;
pub use types::FunctionDefinition;
pub use types::Metadata;
pub use types::RankingOptions;
pub use types::ResponseStatus;
pub use types::Role;
pub use types::StopReason;
pub use types::Tool;
pub use types::ToolChoice;
pub use types::UserLocation;

// Content types
pub use types::Annotation;
pub use types::AudioFormat;
pub use types::ComputerCallOutputData;
pub use types::ImageDetail;
pub use types::ImageMediaType;
pub use types::ImageSource;
pub use types::InputContentBlock;
pub use types::LogprobContent;
pub use types::Logprobs;
pub use types::OutputContentBlock;
pub use types::TokenLogprob;
pub use types::TopLogprob;

// Embedding types
pub use types::CreateEmbeddingResponse;
pub use types::Embedding;
pub use types::EmbeddingCreateParams;
pub use types::EmbeddingInput;
pub use types::EmbeddingUsage;
pub use types::EncodingFormat;

// Response types
pub use types::CodeInterpreterOutput;
pub use types::ComputerAction;
pub use types::ConversationParam;
pub use types::FileSearchResult;
pub use types::ImageGenerationResult;
pub use types::IncompleteDetails;
pub use types::IncompleteReason;
pub use types::InputMessage;
pub use types::InputTokensDetails;
pub use types::MIN_THINKING_BUDGET_TOKENS;
pub use types::McpToolInfo;
pub use types::MpcCallRef;
pub use types::OutputItem;
pub use types::OutputTokensDetails;
pub use types::PromptCacheRetention;
pub use types::PromptCachingConfig;
pub use types::PromptParam;
pub use types::ReasoningConfig;
pub use types::ReasoningEffort;
pub use types::ReasoningSummary;
pub use types::Response;
pub use types::ResponseCreateParams;
pub use types::ResponseError;
pub use types::ResponseIncludable;
pub use types::ResponseInput;
pub use types::ResponsePrompt;
pub use types::SafetyCheck;
pub use types::SdkHttpResponse;
pub use types::ServiceTier;
pub use types::TextConfig;
pub use types::TextFormat;
pub use types::ThinkingConfig;
pub use types::Truncation;
pub use types::Usage;
pub use types::WebSearchResult;

// Stream event types
pub use streaming::ResponseStream;
pub use streaming::ResponseStreamAdapter;
pub use streaming::SSEDecoder;
pub use streaming::ServerSentEvent;
pub use types::ContentPart;
pub use types::ResponseStreamEvent;
pub use types::StreamLogprob;
