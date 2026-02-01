//! cocode-prompt - System prompt builder and template management.
//!
//! This crate provides:
//! - 14 embedded markdown templates for prompt sections
//! - Section assembly with ordered concatenation
//! - Environment template rendering with placeholder substitution
//! - Permission-mode-aware prompt generation
//! - Summarization prompts for context compaction
//! - Subagent prompt generation (explore/plan)
//!
//! All operations are sync — pure string assembly with no I/O.

pub mod builder;
pub mod error;
pub mod sections;
pub mod summarization;
pub mod templates;

// Re-export main types at crate root
pub use builder::SystemPromptBuilder;
pub use error::PromptError;
pub use error::Result;
pub use sections::PromptSection;
pub use summarization::ParsedSummary;
pub use summarization::build_brief_summary_prompt;
pub use summarization::build_summarization_prompt;
pub use summarization::parse_summary_response;
