//! cocode-context - Conversation state and token budget management.
//!
//! This crate provides:
//! - Environment snapshot (platform, cwd, model, context window)
//! - Token budget tracking per category
//! - Sync token estimation and budget computation
//! - Aggregate conversation context for prompt generation
//!
//! Key design: no dependency on cocode-tools. Tool names are stored as
//! `Vec<String>` so the context layer remains decoupled from tool
//! implementations.

pub mod budget;
pub mod calculator;
pub mod conversation_context;
pub mod environment;
pub mod error;

// Re-export main types at crate root
pub use budget::BudgetAllocation;
pub use budget::BudgetCategory;
pub use budget::ContextBudget;
pub use calculator::ContextCalculator;
pub use conversation_context::ContextInjection;
pub use conversation_context::ConversationContext;
pub use conversation_context::ConversationContextBuilder;
pub use conversation_context::InjectionPosition;
pub use conversation_context::MemoryFile;
pub use conversation_context::SubagentType;
pub use environment::EnvironmentInfo;
pub use environment::EnvironmentInfoBuilder;
pub use error::ContextError;
pub use error::Result;
