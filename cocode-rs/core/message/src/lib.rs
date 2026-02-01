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
pub use factory::MessageBuilder;
pub use factory::create_assistant_message;
pub use factory::create_assistant_message_with_content;
pub use factory::create_compaction_summary;
pub use factory::create_subagent_result_message;
pub use factory::create_system_message;
pub use factory::create_tool_error_message;
pub use factory::create_tool_result_message;
pub use factory::create_tool_result_structured;
pub use factory::create_tool_results_batch;
pub use factory::create_user_message;
pub use factory::create_user_message_with_content;
pub use history::HistoryBuilder;
pub use history::HistoryConfig;
pub use history::MessageHistory;
pub use normalization::NormalizationOptions;
pub use normalization::ValidationError;
pub use normalization::estimate_tokens;
pub use normalization::normalize_messages_for_api;
pub use normalization::validate_messages;
pub use tracked::MessageSource;
pub use tracked::TrackedMessage;
pub use turn::ToolCallStatus;
pub use turn::TrackedToolCall;
pub use turn::Turn;
pub use type_guards::count_tool_results;
pub use type_guards::count_tool_uses;
pub use type_guards::extract_text;
pub use type_guards::extract_thinking;
pub use type_guards::extract_tool_result;
pub use type_guards::extract_tool_use;
pub use type_guards::get_text_content;
pub use type_guards::get_thinking_content;
pub use type_guards::get_tool_calls;
pub use type_guards::has_thinking;
pub use type_guards::has_tool_result;
pub use type_guards::has_tool_use;
pub use type_guards::is_assistant_message;
pub use type_guards::is_empty_message;
pub use type_guards::is_image_block;
pub use type_guards::is_system_message;
pub use type_guards::is_text_block;
pub use type_guards::is_thinking_block;
pub use type_guards::is_tool_message;
pub use type_guards::is_tool_result_block;
pub use type_guards::is_tool_use_block;
pub use type_guards::is_user_message;

// Re-export commonly used types from dependencies
pub use cocode_api::ContentBlock;
pub use cocode_api::Message;
pub use cocode_api::Role;
pub use cocode_api::ToolCall;
pub use cocode_api::ToolResultContent;
pub use cocode_protocol::AbortReason;
pub use cocode_protocol::TokenUsage;

/// Prelude module for convenient imports.
pub mod prelude {
    pub use crate::ContentBlock;
    pub use crate::Message;
    pub use crate::Role;
    pub use crate::TokenUsage;
    pub use crate::factory::MessageBuilder;
    pub use crate::history::HistoryBuilder;
    pub use crate::history::MessageHistory;
    pub use crate::tracked::MessageSource;
    pub use crate::tracked::TrackedMessage;
    pub use crate::turn::ToolCallStatus;
    pub use crate::turn::TrackedToolCall;
    pub use crate::turn::Turn;
    pub use crate::type_guards::get_text_content;
    pub use crate::type_guards::get_tool_calls;
    pub use crate::type_guards::has_tool_use;
    pub use crate::type_guards::is_assistant_message;
    pub use crate::type_guards::is_user_message;
}
