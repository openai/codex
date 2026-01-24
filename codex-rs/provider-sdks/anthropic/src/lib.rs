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
mod streaming;
mod types;

// Re-export main types
pub use client::Client;
pub use config::ClientConfig;
pub use config::HttpRequest;
pub use config::RequestHook;
pub use error::AnthropicError;
pub use error::Result;

// Re-export streaming types
pub use streaming::EventStream;
pub use streaming::MessageStream;

// Re-export all types
pub use types::{
    // Content types
    CacheControl,
    CacheControlType,
    // Usage types
    CacheCreation,
    CacheTtl,
    ContentBlock,
    // Streaming types
    ContentBlockDelta,
    ContentBlockParam,
    ContentBlockStartData,
    // Message types
    CountTokensParams,
    ImageMediaType,
    ImageSource,
    Message,
    MessageCreateParams,
    MessageDeltaData,
    MessageDeltaUsage,
    MessageParam,
    MessageStartData,
    MessageTokensCount,
    // Common types
    Metadata,
    RawMessageStreamEvent,
    Role,
    // Server tool usage
    ServerToolUsage,
    // Service tier types
    ServiceTier,
    StopReason,
    StreamError,
    SystemPrompt,
    SystemPromptBlock,
    TextCitation,
    ThinkingConfig,
    Tool,
    ToolChoice,
    ToolResultContent,
    ToolResultContentBlock,
    Usage,
    // Response service tier (standard/priority/batch)
    UsedServiceTier,
};
