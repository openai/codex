//! Rust SDK for Volcengine Ark Response API.
//!
//! This crate provides a minimal, non-streaming client for the Volcengine Ark API.
//!
//! # Features
//!
//! - Response API (non-streaming)
//! - Embeddings API
//! - Chat/text conversations
//! - Image input (base64 and URL)
//! - Tool/function calling
//! - Extended thinking mode
//!
//! # Example
//!
//! ```ignore
//! use volcengine_ark_sdk::{Client, ResponseCreateParams, InputMessage, ThinkingConfig};
//!
//! #[tokio::main]
//! async fn main() -> volcengine_ark_sdk::Result<()> {
//!     // Create client with API key
//!     let client = Client::with_api_key("ark-xxx")?;
//!
//!     // Build request parameters
//!     let params = ResponseCreateParams::new("ep-xxx", vec![
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

mod client;
mod config;
pub mod error;
pub mod resources;
pub mod types;

// Re-export main types at crate root for convenience
pub use client::Client;
pub use config::ClientConfig;
pub use config::HttpRequest;
pub use config::RequestHook;
pub use error::ArkError;
pub use error::Result;
pub use types::CachingConfig;
pub use types::CreateEmbeddingResponse;
pub use types::Embedding;
pub use types::EmbeddingCreateParams;
pub use types::EmbeddingInput;
pub use types::EmbeddingUsage;
pub use types::EncodingFormat;
pub use types::FunctionDefinition;
pub use types::ImageMediaType;
pub use types::ImageSource;
pub use types::InputContentBlock;
pub use types::InputMessage;
pub use types::InputTokensDetails;
pub use types::MIN_THINKING_BUDGET_TOKENS;
pub use types::OutputContentBlock;
pub use types::OutputItem;
pub use types::OutputTokensDetails;
pub use types::ReasoningEffort;
pub use types::ReasoningStatus;
pub use types::ReasoningSummary;
pub use types::Response;
pub use types::ResponseCaching;
pub use types::ResponseCreateParams;
pub use types::ResponseError;
pub use types::ResponseStatus;
pub use types::Role;
pub use types::StopReason;
pub use types::ThinkingConfig;
pub use types::Tool;
pub use types::ToolChoice;
pub use types::Usage;
