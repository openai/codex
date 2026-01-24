//! Z.AI / ZhipuAI SDK for Rust
//!
//! A Rust client library for the Z.AI and ZhipuAI Chat/Embeddings API.
//!
//! # Example
//!
//! ```no_run
//! use z_ai_sdk::{ZaiClient, ChatCompletionsCreateParams, MessageParam};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = ZaiClient::from_env()?;
//!
//! let completion = client.chat().completions().create(
//!     ChatCompletionsCreateParams::new(
//!         "glm-4.7",
//!         vec![MessageParam::user("Hello!")],
//!     )
//! ).await?;
//!
//! println!("{}", completion.text());
//! # Ok(())
//! # }
//! ```

mod client;
mod config;
mod error;
mod jwt;
mod resources;
mod types;

// Re-export client types
pub use client::ZaiClient;
pub use client::ZhipuAiClient;
pub use config::ClientConfig;
pub use config::HttpRequest;
pub use config::RequestHook;
pub use error::Result;
pub use error::ZaiError;

// Re-export all types
pub use types::{
    // Chat types
    ChatCompletionsCreateParams,
    Completion,
    CompletionChoice,
    CompletionMessage,
    CompletionMessageToolCall,
    // Usage types
    CompletionTokensDetails,
    CompletionUsage,
    // Content types
    ContentBlock,
    // Embedding types
    Embedding,
    EmbeddingInput,
    EmbeddingsCreateParams,
    EmbeddingsResponded,
    // Common types
    FinishReason,
    Function,
    FunctionDef,
    ImageUrl,
    MessageParam,
    PromptTokensDetails,
    Role,
    // Thinking types
    ThinkingConfig,
    Tool,
    ToolChoice,
    ToolChoiceFunction,
};
