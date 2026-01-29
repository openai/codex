//! cocode-message - Agent loop message management.
//!
//! This crate provides conversation management for the agent loop:
//! - Turn tracking (TrackedMessage, Turn)
//! - Message history with token budget
//! - Message normalization for API
//! - Factory functions for message creation
//! - Type guards for content blocks
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                       cocode-message                            │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  MessageHistory       │  Turn              │  TrackedMessage   │
//! │  - turns              │  - user_message    │  - inner: Message │
//! │  - compaction         │  - assistant_msg   │  - uuid           │
//! │  - token tracking     │  - tool_calls      │  - turn_id        │
//! │                       │  - usage           │  - source         │
//! ├───────────────────────┴──────────────────────────────────────────┤
//! │                          hyper-sdk                               │
//! │  Message, ContentBlock, Role, ToolCall, ToolResultContent       │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Quick Start
//!
//! ```ignore
//! use cocode_message::{MessageHistory, Turn, TrackedMessage, HistoryBuilder};
//!
//! // Create history with system message
//! let mut history = HistoryBuilder::new()
//!     .system_message("You are a helpful assistant")
//!     .context_window(128000)
//!     .build();
//!
//! // Create a turn with user message
//! let user_msg = TrackedMessage::user("Hello!", "turn-1");
//! let mut turn = Turn::new(1, user_msg);
//!
//! // After getting API response, set assistant message
//! turn.set_assistant_message(TrackedMessage::assistant(
//!     "Hi there!",
//!     "turn-1",
//!     Some("req-123".to_string()),
//! ));
//!
//! // Add turn to history
//! history.add_turn(turn);
//!
//! // Get messages for API request
//! let messages = history.messages_for_api();
//! ```
//!
//! # Module Structure
//!
//! - [`type_guards`] - Content block type checks
//! - [`tracked`] - TrackedMessage with metadata
//! - [`turn`] - Turn and TrackedToolCall
//! - [`factory`] - Message factory functions
//! - [`normalization`] - Message normalization for API
//! - [`history`] - MessageHistory management

pub mod factory;
pub mod history;
pub mod normalization;
pub mod tracked;
pub mod turn;
pub mod type_guards;

// Re-export main types at crate root
pub use factory::{
    MessageBuilder, create_assistant_message, create_assistant_message_with_content,
    create_compaction_summary, create_subagent_result_message, create_system_message,
    create_tool_error_message, create_tool_result_message, create_tool_result_structured,
    create_tool_results_batch, create_user_message, create_user_message_with_content,
};
pub use history::{HistoryBuilder, HistoryConfig, MessageHistory};
pub use normalization::{
    NormalizationOptions, ValidationError, estimate_tokens, normalize_messages_for_api,
    validate_messages,
};
pub use tracked::{MessageSource, TrackedMessage};
pub use turn::{ToolCallStatus, TrackedToolCall, Turn};
pub use type_guards::{
    count_tool_results, count_tool_uses, extract_text, extract_thinking, extract_tool_result,
    extract_tool_use, get_text_content, get_thinking_content, get_tool_calls, has_thinking,
    has_tool_result, has_tool_use, is_assistant_message, is_empty_message, is_image_block,
    is_system_message, is_text_block, is_thinking_block, is_tool_message, is_tool_result_block,
    is_tool_use_block, is_user_message,
};

// Re-export commonly used types from dependencies
pub use cocode_api::{ContentBlock, Message, Role, ToolCall, ToolResultContent};
pub use cocode_protocol::{AbortReason, TokenUsage};

/// Prelude module for convenient imports.
pub mod prelude {
    pub use crate::factory::MessageBuilder;
    pub use crate::history::{HistoryBuilder, MessageHistory};
    pub use crate::tracked::{MessageSource, TrackedMessage};
    pub use crate::turn::{ToolCallStatus, TrackedToolCall, Turn};
    pub use crate::type_guards::{
        get_text_content, get_tool_calls, has_tool_use, is_assistant_message, is_user_message,
    };
    pub use crate::{ContentBlock, Message, Role, TokenUsage};
}
