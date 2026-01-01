//! Anthropic SDK for Rust
//!
//! A Rust client library for the Anthropic Claude API.
//!
//! # Example
//!
//! ```no_run
//! use anthropic_sdk::{Client, MessageCreateParams, MessageParam};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a client using ANTHROPIC_API_KEY environment variable
//! let client = Client::from_env()?;
//!
//! // Create a message
//! let message = client.messages().create(
//!     MessageCreateParams::new(
//!         "claude-3-5-sonnet-20241022",
//!         1024,
//!         vec![MessageParam::user("Hello, Claude!")],
//!     )
//! ).await?;
//!
//! println!("{}", message.text());
//! # Ok(())
//! # }
//! ```

mod client;
mod config;
mod error;
mod resources;
mod types;

// Re-export main types
pub use client::Client;
pub use config::ClientConfig;
pub use error::AnthropicError;
pub use error::Result;

// Re-export all types
pub use types::{
    // Content types
    CacheControl,
    CacheControlType,
    // Usage types
    CacheCreation,
    CacheTtl,
    ContentBlock,
    ContentBlockParam,
    // Message types
    CountTokensParams,
    ImageMediaType,
    ImageSource,
    Message,
    MessageCreateParams,
    MessageParam,
    MessageTokensCount,
    // Common types
    Metadata,
    Role,
    ServiceTier,
    StopReason,
    SystemPrompt,
    SystemPromptBlock,
    TextCitation,
    ThinkingConfig,
    Tool,
    ToolChoice,
    ToolResultContent,
    ToolResultContentBlock,
    Usage,
};
